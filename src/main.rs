mod command;
mod document;
mod export;
mod find;
mod formats;
mod link;
mod location;
mod providers;
mod relevance;
mod workspace;

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use command::{Cli, Command, QueryArguments};
use link::{Argument, Link, Member, SyntaxError};
use location::Span;
use providers::Query;
use workspace::{LinkedDocument, Workspace, find_root};

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(exit_code) => exit_code,
        Err(message) => {
            eprintln!("lynk: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    let root = cli.root;
    match cli.command {
        Command::Lint => Ok(lint(&load_workspace(root)?)),
        Command::Resolve(arguments) => Ok(print_resolved(
            &load_workspace(root)?,
            query_from_command_line(arguments)?.as_ref(),
        )),
        Command::Backlinks(arguments) => Ok(print_backlinks(
            &load_workspace(root)?,
            query_from_command_line(arguments)?.as_ref(),
        )),
        Command::Context { words } => Ok(print_context(&load_workspace(root)?, &words.join(" "))),
        // Reads only the files it exports, so it needs the root but not the
        // whole repository.
        Command::Export { directory, output } => {
            export::export_directory(&find_root_or(root)?, &directory, &output)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The repository at [`root`](lynk:rust.argument), by default the one the
/// current directory is in.
fn load_workspace(root: Option<PathBuf>) -> Result<Workspace, String> {
    Workspace::load(find_root_or(root)?).map_err(|error| error.to_string())
}

/// The [`root`](lynk:rust.argument) given, by default that of the
/// repository the current directory is in.
fn find_root_or(root: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(root) = root {
        return Ok(root);
    }
    let current_directory = std::env::current_dir().map_err(|error| error.to_string())?;
    Ok(find_root(&current_directory).ok_or("not inside a repository; pass --root")?)
}

fn query_from_command_line(query: QueryArguments) -> Result<Box<dyn Query>, String> {
    let arguments: Vec<Argument> = query.arguments.into_iter().map(|text| Argument { text, quoted: false }).collect();
    providers::query(&query.selector, &arguments, None)
}

fn query_of_link(linked: &LinkedDocument, link: &Link) -> Result<(Box<dyn Query>, Vec<Member>), SyntaxError> {
    providers::of_link(link, &linked.document)
}

fn lint(workspace: &Workspace) -> ExitCode {
    let mut error_count = 0;

    for linked in &workspace.documents {
        let document = &linked.document;
        for error in &linked.syntax_errors {
            error_count += 1;
            println!("{}: {}", document.span(error.offset..error.offset), error.message);
        }
        for link in &linked.links {
            let location = document.span(link.range.start..link.range.start);
            let (query, members) = match query_of_link(linked, link) {
                Ok(read) => read,
                Err(error) => {
                    error_count += 1;
                    println!("{}: {}", document.span(error.offset..error.offset), error.message);
                    continue;
                }
            };
            for member_error in query.check_members(&members, workspace) {
                error_count += 1;
                let member_location = member_error
                    .range
                    .map_or(location.clone(), |range| document.span(range.start..range.start));
                println!("{member_location}: {}", member_error.message);
            }
            for message in query.check_target(workspace) {
                error_count += 1;
                println!("{location}: {message}");
            }
            let targets = query.resolve(workspace);
            match targets.len() {
                1 => {}
                0 => {
                    error_count += 1;
                    println!("{location}: points at nothing");
                }
                _ => {
                    error_count += 1;
                    println!("{location}: ambiguous, points at {} places:", targets.len());
                    for target in targets {
                        println!("  {target}");
                    }
                }
            }
        }
    }

    if error_count > 0 {
        println!("{error_count} error(s)");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn print_resolved(workspace: &Workspace, query: &dyn Query) -> ExitCode {
    let targets = query.resolve(workspace);

    for target in &targets {
        println!("{target}\t{}", first_line(workspace, target));
    }

    if targets.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn print_backlinks(workspace: &Workspace, query: &dyn Query) -> ExitCode {
    let targets: HashSet<Span> = query.resolve(workspace).into_iter().collect();
    let mut found = false;
    for linked in &workspace.documents {
        for link in &linked.links {
            let Ok((link_query, _)) = query_of_link(linked, link) else {
                continue;
            };
            if !link_query.resolve(workspace).iter().any(|target| targets.contains(target)) {
                continue;
            }
            found = true;
            let document = &linked.document;
            let link_text = &document.text[link.range.clone()];
            println!(
                "{}\t{}",
                document.span(link.range.clone()),
                link_text.lines().next().unwrap_or_default()
            );
        }
    }
    if found { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn print_context(workspace: &Workspace, query: &str) -> ExitCode {
    const FOUND_LIMIT: usize = 10;

    let mut found = find::find(workspace, query);
    if let Ok(api_key) = std::env::var(relevance::API_KEY_VARIABLE) {
        let shortlist: Vec<&str> = found
            .iter()
            .take(relevance::SHORTLIST_LENGTH)
            .map(|result| result.text.as_str())
            .collect();
        match relevance::relevance(&api_key, query, &shortlist) {
            Ok(relevances) => relevance::re_rank(&mut found, &relevances),
            Err(error) => eprintln!("lynk: not re-ranked, ranking by words: {error}"),
        }
    }
    for result in found.iter().take(FOUND_LIMIT) {
        let score = match result.relevance {
            Some(relevance) => format!("{relevance:.2}"),
            None => format!("{:.1}", result.score),
        };
        println!("{score}\t{}\t{}", result.span, result.title);
    }
    if found.is_empty() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// The first line of the target, for orientation; empty for directories.
fn first_line(workspace: &Workspace, target: &Span) -> String {
    let text = match workspace.document(&target.path) {
        Some(linked) => linked.document.text.clone(),
        None => std::fs::read_to_string(workspace.root.join(&target.path)).unwrap_or_default(),
    };
    text.lines().nth(target.start.line - 1).unwrap_or_default().trim().to_owned()
}
