//! The dependencies of Cargo manifests, read with `toml`, which keeps where
//! every key and value is written; its design record's
//! [Decision](lynk:markdown.section
//! (design/reads-cargo-manifests-with-toml.md)) says why.

use std::ops::Range;
use std::path::{Path, PathBuf};

use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::providers::{Place, Provider, Query, normalize};
use crate::workspace::Workspace;

pub const NAME: &str = "cargo";

/// The tables a package declares its dependencies in, at the top of its
/// manifest or under a platform of its `target` table.
const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

pub struct Cargo;

impl Provider for Cargo {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["dependency"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        match arguments {
            [manifest, name] => Ok(Box::new(CargoDependency {
                manifest: normalize(manifest),
                name: (*name).to_owned(),
            })),
            _ => Err(format!("`{NAME}.{selector}` takes a manifest and a dependency")),
        }
    }
}

/// The dependencies of a Cargo manifest with a name, the key they are
/// declared under.
#[derive(Debug)]
struct CargoDependency {
    manifest: PathBuf,
    name: String,
}

impl Query for CargoDependency {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        find_dependency(&workspace.root, &self.manifest, &self.name)
    }
}

/// The spans of the dependencies called [`name`](lynk:rust.argument) in
/// the Cargo manifest at [`manifest`](lynk:rust.argument), relative to
/// [`root`](lynk:rust.argument), in every table declaring dependencies:
/// the package's, its platforms' and its workspace's. Each span runs from
/// the dependency's key, or its table's header, to the end of its last
/// value.
fn find_dependency(root: &Path, manifest: &Path, name: &str) -> Vec<Span> {
    let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
        return Vec::new();
    };
    let Ok(document) = DeTable::parse(&text) else {
        return Vec::new();
    };
    let line_index = LineIndex::new(&text);
    let mut ranges: Vec<Range<usize>> = dependency_tables(document.get_ref())
        .into_iter()
        .filter_map(|table| table.get_key_value(name))
        .map(|(key, value)| extent(key, value))
        .collect();
    ranges.sort_by_key(|range| range.start);
    ranges.into_iter().map(|range| line_index.span(manifest, range)).collect()
}

/// Every table of a manifest declaring dependencies.
fn dependency_tables<'a, 'i>(manifest: &'a DeTable<'i>) -> Vec<&'a DeTable<'i>> {
    let package_tables = |table: &'a DeTable<'i>| DEPENDENCY_TABLES.iter().filter_map(move |name| as_table(table.get(*name)?));
    let mut tables: Vec<_> = package_tables(manifest).collect();
    if let Some(targets) = manifest.get("target").and_then(as_table) {
        tables.extend(targets.values().filter_map(as_table).flat_map(package_tables));
    }
    if let Some(workspace) = manifest.get("workspace").and_then(as_table) {
        tables.extend(workspace.get("dependencies").and_then(as_table));
    }
    tables
}

fn as_table<'a, 'i>(value: &'a Spanned<DeValue<'i>>) -> Option<&'a DeTable<'i>> {
    match value.get_ref() {
        DeValue::Table(table) => Some(table),
        _ => None,
    }
}

/// Where a key and its value are written, a table's keys included: a table
/// under a header of its own, or written with dotted keys, spreads over
/// several lines, which its value's span alone does not cover.
fn extent(key: &Spanned<DeString>, value: &Spanned<DeValue>) -> Range<usize> {
    let mut range = key.span().start.min(value.span().start)..key.span().end.max(value.span().end);
    if let DeValue::Table(table) = value.get_ref() {
        for (inner_key, inner_value) in table {
            range.end = range.end.max(extent(inner_key, inner_value).end);
        }
    }
    range
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_dependencies_written_every_way() {
        let root = std::env::temp_dir().join(format!("lynk-cargo-{}", std::process::id()));
        let path = root.join("crate/Cargo.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            "[package]\nname = \"a\"\n\n[dependencies]\nplain = \"1\"\ninline = { version = \"1\", features = [\n  \"b\",\n] \
             }\ndotted.version = \"1\"\ndotted.optional = true\n\n[dependencies.header]\nversion = \"1\"\n\n[dev-dependencies]\nplain = \
             \"1\"\n\n[target.'cfg(unix)'.build-dependencies]\nplatform = \"1\"\n\n[workspace.dependencies]\nshared = \"1\"\n",
        )
        .unwrap();
        let spans = |name| -> Vec<String> {
            find_dependency(&root, Path::new("crate/Cargo.toml"), name)
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(spans("plain"), vec!["crate/Cargo.toml:5:1-5:11", "crate/Cargo.toml:16:1-16:11"]);
        assert_eq!(spans("inline"), vec!["crate/Cargo.toml:6:1-8:3"]);
        assert_eq!(spans("dotted"), vec!["crate/Cargo.toml:9:1-10:22"]);
        assert_eq!(spans("header"), vec!["crate/Cargo.toml:12:1-13:13"]);
        assert_eq!(spans("platform"), vec!["crate/Cargo.toml:19:1-19:14"]);
        assert_eq!(spans("shared"), vec!["crate/Cargo.toml:22:1-22:12"]);
        assert!(spans("name").is_empty());
        assert!(spans("version").is_empty());
        assert!(spans("missing").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn takes_a_manifest_and_a_dependency() {
        assert!(Cargo.query("dependency", &["Cargo.toml", "toml"], None).is_ok());
        assert!(Cargo.query("dependency", &["toml"], None).is_err());
    }
}
