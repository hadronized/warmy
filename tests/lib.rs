extern crate warmy;

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use warmy::{FSKey, Load, Loaded, LogicalKey, Res, Storage, Store};

mod utils;

/// Timeout in milliseconds to wait before determining that there’s something wrong with notify.
const QUEUE_TIMEOUT_MS: u64 = 5000; // 5s

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
  type Key = FSKey;

  type Error = FooErr;

  fn load(key: Self::Key, _: &mut Storage<C>, _: &mut C) -> Result<Loaded<Self>, Self::Error> {
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

struct Stupid;

// a tricky version that doesn’t actually read the file but return something constant… it’s stupid,
// but it’s there to test methods
impl<C> Load<C, Stupid> for Foo {
  type Key = FSKey;

  type Error = FooErr;

  fn load(_: Self::Key, _: &mut Storage<C>, _: &mut C) -> Result<Loaded<Self>, Self::Error> {
    eprintln!("hello");
    let foo = Foo("stupid".to_owned());
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
  type Key = FSKey;

  type Error = BarErr;

  fn load(_: Self::Key, _: &mut Storage<C>, _: &mut C) -> Result<Loaded<Self>, Self::Error> {
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

  fn load(key: Self::Key, _: &mut Storage<C>, _: &mut C) -> Result<Loaded<Self>, Self::Error> {
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
    storage: &mut Storage<C>,
    ctx: &mut C,
  ) -> Result<Loaded<Self>, Self::Error>
  {
    let fs_key = FSKey::new(key.as_str());
    let foo: Res<Foo> = storage.get(&fs_key, ctx).unwrap();

    let content = foo.borrow().0.clone();
    let zoo = LogicalFoo(content);

    let r = Loaded::with_deps(zoo, vec![fs_key.into()]);
    Ok(r)
  }
}

#[test]
fn create_store() {
  utils::with_store(|_: Store<()>| {})
}

#[test]
fn foo() {
  utils::with_store(|mut store| {
    let ctx = &mut ();
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let key = FSKey::new("foo.txt");
    let path = store.root().join("foo.txt");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let r: Res<Foo> = store
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
fn foo_with_leading_slash() {
  utils::with_store(|mut store| {
    let ctx = &mut ();
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let key = FSKey::new("/foo.txt");
    let path = store.root().join("foo.txt");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let r: Res<Foo> = store
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
    let ctx = &mut ();
    let foo_key = FSKey::new("a.txt");
    let bar_key = foo_key.clone();
    let path = store.root().join("a.txt");

    // create a.txt
    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(&b"foobarzoo"[..]);
    }

    let foo: Res<Foo> = store.get(&foo_key, ctx).unwrap();
    assert_eq!(foo.borrow().0.as_str(), "foobarzoo");

    let bar: Result<Res<Bar>, _> = store.get(&bar_key, ctx);
    assert!(bar.is_err());
  })
}

#[test]
fn logical_resource() {
  utils::with_store(|mut store| {
    let key = LogicalKey::new("mem/uid/32197");
    let zoo: Res<Zoo> = store.get(&key, &mut ()).unwrap();
    assert_eq!(zoo.borrow().0.as_str(), "mem/uid/32197");
  })
}

#[test]
fn logical_with_deps() {
  utils::with_store(|mut store| {
    let ctx = &mut ();
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let foo_key = FSKey::new("foo.txt");
    let path = store.root().join("foo.txt");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let _: Res<Foo> = store
      .get(&foo_key, ctx)
      .expect("object should be present at the given key");

    let log_foo_key = LogicalKey::new(foo_key.as_path().to_str().unwrap());
    let log_foo: Res<LogicalFoo> = store.get(&log_foo_key, ctx).unwrap();

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
        eprintln!("log_foo = {:?}", log_foo.borrow().0.as_str());
        panic!(
          "more than {} milliseconds were spent waiting for a filesystem event",
          QUEUE_TIMEOUT_MS
        );
      }
    }
  })
}

#[derive(Debug, Eq, PartialEq)]
struct FooWithCtx(String);

#[derive(Debug, Eq, PartialEq)]
struct Ctx {
  count: i32,
}

impl Load<Ctx> for FooWithCtx {
  type Key = FSKey;

  type Error = FooErr;

  fn load(
    key: Self::Key,
    storage: &mut Storage<Ctx>,
    ctx: &mut Ctx,
  ) -> Result<Loaded<Self>, Self::Error>
  {
    // load as if it was a Foo
    let Loaded { res, deps } = <Foo as Load<_, ()>>::load(key, storage, ctx)?;

    // increment the counter
    ctx.count += 1;

    let r = Loaded::with_deps(FooWithCtx(res.0), deps);
    Ok(r)
  }
}

#[test]
fn foo_with_ctx() {
  utils::with_store(|mut store: Store<Ctx>| {
    let mut ctx = Ctx { count: 0 };

    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let key = FSKey::new("foo.txt");
    let path = store.root().join("foo.txt");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let r: Res<FooWithCtx> = store
      .get(&key, &mut ctx)
      .expect("object should be present at the given key");

    assert_eq!(r.borrow().0, expected1);

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected2.as_bytes());
    }

    let start_time = ::std::time::Instant::now();
    loop {
      store.sync(&mut ctx);

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

    assert_eq!(ctx.count, 2);
  })
}

#[test]
fn foo_by_stupid() {
  utils::with_store(|mut store: Store<()>| {
    let ctx = &mut ();
    let expected = "stupid";

    let key = FSKey::new("foo.txt");
    let path = store.root().join("foo.txt");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(&b"Hello, world!"[..]);
    }

    let r: Res<Foo> = store
      .get_by(&key, ctx, Stupid)
      .expect("object should be present at the given key");

    assert_eq!(&r.borrow().0, expected);
  })
}
