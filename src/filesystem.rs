//! The filesystem: a provider of files and directories, by their paths
//! relative to the repository's root.

use std::path::{Path, PathBuf};

use crate::link::{Member, Shape};
use crate::location::Span;
use crate::provider::{MemberError, Place, Provider, Query, check_complete_map, single_path};
use crate::workspace::Workspace;

pub const NAME: &str = "filesystem";

pub struct Filesystem;

impl Provider for Filesystem {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["file", "directory", "map-entry"]
    }

    fn shape(&self, selector: &str) -> Shape {
        Shape {
            map: selector == "map-entry",
            takes_arguments: true,
        }
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        let path = single_path(&format!("{NAME}.{selector}"), arguments)?;
        Ok(Box::new(match selector {
            "file" => FilesystemQuery::File { path },
            "directory" => FilesystemQuery::Directory { path },
            _ => FilesystemQuery::MapEntry { path },
        }))
    }
}

#[derive(Debug)]
enum FilesystemQuery {
    File { path: PathBuf },
    Directory { path: PathBuf },
    /// The entries of a directory, as complete map.
    MapEntry { path: PathBuf },
}

impl Query for FilesystemQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        let (path, exists) = match self {
            Self::File { path } => (path, workspace.root.join(path).is_file()),
            Self::Directory { path } | Self::MapEntry { path } => (path, workspace.root.join(path).is_dir()),
        };
        exists.then(|| Span::whole_file(path)).into_iter().collect()
    }

    fn check_members(&self, members: &[Member], workspace: &Workspace) -> Vec<MemberError> {
        match self {
            Self::MapEntry { path } => check_complete_map(members, &directory_entries(&workspace.root.join(path))),
            _ => Vec::new(),
        }
    }
}

/// The names of a directory's entries, skipping hidden and ignored ones as
/// the search for documents does.
fn directory_entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = ignore::WalkBuilder::new(directory)
        .max_depth(Some(1))
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.depth() == 1)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::tests::{assert_query, read_first_link};

    #[test]
    fn a_map_selector_reads_its_body_after_its_arguments() {
        let (query, members) =
            read_first_link("test.md", "<!-- nexus filesystem.map-entry\n  src -->\n- a: b\n- c: d \"quoted\"\n<!-- /nexus -->\n").unwrap();
        assert_query(query.as_ref(), &FilesystemQuery::MapEntry {
            path: PathBuf::from("src"),
        });
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("a", "b"), ("c", "d \"quoted\"")]);
    }

    #[test]
    fn paths_are_normalized() {
        let query = Filesystem.query("directory", &["./src/"], None).unwrap();
        assert_query(query.as_ref(), &FilesystemQuery::Directory {
            path: PathBuf::from("src"),
        });
    }
}
