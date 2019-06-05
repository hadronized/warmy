// this example can be run with 'cargo run --example toml --features toml_impl'
use serde::Deserialize;
use std::env;
use std::path;
use std::thread::sleep;
use std::time::Duration;
use warmy::toml_impl::Toml;
use warmy::{Res, SimpleKey, Store, StoreOpt};

#[derive(Debug, Deserialize)]
struct Config {
  msg: String,
}

fn main() {
  let mut store: Store<(), SimpleKey> = Store::new(StoreOpt::default()).unwrap();
  let ctx = &mut ();
  // using cargo manifest dir to build our path lets us find our resource
  // even if the cwd is not in the project root
  let resource_path = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
    let mut path = path::PathBuf::from(manifest_dir);
    path.push("examples/toml/hello.toml");
    path
  } else {
    path::PathBuf::from("/examples/toml/hello.toml")
  };

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
