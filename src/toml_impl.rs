//! toml universal implementors.
//!
//! This module provides you with universal implementation for any type that implements [`serde::Deserialize`].
//!
//! [`serde::Deserialize`]: https://docs.rs/serde/1.0.85/serde/trait.Deserialize.html

use serde::Deserialize;
use std::io;
use std::fmt;
use std::fs::File;
use std::path::PathBuf;

use crate::key::Key;
use crate::load::{Load, Loaded, Storage};
use toml::{self,from_str};
use std::io::Read;
use std::fs::read_to_string;

/// The Toml universal method. Use this with [`Storage::get_by`] or [`Storage::get_proxied_by`] to
/// benefit from the automatic implementors.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Toml;

/// Possible error that might occur while loading and reloading TOML formatted scarce resources.
#[derive(Debug)]
pub enum TomlError {
  /// An error in [toml](https://crates.io/crates/toml).
  TomlError(toml::de::Error),
  /// The file specified by the key failed to open.
  CannotReadFile(PathBuf, io::Error),
  /// The input key doesn’t provide enough information to open a file.
  NoKey
}

impl fmt::Display for TomlError {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match *self {
      TomlError::TomlError(ref e) => write!(f, "TOML error: {}", e),

      TomlError::CannotReadFile(ref path, ref e) => {
        write!(f, "cannot read file {}: {}", path.display(), e)
      }

      TomlError::NoKey => f.write_str("no path key available")
    }
  }
}

impl<C, K, T> Load<C, K, Toml> for T
where K: Key + Into<Option<PathBuf>>,
      T: 'static + for<'de> Deserialize<'de> {
  type Error = TomlError;

  fn load(
    key: K,
    _: &mut Storage<C, K>,
    _: &mut C
  ) -> Result<Loaded<Self, K>, Self::Error> {
    if let Some(path) = key.into() {
      let file_content = read_to_string(path)
          .map_err(|ioerr| TomlError::CannotReadFile(path, ioerr))?;

      from_str(&file_content)
        .map(Loaded::without_dep)
        .map_err(TomlError::TomlError)
    } else {
      Err(TomlError::NoKey)
    }
  }
}
