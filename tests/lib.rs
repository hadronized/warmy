extern crate warmy;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use warmy::{Key, Load, LoadResult, Store};

mod utils;

#[test]
fn create_store() {
  utils::with_store(|_, _| {})
}

#[test]
fn foo() {
  #[derive(Debug, Eq, PartialEq)]
  struct Foo(String);

  impl Load for Foo {
    type Error = ();

    fn from_fs<P>(path: P, _: &mut Store) -> Result<LoadResult<Self>, ()> where P: AsRef<Path> {
      let mut s = String::new();

      {
        let mut fh = File::open(path.as_ref()).unwrap();
        let _ = fh.read_to_string(&mut s);
      }

      let foo = Foo(s);

      Ok(foo.into())
    }
  }

  utils::with_store(|mut store, root_dir| {
    let expected1 = "Hello, world!".to_owned();
    let expected2 = "Bye!".to_owned();

    let path = root_dir.join("foo");

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected1.as_bytes());
    }

    let r = store.get(&Key::<Foo>::new(&path)).expect("object should be present at the given key");

    assert_eq!(r.borrow().0, expected1);

    {
      let mut fh = File::create(&path).unwrap();
      let _ = fh.write_all(expected2.as_bytes());
    }

    store.sync();

    assert_eq!(r.borrow().0, expected2);
  })
}
