//! Module exporting all key types recognized by `warmy`.
//!
//! This module provides you with three main types:
//!
//!   - `FSKey`:
//!   - `LogicalKey`.
//!   - `DeyKep`.

use any_cache::CacheKey;
use std::hash;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};

use res::{AsyncRes, Res};

/// A dependency key, used to express dependency.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DepKey {
  /// A key to a resource living on the filesystem – akin to `FSKey`.
  Path(PathBuf),
  /// A key to a resource living in memory or computed on the fly – akin to `LogicalKey`.
  Logical(String),
}

/// Filesystem key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FSKey(PathBuf);

impl FSKey {
  /// Create a new `FSKey` by providing a VFS path.
  ///
  /// The VFS path should start with a leading `"/"` (yet it’s not enforced). This VFS path will
  /// get transformed by a `Store` when used by inspecting the `Store`’s root.
  pub fn new<P>(path: P) -> Self
  where P: AsRef<Path> {
    FSKey(path.as_ref().to_owned())
  }

  /// Get the underlying path.
  pub fn as_path(&self) -> &Path {
    self.0.as_path()
  }
}

impl From<FSKey> for DepKey {
  fn from(key: FSKey) -> Self {
    DepKey::Path(key.0)
  }
}

/// Logical or memory key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LogicalKey(String);

impl LogicalKey {
  /// Create a new `LogicalKey` by prodiving a string of data.
  pub fn new<S>(s: S) -> Self
  where S: AsRef<str> {
    LogicalKey(s.as_ref().to_owned())
  }

  /// Get the data the key holds.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<LogicalKey> for DepKey {
  fn from(key: LogicalKey) -> Self {
    DepKey::Logical(key.0)
  }
}

/// Class of keys recognized by `warmy`.
pub trait Key: Clone + hash::Hash + Into<DepKey> {
  /// Prepare a key.
  ///
  /// If your key is akin to `FSKey`, it’s very likely you need to substitute its VFS path with the
  /// `root` argument. It’s advised to use the `prepare_key` method for your inner key value.
  ///
  /// > General note: you shouldn’t have to worry about implementing this trait as the interface
  /// > will often use any type of key that implements `Into<K> where K: Key` – for instance,
  /// > `FSKey`. You’re **strongly advised** to implement `From<YourKey> for FSKey` instead, unless
  /// > you know exactly what you’re doing.
  fn prepare_key(self, root: &Path) -> Self;
}

impl Key for DepKey {
  fn prepare_key(self, root: &Path) -> Self {
    match self {
      DepKey::Path(path) => DepKey::Path(vfs_substite_path(&path, root)),
      DepKey::Logical(x) => DepKey::Logical(x),
    }
  }
}

impl Key for FSKey {
  fn prepare_key(self, root: &Path) -> Self {
    FSKey(vfs_substite_path(self.as_path(), root))
  }
}

impl Key for LogicalKey {
  fn prepare_key(self, _: &Path) -> Self {
    self
  }
}

/// Substitute a VFS path by a real one.
fn vfs_substite_path(path: &Path, root: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let root_components = root.components();

  match components.peek() {
    Some(&Component::RootDir) => {
      // drop the root component
      root_components.chain(components.skip(1)).collect()
    }

    _ => root_components.chain(components).collect(),
  }
}

pub(crate) struct PrivateKey<T>(DepKey, PhantomData<T>);

impl<T> PrivateKey<T> {
  pub(crate) fn new(dep_key: DepKey) -> Self {
    PrivateKey(dep_key, PhantomData)
  }
}

impl<T> hash::Hash for PrivateKey<T> {
  fn hash<H>(&self, state: &mut H)
  where H: hash::Hasher {
    self.0.hash(state)
  }
}

impl<T> CacheKey for PrivateKey<T>
where T: 'static
{
  type Target = Res<T>;
}

pub(crate) struct AsyncPrivateKey<T>(DepKey, PhantomData<T>);

impl<T> AsyncPrivateKey<T> {
  pub(crate) fn new(dep_key: DepKey) -> Self {
    AsyncPrivateKey(dep_key, PhantomData)
  }
}

impl<T> hash::Hash for AsyncPrivateKey<T> {
  fn hash<H>(&self, state: &mut H)
  where H: hash::Hasher {
    self.0.hash(state)
  }
}

impl<T> CacheKey for AsyncPrivateKey<T>
where T: 'static
{
  type Target = AsyncRes<T>;
}
