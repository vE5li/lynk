use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::document::{Document, Format};
use crate::link::{Argument, Link, Member, Shape, SyntaxError};
use crate::location::{LineIndex, Position, Span};
use crate::workspace::Workspace;
use crate::{command, rust, wit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    MarkdownSection,
    MarkdownLine,
    FilesystemFile,
    FilesystemDirectory,
    FilesystemMapEntry,
    RustModule,
    RustArgument,
    WitFunction,
    NexusSelectors,
    NexusSelf,
    NexusLint,
    NexusWhere,
    NexusRef,
    NexusFind,
}

impl Selector {
    const ALL: [Self; 14] = [
        Self::MarkdownSection,
        Self::MarkdownLine,
        Self::FilesystemFile,
        Self::FilesystemDirectory,
        Self::FilesystemMapEntry,
        Self::RustModule,
        Self::RustArgument,
        Self::WitFunction,
        Self::NexusSelectors,
        Self::NexusSelf,
        Self::NexusLint,
        Self::NexusWhere,
        Self::NexusRef,
        Self::NexusFind,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::MarkdownSection => "markdown.section",
            Self::MarkdownLine => "markdown.line",
            Self::FilesystemFile => "filesystem.file",
            Self::FilesystemDirectory => "filesystem.directory",
            Self::FilesystemMapEntry => "filesystem.map-entry",
            Self::RustModule => "rust.module",
            Self::RustArgument => "rust.argument",
            Self::WitFunction => "wit.function",
            Self::NexusSelectors => "nexus.selectors",
            Self::NexusSelf => "nexus.self",
            Self::NexusLint => "nexus.lint",
            Self::NexusWhere => "nexus.where",
            Self::NexusRef => "nexus.ref",
            Self::NexusFind => "nexus.find",
        }
    }

    /// The name of the subcommand the selector points at.
    fn command(self) -> Option<&'static str> {
        match self {
            Self::NexusLint => Some("lint"),
            Self::NexusWhere => Some("where"),
            Self::NexusRef => Some("ref"),
            Self::NexusFind => Some("find"),
            _ => None,
        }
    }

    /// Whether it is a map selector, taking a body.
    fn is_map(self) -> bool {
        matches!(self, Self::FilesystemMapEntry | Self::NexusSelectors)
    }

    fn takes_arguments(self) -> bool {
        !matches!(
            self,
            Self::NexusSelectors | Self::NexusSelf | Self::NexusLint | Self::NexusWhere | Self::NexusRef | Self::NexusFind
        )
    }

    /// How a selector named [`name`](nexus:rust.argument) reads a link, a
    /// provider's `*` included; an unknown one is read as taking arguments
    /// and no body, to be reported once read.
    fn shape(name: &str) -> Shape {
        match name.split_once('.') {
            Some((_, EVERY_SELECTOR)) => Shape {
                map: true,
                takes_arguments: false,
            },
            _ => Shape {
                map: Self::from_name(name).is_some_and(Self::is_map),
                takes_arguments: Self::from_name(name).is_none_or(Self::takes_arguments),
            },
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|selector| selector.name() == name)
    }

    /// The name's two parts, `<provider>.<selector>`.
    fn provider_and_selector(self) -> (&'static str, &'static str) {
        self.name().split_once('.').expect("a selector's name is `<provider>.<selector>`")
    }

    /// The names of a provider's selectors, without the provider; empty for a
    /// provider that does not exist.
    fn selectors_of(provider: &str) -> Vec<&'static str> {
        Self::ALL
            .into_iter()
            .map(Self::provider_and_selector)
            .filter(|(selector_provider, _)| *selector_provider == provider)
            .map(|(_, selector)| selector)
            .collect()
    }
}

/// Written in place of a selector, it links to all of a provider's selectors.
const EVERY_SELECTOR: &str = "*";

/// A problem with one member of a map's body; without a range it concerns
/// the whole link.
pub struct MemberError {
    pub range: Option<Range<usize>>,
    pub message: String,
}

/// Where a link is written, which selectors may default to.
pub struct Place<'a> {
    pub file: &'a Path,
    pub line: usize,
}

/// A Rust function, by its path as Rust writes it or as the one a link is
/// written on: the innermost whose definition, doc comment included, holds
/// the link's first line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustFunction {
    Path(String),
    Enclosing { file: PathBuf, line: usize },
}

/// A link's target with its arguments interpreted. A file left out (`None`)
/// matches every file; links fill it with the file they are written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    MarkdownSection {
        file: Option<PathBuf>,
        heading: String,
    },
    MarkdownLine {
        file: Option<PathBuf>,
        text: String,
    },
    FilesystemFile {
        path: PathBuf,
    },
    FilesystemDirectory {
        path: PathBuf,
    },
    /// The entries of a directory, as complete map.
    FilesystemMapEntry {
        path: PathBuf,
    },
    /// A module, by its path as Rust writes it, starting with the crate.
    RustModule {
        path: String,
    },
    /// An argument of a function, by name.
    RustArgument {
        function: RustFunction,
        argument: String,
    },
    /// The functions of a WIT file with a name, `<resource>.<function>` for
    /// a resource's.
    WitFunction {
        file: PathBuf,
        name: String,
    },
    /// The selectors this tool implements; defined here, in its own source.
    NexusSelectors,
    /// The selectors of one provider, as complete map.
    ProviderSelectors {
        provider: String,
    },
    /// This tool as a whole.
    NexusSelf,
    /// One of its subcommands, by name.
    NexusCommand(&'static str),
}

impl Query {
    /// Reads a link's content as its selector defines it: its arguments,
    /// then the members of its body for a map selector.
    /// [`document`](nexus:rust.argument) is the one the link is written in.
    pub fn of_link(link: &Link, document: &Document) -> Result<(Self, Vec<Member>), SyntaxError> {
        let selector_name = link.selector_name.as_str();
        let (arguments, members) = link.read(&document.prose, Selector::shape(selector_name))?;
        let place = Place {
            file: &document.path,
            line: document.span(link.range.clone()).start.line,
        };
        let query = Self::new(selector_name, &arguments, Some(&place)).map_err(|message| SyntaxError {
            message,
            offset: link.range.start,
        })?;
        Ok((query, members))
    }

    /// [`place`](nexus:rust.argument) is where a link is written, `None` on
    /// the command line.
    pub fn new(selector_name: &str, arguments: &[Argument], place: Option<&Place>) -> Result<Self, String> {
        let context_file = place.map(|place| place.file);
        let texts: Vec<&str> = arguments.iter().map(|argument| argument.text.as_str()).collect();
        if let Some((provider, EVERY_SELECTOR)) = selector_name.split_once('.') {
            if Selector::selectors_of(provider).is_empty() {
                return Err(format!("unknown provider `{provider}`"));
            }
            if !texts.is_empty() {
                return Err(format!("`{selector_name}` takes no arguments"));
            }
            return Ok(Self::ProviderSelectors {
                provider: provider.to_owned(),
            });
        }
        let selector = Selector::from_name(selector_name).ok_or_else(|| format!("unknown selector `{selector_name}`"))?;
        match selector {
            Selector::MarkdownSection => {
                let (file, heading) = optional_file_and_name(selector_name, &texts, context_file)?;
                Ok(Self::MarkdownSection { file, heading })
            }
            Selector::MarkdownLine => {
                let (file, text) = optional_file_and_name(selector_name, &texts, context_file)?;
                Ok(Self::MarkdownLine { file, text })
            }
            Selector::FilesystemFile => Ok(Self::FilesystemFile {
                path: single_path(selector_name, &texts)?,
            }),
            Selector::FilesystemDirectory => Ok(Self::FilesystemDirectory {
                path: single_path(selector_name, &texts)?,
            }),
            Selector::FilesystemMapEntry => Ok(Self::FilesystemMapEntry {
                path: single_path(selector_name, &texts)?,
            }),
            Selector::RustModule => match texts.as_slice() {
                [path] => Ok(Self::RustModule { path: (*path).to_owned() }),
                _ => Err(format!("`{selector_name}` takes a module path")),
            },
            Selector::RustArgument => {
                let (function, argument) = match (texts.as_slice(), place) {
                    ([function, argument], _) => (RustFunction::Path((*function).to_owned()), argument),
                    ([argument], Some(place)) if Format::of(place.file) == Some(Format::Rust) => (
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
                Ok(Self::RustArgument {
                    function,
                    argument: (*argument).to_owned(),
                })
            }
            Selector::WitFunction => match texts.as_slice() {
                [file, name] => Ok(Self::WitFunction {
                    file: normalize(file),
                    name: (*name).to_owned(),
                }),
                _ => Err(format!("`{selector_name}` takes a file and a function")),
            },
            Selector::NexusSelectors
            | Selector::NexusSelf
            | Selector::NexusLint
            | Selector::NexusWhere
            | Selector::NexusRef
            | Selector::NexusFind => {
                if !texts.is_empty() {
                    return Err(format!("`{selector_name}` takes no arguments"));
                }
                Ok(match (selector, selector.command()) {
                    (_, Some(command)) => Self::NexusCommand(command),
                    (Selector::NexusSelectors, None) => Self::NexusSelectors,
                    _ => Self::NexusSelf,
                })
            }
        }
    }

    /// Checks a map's body against its target's members.
    pub fn check_members(&self, members: &[Member], workspace: &Workspace) -> Vec<MemberError> {
        match self {
            Self::NexusSelectors => check_complete_map(members, &Selector::ALL.map(Selector::name)),
            Self::ProviderSelectors { provider } => check_complete_map(members, &Selector::selectors_of(provider)),
            Self::FilesystemMapEntry { path } => {
                let entries = directory_entries(&workspace.root.join(path));
                let entry_names: Vec<&str> = entries.iter().map(String::as_str).collect();
                check_complete_map(members, &entry_names)
            }
            _ => Vec::new(),
        }
    }

    pub fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        match self {
            Self::MarkdownSection { file, heading } => resolve_section(workspace, file.as_deref(), heading),
            Self::MarkdownLine { file, text } => resolve_line(workspace, file.as_deref(), text),
            Self::FilesystemFile { path } => {
                let exists = workspace.root.join(path).is_file();
                exists.then(|| Span::whole_file(path)).into_iter().collect()
            }
            Self::FilesystemDirectory { path } | Self::FilesystemMapEntry { path } => {
                let exists = workspace.root.join(path).is_dir();
                exists.then(|| Span::whole_file(path)).into_iter().collect()
            }
            Self::RustModule { path } => rust::find_module(&workspace.root, path).into_iter().collect(),
            Self::RustArgument { function, argument } => match function {
                RustFunction::Path(path) => rust::find_argument(&workspace.root, path, argument),
                RustFunction::Enclosing { file, line } => rust::find_enclosing_argument(&workspace.root, file, *line, argument),
            },
            Self::WitFunction { file, name } => wit::find_function(&workspace.root, file, name),
            Self::NexusSelectors | Self::ProviderSelectors { .. } => {
                vec![Span::whole_file(&own_source(workspace, file!()))]
            }
            Self::NexusSelf => vec![Span::whole_file(&own_source(workspace, ""))],
            Self::NexusCommand(command) => {
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
}

/// The names of a directory's entries, skipping hidden and ignored ones as
/// the search for documents does.
fn directory_entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = ignore::WalkBuilder::new(directory)
        .max_depth(Some(1))
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.depth() == 1)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A path inside this tool's crate, relative to the repository root when
/// the tool was built from inside it.
fn own_source(workspace: &Workspace, relative_path: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let relative_to_root = source.strip_prefix(&workspace.root).unwrap_or(&source);
    relative_to_root.components().collect()
}

fn check_complete_map(members: &[Member], expected: &[&str]) -> Vec<MemberError> {
    let mut errors = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let range = Some(member.range.clone());
        if !expected.contains(&member.name.as_str()) {
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
        if !members.iter().any(|member| member.name == *name) {
            errors.push(MemberError {
                range: None,
                message: format!("`{name}` is missing"),
            });
        }
    }
    errors
}

fn optional_file_and_name(selector_name: &str, texts: &[&str], context_file: Option<&Path>) -> Result<(Option<PathBuf>, String), String> {
    match texts {
        [name] => Ok((context_file.map(Path::to_owned), (*name).to_owned())),
        [file, name] => Ok((Some(normalize(file)), (*name).to_owned())),
        _ => Err(format!("`{selector_name}` takes an optional file and a name")),
    }
}

fn single_path(selector_name: &str, texts: &[&str]) -> Result<PathBuf, String> {
    match texts {
        [path] => Ok(normalize(path)),
        _ => Err(format!("`{selector_name}` takes a path")),
    }
}

/// Paths are relative to the repository root; `./` and trailing slashes are
/// dropped so equal paths compare equal.
fn normalize(path: &str) -> PathBuf {
    let normalized: PathBuf = Path::new(path)
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect();
    if normalized.as_os_str().is_empty() {
        return PathBuf::from(".");
    }
    normalized
}

fn resolve_section(workspace: &Workspace, file: Option<&Path>, heading: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for linked in &workspace.documents {
        let document = &linked.document;
        if file.is_some_and(|file| file != document.path) {
            continue;
        }
        for (heading_index, candidate) in document.headings.iter().enumerate() {
            if candidate.text.trim() == heading {
                spans.push(document.span(document.section_range(heading_index)));
            }
        }
    }
    spans
}

fn resolve_line(workspace: &Workspace, file: Option<&Path>, text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for linked in &workspace.documents {
        let document = &linked.document;
        if document.format != Format::Markdown || file.is_some_and(|file| file != document.path) {
            continue;
        }
        for line in LineIndex::new(&document.text).lines() {
            // A link naming the text would otherwise match itself.
            let inside_link = linked
                .links
                .iter()
                .any(|link| link.range.start < line.end && line.start < link.range.end);
            if !inside_link && document.text[line.clone()].contains(text) {
                spans.push(document.span(line));
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    use crate::link::find_links;

    #[test]
    fn every_command_selector_names_a_subcommand() {
        let cli = command::Cli::command();
        for name in Selector::ALL.into_iter().filter_map(Selector::command) {
            assert!(cli.find_subcommand(name).is_some(), "{name}");
        }
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
        let query = Query::new("rust.*", &[], None).unwrap();
        assert_eq!(query, Query::ProviderSelectors {
            provider: "rust".to_owned()
        });
        assert_eq!(Selector::selectors_of("filesystem"), vec!["file", "directory", "map-entry"]);
        assert!(Query::new("unknown.*", &[], None).is_err());
    }

    /// The query and members of the first link in a Markdown text.
    fn read_first_link(text: &str) -> Result<(Query, Vec<Member>), SyntaxError> {
        let document = Document::parse(PathBuf::from("test.md"), Format::Markdown, text.to_owned());
        let found = find_links(&document);
        assert!(found.errors.is_empty(), "{:?}", found.errors);
        Query::of_link(&found.links[0], &document)
    }

    #[test]
    fn an_argument_without_a_function_is_one_of_the_function_the_link_is_on() {
        let source = "fn main() {}\n\n/// With\n/// [`root`](nexus:rust.argument).\nfn run(root: &str) {}\n";
        let document = Document::parse(PathBuf::from("src/main.rs"), Format::Rust, source.to_owned());
        let (query, _) = Query::of_link(&find_links(&document).links[0], &document).unwrap();
        assert_eq!(query, Query::RustArgument {
            function: RustFunction::Enclosing {
                file: PathBuf::from("src/main.rs"),
                line: 4
            },
            argument: "root".to_owned()
        });
        assert!(read_first_link("[`root`](nexus:rust.argument)").is_err());
        assert!(
            Query::new(
                "rust.argument",
                &[Argument {
                    text: "root".to_owned(),
                    quoted: false
                }],
                None
            )
            .is_err()
        );
    }

    #[test]
    fn the_text_of_a_link_is_its_last_argument() {
        let (query, members) = read_first_link("[Two\nwords](nexus:markdown.section\n  (docs/a.md))").unwrap();
        assert_eq!(query, Query::MarkdownSection {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Two words".to_owned()
        });
        assert!(members.is_empty());
    }

    #[test]
    fn arguments_spread_over_rust_comment_lines() {
        let source = "//! [Decision](nexus:markdown.section\n//! (docs/a.md))\nfn main() {}\n";
        let document = Document::parse(PathBuf::from("test.rs"), Format::Rust, source.to_owned());
        let found = find_links(&document);
        assert!(found.errors.is_empty());
        let (query, _) = Query::of_link(&found.links[0], &document).unwrap();
        assert_eq!(query, Query::MarkdownSection {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Decision".to_owned()
        });
    }

    #[test]
    fn a_map_selector_reads_its_body_after_its_arguments() {
        let (query, members) =
            read_first_link("<!-- nexus filesystem.map-entry\n  src -->\n- a: b\n- c: d \"quoted\"\n<!-- /nexus -->\n").unwrap();
        assert_eq!(query, Query::FilesystemMapEntry {
            path: PathBuf::from("src")
        });
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("a", "b"), ("c", "d \"quoted\"")]);
    }

    #[test]
    fn a_star_takes_a_body_and_no_arguments() {
        let (_, members) = read_first_link("<!-- nexus rust.* -->\n- module: a\n<!-- /nexus -->\n").unwrap();
        assert_eq!(members[0].name, "module");
        assert!(read_first_link("<!-- nexus rust.* a -->\n- module: a\n<!-- /nexus -->\n").is_err());
    }

    #[test]
    fn a_selector_without_a_body_is_not_a_map() {
        assert!(read_first_link("<!-- nexus filesystem.file a -->\n- b: c\n<!-- /nexus -->\n").is_err());
    }

    #[test]
    fn a_selector_without_arguments_ignores_the_text_of_its_link() {
        assert_eq!(
            read_first_link("[the lint](nexus:nexus.lint)").unwrap().0,
            Query::NexusCommand("lint")
        );
    }

    #[test]
    fn every_selector_is_found_by_its_name() {
        for selector in Selector::ALL {
            assert_eq!(Selector::from_name(selector.name()), Some(selector));
        }
    }
}
