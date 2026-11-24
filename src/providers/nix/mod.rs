//! The commands of Nix dev shells and the attributes of Nix files, read
//! with `rnix`, without Nix; its design record's
//! [Decision](lynk:markdown.section (design/reads-nix-with-rnix.md))
//! says why.

use std::path::{Path, PathBuf};

mod evaluate;

use rnix::ast::{self, AstToken, InterpolPart};
use rnix::{Root, SyntaxNode, TextRange};
use rowan::ast::AstNode;

use crate::link::{Member, Shape};
use crate::location::{LineIndex, Span};
use crate::providers::rust::literals::Literal;
use crate::providers::{MemberError, Place, Provider, Query, check_complete_map, normalize};
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

/// The flake outputs `nix run` looks a name up in, in the order it tries
/// them.
const RUNNABLE_OUTPUTS: &[&str] = &["apps", "packages", "legacyPackages"];

/// The attributes flake-parts writes flake outputs under: those of every
/// system, and those of one.
const FLAKE_PARTS_OUTPUTS: &[&str] = &["flake", "perSystem"];

/// The kernels a system's name ends with, after its CPU and a hyphen, as
/// flakes write their outputs' systems.
const SYSTEM_KERNELS: &[&str] = &["linux", "darwin", "freebsd", "netbsd", "openbsd"];

pub struct Nix;

impl Provider for Nix {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["dev-shell-command", "attribute", "literal", "map-attribute", "app"]
    }

    fn shape(&self, selector: &str) -> Shape {
        Shape {
            map: selector == "map-attribute",
            takes_arguments: true,
        }
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        if selector == "dev-shell-command" {
            let [file, command] = arguments else {
                return Err(format!("`{NAME}.{selector}` takes a file and a command"));
            };
            return Ok(Box::new(NixQuery::DevShellCommand {
                file: normalize(file),
                command: (*command).to_owned(),
            }));
        }
        if selector == "app" {
            let [file, name] = arguments else {
                return Err(format!("`{NAME}.{selector}` takes a file and a name"));
            };
            return Ok(Box::new(NixQuery::App {
                file: normalize(file),
                name: (*name).to_owned(),
            }));
        }
        if selector == "literal" {
            let [file, path, value] = arguments else {
                return Err(format!("`{NAME}.{selector}` takes a file, an attribute path and a value"));
            };
            return Ok(Box::new(NixQuery::Literal {
                file: normalize(file),
                path: parse_attribute_path(path)?,
                value: (*value).to_owned(),
            }));
        }
        let [file, path] = arguments else {
            return Err(format!("`{NAME}.{selector}` takes a file and an attribute path"));
        };
        let file = normalize(file);
        let path = parse_attribute_path(path)?;
        Ok(Box::new(match selector {
            "attribute" => NixQuery::Attribute { file, path },
            _ => NixQuery::MapAttribute { file, path },
        }))
    }
}

#[derive(Debug)]
enum NixQuery {
    DevShellCommand {
        file: PathBuf,
        command: String,
    },
    Attribute {
        file: PathBuf,
        path: Vec<String>,
    },
    /// An attribute, by its path, whose value is a literal stating a
    /// value.
    Literal {
        file: PathBuf,
        path: Vec<String>,
        value: String,
    },
    /// The names of the attributes directly under an attribute path, as
    /// complete map.
    MapAttribute {
        file: PathBuf,
        path: Vec<String>,
    },
    /// What `nix run` runs for a name.
    App {
        file: PathBuf,
        name: String,
    },
}

impl Query for NixQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        match self {
            Self::DevShellCommand { file, command } => find_dev_shell_command(&workspace.root, file, command),
            Self::Attribute { file, path } | Self::MapAttribute { file, path } | Self::Literal { file, path, .. } => {
                find_attribute(&workspace.root, file, path)
            }
            Self::App { file, name } => find_app(&workspace.root, file, name),
        }
    }

    /// An attribute that is not found has no attributes to check, its link
    /// points at nothing.
    fn check_members(&self, members: &[Member], workspace: &Workspace) -> Vec<MemberError> {
        let Self::MapAttribute { file, path } = self else {
            return Vec::new();
        };
        let names = attribute_names_under(&workspace.root, file, path);
        if names.is_empty() {
            return Vec::new();
        }
        check_complete_map(members, &names)
    }

    /// A literal's value is checked at every definition found.
    fn check_target(&self, workspace: &Workspace) -> Vec<String> {
        let Self::Literal { file, path, value } = self else {
            return Vec::new();
        };
        let name = path.join(".");
        find_values(&workspace.root, file, path)
            .iter()
            .filter_map(|(span, literal)| match literal {
                None => Some(format!("`{name}` at {span} is not a literal")),
                Some(literal) if !literal.matches(value) => Some(format!("`{name}` at {span} is `{literal}`, not `{value}`")),
                Some(_) => None,
            })
            .collect()
    }
}

/// An attribute path as Nix writes it, its names separated by dots, a name
/// holding a dot quoted with `"`.
fn parse_attribute_path(text: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut name = String::new();
    let mut quoted = false;
    for character in text.chars() {
        match character {
            '"' => quoted = !quoted,
            '.' if !quoted => names.push(std::mem::take(&mut name)),
            _ => name.push(character),
        }
    }
    names.push(name);
    if quoted || names.iter().any(String::is_empty) {
        return Err(format!("`{text}` is not an attribute path"));
    }
    Ok(names)
}

/// The text and syntax tree of the Nix file at [`file`](lynk:rust.argument),
/// relative to [`root`](lynk:rust.argument), if it parses.
fn parse(root: &Path, file: &Path) -> Option<(String, SyntaxNode)> {
    let text = std::fs::read_to_string(root.join(file)).ok()?;
    let parse = Root::parse(&text);
    if !parse.errors().is_empty() {
        return None;
    }
    let syntax = parse.syntax();
    Some((text, syntax))
}

fn spans_of(text: &str, file: &Path, ranges: impl Iterator<Item = TextRange>) -> Vec<Span> {
    let line_index = LineIndex::new(text);
    let mut spans: Vec<Span> = ranges
        .map(|range| line_index.span(file, usize::from(range.start())..usize::from(range.end())))
        .collect();
    spans.sort();
    spans.dedup();
    spans
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
    let Some((text, syntax)) = parse(root, file) else {
        return Vec::new();
    };
    let ranges = dev_shell_inputs(&syntax)
        .into_iter()
        .flat_map(|input| evaluate::built(&evaluate::evaluate(&input, &None, 0)))
        .filter(|(names, _)| names.iter().any(|name| name == command))
        .map(|(_, range)| range);
    spans_of(&text, file, ranges)
}

/// A binding of an attribute, or an attribute inherited, with its whole
/// attribute path.
struct Definition {
    path: Vec<String>,
    /// The outermost attribute set its path is counted from, or the `let`
    /// binding it: the definitions sharing one define one value.
    root: TextRange,
    /// Whether its root is a `let`.
    bound_by_let: bool,
    /// How many names of its path the definition writes itself, the rest
    /// being those of the attribute sets it is nested in.
    own: usize,
    range: TextRange,
    /// The value a binding gives its path; `None` for an inherited
    /// attribute.
    value: Option<ast::Expr>,
}

/// Every definition of a file whose attribute path is known, in the order
/// they are written. A binding spans from its name to the end of its value,
/// an inherited attribute covers its name.
fn definitions(syntax: &SyntaxNode) -> Vec<Definition> {
    let mut found = Vec::new();
    for node in syntax.descendants() {
        if let Some(binding) = ast::AttrpathValue::cast(node.clone()) {
            let (Some((path, root)), Some(names), Some(attrpath), Some(value)) = (
                attribute_path(&binding),
                attribute_names(&binding),
                binding.attrpath(),
                binding.value(),
            ) else {
                continue;
            };
            found.push(Definition {
                path,
                root: root.text_range(),
                bound_by_let: ast::LetIn::can_cast(root.kind()),
                own: names.len(),
                range: TextRange::new(attrpath.syntax().text_range().start(), value.syntax().text_range().end()),
                value: Some(value),
            });
            continue;
        }
        let Some(inherit) = ast::Inherit::cast(node) else {
            continue;
        };
        let Some((container, root)) = inherit.syntax().parent().and_then(container_path) else {
            continue;
        };
        for attribute in inherit.attrs() {
            let Some(name) = attr_name(&attribute) else {
                continue;
            };
            let mut path = container.clone();
            path.push(name);
            found.push(Definition {
                path,
                root: root.text_range(),
                bound_by_let: ast::LetIn::can_cast(root.kind()),
                own: 1,
                range: attribute.syntax().text_range(),
                value: None,
            });
        }
    }
    found
}

/// The spans of the definitions of the attribute at
/// [`path`](lynk:rust.argument) in the Nix file at
/// [`file`](lynk:rust.argument), relative to [`root`](lynk:rust.argument):
/// each binding whose own names hold its last name, so `a.b` is defined by
/// `a.b = …`, by `a.b.c = …` and by the `b = …` of `a = {b = …;}`. The
/// definitions sharing a root are one span, from the first to the end of
/// the last.
fn find_attribute(root: &Path, file: &Path, path: &[String]) -> Vec<Span> {
    let Some((text, syntax)) = parse(root, file) else {
        return Vec::new();
    };
    let found = definitions(&syntax);
    let defining = found.iter().filter(|definition| defines(definition, path));
    spans_of(&text, file, merge_by_root(defining).into_iter())
}

/// Whether [`definition`](lynk:rust.argument) defines the attribute at
/// [`path`](lynk:rust.argument), its own names holding the last name of
/// the path.
fn defines(definition: &Definition, path: &[String]) -> bool {
    definition.path.starts_with(path) && definition.path.len() - definition.own < path.len()
}

/// The span of each definition of the attribute at
/// [`path`](lynk:rust.argument) in the Nix file at
/// [`file`](lynk:rust.argument), relative to [`root`](lynk:rust.argument),
/// with the literal it gives the attribute: `None` for an inherited
/// attribute, a binding of an attribute under it, or a value that is not a
/// literal.
fn find_values(root: &Path, file: &Path, path: &[String]) -> Vec<(Span, Option<Literal>)> {
    let Some((text, syntax)) = parse(root, file) else {
        return Vec::new();
    };
    let line_index = LineIndex::new(&text);
    definitions(&syntax)
        .into_iter()
        .filter(|definition| defines(definition, path))
        .map(|definition| {
            let span = line_index.span(file, usize::from(definition.range.start())..usize::from(definition.range.end()));
            let literal = definition
                .value
                .filter(|_| definition.path.len() == path.len())
                .and_then(|value| literal(&value));
            (span, literal)
        })
        .collect()
}

/// The value of an expression that is a literal, maybe negated: a string
/// without interpolations, an integer, a float, or `true` or `false`;
/// `None` for any other expression.
fn literal(expression: &ast::Expr) -> Option<Literal> {
    match expression {
        ast::Expr::Str(text) => Some(Literal::Text(plain_string(text)?)),
        ast::Expr::Literal(literal) => match literal.kind() {
            ast::LiteralKind::Integer(integer) => {
                let digits = integer.syntax().text().trim_start_matches('0');
                Some(Literal::Integer {
                    negative: false,
                    digits: if digits.is_empty() { "0" } else { digits }.to_owned(),
                })
            }
            ast::LiteralKind::Float(float) => Some(Literal::Float(float.syntax().text().parse().ok()?)),
            ast::LiteralKind::Uri(uri) => Some(Literal::Text(uri.syntax().text().to_owned())),
        },
        ast::Expr::Ident(identifier) => match identifier.ident_token()?.text() {
            "true" => Some(Literal::Bool(true)),
            "false" => Some(Literal::Bool(false)),
            _ => None,
        },
        ast::Expr::Paren(parenthesized) => literal(&parenthesized.expr()?),
        ast::Expr::UnaryOp(unary) if unary.operator() == Some(ast::UnaryOpKind::Negate) => literal(&unary.expr()?)?.negated(),
        _ => None,
    }
}

/// The ranges of [`definitions`](lynk:rust.argument), those sharing a
/// root merged into one, from the first to the end of the last.
fn merge_by_root<'a>(definitions: impl Iterator<Item = &'a Definition>) -> Vec<TextRange> {
    let mut merged: Vec<(TextRange, TextRange)> = Vec::new();
    for definition in definitions {
        match merged.iter_mut().find(|(root, _)| *root == definition.root) {
            Some((_, range)) => *range = range.cover(definition.range),
            None => merged.push((definition.root, definition.range)),
        }
    }
    merged.into_iter().map(|(_, range)| range).collect()
}

/// The spans of what `nix run` runs for [`name`](lynk:rust.argument) in
/// the Nix file at [`file`](lynk:rust.argument), relative to
/// [`root`](lynk:rust.argument): the definitions of the attribute named so
/// under the first of the outputs it looks a name up in that holds one,
/// merged as for an attribute.
fn find_app(root: &Path, file: &Path, name: &str) -> Vec<Span> {
    let Some((text, syntax)) = parse(root, file) else {
        return Vec::new();
    };
    let found = definitions(&syntax);
    let ranges = RUNNABLE_OUTPUTS
        .iter()
        .map(|output| merge_by_root(found.iter().filter(|definition| defines_runnable(definition, output, name))))
        .find(|ranges| !ranges.is_empty())
        .unwrap_or_default();
    spans_of(&text, file, ranges.into_iter())
}

/// Whether [`definition`](lynk:rust.argument) defines the runnable
/// [`name`](lynk:rust.argument) of the flake output
/// [`output`](lynk:rust.argument), as
/// [`lynk::providers::nix::runnable`](lynk:rust.item) reads its path. A
/// `let` binds no output.
fn defines_runnable(definition: &Definition, output: &str, name: &str) -> bool {
    if definition.bound_by_let {
        return false;
    }
    let Some((found, length)) = runnable(&definition.path) else {
        return false;
    };
    found == output && definition.path[length - 1] == name && defines(definition, &definition.path[..length])
}

/// The flake output a definition's [`path`](lynk:rust.argument) is under
/// and the length of the path of the runnable it defines, ending with its
/// name: `<output>.<system>.<name>` as a flake writes its outputs, or
/// `<output>.<name>` as a function building one system's outputs writes
/// them, either under an attribute flake-parts writes outputs under.
fn runnable(path: &[String]) -> Option<(&str, usize)> {
    let prefix = usize::from(path.first().is_some_and(|first| FLAKE_PARTS_OUTPUTS.contains(&first.as_str())));
    let [output, next, after @ ..] = &path[prefix..] else {
        return None;
    };
    if !RUNNABLE_OUTPUTS.contains(&output.as_str()) {
        return None;
    }
    if is_system(next) && !after.is_empty() {
        return Some((output, prefix + 3));
    }
    Some((output, prefix + 2))
}

/// Whether [`name`](lynk:rust.argument) names a system, a CPU and a kernel
/// joined by a hyphen.
fn is_system(name: &str) -> bool {
    name.rsplit_once('-')
        .is_some_and(|(cpu, kernel)| !cpu.is_empty() && SYSTEM_KERNELS.contains(&kernel))
}

/// The names of the attributes directly under the attribute at
/// [`path`](lynk:rust.argument), in the order they are first defined.
fn attribute_names_under(root: &Path, file: &Path, path: &[String]) -> Vec<String> {
    let Some((_, syntax)) = parse(root, file) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for definition in definitions(&syntax) {
        if definition.path.len() <= path.len() || !definition.path.starts_with(path) {
            continue;
        }
        let name = &definition.path[path.len()];
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
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
/// `let` holding it. `None` for a name computed with an interpolation.
fn attribute_path(binding: &ast::AttrpathValue) -> Option<(Vec<String>, SyntaxNode)> {
    let (mut path, outermost) = container_path(binding.syntax().parent()?)?;
    path.extend(attribute_names(binding)?);
    Some((path, outermost))
}

/// The attribute path of the attribute set at
/// [`container`](lynk:rust.argument), through the attribute sets it is
/// nested in as values of bindings, and the outermost of them; empty for
/// any other container of bindings, such as a `let`. An attribute set is
/// the value of a binding through parentheses and as the body of a `let`.
fn container_path(mut container: SyntaxNode) -> Option<(Vec<String>, SyntaxNode)> {
    let mut path = Vec::new();
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
    binding.attrpath()?.attrs().map(|attribute| attr_name(&attribute)).collect()
}

fn attr_name(attribute: &ast::Attr) -> Option<String> {
    match attribute {
        ast::Attr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
        ast::Attr::Str(text) => plain_string(text),
        ast::Attr::Dynamic(_) => None,
    }
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
    use crate::providers::tests::{assert_query, read_first_link};

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

    fn app_spans(test: &str, text: &str, name: &str) -> Vec<String> {
        let root = std::env::temp_dir().join(format!("lynk-nix-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("flake.nix"), text).unwrap();
        let spans = find_app(&root, Path::new("flake.nix"), name)
            .iter()
            .map(ToString::to_string)
            .collect();
        std::fs::remove_dir_all(root).unwrap();
        spans
    }

    #[test]
    fn finds_what_nix_run_runs() {
        let text = r#"{
  outputs = {nixpkgs, ...}: let
    apps.ignored = 1;
  in {
    apps.x86_64-linux.run = {
      type = "app";
      program = "run";
    };
    apps.aarch64-darwin.run.program = "run";
    packages.x86_64-linux.run = 1;
    packages.x86_64-linux.default = 2;
    flake.apps.x86_64-linux.flake-parts = 3;
    perSystem.apps.per-system = 4;
    apps.build.program = 5;
  };
}
"#;
        let spans = |name| app_spans("app", text, name);
        assert_eq!(spans("run"), vec!["flake.nix:5:5-9:43"]);
        assert_eq!(spans("default"), vec!["flake.nix:11:5-11:37"]);
        assert_eq!(spans("flake-parts"), vec!["flake.nix:12:5-12:43"]);
        assert_eq!(spans("per-system"), vec!["flake.nix:13:5-13:33"]);
        assert_eq!(spans("build"), vec!["flake.nix:14:5-14:26"]);
        assert!(spans("ignored").is_empty());
        assert!(spans("program").is_empty());
        assert!(spans("x86_64-linux").is_empty());
        assert!(Nix.query("app", &["flake.nix"], None).is_err());
    }

    #[test]
    fn takes_a_file_and_a_command() {
        assert!(Nix.query("dev-shell-command", &["flake.nix", "build"], None).is_ok());
        assert!(Nix.query("dev-shell-command", &["build"], None).is_err());
    }

    const ATTRIBUTES: &str = r#"{lib, ...}: let
  port = 8080;
  inherit (lib) mkIf;
in {
  services.web = {
    enable = true;
    inherit port;
  };
  services.web.settings = let
    level = "info";
  in {
    log.level = level;
    "tls.cert" = ./cert.pem;
  };
  services.db.enable = mkIf true true;
  environment = lib.mkMerge [{SKIPPED = 1;} {SKIPPED = 2;}];
}
"#;

    /// Calls [`check`](lynk:rust.argument) with the root of a directory
    /// holding `ATTRIBUTES` as `default.nix`.
    fn with_attributes<T>(test: &str, check: impl FnOnce(&Path) -> T) -> T {
        let root = std::env::temp_dir().join(format!("lynk-nix-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("default.nix"), ATTRIBUTES).unwrap();
        let result = check(&root);
        std::fs::remove_dir_all(root).unwrap();
        result
    }

    fn path(text: &str) -> Vec<String> {
        parse_attribute_path(text).unwrap()
    }

    #[test]
    fn finds_attributes_however_their_paths_are_written() {
        let spans = with_attributes("attribute", |root| {
            let spans = |text: &str| -> Vec<String> {
                find_attribute(root, Path::new("default.nix"), &path(text))
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            };
            [
                spans("services.web"),
                spans("services.web.port"),
                spans("services.web.settings.log.level"),
                spans("services.web.settings.\"tls.cert\""),
                spans("port"),
                spans("mkIf"),
                spans("environment.SKIPPED"),
                spans("SKIPPED"),
                spans("services.web.missing"),
            ]
        });
        assert_eq!(spans, [
            vec!["default.nix:5:3-14:3"],
            vec!["default.nix:7:13-7:16"],
            vec!["default.nix:12:5-12:21"],
            vec!["default.nix:13:5-13:27"],
            vec!["default.nix:2:3-2:13"],
            vec!["default.nix:3:17-3:20"],
            vec![],
            vec!["default.nix:16:31-16:41", "default.nix:16:46-16:56"],
            vec![],
        ]);
    }

    #[test]
    fn finds_the_literal_each_definition_gives_an_attribute() {
        let values = with_attributes("literal", |root| {
            let values = |text: &str| -> Vec<(String, Option<Literal>)> {
                find_values(root, Path::new("default.nix"), &path(text))
                    .into_iter()
                    .map(|(span, literal)| (span.to_string(), literal))
                    .collect()
            };
            [
                values("port"),
                values("services.web.enable"),
                values("services.web.port"),
                values("services.web.settings.log.level"),
                values("services.web.settings"),
                values("services.web"),
            ]
        });
        let integer = Literal::Integer {
            negative: false,
            digits: "8080".to_owned(),
        };
        assert_eq!(values, [
            vec![("default.nix:2:3-2:13".to_owned(), Some(integer))],
            vec![("default.nix:6:5-6:17".to_owned(), Some(Literal::Bool(true)))],
            vec![("default.nix:7:13-7:16".to_owned(), None)],
            vec![("default.nix:12:5-12:21".to_owned(), None)],
            vec![("default.nix:9:3-14:3".to_owned(), None)],
            vec![("default.nix:5:3-8:3".to_owned(), None), ("default.nix:9:3-14:3".to_owned(), None)],
        ]);
    }

    #[test]
    fn reads_literals_maybe_negated() {
        let value = |source: &str| {
            let syntax = Root::parse(source).tree().expr().unwrap();
            literal(&syntax)
        };
        assert_eq!(value("\"tool\""), Some(Literal::Text("tool".to_owned())));
        assert_eq!(value("''\n  tool\n''"), Some(Literal::Text("tool\n".to_owned())));
        assert_eq!(
            value("-(007)"),
            Some(Literal::Integer {
                negative: true,
                digits: "7".to_owned()
            })
        );
        assert_eq!(value("1.5e1"), Some(Literal::Float(15.0)));
        assert_eq!(value("false"), Some(Literal::Bool(false)));
        assert!(value("-0").unwrap().matches("0"));
        assert!(value("2.0").unwrap().matches("2"));
        assert!(value("\"a tool\"").unwrap().matches("a tool"));
        assert_eq!(value("\"${name}\""), None);
        assert_eq!(value("-\"tool\""), None);
        assert_eq!(value("1 + 2"), None);
        assert_eq!(value("null"), None);
        assert_eq!(value("./cert.pem"), None);
    }

    #[test]
    fn a_literal_takes_its_file_and_path_in_the_title_and_its_value_as_text() {
        let (query, _) = read_first_link("test.md", "[`8080`](lynk:nix.literal (default.nix services.port))").unwrap();
        assert_query(query.as_ref(), &NixQuery::Literal {
            file: PathBuf::from("default.nix"),
            path: vec!["services".to_owned(), "port".to_owned()],
            value: "8080".to_owned(),
        });
        assert!(Nix.query("literal", &["default.nix", "port"], None).is_err());
    }

    #[test]
    fn lists_the_attributes_under_a_path_in_the_order_they_are_first_defined() {
        let names = with_attributes("map-attribute", |root| {
            let names = |text: &str| attribute_names_under(root, Path::new("default.nix"), &path(text));
            [
                names("services"),
                names("services.web"),
                names("services.web.settings"),
                names("environment"),
            ]
        });
        assert_eq!(names, [
            vec!["web", "db"],
            vec!["enable", "port", "settings"],
            vec!["log", "tls.cert"],
            vec![]
        ]);
    }

    #[test]
    fn reads_attribute_paths_as_nix_writes_them() {
        assert_eq!(path("a.\"b.c\".d"), ["a", "b.c", "d"]);
        assert!(parse_attribute_path("a..b").is_err());
        assert!(parse_attribute_path("a.\"b").is_err());
        assert!(Nix.query("attribute", &["default.nix"], None).is_err());
    }

    #[test]
    fn a_map_of_attributes_takes_a_file_a_path_and_a_body() {
        let (query, members) = read_first_link(
            "test.md",
            "<!-- lynk nix.map-attribute default.nix services.web -->\n| Attribute | Use |\n| --- | --- |\n| `enable` | on |\n<!-- /lynk \
             -->\n",
        )
        .unwrap();
        assert_query(query.as_ref(), &NixQuery::MapAttribute {
            file: PathBuf::from("default.nix"),
            path: vec!["services".to_owned(), "web".to_owned()],
        });
        assert_eq!(members[0].name, "enable");
    }
}
