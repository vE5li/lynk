use std::collections::HashSet;
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

    /// The document as if it held no links, for a file that is not
    /// scanned for them.
    fn without_links(self) -> Self {
        Self {
            links: Vec::new(),
            syntax_errors: Vec::new(),
            link_syntax: Vec::new(),
            ..self
        }
    }
}

/// The name of the files listing, as `.gitignore` files do, what lynk does
/// not scan for links, in the directory holding one and below it. What they
/// list is still read, so links may point into it.
pub const IGNORE_FILE_NAME: &str = ".lynkignore";

/// Every file of the repository that may contain links.
pub struct Workspace {
    pub root: PathBuf,
    pub documents: Vec<LinkedDocument>,
}

impl Workspace {
    pub fn load(root: PathBuf) -> std::io::Result<Self> {
        let scanned = scanned_files(&root)?;
        let mut documents = Vec::new();
        for path in files(&root)? {
            let Some(linked) = read(&root, &path, &scanned)? else {
                continue;
            };
            documents.push(linked);
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
    walk(ignore::WalkBuilder::new(directory))
}

/// The files of [`directory`](lynk:rust.argument), as
/// [`lynk::workspace::files`](lynk:rust.item) finds them, that are scanned
/// for links: those no file named
/// [`lynk::workspace::IGNORE_FILE_NAME`](lynk:rust.item) leaves out.
pub fn scanned_files(directory: &Path) -> std::io::Result<HashSet<PathBuf>> {
    let mut builder = ignore::WalkBuilder::new(directory);
    builder.add_custom_ignore_filename(IGNORE_FILE_NAME);
    Ok(walk(builder)?.into_iter().collect())
}

fn walk(builder: ignore::WalkBuilder) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(std::io::Error::other)?;
        if entry.file_type().is_some_and(|file_type| file_type.is_file()) {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// The file at [`path`](lynk:rust.argument), relative to
/// [`root`](lynk:rust.argument), read by its format, if it has one; its links
/// are left out unless it is one of the [`scanned`](lynk:rust.argument)
/// files.
pub fn read(root: &Path, path: &Path, scanned: &HashSet<PathBuf>) -> std::io::Result<Option<LinkedDocument>> {
    let Some(format) = formats::of(path) else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)?;
    let relative_path = path.strip_prefix(root).unwrap_or(path).to_owned();
    let linked = format.read(relative_path, text);
    if scanned.contains(path) {
        return Ok(Some(linked));
    }
    Ok(Some(linked.without_links()))
}

/// The nearest ancestor of [`start`](lynk:rust.argument) holding a `.jj` or
/// `.git` directory.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|directory| directory.join(".jj").is_dir() || directory.join(".git").is_dir())
        .map(Path::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_files_lynkignore_lists_without_their_links() {
        let root = std::env::temp_dir().join(format!("lynk-workspace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("docs/vendored")).unwrap();
        std::fs::write(root.join("docs/.lynkignore"), "vendored/\n").unwrap();
        std::fs::write(root.join("docs/a.md"), "[b](lynk:a.b)\n").unwrap();
        std::fs::write(root.join("docs/vendored/c.md"), "# C\n\n[d](lynk:a.b)\n").unwrap();
        let workspace = Workspace::load(root).unwrap();
        let scanned = workspace.document(Path::new("docs/a.md")).unwrap();
        assert_eq!(scanned.links.len(), 1);
        let unscanned = workspace.document(Path::new("docs/vendored/c.md")).unwrap();
        assert!(unscanned.links.is_empty());
        assert_eq!(unscanned.document.headings.len(), 1);
    }
}
