// this example can be run with 'cargo run --example toml --features toml_impl'
use serde::Deserialize;
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

  let resource: Result<Res<Config>, _> = store.get_by(
    &SimpleKey::from_path("/examples/toml/hello.toml"),
    ctx,
    Toml,
  );

  match resource {
    Ok(config) => loop {
      store.sync(ctx);

      println!("The msg is: '{}'", config.borrow().msg);
      sleep(Duration::from_millis(1000));
    },

    Err(e) => eprintln!("{}", e),
  }
}
