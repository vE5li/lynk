//! WIT: a format whose comments are written as in Rust, and a provider of
//! its functions. The provider reads WIT files with `wit-parser`, without
//! resolving dependencies; its ADR's
//! [Decision](nexus:markdown.section (adrs/reads-wit-with-wit-parser.md))
//! says why.

use std::path::{Path, PathBuf};

use wit_parser::{Function, FunctionKind, SourceMap, UnresolvedPackage, WorldItem};

use crate::format::Format;
use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::provider::{Place, Provider, Query, normalize};
use crate::rust::comments;
use crate::workspace::{LinkedDocument, Workspace};

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
        comments::read(NAME, path, text)
    }
}

impl Provider for Wit {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["function"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        match arguments {
            [file, name] => Ok(Box::new(WitFunction {
                file: normalize(file),
                name: (*name).to_owned(),
            })),
            _ => Err(format!("`{NAME}.{selector}` takes a file and a function")),
        }
    }
}

/// The functions of a WIT file with a name, `<resource>.<function>` for a
/// resource's.
#[derive(Debug)]
struct WitFunction {
    file: PathBuf,
    name: String,
}

impl Query for WitFunction {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        find_function(&workspace.root, &self.file, &self.name)
    }
}

/// The spans of the names of the functions called [`name`](nexus:rust.argument)
/// in the WIT file at [`file`](nexus:rust.argument), relative to
/// [`root`](nexus:rust.argument). A resource's functions are named after it,
/// `<resource>.<function>`, its constructor `<resource>.constructor`.
fn find_function(root: &Path, file: &Path, name: &str) -> Vec<Span> {
    let absolute = root.join(file);
    let Ok(text) = std::fs::read_to_string(&absolute) else {
        return Vec::new();
    };
    let Some((packages, own_offsets)) = parse_package(&absolute) else {
        return Vec::new();
    };
    let line_index = LineIndex::new(&text);
    let mut spans: Vec<Span> = packages
        .iter()
        .flat_map(functions)
        .filter(|function| link_name(function) == name && function.span.is_known())
        .filter_map(|function| {
            let start = usize::try_from(function.span.start()).ok()?;
            let end = usize::try_from(function.span.end()).ok()?;
            let range = start.checked_sub(own_offsets.start)?..end.checked_sub(own_offsets.start)?;
            (range.end <= text.len()).then(|| line_index.span(file, range))
        })
        .collect();
    spans.sort();
    spans
}

/// The packages of the file at [`path`](nexus:rust.argument), parsed with the
/// other WIT files of its directory, since a package may span them and a file
/// alone may not parse; failing that, of the file alone. Also returns the byte
/// offsets the file occupies in the parsed sources, which the spans count in.
fn parse_package(path: &Path) -> Option<(Vec<UnresolvedPackage>, std::ops::Range<usize>)> {
    let mut siblings: Vec<PathBuf> = path
        .parent()
        .and_then(|directory| std::fs::read_dir(directory).ok())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|sibling| sibling.extension().is_some_and(|extension| extension == "wit") && sibling.is_file())
        .collect();
    siblings.sort();
    parse_files(&siblings, path).or_else(|| parse_files(&[path.to_owned()], path))
}

fn parse_files(files: &[PathBuf], own_file: &Path) -> Option<(Vec<UnresolvedPackage>, std::ops::Range<usize>)> {
    let mut source_map = SourceMap::new();
    let mut offset = 0;
    let mut own_offsets = None;
    for file in files {
        let text = std::fs::read_to_string(file).ok()?;
        if file == own_file {
            own_offsets = Some(offset..offset + text.len());
        }
        // The source map appends a line break to every file.
        offset += text.len() + 1;
        source_map.push(file, text);
    }
    let group = source_map.parse().ok()?;
    let packages = std::iter::once(group.main).chain(group.nested).collect();
    Some((packages, own_offsets?))
}

/// Every function of a package, in its interfaces and its worlds.
fn functions(package: &UnresolvedPackage) -> Vec<&Function> {
    let in_interfaces = package.interfaces.iter().flat_map(|(_, interface)| interface.functions.values());
    let in_worlds = package.worlds.iter().flat_map(|(_, world)| {
        world.imports.values().chain(world.exports.values()).filter_map(|item| match item {
            WorldItem::Function(function) => Some(function),
            _ => None,
        })
    });
    in_interfaces.chain(in_worlds).collect()
}

/// The name a link gives a function: wit-parser names a resource's
/// functions `[method]<resource>.<function>`, `[constructor]<resource>` and
/// so on.
fn link_name(function: &Function) -> String {
    match function.kind {
        FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => function.name.clone(),
        _ => {
            let unprefixed = function.name.split_once(']').map_or(function.name.as_str(), |(_, rest)| rest);
            let resource = unprefixed.split('.').next().unwrap_or(unprefixed);
            format!("{resource}.{}", function.item_name())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_spans(root: &Path, name: &str) -> Vec<String> {
        find_function(root, Path::new("wit/world.wit"), name)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn finds_functions_of_interfaces_worlds_and_resources() {
        let root = std::env::temp_dir().join(format!("nexus-wit-{}", std::process::id()));
        let files =
            [
                (
                    "wit/types.wit",
                    "interface types {\n    use host.{handle};\n    open: func() -> handle;\n}\n",
                ),
                (
                    "wit/world.wit",
                    "package example:app;\n\ninterface host {\n    resource handle {\n        constructor();\n        close: func();\n    \
                     }\n    open: func(name: string);\n}\n\nworld app {\n    import host;\n    export run: func();\n}\n",
                ),
            ];
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        assert_eq!(function_spans(&root, "open"), vec!["wit/world.wit:8:5-8:8"]);
        assert_eq!(function_spans(&root, "run"), vec!["wit/world.wit:13:12-13:14"]);
        assert_eq!(function_spans(&root, "handle.close"), vec!["wit/world.wit:6:9-6:13"]);
        assert_eq!(function_spans(&root, "handle.constructor").len(), 1);
        assert!(function_spans(&root, "close").is_empty());
        assert!(function_spans(&root, "missing").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
