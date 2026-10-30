//! What links point at in Rust source: its items, named by their paths, the
//! members of its enums and structs, the values of its constants, and its
//! comments, found by their text.

mod comments;
mod items;
mod literals;

use std::path::PathBuf;

use crate::formats::rust::NAME;
use crate::formats::{self};
use crate::link::{Member, Shape};
use crate::location::Span;
use crate::providers::rust::literals::Literal;
use crate::providers::{MemberError, Place, Provider, Query, check_complete_map, normalize};
use crate::workspace::Workspace;

pub struct Rust;

impl Provider for Rust {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["module", "item", "literal", "argument", "comment", "map-variant", "map-field"]
    }

    fn shape(&self, selector: &str) -> Shape {
        Shape {
            map: selector.starts_with("map-"),
            takes_arguments: true,
        }
    }

    fn query(&self, selector: &str, arguments: &[&str], place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        let selector_name = format!("{NAME}.{selector}");
        let query = match selector {
            "module" => match arguments {
                [path] => RustQuery::Module { path: (*path).to_owned() },
                _ => return Err(format!("`{selector_name}` takes a module path")),
            },
            "item" => match arguments {
                [path] => RustQuery::Item { path: (*path).to_owned() },
                _ => return Err(format!("`{selector_name}` takes an item path")),
            },
            "literal" => match arguments {
                [path, value] => RustQuery::Literal {
                    path: (*path).to_owned(),
                    value: (*value).to_owned(),
                },
                _ => return Err(format!("`{selector_name}` takes an item path and a value")),
            },
            "map-variant" => match arguments {
                [path] => RustQuery::MapVariant { path: (*path).to_owned() },
                _ => return Err(format!("`{selector_name}` takes an enum path")),
            },
            "map-field" => match arguments {
                [path] => RustQuery::MapField { path: (*path).to_owned() },
                _ => return Err(format!("`{selector_name}` takes a struct or variant path")),
            },
            "comment" => {
                let (file, text) = match (arguments, place) {
                    ([file, text], _) => (Some(normalize(file)), text),
                    ([text], Some(place)) if is_rust(place) => (Some(place.file.to_owned()), text),
                    ([text], None) => (None, text),
                    ([_], _) => {
                        return Err(format!("`{selector_name}` takes a file outside a Rust file"));
                    }
                    _ => return Err(format!("`{selector_name}` takes an optional file and a text")),
                };
                RustQuery::Comment {
                    file,
                    text: (*text).to_owned(),
                }
            }
            _ => {
                let (function, argument) = match (arguments, place) {
                    ([function, argument], _) => (RustFunction::Path((*function).to_owned()), argument),
                    ([argument], Some(place)) if is_rust(place) => (
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
                    _ => {
                        return Err(format!("`{selector_name}` takes an optional function path and an argument"));
                    }
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

fn is_rust(place: &Place) -> bool {
    formats::of(place.file).is_some_and(|format| format.name() == NAME)
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
    /// Any other item, by its path as Rust writes it, starting with the
    /// crate.
    Item { path: String },
    /// A constant or static, by its path, whose value is a literal stating
    /// a value.
    Literal { path: String, value: String },
    /// An argument of a function, by name.
    Argument { function: RustFunction, argument: String },
    /// The comments holding a text; a file left out (`None`) matches every
    /// file, links fill it with the Rust file they are written in.
    Comment { file: Option<PathBuf>, text: String },
    /// The variants of an enum, by its path, as complete map.
    MapVariant { path: String },
    /// The named fields of a struct or variant, by its path, as complete
    /// map.
    MapField { path: String },
}

impl RustQuery {
    /// The enums, structs or variants a map selector lists the members of,
    /// each with its members' names.
    fn map_targets(&self, workspace: &Workspace) -> Vec<(Span, Vec<String>)> {
        match self {
            Self::MapVariant { path } => items::find_variants(&workspace.root, path),
            Self::MapField { path } => items::find_fields(&workspace.root, path),
            _ => Vec::new(),
        }
    }
}

impl Query for RustQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        let root = &workspace.root;
        match self {
            Self::Module { path } => items::find_module(root, path).into_iter().collect(),
            Self::Item { path } => items::find_item(root, path),
            Self::Literal { path, .. } => items::find_values(root, path).into_iter().map(|(span, _)| span).collect(),
            Self::Argument { function, argument } => match function {
                RustFunction::Path(path) => items::find_argument(root, path, argument),
                RustFunction::Enclosing { file, line } => items::find_enclosing_argument(root, file, *line, argument),
            },
            Self::Comment { file, text } => comments::find_comments(workspace, file.as_deref(), text),
            Self::MapVariant { .. } | Self::MapField { .. } => self.map_targets(workspace).into_iter().map(|(span, _)| span).collect(),
        }
    }

    /// Members are checked against those of every definition found; a
    /// target found nowhere has none to check, its link points at nothing.
    fn check_members(&self, members: &[Member], workspace: &Workspace) -> Vec<MemberError> {
        let targets = self.map_targets(workspace);
        if targets.is_empty() {
            return Vec::new();
        }
        let mut expected: Vec<String> = Vec::new();
        for name in targets.into_iter().flat_map(|(_, names)| names) {
            if !expected.contains(&name) {
                expected.push(name);
            }
        }
        check_complete_map(members, &expected)
    }

    /// A literal's value is checked at every definition found.
    fn check_target(&self, workspace: &Workspace) -> Vec<String> {
        let Self::Literal { path, value } = self else {
            return Vec::new();
        };
        items::find_values(&workspace.root, path)
            .iter()
            .filter_map(|(span, expression)| match Literal::of_expression(expression) {
                None => Some(format!("`{path}` at {span} is not a literal")),
                Some(literal) if !literal.matches(value) => Some(format!("`{path}` at {span} is `{literal}`, not `{value}`")),
                Some(_) => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::tests::{assert_query, read_first_link};

    #[test]
    fn an_argument_without_a_function_is_one_of_the_function_the_link_is_on() {
        let source = "fn main() {}\n\n/// With\n/// [`root`](lynk:rust.argument).\nfn run(root: &str) {}\n";
        let (query, _) = read_first_link("src/main.rs", source).unwrap();
        assert_query(query.as_ref(), &RustQuery::Argument {
            function: RustFunction::Enclosing {
                file: PathBuf::from("src/main.rs"),
                line: 4,
            },
            argument: "root".to_owned(),
        });
        assert!(read_first_link("test.md", "[`root`](lynk:rust.argument)").is_err());
        assert!(Rust.query("argument", &["root"], None).is_err());
    }

    #[test]
    fn a_comment_without_a_file_is_in_the_rust_file_the_link_is_in() {
        let (query, _) = read_first_link("src/main.rs", "/// As [kept sorted](lynk:rust.comment).\nfn run() {}\n").unwrap();
        assert_query(query.as_ref(), &RustQuery::Comment {
            file: Some(PathBuf::from("src/main.rs")),
            text: "kept sorted".to_owned(),
        });
        assert!(read_first_link("test.md", "[kept sorted](lynk:rust.comment)").is_err());
        let (query, _) = read_first_link("test.md", "[kept sorted](lynk:rust.comment (./src/main.rs))").unwrap();
        assert_query(query.as_ref(), &RustQuery::Comment {
            file: Some(PathBuf::from("src/main.rs")),
            text: "kept sorted".to_owned(),
        });
        assert_query(
            Rust.query("comment", &["kept sorted"], None).unwrap().as_ref(),
            &RustQuery::Comment {
                file: None,
                text: "kept sorted".to_owned(),
            },
        );
    }

    #[test]
    fn map_selectors_take_a_path_and_a_body() {
        let (query, members) = read_first_link(
            "test.md",
            "<!-- lynk rust.map-field a::Event::Move -->\n- x: across\n<!-- /lynk -->\n",
        )
        .unwrap();
        assert_query(query.as_ref(), &RustQuery::MapField {
            path: "a::Event::Move".to_owned(),
        });
        assert_eq!(members[0].name, "x");
        assert!(read_first_link("test.md", "[`a::Event`](lynk:rust.map-variant)").is_err());
        assert!(Rust.query("map-variant", &[], None).is_err());
    }

    #[test]
    fn a_literal_takes_its_path_in_the_title_and_its_value_as_text() {
        let (query, _) = read_first_link("test.md", "[`16`](lynk:rust.literal (a::LIMIT))").unwrap();
        assert_query(query.as_ref(), &RustQuery::Literal {
            path: "a::LIMIT".to_owned(),
            value: "16".to_owned(),
        });
        assert!(read_first_link("test.md", "[`a::LIMIT`](lynk:rust.literal)").is_err());
    }

    #[test]
    fn an_argument_of_a_named_function_takes_its_path_in_the_title() {
        let (query, _) = read_first_link("test.md", "[`root`](lynk:rust.argument (a::run))").unwrap();
        assert_query(query.as_ref(), &RustQuery::Argument {
            function: RustFunction::Path("a::run".to_owned()),
            argument: "root".to_owned(),
        });
    }
}
