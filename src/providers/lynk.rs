//! lynk itself: a provider of the tool, its commands and its selectors.

use std::path::{Path, PathBuf};

use crate::command;
use crate::link::{Member, Shape};
use crate::location::{Position, Span};
use crate::providers::{self, MemberError, Place, Provider, Query, check_complete_map, no_arguments};
use crate::workspace::Workspace;

pub const NAME: &str = "lynk";

pub struct Lynk;

impl Provider for Lynk {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["selectors", "self", "lint", "where", "ref", "find"]
    }

    fn shape(&self, selector: &str) -> Shape {
        Shape {
            map: selector == "selectors",
            takes_arguments: false,
        }
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        no_arguments(&format!("{NAME}.{selector}"), arguments)?;
        Ok(Box::new(match selector {
            "selectors" => LynkQuery::Selectors,
            "self" => LynkQuery::Tool,
            // Every other selector is named after the subcommand it points at.
            command => LynkQuery::Command(command.to_owned()),
        }))
    }
}

#[derive(Debug)]
enum LynkQuery {
    /// The selectors of every provider, as complete map; defined where the
    /// providers are listed.
    Selectors,
    /// The tool as a whole.
    Tool,
    /// One of its subcommands, by name.
    Command(String),
}

impl Query for LynkQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        match self {
            Self::Selectors => vec![Span::whole_file(&own_source(workspace, providers::SOURCE))],
            Self::Tool => vec![Span::whole_file(&own_source(workspace, ""))],
            Self::Command(command) => {
                let position = Position {
                    line: command::source_line(command),
                    column: 1,
                };
                vec![Span {
                    path: own_source(workspace, command::SOURCE),
                    start: position,
                    end: position,
                }]
            }
        }
    }

    fn check_members(&self, members: &[Member], _workspace: &Workspace) -> Vec<MemberError> {
        match self {
            Self::Selectors => check_complete_map(members, &providers::selector_names()),
            _ => Vec::new(),
        }
    }
}

/// A path inside this tool's crate, relative to the repository root when
/// the tool was built from inside it.
pub fn own_source(workspace: &Workspace, relative_path: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let relative_to_root = source.strip_prefix(&workspace.root).unwrap_or(&source);
    relative_to_root.components().collect()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    use crate::providers::tests::{assert_query, read_first_link};

    #[test]
    fn every_command_selector_names_a_subcommand() {
        let cli = command::Cli::command();
        for selector in Lynk
            .selectors()
            .iter()
            .filter(|selector| !matches!(**selector, "selectors" | "self"))
        {
            assert!(cli.find_subcommand(selector).is_some(), "{selector}");
        }
    }

    #[test]
    fn a_selector_without_arguments_ignores_the_text_of_its_link() {
        let (query, _) = read_first_link("test.md", "[the lint](lynk:lynk.lint)").unwrap();
        assert_query(query.as_ref(), &LynkQuery::Command("lint".to_owned()));
    }
}
