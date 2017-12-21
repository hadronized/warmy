//! Tooling for testing the crate.

extern crate tempdir;
extern crate warmy;

use self::tempdir::TempDir;
pub use std::fs::File;
use std::path::Path;

pub fn with_tmp_dir<F, B>(f: F) where F: Fn(&Path) -> B {
  let tmp_dir = TempDir::new("warmy").expect("create temporary directory");
  let _ = f(tmp_dir.path());
  tmp_dir.close().expect("close the temporary directory");
}

pub fn with_store<F, B>(f: F) where F: Fn(warmy::Store<()>, &Path) -> B {
  with_tmp_dir(|tmp_dir| {
    let opt =
      warmy::StoreOpt::default()
        .set_root(tmp_dir.to_owned())
        .set_update_await_time_ms(0);

    let store = warmy::Store::new(opt).expect("create store");
    f(store, tmp_dir)
  })
}
