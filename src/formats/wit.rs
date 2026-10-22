//! WIT files, whose comments are written as in Rust and read the same way.

use std::path::PathBuf;

use super::Format;
use crate::workspace::LinkedDocument;

pub const NAME: &str = "wit";

pub struct Wit;

impl Format for Wit {
    fn name(&self) -> &'static str {
        NAME
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["wit"]
    }

    fn read(&self, path: PathBuf, text: String) -> LinkedDocument {
        super::rust::read(NAME, path, text)
    }
}
