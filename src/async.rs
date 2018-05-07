//! Load and reload resources… asynchronously.
//!
//! This module exposes traits, types and functions you need to use to load and reload objects via
//! the [futures] crate.
//!
//! [futures]: https://crates.io/crates/futures

use any_cache::{Cache, HashCache};
use futures::Future;
use futures::executor::Executor;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use key::{self, DepKey, Key, AsyncPrivateKey};
use res::AsyncRes;
use store_opt::StoreOpt;

pub trait Load<C, Method = ()>: 'static + Sized
where Method: ?Sized {
  /// Type of the key used to load the resource.
  type Key: key::Key + 'static;

  /// Type of error that might happen while loading.
  type Error: Error + 'static;

  /// Load a resource.
  ///
  /// The `Storage` can be used to load additional resource dependencies.
  ///
  /// The result type is used to register for dependency events. If you do not need any, you can
  /// lift your return value in `Loaded<_>` with `your_value.into()`.
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

/// Resource storage.
///
/// This type is responsible for storing resources, giving functions to look them up and update
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
  fn new(canon_root: PathBuf) -> Self {
    Storage {
      canon_root,
      cache: HashCache::new(),
      deps: HashMap::new(),
      metadata: HashMap::new(),
    }
  }

  /// The canonicalized root the `Storage` is configured with.
  pub fn root(&self) -> &Path {
    &self.canon_root
  }

  fn inject<T, M>(
    &mut self,
    key: T::Key,
    resource: T,
    deps: Vec<DepKey>,
  ) -> Result<AsyncRes<T>, StoreError>
  where
    T: Load<C, M>,
    T::Key: Clone + hash::Hash + Into<DepKey>,
  {
    let dep_key = key.clone().into();

    // we forbid having two resources sharing the same key
    if self.metadata.contains_key(&dep_key) {
      return Err(StoreError::AlreadyRegisteredKey(dep_key));
    }

    // wrap the resource to make it shared mutably
    let res = AsyncRes::new(resource);

    // create the metadata for the resource
    let res_ = res.clone();
    let key_ = key.clone();
    let metadata = ResMetaData::new(move |storage, ctx| {
      let reloaded = <T as Load<C, M>>::reload(&res_.borrow(), key_.clone(), storage, ctx);

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
    let pkey = AsyncPrivateKey::new(dep_key);

    // cache the resource
    self.cache.save(pkey, res.clone());

    Ok(res)
  }

  pub fn get_by<K, T, M, Ex>(
    &mut self,
    key: &K,
    ctx: &mut C,
    _: M,
  ) -> Result<AsyncRes<T>, StoreErrorOr<T, C, M>>
  where
    T: Load<C, M>,
    K: Clone + Into<T::Key> {
    let key_ = key.clone().into().prepare_key(self.root());
    let dep_key = key_.clone().into();
    let pkey = AsyncPrivateKey::<T>::new(dep_key);

    let x: Option<AsyncRes<T>> = self.cache.get(&pkey).cloned();

    match x {
      Some(resource) => Ok(resource),
      None => {
        let loaded =
          <T as Load<C, M>>::load(key_.clone(), self, ctx).map_err(StoreErrorOr::ResError)?;
        self
          .inject::<T, M>(key_, loaded.res, loaded.deps)
          .map_err(StoreErrorOr::StoreError)
      }
    }
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

/// Error that might happen when handling a resource store around.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
  /// The root path for a filesystem resource was not found.
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
pub enum StoreErrorOr<T, C, M = ()>
where T: Load<C, M> {
  /// A store error.
  StoreError(StoreError),
  /// A resource error.
  ResError(T::Error),
}

impl<T, C, M> Clone for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
  T::Error: Clone,
{
  fn clone(&self) -> Self {
    match *self {
      StoreErrorOr::StoreError(ref e) => StoreErrorOr::StoreError(e.clone()),
      StoreErrorOr::ResError(ref e) => StoreErrorOr::ResError(e.clone()),
    }
  }
}

impl<T, C, M> Eq for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
  T::Error: Eq,
{
}

impl<T, C, M> PartialEq for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
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

impl<T, C, M> fmt::Debug for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
  T::Error: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match *self {
      StoreErrorOr::StoreError(ref e) => f.debug_tuple("StoreError").field(e).finish(),
      StoreErrorOr::ResError(ref e) => f.debug_tuple("ResError").field(e).finish(),
    }
  }
}

impl<T, C, M> fmt::Display for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
  T::Error: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<T, C, M> Error for StoreErrorOr<T, C, M>
where
  T: Load<C, M>,
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
