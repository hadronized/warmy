extern crate warmy;

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use warmy::{Key, Load, Loaded, Store};

mod utils;

/// Timeout in milliseconds to wait before determining that there’s something wrong with notify.
const QUEUE_TIMEOUT_MS: u64 = 10000; // 10s

#[test]
fn create_store() {
  utils::with_store(|_, _| {})
}

#[test]
fn foo() {
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

  impl Load for Foo {
    type Error = FooErr;

    fn from_fs<P>(path: P, store: &mut Store) -> Result<Loaded<Self>, Self::Error> where P: AsRef<Path> {
      let mut s = String::new();

      {
        let path = store.root().join(path.as_ref());
        let mut fh = File::open(path).unwrap();
        let _ = fh.read_to_string(&mut s);
      }

      let foo = Foo(s);

      Ok(foo.into())
    }
  }

  utils::with_store(|mut store, root_dir| {
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let key: Key<Foo> = Key::new("foo");
    let path = root_dir.join(key.as_path());

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let r = store.get(&key).expect("object should be present at the given key");

    assert_eq!(r.borrow().0, expected1);

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected2.as_bytes());
    }

    let start_time = ::std::time::Instant::now();
    loop {
      store.sync();

      if r.borrow().0 == expected2 {
        break;
      }

      if start_time.elapsed() >= ::std::time::Duration::from_millis(QUEUE_TIMEOUT_MS) {
        panic!("more than {} milliseconds were spent waiting for a filesystem event", QUEUE_TIMEOUT_MS);
      }
    }
  })
}
