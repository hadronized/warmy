extern crate warmy;

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use warmy::{Key, Load, Loaded, LogicalKey, PathKey, Storage};

mod utils;

/// Timeout in milliseconds to wait before determining that there’s something wrong with notify.
const QUEUE_TIMEOUT_MS: u64 = 10000; // 10s

#[derive(Debug, Eq, PartialEq)]
struct Foo(String);

#[derive(Debug, Eq, PartialEq)]
struct FooErr;

impl Error for FooErr {
  fn description(&self) -> &str {
    "Foo error!"
  }
}

impl fmt::Display for FooErr {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<C> Load<C> for Foo {
  type Key = PathKey;

  type Error = FooErr;

  fn load(key: Self::Key, _: &mut C, _: &mut Storage<C>) -> Result<Loaded<Self>, Self::Error> {
    let mut s = String::new();

    {
      let path = key.as_path();
      eprintln!("KEY: {}", path.display());
      let mut fh = File::open(path).unwrap();
      let _ = fh.read_to_string(&mut s);
    }

    let foo = Foo(s);

    Ok(foo.into())
  }
}

#[derive(Debug, Eq, PartialEq)]
struct Bar(String);

#[derive(Debug, Eq, PartialEq)]
struct BarErr;

impl Error for BarErr {
  fn description(&self) -> &str {
    "Bar error!"
  }
}

impl fmt::Display for BarErr {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<C> Load<C> for Bar {
  type Key = PathKey;

  type Error = BarErr;

  fn load(_: Self::Key, _: &mut C, _: &mut Storage<C>) -> Result<Loaded<Self>, Self::Error> {
    let bar = Bar("bar".to_owned());
    Ok(bar.into())
  }
}

#[derive(Debug, Eq, PartialEq)]
struct Zoo(String);

#[derive(Debug, Eq, PartialEq)]
struct ZooErr;

impl Error for ZooErr {
  fn description(&self) -> &str {
    "Zoo error!"
  }
}

impl fmt::Display for ZooErr {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<C> Load<C> for Zoo {
  type Key = LogicalKey;

  type Error = ZooErr;

  fn load(key: Self::Key, _: &mut C, _: &mut Storage<C>) -> Result<Loaded<Self>, Self::Error> {
    let content = key.as_str().to_owned();
    let zoo = Zoo(content);

    Ok(zoo.into())
  }
}

#[derive(Debug, Eq, PartialEq)]
struct LogicalFoo(String);

#[derive(Debug, Eq, PartialEq)]
struct LogicalFooErr;

impl Error for LogicalFooErr {
  fn description(&self) -> &str {
    "Logical Foo error!"
  }
}

impl fmt::Display for LogicalFooErr {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    f.write_str(self.description())
  }
}

impl<C> Load<C> for LogicalFoo {
  type Key = LogicalKey;

  type Error = LogicalFooErr;

  fn load(
    key: Self::Key,
    c: &mut C,
    storage: &mut Storage<C>,
  ) -> Result<Loaded<Self>, Self::Error> {
    let key: Key<Foo, C> = Key::path(key.as_str()).expect("logical foo path");
    let foo = storage.get(&key, c).unwrap();

    let content = foo.borrow().0.clone();
    let zoo = LogicalFoo(content);

    let r = Loaded::with_deps(zoo, vec![key.into()]);
    Ok(r)
  }
}

#[test]
fn create_store() {
  utils::with_store(|_: warmy::Store<()>| {})
}

#[test]
fn foo() {
  utils::with_store(|mut store| {
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();
    let path = store.root().join("foo.txt");
    let ctx = &mut ();

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let key: Key<Foo, ()> = Key::path(&path).unwrap();

    let r = store
      .get(&key, ctx)
      .expect("object should be present at the given key");

    assert_eq!(r.borrow().0, expected1);

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected2.as_bytes());
    }

    let start_time = ::std::time::Instant::now();
    loop {
      store.sync(ctx);

      if r.borrow().0.as_str() == expected2.as_str() {
        break;
      }

      if start_time.elapsed() >= ::std::time::Duration::from_millis(QUEUE_TIMEOUT_MS) {
        panic!(
          "more than {} milliseconds were spent waiting for a filesystem event",
          QUEUE_TIMEOUT_MS
        );
      }
    }
  })
}

#[test]
fn two_same_paths_diff_types() {
  utils::with_store(|mut store| {
    let path = store.root().join("a.txt");
    let ctx = &mut ();

    // create a.txt
    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(&b"foobarzoo"[..]);
    }

    let foo_key: Key<Foo, ()> = Key::path(&path).unwrap();
    let bar_key: Key<Bar, ()> = Key::path(&path).unwrap();

    let foo = store.get(&foo_key, ctx).unwrap();
    assert_eq!(foo.borrow().0.as_str(), "foobarzoo");

    let bar = store.get(&bar_key, ctx);
    assert!(bar.is_err());
  })
}

#[test]
fn logical_resource() {
  utils::with_store(|mut store| {
    let ctx = &mut ();
    let key: Key<Zoo, ()> = Key::logical("mem/uid/32197");
    let zoo = store.get(&key, ctx).unwrap();
    assert_eq!(zoo.borrow().0.as_str(), "mem/uid/32197");
  })
}

#[test]
fn logical_with_deps() {
  utils::with_store(|mut store| {
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();
    let path = store.root().join("foo.txt");
    let ctx = &mut ();

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let foo_key: Key<Foo, ()> = Key::path(&path).unwrap();

    let _ = store
      .get(&foo_key, ctx)
      .expect("object should be present at the given key");

    let log_foo_key: Key<LogicalFoo, ()> = Key::logical(path.to_str().unwrap());
    let log_foo = store.get(&log_foo_key, ctx).unwrap();

    assert_eq!(log_foo.borrow().0.as_str(), "Hello, world!");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected2.as_bytes());
    }

    let start_time = ::std::time::Instant::now();
    loop {
      store.sync(ctx);

      if log_foo.borrow().0.as_str() == expected2.as_str() {
        break;
      }

      if start_time.elapsed() >= ::std::time::Duration::from_millis(QUEUE_TIMEOUT_MS) {
        panic!(
          "more than {} milliseconds were spent waiting for a filesystem event",
          QUEUE_TIMEOUT_MS
        );
      }
    }
  })
}
