use std::path::{Path, PathBuf};

/// Various options to customize a `Store`.
///
/// Feel free to inspect all of its declared methods for further information.
pub struct StoreOpt {
  root: PathBuf,
  update_await_time_ms: u64,
}

impl Default for StoreOpt {
  fn default() -> Self {
    StoreOpt {
      root: PathBuf::from("."),
      update_await_time_ms: 50,
    }
  }
}

impl StoreOpt {
  /// Change the update await time (milliseconds) used to determine whether a resource should be
  /// reloaded or not.
  ///
  /// A `Store` will wait that amount of time before deciding an resource should be reloaded after
  /// it has changed on the filesystem. That is required in order to cope with write streaming, that
  /// generates a lot of write event.
  ///
  /// # Default
  ///
  /// Defaults to `50` milliseconds.
  #[inline]
  pub fn set_update_await_time_ms(self, ms: u64) -> Self {
    StoreOpt {
      update_await_time_ms: ms,
      ..self
    }
  }

  /// Get the update await time (milliseconds).
  #[inline]
  pub fn update_await_time_ms(&self) -> u64 {
    self.update_await_time_ms
  }

  /// Change the root directory from which the `Store` will be watching file changes.
  ///
  /// # Default
  ///
  /// Defaults to `"."`.
  #[inline]
  pub fn set_root<P>(self, root: P) -> Self
  where P: AsRef<Path> {
    StoreOpt {
      root: root.as_ref().to_owned(),
      ..self
    }
  }

  /// Get root directory.
  #[inline]
  pub fn root(&self) -> &Path {
    &self.root
  }
}

