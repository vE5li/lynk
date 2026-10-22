//! Rust modules and the arguments of Rust functions, named by their paths.

mod items;

use std::path::PathBuf;

use crate::formats::rust::NAME;
use crate::formats::{self};
use crate::link::Shape;
use crate::location::Span;
use crate::providers::{Place, Provider, Query};
use crate::workspace::Workspace;

pub struct Rust;

impl Provider for Rust {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["module", "argument"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        let selector_name = format!("{NAME}.{selector}");
        let query = match selector {
            "module" => match arguments {
                [path] => RustQuery::Module { path: (*path).to_owned() },
                _ => return Err(format!("`{selector_name}` takes a module path")),
            },
            _ => {
                let (function, argument) = match (arguments, place) {
                    ([function, argument], _) => (RustFunction::Path((*function).to_owned()), argument),
                    ([argument], Some(place)) if formats::of(place.file).is_some_and(|format| format.name() == NAME) => (
                        RustFunction::Enclosing {
                            file: place.file.to_owned(),
                            line: place.line,
                        },
                        argument,
                    ),
                    ([_], _) => {
                        return Err(format!(
                            "`{selector_name}` takes a function path outside the Rust function it is written on"
                        ));
                    }
                    _ => return Err(format!("`{selector_name}` takes an optional function path and an argument")),
                };
                RustQuery::Argument {
                    function,
                    argument: (*argument).to_owned(),
                }
            }
        };
        Ok(Box::new(query))
    }
}

/// A Rust function, by its path as Rust writes it or as the one a link is
/// written on: the innermost whose definition, doc comment included, holds
/// the link's first line.
#[derive(Debug)]
enum RustFunction {
    Path(String),
    Enclosing { file: PathBuf, line: usize },
}

#[derive(Debug)]
enum RustQuery {
    /// A module, by its path as Rust writes it, starting with the crate.
    Module { path: String },
    /// An argument of a function, by name.
    Argument { function: RustFunction, argument: String },
}

impl Query for RustQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        let root = &workspace.root;
        match self {
            Self::Module { path } => items::find_module(root, path).into_iter().collect(),
            Self::Argument { function, argument } => match function {
                RustFunction::Path(path) => items::find_argument(root, path, argument),
                RustFunction::Enclosing { file, line } => items::find_enclosing_argument(root, file, *line, argument),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::tests::{assert_query, read_first_link};

    #[test]
    fn an_argument_without_a_function_is_one_of_the_function_the_link_is_on() {
        let source = "fn main() {}\n\n/// With\n/// [`root`](nexus:rust.argument).\nfn run(root: &str) {}\n";
        let (query, _) = read_first_link("src/main.rs", source).unwrap();
        assert_query(query.as_ref(), &RustQuery::Argument {
            function: RustFunction::Enclosing {
                file: PathBuf::from("src/main.rs"),
                line: 4,
            },
            argument: "root".to_owned(),
        });
        assert!(read_first_link("test.md", "[`root`](nexus:rust.argument)").is_err());
        assert!(Rust.query("argument", &["root"], None).is_err());
    }

    #[test]
    fn an_argument_of_a_named_function_takes_its_path_in_the_title() {
        let (query, _) = read_first_link("test.md", "[`root`](nexus:rust.argument (a::run))").unwrap();
        assert_query(query.as_ref(), &RustQuery::Argument {
            function: RustFunction::Path("a::run".to_owned()),
            argument: "root".to_owned(),
        });
    }
}
