//! Hot-reloading, loadable and reloadable resources.
//!
//! A resource is a (possibly) disk-cached object that can be hot-reloaded while you use it.
//! Resources can be serialized and deserialized as you see fit. The concept of *caching* and
//! *loading* are split in different code locations so that you can easily compose both – provide
//! the loading code and ask the resource system to cache it for you.
//!
//! This flexibility is exposed in the public interface so that the cache can be augmented with
//! user-provided objects. You might be interested in implementing `Load` and `CacheKey` – from
//! the [any-cache](https://crates.io/crates/any-cache) crate.
//!
//! In order to have hot-reloading working, you have to call the `Store::sync` function that will
//! perform disk syncing. This function will unqueue disk events.
//!
//! > Note: this is not the queue used by the underlying library (depending on your platform; for
//! > instance, inotify). This queue cannot, in theory, overflow. It’ll get bigger and bigger if you
//! > never sync.
//!
//! # Key wrapping
//!
//! If you use the resource system, your resources will be cached and accessible by their keys. The
//! key type is not enforced. Resource’s keys are typed to enable namespacing: if you have two
//! resources which ID is `34`, because the key types are different, you can safely cache the
//! resource with the ID `34` without any clashing or undefined behaviors. More in the any-cache
//! crate.
//!
//! # Borrowing
//!
//! Because the resource you access might change at anytime, you have to ensure you are the single
//! one handling them around. This is done via the `Rc::borrow` and `Rc::borrow_mut` functions.
//!
//! > Important note: keep in mind `Rc` is not `Send`. This is a limitation that might be fixed in
//! > the near future.

extern crate any_cache;
extern crate notify;

use any_cache::{Cache, HashCache};
pub use any_cache::CacheKey;
use notify::{Op, RawEvent, RecommendedWatcher, RecursiveMode, Watcher, raw_watcher};
use notify::op::WRITE;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Loadable object from disk.
///
/// An object can be loaded from disk if, given a path, it can output a `Loaded<_>`. It’s
/// important to note that you’re not supposed to use that trait directly. Instead, you should use
/// the `Store`’s functions.
pub trait Load: 'static + Sized {
  /// Type of error that might happen while loading.
  type Error: Error;

  /// Load a resource from the file system. The `Store` can be used to load or declare additional
  /// resource dependencies. The result type is used to register for dependency events.
  fn from_fs<P>(path: P, store: &mut Store) -> Result<Loaded<Self>, Self::Error> where P: AsRef<Path>;

  // FIXME: add support for redeclaring the dependencies?
  /// Function called when a resource must be reloaded.
  ///
  /// The default implementation of that function calls `from_fs` and returns its result.
  fn reload<P>(_: &Self, path: P, store: &mut Store) -> Result<Self, Self::Error> where P: AsRef<Path> {
    Self::from_fs(path, store).map(|lr| lr.res)
  }
}

/// Result of a resource loading.
///
/// This type enables you to register a resource for reloading events of other resources. Those are
/// named dependencies. If you don’t need to run specific code on a dependency reloading, use
/// the `.into()` function to lift your return value to `Loaded<_>` or use the provided
/// function.
pub struct Loaded<T> {
  /// The loaded object.
  pub res: T,
  /// The list of dependencies to listen for events.
  pub deps: Vec<PathBuf>
}

impl<T> Loaded<T> {
  /// Return a resource declaring no dependency at all.
  pub fn without_dep(res: T) -> Self {
    Loaded { res, deps: Vec::new() }
  }

  /// Return a resource along with its dependencies.
  pub fn with_deps(res: T, deps: Vec<PathBuf>) -> Self {
    Loaded { res, deps }
  }
}

impl<T> From<T> for Loaded<T> {
  fn from(res: T) -> Self {
    Loaded::without_dep(res)
  }
}

/// Resources are wrapped in this type.
pub type Res<T> = Rc<RefCell<T>>;

/// Key used to get resource.
///
/// This is the entry point of the `Store`. A key is a simple path with phantom typing, giving type
/// hints on the target resource.
///
/// > Note: because of being a key, you must not provide an absolute nor canonicalized path:
/// > instead, you pass local paths without any leading characters (drop any `./` for instance).
/// >
/// > `let the_key = Key::new("maps/deck_16/room_3.bsp");`
pub struct Key<T> {
  path: PathBuf,
  _t: PhantomData<*const T>
}

impl<T> Key<T> {
  /// Create a new key from a local path, without leading special characters.
  pub fn new<P>(path: P) -> Self where P: AsRef<Path> {
    Key {
      path: path.as_ref().to_owned(),
      _t: PhantomData
    }
  }

  /// Get the underlying path.
  ///
  /// This path is relative to the root path of the store the key is used in.
  pub fn as_path(&self) -> &Path {
    &self.path
  }
}

impl<T> Clone for Key<T> {
  fn clone(&self) -> Self {
    Key {
      path: self.path.clone(),
      _t: PhantomData
    }
  }
}

impl<T> fmt::Debug for Key<T> {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    self.path.fmt(f)
  }
}

impl<T> Eq for Key<T> {}

impl<T> hash::Hash for Key<T> {
  fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
    self.path.hash(state)
  }
}

impl<T> PartialEq for Key<T> {
  fn eq(&self, rhs: &Self) -> bool {
    self.path.eq(&rhs.path)
  }
}

impl<T> CacheKey for Key<T> where T: 'static {
  type Target = Res<T>;
}

/// Resource store. Responsible for holding and presenting resources.
pub struct Store {
  // store options
  opt: StoreOpt,
  // canonicalized root path
  canon_root: PathBuf,
  // resource cache, containing all living resources
  cache: HashCache,
  // contains all metadata on resources (reload functions, last time updated, etc.)
  metadata: HashMap<PathBuf, ResMetaData>,
  // dependencies, mapping a dependency to its dependent resources
  deps: HashMap<PathBuf, PathBuf>,
  // keep the watcher around so that we don’t have it disconnected
  #[allow(dead_code)]
  watcher: RecommendedWatcher,
  // watcher receiver part of the channel
  watcher_rx: Receiver<RawEvent>,
}

impl Store {
  /// Create a new store.
  ///
  /// The `root` represents the root directory from all the resources come from and is relative to
  /// the binary’s location by default (unless you specify it as absolute).
  pub fn new(opt: StoreOpt) -> Result<Self, StoreError> {
    let root = opt.root().to_owned();

    // canonicalize the root because some platforms won’t correctly report file changes otherwise
    let canon_root = root.canonicalize().map_err(|_| StoreError::RootDoesDotExit(root))?;

    // create the mpsc channel to communicate with the file watcher
    let (wsx, wrx) = channel();
    let mut watcher = raw_watcher(wsx).unwrap();

    // spawn a new thread in which we look for events
    let _ = watcher.watch(&canon_root, RecursiveMode::Recursive);

    Ok(Store {
      opt,
      canon_root,
      cache: HashCache::new(),
      metadata: HashMap::new(),
      deps: HashMap::new(),
      watcher,
      watcher_rx: wrx,
    })
  }

  /// The canonicalized root the `Store` is configured with.
  pub fn root(&self) -> &Path {
    &self.canon_root
  }

  /// Inject a new resource in the store.
  fn inject<T>(
    &mut self,
    key: Key<T>,
    resource: T,
    deps: Vec<PathBuf>
  ) -> Res<T>
    where T: Load {
    // wrap the resource to make it shared mutably
    let res = Rc::new(RefCell::new(resource));
    let res_ = res.clone();

    let path = key.path.clone();
    let path_ = self.canon_root.join(&key.path);

    // closure used to reload the object when needed
    let on_reload: Box<for<'a> Fn(&'a mut Store) -> Result<(), Box<Error>>> = Box::new(move |store| {
      let reloaded = T::reload(&res_.borrow(), &path_, store);

      match reloaded {
        Ok(r) => {
          // replace the current resource with the freshly loaded one
          *res_.borrow_mut() = r;
          Ok(())
        },
        Err(e) => Err(Box::new(e))
      }
    });

    let metadata = ResMetaData {
      on_reload: on_reload,
      last_update_instant: Instant::now(),
    };

    // cache the resource and its meta data
    self.cache.save(key, res.clone());
    self.metadata.insert(path.clone(), metadata);

    // register the resource as an observer of its dependencies in the dependencies graph
    for dep_key in deps {
      self.deps.insert(dep_key, path.clone());
    }

    res
  }

  /// Get a resource from the store and return an error if loading failed.
  pub fn get<T>(&mut self, key: &Key<T>) -> Result<Res<T>, T::Error> where T: Load {
    match self.cache.get(key).cloned() {
      Some(resource) => {
        Ok(resource)
      },
      None => {
        // specific loading
        let load_result = T::from_fs(self.canon_root.join(&key.path), self)?;
        Ok(self.inject(key.clone(), load_result.res, load_result.deps))
      }
    }
  }

  /// Get a resource from the store for the given key. If it fails, a proxied version is used, which
  /// will get replaced by the resource once it’s available and reloaded.
  pub fn get_proxied<T, P>(
    &mut self,
    key: &Key<T>,
    proxy: P
  ) -> Res<T>
    where T: Load,
          P: FnOnce() -> T {
    match self.get(key) {
      Ok(resource) => resource,
      Err(_) => {
        // FIXME: we set the dependencies to none here, which is silly; find a better design
        self.inject(key.clone(), proxy(), Vec::new())
      }
    }
  }

  /// Synchronize the store by updating the resources that ought to.
  pub fn sync(&mut self) {
    let update_await_time_ms = self.opt.update_await_time_ms();

    let paths = dequeue_file_changes(&mut self.watcher_rx, &self.canon_root);

    for path in paths {
      // find the path in our watched paths; if we find it, we remove it prior to doing anything else
      if let Some(mut metadata) = self.metadata.remove(&path) {
        let now = Instant::now();

        // perform a timed check so that we don’t reload several times in a row the same goddamn
        // resource
        if now.duration_since(metadata.last_update_instant) >= Duration::from_millis(update_await_time_ms) {
          if (metadata.on_reload)(self).is_ok() {
            // if we have successfully reloaded the resource, notify the observers that this
            // dependency has changed
            for dep in self.deps.get(path.as_path()).cloned() {
              if let Some(obs_metadata) = self.metadata.remove(dep.as_path()) {
                match (obs_metadata.on_reload)(self) {
                  Ok(_) => { self.metadata.insert(dep, obs_metadata); }
                  _ => ()
                }
              }
            }
          }
        }

        metadata.last_update_instant = now;
        self.metadata.insert(path.clone(), metadata);
      }
    }
  }
}

/// Various options to customize a `Store`.
pub struct StoreOpt {
  root: PathBuf,
  update_await_time_ms: u64
}

impl Default for StoreOpt {
  fn default() -> Self {
    StoreOpt {
      root: PathBuf::from("."),
      update_await_time_ms: 1000
    }
  }
}

impl StoreOpt {
  /// Change the update await time (milliseconds) used to determine whether a resource should be
  /// reloaded or not.
  ///
  /// # Default
  ///
  /// Defaults to `1000`.
  #[inline]
  pub fn set_update_await_time_ms(self, ms: u64) -> Self {
    StoreOpt {
      update_await_time_ms: ms,
      .. self
    }
  }

  /// Get the update await time (milliseconds).
  #[inline]
  pub fn update_await_time_ms(&self) -> u64 {
    self.update_await_time_ms
  }

  /// Change the root directory from which the `Store` will be watching file changes.
  ///
  /// # Default
  ///
  /// Defaults to `"."`.
  #[inline]
  pub fn set_root<P>(self, root: P) -> Self where P: AsRef<Path> {
    StoreOpt {
      root: root.as_ref().to_owned(),
      .. self
    }
  }

  /// Get root directory.
  #[inline]
  pub fn root(&self) -> &Path {
    &self.root
  }
}

/// Meta data about a resource.
struct ResMetaData {
  /// Function to call each time the resource must be reloaded.
  on_reload: Box<Fn(&mut Store) -> Result<(), Box<Error>>>,
  /// The last time the resource was updated.
  last_update_instant: Instant,
}

/// Error that might happen when creating a resource store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
  /// The root path for the resources was not found.
  RootDoesDotExit(PathBuf)
}

// TODO: profile and see how not to allocate anything
/// Dequeue any file changes from a watcher channel and output them in a buffer.
fn dequeue_file_changes(rx: &mut Receiver<RawEvent>, prefix: &Path) -> Vec<PathBuf> {
  rx.try_iter().filter_map(|event| {
    match event {
      RawEvent { path: Some(ref path), op: Ok(op), .. } if op | WRITE != Op::empty() => {
        // remove the root path so that we end up with the same path as we have stored as keys
        let path = path.strip_prefix(prefix).unwrap().to_owned();
        Some(path)
      },
      _ => None
    }
  }).collect()
}
