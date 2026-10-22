use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Checks and navigates the links between documentation and code.
#[derive(Debug, Parser)]
#[command(
    name = "nexus",
    after_help = "Arguments given on the command line are never quoted; a file left out matches every file."
)]
pub struct Cli {
    /// The repository; by default the nearest directory up holding `.jj` or
    /// `.git`.
    #[arg(long, value_name = "directory")]
    pub root: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check every link of the repository.
    Lint,
    /// Print the places a link points at.
    Where(QueryArguments),
    /// Print the links pointing at the places a link points at.
    Ref(QueryArguments),
    /// Print the parts of the documentation about some words, best first.
    Find {
        #[arg(required = true, value_name = "words")]
        words: Vec<String>,
    },
}

/// A link written on the command line.
#[derive(Debug, Args)]
pub struct QueryArguments {
    #[arg(value_name = "provider.selector")]
    pub selector: String,
    #[arg(value_name = "arguments")]
    pub arguments: Vec<String>,
}

/// This file, relative to the crate.
pub const SOURCE: &str = file!();

/// The line declaring the subcommand named [`name`](nexus:rust.argument), which
/// is where links to it point.
pub fn source_line(name: &str) -> usize {
    let variant: String = name
        .split('-')
        .flat_map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase())
                .into_iter()
                .chain(characters)
        })
        .collect();
    let source = include_str!("command.rs");
    let enum_start = source.lines().position(|line| line.starts_with("pub enum Command")).unwrap_or(0);
    source
        .lines()
        .enumerate()
        .skip(enum_start)
        .find(|(_, line)| {
            line.trim_start()
                .strip_prefix(variant.as_str())
                .and_then(|rest| rest.chars().next())
                .is_some_and(|next| matches!(next, ',' | '(' | ' ' | '{'))
        })
        .map_or(1, |(index, _)| index + 1)
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn every_subcommand_finds_its_source_line() {
        for subcommand in Cli::command().get_subcommands() {
            let name = subcommand.get_name();
            assert!(source_line(name) > 1, "{name}");
        }
    }
}
