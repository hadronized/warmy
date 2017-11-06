extern crate warmy;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use warmy::{Key, Load, Loaded, Store};

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

    fn from_fs<P>(path: P, store: &mut Store) -> Result<Loaded<Self>, ()> where P: AsRef<Path> {
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

    // this sleep is necessary to prevent any race between our thread and the notify’s one
    ::std::thread::sleep(::std::time::Duration::from_millis(100));

    store.sync();

    assert_eq!(r.borrow().0, expected2);
  })
}
