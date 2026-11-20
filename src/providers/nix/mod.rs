//! The commands of Nix dev shells, read with `rnix` and evaluated without
//! Nix; its design record's
//! [Decision](lynk:markdown.section (design/reads-nix-with-rnix.md))
//! says why.

use std::path::{Path, PathBuf};

mod evaluate;

use rnix::ast::{self, InterpolPart};
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

/// The attributes whose own attributes are dev shells, as flake-parts'
/// `make-shells` module declares them.
const DEV_SHELL_ATTRIBUTES: &[&str] = &["make-shells"];

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

/// The spans in the Nix file at [`file`](lynk:rust.argument), relative to
/// [`root`](lynk:rust.argument), building the command
/// [`command`](lynk:rust.argument) among a dev shell's inputs, as
/// [`lynk::providers::nix::evaluate`](lynk:rust.module) evaluates them. A
/// command is built by applying a function the file does not define to its
/// name, given as a string or as the `name` of an attribute set. Its span runs
/// from the name of the binding holding it to the end of its value, or covers
/// the application alone when it is not bound.
fn find_dev_shell_command(root: &Path, file: &Path, command: &str) -> Vec<Span> {
    let Ok(text) = std::fs::read_to_string(root.join(file)) else {
        return Vec::new();
    };
    let parse = Root::parse(&text);
    if !parse.errors().is_empty() {
        return Vec::new();
    }
    let line_index = LineIndex::new(&text);
    let mut spans: Vec<Span> = dev_shell_inputs(&parse.syntax())
        .iter()
        .flat_map(|input| evaluate::built(&evaluate::evaluate(input, &None, 0)))
        .filter(|(names, _)| names.iter().any(|name| name == command))
        .map(|(_, range)| line_index.span(file, usize::from(range.start())..usize::from(range.end())))
        .collect();
    spans.sort();
    spans.dedup();
    spans
}

/// The values of every dev shell's inputs in a file.
fn dev_shell_inputs(syntax: &SyntaxNode) -> Vec<ast::Expr> {
    syntax
        .descendants()
        .filter_map(ast::AttrpathValue::cast)
        .filter(is_dev_shell_input)
        .filter_map(|binding| binding.value())
        .collect()
}

/// Whether a binding is one of a dev shell's inputs: in the attribute set
/// a dev shell function is applied to, or in an attribute of a dev shell
/// attribute.
fn is_dev_shell_input(binding: &ast::AttrpathValue) -> bool {
    let Some((path, container)) = attribute_path(binding) else {
        return false;
    };
    let Some(input) = path.last() else {
        return false;
    };
    if !DEV_SHELL_INPUTS.contains(&input.as_str()) {
        return false;
    }
    match path.as_slice() {
        [_] => is_dev_shell_argument(&container),
        [.., shells, _, _] => DEV_SHELL_ATTRIBUTES.contains(&shells.as_str()),
        _ => false,
    }
}

/// Whether [`container`](lynk:rust.argument) is the attribute set a dev
/// shell function is applied to.
fn is_dev_shell_argument(container: &SyntaxNode) -> bool {
    let argument = outermost_paren(container.clone());
    let Some(apply) = argument.parent().and_then(ast::Apply::cast) else {
        return false;
    };
    apply.argument().is_some_and(|expression| expression.syntax() == &argument)
        && apply
            .lambda()
            .and_then(|function| function_name(&function))
            .is_some_and(|name| DEV_SHELL_FUNCTIONS.contains(&name.as_str()))
}

fn outermost_paren(mut node: SyntaxNode) -> SyntaxNode {
    while let Some(parent) = node.parent().filter(|parent| ast::Paren::can_cast(parent.kind())) {
        node = parent;
    }
    node
}

/// The attribute path a binding binds, through the attribute sets it is
/// nested in as values of bindings, and the outermost of them, or the
/// `let` holding it. An attribute set is the value of a binding through
/// parentheses and as the body of a `let`. `None` for a name computed with
/// an interpolation.
fn attribute_path(binding: &ast::AttrpathValue) -> Option<(Vec<String>, SyntaxNode)> {
    let mut path = attribute_names(binding)?;
    let mut container = binding.syntax().parent()?;
    loop {
        if !ast::AttrSet::can_cast(container.kind()) {
            return Some((path, container));
        }
        let value = value_node(container.clone());
        let Some(outer) = value
            .parent()
            .and_then(ast::AttrpathValue::cast)
            .filter(|outer| outer.value().is_some_and(|outer_value| outer_value.syntax() == &value))
        else {
            return Some((path, container));
        };
        let mut outer_path = attribute_names(&outer)?;
        outer_path.extend(path);
        path = outer_path;
        container = outer.syntax().parent()?;
    }
}

/// The outermost expression whose value is the expression at
/// [`node`](lynk:rust.argument), through parentheses and `let` bodies.
fn value_node(mut node: SyntaxNode) -> SyntaxNode {
    loop {
        let Some(parent) = node.parent() else {
            return node;
        };
        let is_let_body = ast::LetIn::cast(parent.clone())
            .and_then(|let_in| let_in.body())
            .is_some_and(|body| body.syntax() == &node);
        if !ast::Paren::can_cast(parent.kind()) && !is_let_body {
            return node;
        }
        node = parent;
    }
}

fn attribute_names(binding: &ast::AttrpathValue) -> Option<Vec<String>> {
    binding
        .attrpath()?
        .attrs()
        .map(|attribute| match attribute {
            ast::Attr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
            ast::Attr::Str(text) => plain_string(&text),
            ast::Attr::Dynamic(_) => None,
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

    /// The spans of the commands named [`command`](lynk:rust.argument) in
    /// [`text`](lynk:rust.argument), written as `flake.nix`.
    fn command_spans(test: &str, text: &str, command: &str) -> Vec<String> {
        let root = std::env::temp_dir().join(format!("lynk-nix-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("flake.nix"), text).unwrap();
        let spans = find_dev_shell_command(&root, Path::new("flake.nix"), command)
            .iter()
            .map(ToString::to_string)
            .collect();
        std::fs::remove_dir_all(root).unwrap();
        spans
    }

    #[test]
    fn finds_commands_listed_in_a_dev_shell() {
        let text = r#"{
  outputs = {nixpkgs, ...}: let
    pkgs = nixpkgs.legacyPackages.x86_64-linux;
    script = pkgs.writeShellScriptBin "script" ''
      echo
    '';
    application = pkgs.writeShellApplication {
      name = "renamed";
      text = "echo";
    };
    unlisted = pkgs.writeShellScriptBin "unlisted" "echo";
    interpolated = pkgs.writeShellScriptBin "${"interpolated"}" "echo";
  in {
    devShells.x86_64-linux.default = pkgs.mkShell {
      buildInputs = [script];
      packages = [pkgs.git] ++ [application interpolated];
    };
  };
}
"#;
        let spans = |command| command_spans("listed", text, command);
        assert_eq!(spans("script"), vec!["flake.nix:4:5-6:6"]);
        assert_eq!(spans("renamed"), vec!["flake.nix:7:5-10:5"]);
        assert_eq!(spans("interpolated"), vec!["flake.nix:12:5-12:70"]);
        assert!(spans("application").is_empty());
        assert!(spans("unlisted").is_empty());
        assert!(spans("git").is_empty());
    }

    #[test]
    fn finds_commands_built_by_functions_over_lists() {
        let text = r#"{lib, ...}: let
  machines = ["dev" "test"];
in {
  perSystem = {pkgs, ...}: let
    machine-scripts = machine: [
      (pkgs.writeShellScriptBin "start-${machine}" "echo")
      (pkgs.writeShellScriptBin "stop-${machine}" "echo")
    ];
    every-machine-script = lib.pipe machines [
      (lib.map machine-scripts)
      lib.flatten
    ];
    greetings = map (greeting: pkgs.writeShellScriptBin greeting "echo") ["hello"];
  in {
    make-shells.default = {
      packages =
        [pkgs.git]
        ++ every-machine-script
        ++ builtins.concatLists [greetings]
        ++ [
          (pkgs.writeShellScriptBin "inline" "echo")
        ];
    };
    make-shells.other.nativeBuildInputs = builtins.concatMap machine-scripts ["prod"];
  };
}
"#;
        let spans = |command| command_spans("lists", text, command);
        assert_eq!(spans("start-dev"), vec!["flake.nix:6:8-6:57"]);
        assert_eq!(spans("stop-test"), vec!["flake.nix:7:8-7:56"]);
        assert_eq!(spans("start-prod"), vec!["flake.nix:6:8-6:57"]);
        assert_eq!(spans("hello"), vec!["flake.nix:13:32-13:71"]);
        assert_eq!(spans("inline"), vec!["flake.nix:21:12-21:51"]);
        assert!(spans("start-${machine}").is_empty());
        assert!(spans("start-staging").is_empty());
    }

    #[test]
    fn ignores_what_only_looks_like_a_dev_shell() {
        let text = r#"{
  perSystem = {pkgs, ...}: let
    script = pkgs.writeShellScriptBin "script" "echo";
    recursive = recursive;
  in {
    shells.default.packages = [script];
    make-shells.packages = [script];
    make-shells.default.packages = [recursive (pkgs.writeShellScriptBin pkgs.name "echo")];
  };
}
"#;
        assert!(command_spans("lookalike", text, "script").is_empty());
    }

    #[test]
    fn takes_a_file_and_a_command() {
        assert!(Nix.query("dev-shell-command", &["flake.nix", "build"], None).is_ok());
        assert!(Nix.query("dev-shell-command", &["build"], None).is_err());
    }
}
