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
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Loadable object from either the file system or memory.
///
/// An object can be loaded if it can output a `Loaded<_>`. It’s important to note that you’re not
/// supposed to use that trait directly. Instead, you should use the `Store`’s functions.
pub trait Load: 'static + Sized {
  /// Type of the key used to load the resource.
  ///
  /// You have two choices:
  ///
  /// - `PathKey`, used to load your resource from the file system.
  /// - `LogicalKey`, used to compute your resource from memory.
  type Key: Into<DepKey>;

  /// Type of error that might happen while loading.
  type Error: Error;

  /// Load a resource.
  ///
  /// The `Store` can be used to load or declare additional resource dependencies.
  ///
  /// The result type is used to register for dependency events. If you need that feature, you can
  /// use the `From` / `Into` traits to convert from a key to a `DepKey`:
  ///
  /// ```ignore
  ///     Ok(Loaded::with_deps(the_resource, vec![a_key.into(), another_key.into()]))
  /// ```
  fn load(key: Self::Key, store: &mut Store) -> Result<Loaded<Self>, Self::Error>;

  // FIXME: add support for redeclaring the dependencies?
  /// Function called when a resource must be reloaded.
  ///
  /// The default implementation of that function calls `load` and returns its result.
  fn reload(_: &Self, key: Self::Key, store: &mut Store) -> Result<Self, Self::Error> {
    Self::load(key, store).map(|lr| lr.res)
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
  pub deps: Vec<DepKey>
}

impl<T> Loaded<T> {
  /// Return a resource declaring no dependency at all.
  pub fn without_dep(res: T) -> Self {
    Loaded { res, deps: Vec::new() }
  }

  /// Return a resource along with its dependencies.
  pub fn with_deps(res: T, deps: Vec<DepKey>) -> Self {
    Loaded { res, deps }
  }
}

impl<T> From<T> for Loaded<T> {
  fn from(res: T) -> Self {
    Loaded::without_dep(res)
  }
}

/// A dependency key, which is either a `PathKey` or a `LogicalKey`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DepKey {
  Path(PathKey),
  Logical(LogicalKey)
}

impl From<PathKey> for DepKey {
  fn from(key: PathKey) -> Self {
    DepKey::Path(key)
  }
}

impl From<LogicalKey> for DepKey {
  fn from(key: LogicalKey) -> Self {
    DepKey::Logical(key)
  }
}

/// Path key – used to load resources from the file system.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PathKey(PathBuf);

impl PathKey {
  pub fn as_path(&self) -> &Path {
    &self.0
  }
}

/// Logical key – used to compute resources from memory.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LogicalKey(String);

impl LogicalKey {
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Resources are wrapped in this type.
pub type Res<T> = Rc<RefCell<T>>;

/// Key used to get resource.
///
/// This is the single type used to query into `Store`. A key is a `DepKey` with phantom typing,
/// giving type hints on the target resource.
///
/// You can use the `Key::path` and `Key::logical` functions to create new keys.
///
/// > `let the_key = Key::path("maps/deck_16/room_3.bsp");`
pub struct Key<T> where T: Load {
  inner: T::Key,
  _t: PhantomData<*const T>
}

impl<T> Key<T> where T: Load<Key = PathKey> {
  /// Create a new key from a path.
  ///
  /// Because the path needs to be canonicalized, this function might fail if the canonicalization
  /// cannot happen.
  pub fn path<P>(path: P) -> io::Result<Self> where P: AsRef<Path> {
    let canon_path = path.as_ref().canonicalize()?;

    Ok(Key {
      inner: PathKey(canon_path),
      _t: PhantomData
    })
  }

  /// Get the underlying, canonicalized path.
  pub fn as_path(&self) -> &Path {
    self.inner.as_path()
  }
}

impl<T> Key<T> where T: Load<Key = LogicalKey> {
  /// Create a new logical key.
  pub fn logical(id: &str) -> Self {
    Key {
      inner: LogicalKey(id.to_owned()),
      _t: PhantomData
    }
  }

  /// Get the underlying name.
  pub fn as_str(&self) -> &str {
    self.inner.as_str()
  }
}

impl<T> Clone for Key<T> where T: Load, T::Key: Clone {
  fn clone(&self) -> Self {
    Key {
      inner: self.inner.clone(),
      _t: PhantomData
    }
  }
}

impl<T> fmt::Debug for Key<T> where T: Load, T::Key: fmt::Debug {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    self.inner.fmt(f)
  }
}

impl<T> Eq for Key<T> where T: Load, T::Key: Eq {}

impl<T> hash::Hash for Key<T> where T: Load, T::Key: hash::Hash {
  fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
    self.inner.hash(state)
  }
}

impl<T> PartialEq for Key<T> where T: Load, T::Key: PartialEq {
  fn eq(&self, rhs: &Self) -> bool {
    self.inner.eq(&rhs.inner)
  }
}

impl<T> CacheKey for Key<T> where T: 'static + Load, T::Key: hash::Hash {
  type Target = Res<T>;
}

impl<T> From<Key<T>> for DepKey where T: Load {
  fn from(key: Key<T>) -> Self {
    key.inner.into()
  }
}

/// Resource store. Responsible for holding and presenting resources.
pub struct Store {
  // store options
  opt: StoreOpt,
  // canonicalized root path (used for resources loaded from the file system)
  canon_root: PathBuf,
  // resource cache, containing all living resources
  cache: HashCache,
  // contains all metadata on resources (reload functions, last time updated, etc.)
  metadata: HashMap<DepKey, ResMetaData>,
  // dependencies, mapping a dependency to its dependent resources
  deps: HashMap<DepKey, Vec<DepKey>>,
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
  ///
  /// The resource might be refused for several reasons. Further information in the documentation of
  /// the `StoreError` error type.
  fn inject<T>(
    &mut self,
    key: Key<T>,
    resource: T,
    deps: Vec<DepKey>
  ) -> Result<Res<T>, StoreError>
  where T: Load,
        T::Key: Clone + hash::Hash {
    let inner_key = key.inner.clone();
    let dep_key = inner_key.clone().into();

    // we forbid having two resources sharing the same key
    if self.metadata.contains_key(&dep_key) {
      return Err(StoreError::AlreadyRegisteredKey(dep_key.clone()));
    }

    // wrap the resource to make it shared mutably
    let res = Rc::new(RefCell::new(resource));
    let res_ = res.clone();

    // closure used to reload the object when needed
    let on_reload: Box<for<'a> Fn(&'a mut Store) -> Result<(), Box<Error>>> = Box::new(move |store| {
      let reloaded = T::reload(&res_.borrow(), inner_key.clone(), store);

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
    self.metadata.insert(dep_key.clone(), metadata);

    // register the resource as an observer of its dependencies in the dependencies graph
    for dep in deps {
      self.deps.entry(dep.clone()).or_insert(Vec::new()).push(dep_key.clone());
    }

    Ok(res)
  }

  /// Get a resource from the store and return an error if loading failed.
  pub fn get<T>(
    &mut self,
    key: &Key<T>
  ) -> Result<Res<T>, StoreErrorOr<T>>
  where T: Load,
        T::Key: Clone + hash::Hash {
    match self.cache.get(key).cloned() {
      Some(resource) => {
        Ok(resource)
      },
      None => {
        let loaded = T::load(key.inner.clone(), self).map_err(StoreErrorOr::ResError)?;
        self.inject(key.clone(), loaded.res, loaded.deps).map_err(StoreErrorOr::StoreError)
      }
    }
  }
  
  /// Get a resource from the store for the given key. If it fails, a proxied version is used, which
  /// will get replaced by the resource once it’s available and reloaded.
  pub fn get_proxied<T, P>(
    &mut self,
    key: &Key<T>,
    proxy: P
  ) -> Result<Res<T>, StoreError>
  where T: Load,
        T::Key: Clone + hash::Hash,
        P: FnOnce() -> T {
    self.get(key).or(self.inject(key.clone(), proxy(), Vec::new()))
  }

  /// Synchronize the store by updating the resources that ought to.
  pub fn sync(&mut self) {
    let update_await_time_ms = self.opt.update_await_time_ms();

    let dep_keys = dequeue_file_changes(&mut self.watcher_rx);

    for dep_key in dep_keys {
      // find the path in our watched paths; if we find it, we remove it prior to doing anything else
      if let Some(mut metadata) = self.metadata.remove(&dep_key) {
        let now = Instant::now();

        // perform a timed check so that we don’t reload several times in a row the same goddamn
        // resource
        if now.duration_since(metadata.last_update_instant) >= Duration::from_millis(update_await_time_ms) {
          if (metadata.on_reload)(self).is_ok() {
            // if we have successfully reloaded the resource, notify the observers that this
            // dependency has changed
            if let Some(deps) = self.deps.get(&dep_key).cloned() {
              for dep in deps {
                if let Some(obs_metadata) = self.metadata.remove(&dep) {
                  match (obs_metadata.on_reload)(self) {
                    Ok(_) => { self.metadata.insert(dep, obs_metadata); }
                    _ => ()
                  }
                }
              }
            }
          }
        }

        metadata.last_update_instant = now;
        self.metadata.insert(dep_key, metadata);
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
  RootDoesDotExit(PathBuf),
  /// The key associated with a resource already exists in the `Store`.
  ///
  /// > Note: it is not currently possible to have two resources living in a `Store` and using an
  /// > identical key at the same time.
  AlreadyRegisteredKey(DepKey)
}

impl fmt::Display for StoreError {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl Error for StoreError {
  fn description(&self) -> &str {
    match *self {
      StoreError::RootDoesDotExit(_) => "root doesn’t exist",
      StoreError::AlreadyRegisteredKey(_) => "already registered key"
    }
  }
}

/// Either a store error or a resource loading error.
pub enum StoreErrorOr<T> where T: Load {
  /// A store error.
  StoreError(StoreError),
  /// A resource error.
  ResError(T::Error)
}

impl<T> Clone for StoreErrorOr<T> where T: Load, T::Error: Clone {
  fn clone(&self) -> Self {
    match *self {
      StoreErrorOr::StoreError(ref e) => StoreErrorOr::StoreError(e.clone()),
      StoreErrorOr::ResError(ref e) => StoreErrorOr::ResError(e.clone())
    }
  }
}

impl<T> Eq for StoreErrorOr<T> where T: Load, T::Error: Eq {}

impl<T> PartialEq for StoreErrorOr<T> where T: Load, T::Error: PartialEq {
  fn eq(&self, rhs: &Self) -> bool {
    match (self, rhs) {
      (&StoreErrorOr::StoreError(ref a), &StoreErrorOr::StoreError(ref b)) => a == b,
      (&StoreErrorOr::ResError(ref a), &StoreErrorOr::ResError(ref b)) => a == b,
      _ => false
    }
  }
}

impl<T> fmt::Debug for StoreErrorOr<T> where T: Load, T::Error: fmt::Debug {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match *self {
      StoreErrorOr::StoreError(ref e) => f.debug_tuple("StoreError").field(e).finish(),
      StoreErrorOr::ResError(ref e) => f.debug_tuple("ResError").field(e).finish()
    }
  }
}

impl<T> fmt::Display for StoreErrorOr<T> where T: Load, T::Error: fmt::Debug {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<T> Error for StoreErrorOr<T> where T: Load, T::Error: fmt::Debug {
  fn description(&self) -> &str {
    match *self {
      StoreErrorOr::StoreError(ref e) => e.description(),
      StoreErrorOr::ResError(ref e) => e.description()
    }
  }

  fn cause(&self) -> Option<&Error> {
    match *self {
      StoreErrorOr::StoreError(ref e) => e.cause(),
      StoreErrorOr::ResError(ref e) => e.cause()
    }
  }
}

// TODO: profile and see how not to allocate anything
/// Dequeue any file changes from a watcher channel and output them in a buffer.
fn dequeue_file_changes(rx: &mut Receiver<RawEvent>) -> Vec<DepKey> {
  rx.try_iter().filter_map(|event| {
    match event {
      RawEvent { path: Some(ref path), op: Ok(op), .. } if op | WRITE != Op::empty() => {
        Some(DepKey::Path(PathKey(path.to_owned())))
      },
      _ => None
    }
  }).collect()
}
