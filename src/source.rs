//! Resource sources.
//!
//! Sources are means to get resources. Typicale resources sources are:
//!
//!   - File systems.
//!   - Networks.
//!   - Tarball or any kind of complex system holding resources.

use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use key::Key;

/// Type of error that can occur while getting a resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
  /// The resource wasn’t found at the provided key.
  Unfound,
  /// The resource failed to parse.
  ParseFailure(String)
}

/// The file system source, providing methods to get resources from the file system.
struct FileSystem {
  _void: ()
}

impl FileSystem {
  /// Read the content of a path and return a readable object.
  pub fn read(&self, path: &Path) -> Result<impl Read, Error> {
    File::open(path).map_err(|_| Error::Unfound)
  }

  /// Read the content of a path and invoke a parser function on it.
  pub fn parse<A, R, F>(&self, path: &Path, parse: F) -> Result<A, Error>
  where F: FnOnce(&str) -> Result<A, R>,
        R: Display {
    let mut file = self.read(path)?;
    let mut buf = String::new();

    file.read_to_string(&mut buf);

    parse(&buf).map_err(|e| Error::ParseFailure(format!("{}", e)))
  }
}
