//! This example can be run with `cargo run --example toml --features toml-impl`.

use serde::Deserialize;
use std::env;
use std::thread::sleep;
use std::time::Duration;
use warmy::toml::Toml;
use warmy::{Res, SimpleKey, Store, StoreOpt};

#[derive(Debug, Deserialize)]
struct Config {
  msg: String,
}

fn main() {
  // using cargo manifest directory to build the path lets us find our resource even if the cwd is
  // not the project root
  let store_opt = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
    StoreOpt::default().set_root(manifest_dir)
  } else {
    StoreOpt::default()
  };
  let resource_path = "/examples/toml/hello.toml";

  let mut store: Store<(), SimpleKey> = Store::new(store_opt).unwrap();
  let ctx = &mut ();

  let resource: Result<Res<Config>, _> =
    store.get_by(&SimpleKey::from_path(resource_path), ctx, Toml);

  match resource {
    Ok(config) => loop {
      store.sync(ctx);

      println!("The msg is: '{}'", config.borrow().msg);
      sleep(Duration::from_millis(1000));
    },

    Err(e) => eprintln!("{}", e),
  }
}
