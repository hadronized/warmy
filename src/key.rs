use any_cache::CacheKey;
use std::{hash,
          marker::PhantomData,
          path::{Component, Path, PathBuf}};

use res::Res;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DepKey {
  Path(PathBuf),
  Logical(String),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FSKey(PathBuf);

impl FSKey {
  pub fn new<P>(path: P) -> Self
  where P: AsRef<Path> {
    FSKey(path.as_ref().to_owned())
  }

  pub fn as_path(&self) -> &Path {
    self.0.as_path()
  }
}

impl From<FSKey> for DepKey {
  fn from(key: FSKey) -> Self {
    DepKey::Path(key.0)
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LogicalKey(String);

impl LogicalKey {
  pub fn new<S>(s: S) -> Self
  where S: AsRef<str> {
    LogicalKey(s.as_ref().to_owned())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<LogicalKey> for DepKey {
  fn from(key: LogicalKey) -> Self {
    DepKey::Logical(key.0)
  }
}

pub trait Key: Clone + hash::Hash + Into<DepKey> {
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
