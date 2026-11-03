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
pub mod lynk;
mod markdown;
mod nix;
mod protobuf;
mod rust;
mod wit;

/// A kind of target, named by the first part of its selectors' names.
pub trait Provider {
    fn name(&self) -> &'static str;
    /// The names of its selectors, without the provider.
    fn selectors(&self) -> &'static [&'static str];
    /// How one of its [`selector`](lynk:rust.argument)s reads a link.
    fn shape(&self, selector: &str) -> Shape;
    /// One of its [`selector`](lynk:rust.argument)s with its arguments
    /// interpreted. [`place`](lynk:rust.argument) is where its link is
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
    /// Checks what a link states about its target besides where it is, such
    /// as its value; each problem concerns the whole link.
    fn check_target(&self, _workspace: &Workspace) -> Vec<String> {
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
    &nix::Nix,
    &lynk::Lynk,
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
/// [`document`](lynk:rust.argument) is the one the link is written in.
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
        vec![Span::whole_file(&lynk::own_source(workspace, SOURCE))]
    }

    fn check_members(&self, members: &[Member], _workspace: &Workspace) -> Vec<MemberError> {
        check_complete_map(members, self.selectors)
    }
}

/// Checks that [`members`](lynk:rust.argument) lists every name of
/// [`expected`](lynk:rust.argument) exactly once, in the same order.
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
    errors.extend(check_order(members, expected));
    errors
}

/// Checks that the members found in [`expected`](lynk:rust.argument) are
/// listed in its order, each counted where it is first listed. The longest
/// run already in order is kept, and every other member is reported against
/// the nearest kept member it belongs before, so moving one member reports
/// only that member.
fn check_order(members: &[Member], expected: &[impl AsRef<str>]) -> Vec<MemberError> {
    let listed: Vec<(&Member, usize)> = members
        .iter()
        .enumerate()
        .filter(|(index, member)| !members[..*index].iter().any(|earlier| earlier.name == member.name))
        .filter_map(|(_, member)| {
            let position = expected.iter().position(|name| name.as_ref() == member.name)?;
            Some((member, position))
        })
        .collect();
    let positions: Vec<usize> = listed.iter().map(|(_, position)| *position).collect();
    let kept = longest_increasing(&positions);

    let mut errors = Vec::new();
    for (index, (member, position)) in listed.iter().enumerate() {
        if kept.contains(&index) {
            continue;
        }
        let before = kept.iter().map(|&kept| listed[kept]).find(|(_, kept)| kept > position);
        let message = match before {
            Some((neighbour, _)) => {
                format!("`{}` belongs before `{}`", member.name, neighbour.name)
            }
            None => {
                let (neighbour, _) = listed[*kept.last().expect("an unordered member has an ordered one")];
                format!("`{}` belongs after `{}`", member.name, neighbour.name)
            }
        };
        errors.push(MemberError {
            range: Some(member.range.clone()),
            message,
        });
    }
    errors
}

/// The indices of a longest strictly increasing subsequence of
/// [`values`](lynk:rust.argument), in order.
fn longest_increasing(values: &[usize]) -> Vec<usize> {
    // For each index, the length of the longest run ending there and the
    // index before it in that run.
    let mut runs: Vec<(usize, Option<usize>)> = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let best = (0..index)
            .filter(|&earlier| values[earlier] < *value)
            .max_by_key(|&earlier| (runs[earlier].0, std::cmp::Reverse(earlier)));
        runs.push(match best {
            Some(earlier) => (runs[earlier].0 + 1, Some(earlier)),
            None => (1, None),
        });
    }
    let Some(mut last) = (0..values.len()).max_by_key(|&index| (runs[index].0, std::cmp::Reverse(index))) else {
        return Vec::new();
    };
    let mut indices = vec![last];
    while let Some(earlier) = runs[last].1 {
        indices.push(earlier);
        last = earlier;
    }
    indices.reverse();
    indices
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
    /// [`text`](lynk:rust.argument), read as a file at
    /// [`path`](lynk:rust.argument).
    pub fn read_first_link(path: &str, text: &str) -> Result<(Box<dyn Query>, Vec<Member>), SyntaxError> {
        let format = formats::of(Path::new(path)).unwrap();
        let linked = format.read(PathBuf::from(path), text.to_owned());
        assert!(linked.syntax_errors.is_empty(), "{:?}", linked.syntax_errors);
        of_link(&linked.links[0], &linked.document)
    }

    /// Whether [`query`](lynk:rust.argument) is
    /// [`expected`](lynk:rust.argument), queries being compared as they
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

    fn order_messages(members: &[&str], expected: &[&str]) -> Vec<String> {
        let members: Vec<Member> = members.iter().map(|name| member(name)).collect();
        check_complete_map(&members, expected)
            .into_iter()
            .map(|error| error.message)
            .collect()
    }

    #[test]
    fn a_complete_map_lists_its_members_in_order() {
        assert_eq!(order_messages(&["b", "a"], &["a", "b"]), vec!["`a` belongs before `b`"]);
    }

    #[test]
    fn a_moved_member_is_the_only_one_out_of_order() {
        assert_eq!(order_messages(&["b", "c", "d", "a", "e"], &["a", "b", "c", "d", "e"]), vec![
            "`a` belongs before `b`"
        ]);
        assert_eq!(order_messages(&["e", "a", "b", "c", "d"], &["a", "b", "c", "d", "e"]), vec![
            "`e` belongs after `d`"
        ]);
    }

    #[test]
    fn unknown_and_repeated_members_are_not_out_of_order() {
        assert_eq!(order_messages(&["a", "x", "b", "a"], &["a", "b"]), vec![
            "`x` is not a member",
            "`a` is listed twice"
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
        let (_, members) = read_first_link("test.md", "<!-- lynk rust.* -->\n- module: a\n<!-- /lynk -->\n").unwrap();
        assert_eq!(members[0].name, "module");
        assert!(read_first_link("test.md", "<!-- lynk rust.* a -->\n- module: a\n<!-- /lynk -->\n").is_err());
    }

    #[test]
    fn a_selector_without_a_body_is_not_a_map() {
        assert!(read_first_link("test.md", "<!-- lynk filesystem.file a -->\n- b: c\n<!-- /lynk -->\n").is_err());
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
