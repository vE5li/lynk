//! The file types links are written in, each reading its files in its own
//! way: what of them is prose, what is code, and how links are written.

pub mod markdown;
pub mod protobuf;
pub mod rust;
pub mod wit;

use std::path::{Path, PathBuf};

use crate::workspace::LinkedDocument;

/// A file type that may hold links.
pub trait Format {
    /// Also the name of its provider, if it has one.
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    /// Reads a file at [`path`](nexus:rust.argument), relative to the root,
    /// as far as links need it, links included.
    fn read(&self, path: PathBuf, text: String) -> LinkedDocument;
}

pub const FORMATS: &[&dyn Format] = &[&markdown::Markdown, &rust::Rust, &wit::Wit, &protobuf::Protobuf];

/// The format of the file at [`path`](nexus:rust.argument), by its
/// extension.
pub fn of(path: &Path) -> Option<&'static dyn Format> {
    let extension = path.extension()?.to_str()?;
    let format = FORMATS.iter().find(|format| format.extensions().contains(&extension))?;
    Some(*format)
}
