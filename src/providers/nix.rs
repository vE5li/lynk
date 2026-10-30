//! The commands of Nix dev shells, read with `rnix` without evaluating
//! anything; its design record's
//! [Decision](lynk:markdown.section (design/reads-nix-with-rnix.md))
//! says why.

use std::path::{Path, PathBuf};

use rnix::ast::{self, HasEntry, InterpolPart};
use rnix::{Root, SyntaxNode};
use rowan::ast::AstNode;

use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::providers::{Place, Provider, Query, normalize};
use crate::workspace::Workspace;

pub const NAME: &str = "nix";

/// The functions building a dev shell, named by the last attribute they are
/// selected with.
const DEV_SHELL_FUNCTIONS: &[&str] = &["mkShell", "mkShellNoCC"];

/// The attributes of a dev shell listing what it puts on `PATH`.
const DEV_SHELL_INPUTS: &[&str] = &["packages", "buildInputs", "nativeBuildInputs"];

pub struct Nix;

impl Provider for Nix {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["dev-shell-command"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        match arguments {
            [file, command] => Ok(Box::new(NixDevShellCommand {
                file: normalize(file),
                command: (*command).to_owned(),
            })),
            _ => Err(format!("`{NAME}.{selector}` takes a file and a command")),
        }
    }
}

/// The bindings of a Nix file building a command that a dev shell puts on
/// `PATH`.
#[derive(Debug)]
struct NixDevShellCommand {
    file: PathBuf,
    command: String,
}

impl Query for NixDevShellCommand {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        find_dev_shell_command(&workspace.root, &self.file, &self.command)
    }
}

/// The spans of the bindings in the Nix file at
/// [`file`](lynk:rust.argument), relative to [`root`](lynk:rust.argument),
/// that build the command [`command`](lynk:rust.argument) and are listed
/// among a dev shell's inputs. A binding builds a command when its value
/// applies a function to the command's name, given as a string or as the
/// `name` of an attribute set. Each span runs from the binding's name to the
/// end of its value.
fn find_dev_shell_command(root: &Path, file: &Path, command: &str) -> Vec<Span> {
    let Ok(text) = std::fs::read_to_string(root.join(file)) else {
        return Vec::new();
    };
    let parse = Root::parse(&text);
    if !parse.errors().is_empty() {
        return Vec::new();
    }
    let syntax = parse.syntax();
    let inputs = dev_shell_inputs(&syntax);
    let line_index = LineIndex::new(&text);
    let mut spans: Vec<Span> = syntax
        .descendants()
        .filter_map(ast::AttrpathValue::cast)
        .filter(|binding| binding_name(binding).is_some_and(|name| inputs.contains(&name)))
        .filter_map(|binding| {
            let value = binding.value()?;
            if !builds_command(&value, command) {
                return None;
            }
            let start = usize::from(binding.attrpath()?.syntax().text_range().start());
            let end = usize::from(value.syntax().text_range().end());
            Some(line_index.span(file, start..end))
        })
        .collect();
    spans.sort();
    spans
}

/// The names listed in the inputs of every dev shell of a file, each list
/// item being a name in scope; lists joined with `++` are read one by one.
fn dev_shell_inputs(syntax: &SyntaxNode) -> Vec<String> {
    syntax
        .descendants()
        .filter_map(ast::Apply::cast)
        .filter(|apply| {
            apply
                .lambda()
                .and_then(|function| function_name(&function))
                .is_some_and(|name| DEV_SHELL_FUNCTIONS.contains(&name.as_str()))
        })
        .filter_map(|apply| match apply.argument()? {
            ast::Expr::AttrSet(arguments) => Some(arguments),
            _ => None,
        })
        .flat_map(|arguments| arguments.attrpath_values())
        .filter(|binding| binding_name(binding).is_some_and(|name| DEV_SHELL_INPUTS.contains(&name.as_str())))
        .filter_map(|binding| binding.value())
        .flat_map(|value| value.syntax().descendants().filter_map(ast::List::cast).collect::<Vec<_>>())
        .flat_map(|list| list.items())
        .filter_map(|item| match item {
            ast::Expr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
            _ => None,
        })
        .collect()
}

/// The name a function is called by: a name in scope, or the last attribute
/// it is selected with.
fn function_name(function: &ast::Expr) -> Option<String> {
    match function {
        ast::Expr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
        ast::Expr::Select(select) => match select.attrpath()?.attrs().last()? {
            ast::Attr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
            _ => None,
        },
        _ => None,
    }
}

/// The name a binding binds, if its path is a single name.
fn binding_name(binding: &ast::AttrpathValue) -> Option<String> {
    let mut attributes = binding.attrpath()?.attrs();
    let (Some(ast::Attr::Ident(identifier)), None) = (attributes.next(), attributes.next()) else {
        return None;
    };
    Some(identifier.ident_token()?.text().to_owned())
}

/// Whether [`value`](lynk:rust.argument) applies a function to
/// [`command`](lynk:rust.argument), as one of its arguments however many it
/// is applied to.
fn builds_command(value: &ast::Expr, command: &str) -> bool {
    let mut expression = value.clone();
    loop {
        let ast::Expr::Apply(apply) = expression else {
            return false;
        };
        if apply.argument().is_some_and(|argument| names_command(&argument, command)) {
            return true;
        }
        let Some(function) = apply.lambda() else {
            return false;
        };
        expression = function;
    }
}

/// Whether an argument is [`command`](lynk:rust.argument) as a string, or
/// an attribute set whose `name` is that string.
fn names_command(argument: &ast::Expr, command: &str) -> bool {
    match argument {
        ast::Expr::Str(text) => plain_string(text).is_some_and(|text| text == command),
        ast::Expr::AttrSet(attributes) => attributes.attrpath_values().any(|binding| {
            binding_name(&binding).is_some_and(|name| name == "name")
                && matches!(binding.value(), Some(ast::Expr::Str(text)) if plain_string(&text).is_some_and(|text| text == command))
        }),
        _ => false,
    }
}

/// The content of a string without interpolations.
fn plain_string(text: &ast::Str) -> Option<String> {
    let mut content = String::new();
    for part in text.normalized_parts() {
        let InterpolPart::Literal(literal) = part else {
            return None;
        };
        content.push_str(&literal);
    }
    Some(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_commands_listed_in_a_dev_shell() {
        let root = std::env::temp_dir().join(format!("lynk-nix-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("flake.nix"),
            "{\n  outputs = {nixpkgs, ...}: let\n    pkgs = nixpkgs.legacyPackages.x86_64-linux;\n    script = \
             pkgs.writeShellScriptBin \"script\" ''\n      echo\n    '';\n    application = pkgs.writeShellApplication {\n      name \
             = \"renamed\";\n      text = \"echo\";\n    };\n    unlisted = pkgs.writeShellScriptBin \"unlisted\" \"echo\";\n    \
             interpolated = pkgs.writeShellScriptBin \"${\"interpolated\"}\" \"echo\";\n  in {\n    devShells.x86_64-linux.default = \
             pkgs.mkShell {\n      buildInputs = [script];\n      packages = [pkgs.git] ++ [application interpolated];\n    };\n  \
             };\n}\n",
        )
        .unwrap();
        let spans = |command| -> Vec<String> {
            find_dev_shell_command(&root, Path::new("flake.nix"), command)
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(spans("script"), vec!["flake.nix:4:5-6:6"]);
        assert_eq!(spans("renamed"), vec!["flake.nix:7:5-10:5"]);
        assert!(spans("application").is_empty());
        assert!(spans("unlisted").is_empty());
        assert!(spans("interpolated").is_empty());
        assert!(spans("git").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn takes_a_file_and_a_command() {
        assert!(Nix.query("dev-shell-command", &["flake.nix", "build"], None).is_ok());
        assert!(Nix.query("dev-shell-command", &["build"], None).is_err());
    }
}
