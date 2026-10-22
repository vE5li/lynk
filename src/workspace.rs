use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::format;
use crate::link::{Found, Link, SyntaxError};

pub struct LinkedDocument {
    pub document: Document,
    pub links: Vec<Link>,
    pub syntax_errors: Vec<SyntaxError>,
}

impl LinkedDocument {
    /// The [`document`](nexus:rust.argument) with the links
    /// [`find`](nexus:rust.argument) finds in it.
    pub fn new(document: Document, find: impl FnOnce(&Document) -> Found) -> Self {
        let found = find(&document);
        Self {
            document,
            links: found.links,
            syntax_errors: found.errors,
        }
    }
}

/// Every file of the repository that may contain links.
pub struct Workspace {
    pub root: PathBuf,
    pub documents: Vec<LinkedDocument>,
}

impl Workspace {
    pub fn load(root: PathBuf) -> std::io::Result<Self> {
        let mut documents = Vec::new();
        // Respects ignore files and skips hidden entries; symlinks are skipped
        // so a file reachable under two names is read once.
        for entry in ignore::WalkBuilder::new(&root).build() {
            let entry = entry.map_err(std::io::Error::other)?;
            let Some(format) = format::of(entry.path()) else {
                continue;
            };
            if !entry.file_type().is_some_and(|file_type| file_type.is_file()) {
                continue;
            }
            let text = std::fs::read_to_string(entry.path())?;
            let relative_path = entry.path().strip_prefix(&root).unwrap_or(entry.path()).to_owned();
            documents.push(format.read(relative_path, text));
        }
        documents.sort_by(|first, second| first.document.path.cmp(&second.document.path));
        Ok(Self { root, documents })
    }

    pub fn document(&self, path: &Path) -> Option<&LinkedDocument> {
        self.documents.iter().find(|linked| linked.document.path == path)
    }
}

/// The nearest ancestor of [`start`](nexus:rust.argument) holding a `.jj` or
/// `.git` directory.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|directory| directory.join(".jj").is_dir() || directory.join(".git").is_dir())
        .map(Path::to_owned)
}
