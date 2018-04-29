//! Recognized methods by warmy.
//!
//! > Disclaimer: those methods are just there for indications for now. It’s very likely that they
//! > will get moved in separate crates when time comes.

/// JSON method.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct JSON;

/// YAML method.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct YAML;

/// XML method.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct XML;
