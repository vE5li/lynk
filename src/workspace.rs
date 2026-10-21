use std::path::{Path, PathBuf};

use crate::document::{Document, Format};
use crate::link::{Link, SyntaxError, find_links};

pub struct LinkedDocument {
    pub document: Document,
    pub links: Vec<Link>,
    pub syntax_errors: Vec<SyntaxError>,
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
            let Some(format) = Format::of(entry.path()) else {
                continue;
            };
            if !entry.file_type().is_some_and(|file_type| file_type.is_file()) {
                continue;
            }
            let text = std::fs::read_to_string(entry.path())?;
            let relative_path = entry.path().strip_prefix(&root).unwrap_or(entry.path()).to_owned();
            let document = Document::parse(relative_path, format, text);
            let (links, syntax_errors) = find_links(&document);
            documents.push(LinkedDocument {
                document,
                links,
                syntax_errors,
            });
        }
        documents.sort_by(|first, second| first.document.path.cmp(&second.document.path));
        Ok(Self { root, documents })
    }

    pub fn document(&self, path: &Path) -> Option<&LinkedDocument> {
        self.documents.iter().find(|linked| linked.document.path == path)
    }
}

/// The nearest ancestor of <[rust.argument start]> holding a `.jj` or `.git`
/// directory.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|directory| directory.join(".jj").is_dir() || directory.join(".git").is_dir())
        .map(Path::to_owned)
}
