//! What links point at: each provider names the selectors of one kind of
//! target and finds what they pick out.

use std::fmt::Debug;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::document::Document;
use crate::link::{Argument, Link, Member, Shape, SyntaxError};
use crate::location::Span;
use crate::workspace::Workspace;
mod cargo;
mod filesystem;
mod markdown;
pub mod nexus;
mod protobuf;
mod rust;
mod wit;

/// A kind of target, named by the first part of its selectors' names.
pub trait Provider {
    fn name(&self) -> &'static str;
    /// The names of its selectors, without the provider.
    fn selectors(&self) -> &'static [&'static str];
    /// How one of its [`selector`](nexus:rust.argument)s reads a link.
    fn shape(&self, selector: &str) -> Shape;
    /// One of its [`selector`](nexus:rust.argument)s with its arguments
    /// interpreted. [`place`](nexus:rust.argument) is where its link is
    /// written, `None` on the command line.
    fn query(&self, selector: &str, arguments: &[&str], place: Option<&Place>) -> Result<Box<dyn Query>, String>;
}

/// A link's target with its arguments interpreted.
pub trait Query: Debug {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span>;
    /// Checks a map's body against its target's members.
    fn check_members(&self, _members: &[Member], _workspace: &Workspace) -> Vec<MemberError> {
        Vec::new()
    }
}

pub const PROVIDERS: &[&dyn Provider] = &[
    &markdown::Markdown,
    &filesystem::Filesystem,
    &rust::Rust,
    &wit::Wit,
    &protobuf::Protobuf,
    &cargo::Cargo,
    &nexus::Nexus,
];

/// This file, relative to the crate: where the providers are listed.
pub const SOURCE: &str = file!();

/// Written in place of a selector, it links to all of a provider's selectors.
const EVERY_SELECTOR: &str = "*";

/// Where a link is written, which selectors may default to.
pub struct Place<'a> {
    pub file: &'a Path,
    pub line: usize,
}

/// A problem with one member of a map's body; without a range it concerns
/// the whole link.
pub struct MemberError {
    pub range: Option<Range<usize>>,
    pub message: String,
}

/// Every selector's full name, `<provider>.<selector>`.
pub fn selector_names() -> Vec<String> {
    PROVIDERS
        .iter()
        .flat_map(|provider| {
            provider
                .selectors()
                .iter()
                .map(move |selector| format!("{}.{selector}", provider.name()))
        })
        .collect()
}

/// The provider and selector a full name is made of; `Err` for an unknown
/// one.
fn find(selector_name: &str) -> Result<(&'static dyn Provider, &str), String> {
    let (provider_name, selector) = selector_name
        .split_once('.')
        .ok_or_else(|| format!("a selector is named `<provider>.<selector>`, not `{selector_name}`"))?;
    let provider = PROVIDERS
        .iter()
        .find(|provider| provider.name() == provider_name)
        .ok_or_else(|| format!("unknown provider `{provider_name}`"))?;

    if selector != EVERY_SELECTOR && !provider.selectors().contains(&selector) {
        return Err(format!("unknown selector `{selector_name}`"));
    }

    Ok((*provider, selector))
}

fn shape(provider: &dyn Provider, selector: &str) -> Shape {
    match selector {
        EVERY_SELECTOR => Shape {
            map: true,
            takes_arguments: false,
        },
        _ => provider.shape(selector),
    }
}

/// The query of a selector named in full, `<provider>.*` included.
pub fn query(selector_name: &str, arguments: &[Argument], place: Option<&Place>) -> Result<Box<dyn Query>, String> {
    let (provider, selector) = find(selector_name)?;
    let texts: Vec<&str> = arguments.iter().map(|argument| argument.text.as_str()).collect();
    if selector != EVERY_SELECTOR {
        return provider.query(selector, &texts, place);
    }
    no_arguments(selector_name, &texts)?;
    Ok(Box::new(EverySelector {
        selectors: provider.selectors(),
    }))
}

/// Reads a link's content as its selector defines it: its arguments, then
/// the members of its body for a map selector.
/// [`document`](nexus:rust.argument) is the one the link is written in.
pub fn of_link(link: &Link, document: &Document) -> Result<(Box<dyn Query>, Vec<Member>), SyntaxError> {
    let error = |message| SyntaxError {
        message,
        offset: link.range.start,
    };
    let (provider, selector) = find(&link.selector_name).map_err(error)?;
    let (arguments, members) = link.read(&document.prose, shape(provider, selector))?;
    let place = Place {
        file: &document.path,
        line: document.span(link.range.clone()).start.line,
    };
    let query = query(&link.selector_name, &arguments, Some(&place)).map_err(error)?;
    Ok((query, members))
}

/// The selectors of one provider, as complete map.
#[derive(Debug)]
struct EverySelector {
    selectors: &'static [&'static str],
}

impl Query for EverySelector {
    /// The list of providers, which their selectors are reached from.
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        vec![Span::whole_file(&nexus::own_source(workspace, SOURCE))]
    }

    fn check_members(&self, members: &[Member], _workspace: &Workspace) -> Vec<MemberError> {
        check_complete_map(members, self.selectors)
    }
}

pub fn check_complete_map(members: &[Member], expected: &[impl AsRef<str>]) -> Vec<MemberError> {
    let mut errors = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let range = Some(member.range.clone());
        if !expected.iter().any(|name| name.as_ref() == member.name) {
            errors.push(MemberError {
                range,
                message: format!("`{}` is not a member", member.name),
            });
        } else if members[..index].iter().any(|earlier| earlier.name == member.name) {
            errors.push(MemberError {
                range,
                message: format!("`{}` is listed twice", member.name),
            });
        }
    }
    for name in expected {
        if !members.iter().any(|member| member.name == name.as_ref()) {
            errors.push(MemberError {
                range: None,
                message: format!("`{}` is missing", name.as_ref()),
            });
        }
    }
    errors
}

pub fn no_arguments(selector_name: &str, arguments: &[&str]) -> Result<(), String> {
    match arguments {
        [] => Ok(()),
        _ => Err(format!("`{selector_name}` takes no arguments")),
    }
}

pub fn single_path(selector_name: &str, arguments: &[&str]) -> Result<PathBuf, String> {
    match arguments {
        [path] => Ok(normalize(path)),
        _ => Err(format!("`{selector_name}` takes a path")),
    }
}

/// Paths are relative to the repository root; `./` and trailing slashes are
/// dropped so equal paths compare equal.
pub fn normalize(path: &str) -> PathBuf {
    let normalized: PathBuf = Path::new(path)
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect();
    if normalized.as_os_str().is_empty() {
        return PathBuf::from(".");
    }
    normalized
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::formats;

    /// The query and members of the first link in
    /// [`text`](nexus:rust.argument), read as a file at
    /// [`path`](nexus:rust.argument).
    pub fn read_first_link(path: &str, text: &str) -> Result<(Box<dyn Query>, Vec<Member>), SyntaxError> {
        let format = formats::of(Path::new(path)).unwrap();
        let linked = format.read(PathBuf::from(path), text.to_owned());
        assert!(linked.syntax_errors.is_empty(), "{:?}", linked.syntax_errors);
        of_link(&linked.links[0], &linked.document)
    }

    /// Whether [`query`](nexus:rust.argument) is
    /// [`expected`](nexus:rust.argument), queries being compared as they
    /// print.
    pub fn assert_query(query: &dyn Query, expected: &dyn Query) {
        assert_eq!(format!("{query:?}"), format!("{expected:?}"));
    }

    fn member(name: &str) -> Member {
        Member {
            name: name.to_owned(),
            description: String::new(),
            range: 0..0,
        }
    }

    #[test]
    fn a_complete_map_lists_every_member_once() {
        let members = [member("first"), member("second")];
        assert!(check_complete_map(&members, &["first", "second"]).is_empty());
    }

    #[test]
    fn a_complete_map_reports_unknown_duplicate_and_missing_members() {
        let members = [member("first"), member("first"), member("unknown")];
        let messages: Vec<String> = check_complete_map(&members, &["first", "second"])
            .into_iter()
            .map(|error| error.message)
            .collect();
        assert_eq!(messages, vec![
            "`first` is listed twice",
            "`unknown` is not a member",
            "`second` is missing"
        ]);
    }

    #[test]
    fn a_providers_selectors_are_linked_with_a_star() {
        let query = query("filesystem.*", &[], None).unwrap();
        assert_query(query.as_ref(), &EverySelector {
            selectors: &["file", "directory", "map-entry"],
        });
        assert!(super::query("unknown.*", &[], None).is_err());
        assert!(super::query("filesystem.unknown", &[], None).is_err());
    }

    #[test]
    fn a_star_takes_a_body_and_no_arguments() {
        let (_, members) = read_first_link("test.md", "<!-- nexus rust.* -->\n- module: a\n<!-- /nexus -->\n").unwrap();
        assert_eq!(members[0].name, "module");
        assert!(read_first_link("test.md", "<!-- nexus rust.* a -->\n- module: a\n<!-- /nexus -->\n").is_err());
    }

    #[test]
    fn a_selector_without_a_body_is_not_a_map() {
        assert!(read_first_link("test.md", "<!-- nexus filesystem.file a -->\n- b: c\n<!-- /nexus -->\n").is_err());
    }

    #[test]
    fn every_selector_is_named_after_its_provider() {
        let names = selector_names();
        assert!(names.contains(&"markdown.section".to_owned()));
        for name in names {
            assert!(find(&name).is_ok(), "{name}");
        }
    }
}
