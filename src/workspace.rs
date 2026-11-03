use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::formats;
use crate::link::{Found, Link, SyntaxError};

pub struct LinkedDocument {
    pub document: Document,
    pub links: Vec<Link>,
    pub syntax_errors: Vec<SyntaxError>,
    /// What only its links are written in.
    pub link_syntax: Vec<Range<usize>>,
}

impl LinkedDocument {
    /// The [`document`](lynk:rust.argument) with the links
    /// [`find`](lynk:rust.argument) finds in it.
    pub fn new(document: Document, find: impl FnOnce(&Document) -> Found) -> Self {
        let found = find(&document);
        Self {
            document,
            links: found.links,
            syntax_errors: found.errors,
            link_syntax: found.syntax,
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
        for path in files(&root)? {
            let Some(format) = formats::of(&path) else {
                continue;
            };
            let text = std::fs::read_to_string(&path)?;
            let relative_path = path.strip_prefix(&root).unwrap_or(&path).to_owned();
            documents.push(format.read(relative_path, text));
        }
        documents.sort_by(|first, second| first.document.path.cmp(&second.document.path));
        Ok(Self { root, documents })
    }

    pub fn document(&self, path: &Path) -> Option<&LinkedDocument> {
        self.documents.iter().find(|linked| linked.document.path == path)
    }
}

/// Every file inside [`directory`](lynk:rust.argument), at any depth, that
/// belongs to the repository: ignore files are respected and hidden entries
/// skipped. Symlinks are skipped too, so a file reachable under two names
/// is found once.
pub fn files(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in ignore::WalkBuilder::new(directory).build() {
        let entry = entry.map_err(std::io::Error::other)?;
        if entry.file_type().is_some_and(|file_type| file_type.is_file()) {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// The nearest ancestor of [`start`](lynk:rust.argument) holding a `.jj` or
/// `.git` directory.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|directory| directory.join(".jj").is_dir() || directory.join(".git").is_dir())
        .map(Path::to_owned)
}
