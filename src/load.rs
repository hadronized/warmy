use any_cache::{Cache, HashCache};
use notify::{op::WRITE, raw_watcher, Op, RawEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::{collections::HashMap,
          error::Error,
          fmt,
          hash,
          ops::{Deref, DerefMut},
          path::{Path, PathBuf},
          sync::mpsc::{channel, Receiver},
          time::{Duration, Instant}};

use key::{self, DepKey, Key, PrivateKey};
use res::Res;

pub trait Load<C>: 'static + Sized {
  /// Type of the key used to load the resource.
  type Key: key::Key + 'static;

  /// Type of error that might happen while loading.
  type Error: Error + 'static;

  /// Load a resource.
  ///
  /// The `Store` can be used to load or declare additional resource dependencies.
  ///
  /// The result type is used to register for dependency events.
  fn load(
    key: Self::Key,
    storage: &mut Storage<C>,
    ctx: &mut C,
  ) -> Result<Loaded<Self>, Self::Error>;

  // FIXME: add support for redeclaring the dependencies?
  /// Function called when a resource must be reloaded.
  ///
  /// The default implementation of that function calls `load` and returns its result.
  fn reload(
    &self,
    key: Self::Key,
    storage: &mut Storage<C>,
    ctx: &mut C,
  ) -> Result<Self, Self::Error>
  {
    Self::load(key, storage, ctx).map(|lr| lr.res)
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
  pub deps: Vec<DepKey>,
}

impl<T> Loaded<T> {
  /// Return a resource declaring no dependency at all.
  pub fn without_dep(res: T) -> Self {
    Loaded {
      res,
      deps: Vec::new(),
    }
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

/// Metadata about a resource.
struct ResMetaData<C> {
  /// Function to call each time the resource must be reloaded.
  on_reload: Box<Fn(&mut Storage<C>, &mut C) -> Result<(), Box<Error>>>,
}

impl<C> ResMetaData<C> {
  fn new<F>(f: F) -> Self
  where F: 'static + Fn(&mut Storage<C>, &mut C) -> Result<(), Box<Error>> {
    ResMetaData {
      on_reload: Box::new(f),
    }
  }
}

/// Resource storage.
///
/// This type is responsible of storing resources and giving functions to look them up and update
/// them whenever needed.
pub struct Storage<C> {
  // canonicalized root path (used for resources loaded from the file system)
  canon_root: PathBuf,
  // resource cache, containing all living resources
  cache: HashCache,
  // dependencies, mapping a dependency to its dependent resources
  deps: HashMap<DepKey, Vec<DepKey>>,
  // contains all metadata on resources (reload functions)
  metadata: HashMap<DepKey, ResMetaData<C>>,
}

impl<C> Storage<C> {
  fn new(canon_root: PathBuf) -> Result<Self, StoreError> {
    Ok(Storage {
      canon_root,
      cache: HashCache::new(),
      deps: HashMap::new(),
      metadata: HashMap::new(),
    })
  }

  /// The canonicalized root the `Storage` is configured with.
  pub fn root(&self) -> &Path {
    &self.canon_root
  }

  /// Inject a new resource in the store.
  ///
  /// The resource might be refused for several reasons. Further information in the documentation of
  /// the `StoreError` error type.
  fn inject<T>(
    &mut self,
    key: T::Key,
    resource: T,
    deps: Vec<DepKey>,
  ) -> Result<Res<T>, StoreError>
  where
    T: Load<C>,
    T::Key: Clone + hash::Hash + Into<DepKey>,
  {
    let dep_key = key.clone().into();

    // we forbid having two resources sharing the same key
    if self.metadata.contains_key(&dep_key) {
      return Err(StoreError::AlreadyRegisteredKey(dep_key));
    }

    // wrap the resource to make it shared mutably
    let res = Res::new(resource);

    // create the metadata for the resource
    let res_ = res.clone();
    let key_ = key.clone();
    let metadata = ResMetaData::new(move |storage, ctx| {
      let reloaded = T::reload(&res_.borrow(), key_.clone(), storage, ctx);

      match reloaded {
        Ok(r) => {
          // replace the current resource with the freshly loaded one
          *res_.borrow_mut() = r;
          Ok(())
        }
        Err(e) => Err(Box::new(e)),
      }
    });

    self.metadata.insert(dep_key.clone(), metadata);

    // register the resource as an observer of its dependencies in the dependencies graph
    let root = &self.canon_root;
    for dep in deps {
      self
        .deps
        .entry(dep.clone().prepare_key(root))
        .or_insert(Vec::new())
        .push(dep_key.clone());
    }

    // wrap the key in our private key so that we can use it in the cache
    let pkey = PrivateKey::new(dep_key);

    // cache the resource
    self.cache.save(pkey, res.clone());

    Ok(res)
  }

  /// Get a resource from the `Storage` and return an error if loading failed.
  pub fn get<K, T>(&mut self, key: &K, ctx: &mut C) -> Result<Res<T>, StoreErrorOr<T, C>>
  where
    T: Load<C>,
    K: Clone + Into<T::Key>, {
    let key_ = key.clone().into().prepare_key(self.root());
    let dep_key = key_.clone().into();
    let pkey = PrivateKey::<T>::new(dep_key);

    let x: Option<Res<T>> = self.cache.get(&pkey).cloned();

    match x {
      Some(resource) => Ok(resource),
      None => {
        let loaded = T::load(key_.clone(), self, ctx).map_err(StoreErrorOr::ResError)?;
        self
          .inject(key_, loaded.res, loaded.deps)
          .map_err(StoreErrorOr::StoreError)
      }
    }
  }

  /// Get a resource from the `Storage` for the given key. If it fails, a proxied version is used,
  /// which will get replaced by the resource once it’s available and reloaded.
  pub fn get_proxied<K, T, P>(
    &mut self,
    key: &K,
    proxy: P,
    ctx: &mut C,
  ) -> Result<Res<T>, StoreError>
  where
    T: Load<C>,
    K: Clone + Into<T::Key>,
    P: FnOnce() -> T,
  {
    self
      .get(key, ctx)
      .or_else(|_| self.inject(key.clone().into(), proxy(), Vec::new()))
  }
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
  AlreadyRegisteredKey(DepKey),
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
      StoreError::AlreadyRegisteredKey(_) => "already registered key",
    }
  }
}

/// Either a store error or a resource loading error.
pub enum StoreErrorOr<T, C>
where T: Load<C> {
  /// A store error.
  StoreError(StoreError),
  /// A resource error.
  ResError(T::Error),
}

impl<T, C> Clone for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: Clone,
{
  fn clone(&self) -> Self {
    match *self {
      StoreErrorOr::StoreError(ref e) => StoreErrorOr::StoreError(e.clone()),
      StoreErrorOr::ResError(ref e) => StoreErrorOr::ResError(e.clone()),
    }
  }
}

impl<T, C> Eq for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: Eq,
{
}

impl<T, C> PartialEq for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: PartialEq,
{
  fn eq(&self, rhs: &Self) -> bool {
    match (self, rhs) {
      (&StoreErrorOr::StoreError(ref a), &StoreErrorOr::StoreError(ref b)) => a == b,
      (&StoreErrorOr::ResError(ref a), &StoreErrorOr::ResError(ref b)) => a == b,
      _ => false,
    }
  }
}

impl<T, C> fmt::Debug for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match *self {
      StoreErrorOr::StoreError(ref e) => f.debug_tuple("StoreError").field(e).finish(),
      StoreErrorOr::ResError(ref e) => f.debug_tuple("ResError").field(e).finish(),
    }
  }
}

impl<T, C> fmt::Display for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<T, C> Error for StoreErrorOr<T, C>
where
  T: Load<C>,
  T::Error: fmt::Debug,
{
  fn description(&self) -> &str {
    match *self {
      StoreErrorOr::StoreError(ref e) => e.description(),
      StoreErrorOr::ResError(ref e) => e.description(),
    }
  }

  fn cause(&self) -> Option<&Error> {
    match *self {
      StoreErrorOr::StoreError(ref e) => e.cause(),
      StoreErrorOr::ResError(ref e) => e.cause(),
    }
  }
}

/// Resource synchronizer.
///
/// An object of this type is responsible to synchronize resources living in a store. It keeps in
/// internal, optimized state to perform correct and efficient synchronization.
struct Synchronizer {
  // all the resources that must be reloaded; they’re mapped to the instant they were found updated
  dirties: HashMap<DepKey, Instant>,
  // keep the watcher around so that we don’t have it disconnected
  #[allow(dead_code)]
  watcher: RecommendedWatcher,
  // watcher receiver part of the channel
  watcher_rx: Receiver<RawEvent>,
  // time in milleseconds to wait before actually invoking the reloading function on a given
  // resource; the wait is done between the current time and the last time the resource was touched
  // by the event loop
  update_await_time_ms: u64,
}

impl Synchronizer {
  fn new(
    watcher: RecommendedWatcher,
    watcher_rx: Receiver<RawEvent>,
    update_await_time_ms: u64,
  ) -> Self
  {
    Synchronizer {
      dirties: HashMap::new(),
      watcher,
      watcher_rx,
      update_await_time_ms,
    }
  }

  /// Dequeue any file system events.
  fn dequeue_fs_events<C>(&mut self, storage: &Storage<C>) {
    for event in self.watcher_rx.try_iter() {
      match event {
        RawEvent {
          path: Some(ref path),
          op: Ok(op),
          ..
        } if op | WRITE != Op::empty() =>
        {
          let dep_key = DepKey::Path(path.to_owned());

          if storage.metadata.contains_key(&dep_key) {
            self.dirties.insert(dep_key, Instant::now());
          }
        }

        _ => (),
      }
    }
  }

  /// Reload any dirty resource that fulfill its time predicate.
  fn reload_dirties<C>(&mut self, storage: &mut Storage<C>, ctx: &mut C) {
    let update_await_time_ms = self.update_await_time_ms;

    self.dirties.retain(|dep_key, dirty_instant| {
      let now = Instant::now();

      // check whether we’ve waited enough to actually invoke the reloading code
      if now.duration_since(dirty_instant.clone()) >= Duration::from_millis(update_await_time_ms) {
        // we’ve waited enough; reload
        if let Some(metadata) = storage.metadata.remove(&dep_key) {
          if (metadata.on_reload)(storage, ctx).is_ok() {
            // if we have successfully reloaded the resource, notify the observers that this
            // dependency has changed
            if let Some(deps) = storage.deps.get(&dep_key).cloned() {
              for dep in deps {
                if let Some(obs_metadata) = storage.metadata.remove(&dep) {
                  // FIXME: decide what to do with the result (error?)
                  let _ = (obs_metadata.on_reload)(storage, ctx);

                  // reinject the dependency once afterwards
                  storage.metadata.insert(dep, obs_metadata);
                }
              }
            }
          }

          storage.metadata.insert(dep_key.clone(), metadata);
        }

        false
      } else {
        true
      }
    });
  }

  /// Synchronize the `Storage` by updating the resources that ought to.
  fn sync<C>(&mut self, storage: &mut Storage<C>, ctx: &mut C) {
    self.dequeue_fs_events(storage);
    self.reload_dirties(storage, ctx);
  }
}

/// Resource store. Responsible for holding and presenting resources.
pub struct Store<C> {
  storage: Storage<C>,
  synchronizer: Synchronizer,
}

impl<C> Store<C> {
  /// Create a new store.
  ///
  /// The `root` represents the root directory from all the resources come from and is relative to
  /// the binary’s location by default (unless you specify it as absolute).
  pub fn new(opt: StoreOpt) -> Result<Self, StoreError> {
    // canonicalize the root because some platforms won’t correctly report file changes otherwise
    let root = &opt.root;
    let canon_root = root
      .canonicalize()
      .map_err(|_| StoreError::RootDoesDotExit(root.to_owned()))?;

    // create the mpsc channel to communicate with the file watcher
    let (wsx, wrx) = channel();
    let mut watcher = raw_watcher(wsx).unwrap();

    // spawn a new thread in which we look for events
    let _ = watcher.watch(&canon_root, RecursiveMode::Recursive);

    // create the storage
    let storage = Storage::new(canon_root)?;

    // create the synchronizer
    let synchronizer = Synchronizer::new(watcher, wrx, opt.update_await_time_ms);

    let store = Store {
      storage,
      synchronizer,
    };

    Ok(store)
  }

  /// Synchronize the `Store` by updating the resources that ought to.
  pub fn sync(&mut self, ctx: &mut C) {
    self.synchronizer.sync(&mut self.storage, ctx);
  }
}

impl<C> Deref for Store<C> {
  type Target = Storage<C>;

  fn deref(&self) -> &Self::Target {
    &self.storage
  }
}

impl<C> DerefMut for Store<C> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.storage
  }
}

/// Various options to customize a `Store`.
pub struct StoreOpt {
  root: PathBuf,
  update_await_time_ms: u64,
}

impl Default for StoreOpt {
  fn default() -> Self {
    StoreOpt {
      root: PathBuf::from("."),
      update_await_time_ms: 50,
    }
  }
}

impl StoreOpt {
  /// Change the update await time (milliseconds) used to determine whether a resource should be
  /// reloaded or not.
  ///
  /// # Default
  ///
  /// Defaults to `50` milliseconds.
  #[inline]
  pub fn set_update_await_time_ms(self, ms: u64) -> Self {
    StoreOpt {
      update_await_time_ms: ms,
      ..self
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
  pub fn set_root<P>(self, root: P) -> Self
  where P: AsRef<Path> {
    StoreOpt {
      root: root.as_ref().to_owned(),
      ..self
    }
  }

  /// Get root directory.
  #[inline]
  pub fn root(&self) -> &Path {
    &self.root
  }
}
