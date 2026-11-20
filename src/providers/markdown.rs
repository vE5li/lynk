//! The sections and lines of Markdown files.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::formats::markdown::NAME;
use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::providers::{Place, Provider, Query, normalize};
use crate::workspace::Workspace;

pub struct Markdown;

/// From the heading to the next heading of the same or a higher level.
fn section_range(document: &Document, heading_index: usize) -> Range<usize> {
    let heading = &document.headings[heading_index];
    let end = document.headings[heading_index + 1..]
        .iter()
        .find(|later| later.level <= heading.level)
        .map_or(document.text.len(), |later| later.range.start);
    let trimmed_length = document.text[heading.range.start..end].trim_end().len();
    heading.range.start..heading.range.start + trimmed_length
}

impl Provider for Markdown {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["section", "line"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        let (file, name) = match arguments {
            [name] => (place.map(|place| place.file.to_owned()), (*name).to_owned()),
            [file, name] => (Some(normalize(file)), (*name).to_owned()),
            _ => {
                return Err(format!("`{NAME}.{selector}` takes an optional file and a name"));
            }
        };
        Ok(Box::new(match selector {
            "section" => MarkdownQuery::Section { file, heading: name },
            _ => MarkdownQuery::Line { file, text: name },
        }))
    }
}

/// A file left out (`None`) matches every file; links fill it with the file
/// they are written in.
#[derive(Debug)]
enum MarkdownQuery {
    Section { file: Option<PathBuf>, heading: String },
    Line { file: Option<PathBuf>, text: String },
}

impl Query for MarkdownQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        match self {
            Self::Section { file, heading } => resolve_section(workspace, file.as_deref(), heading),
            Self::Line { file, text } => resolve_line(workspace, file.as_deref(), text),
        }
    }
}

fn resolve_section(workspace: &Workspace, file: Option<&Path>, heading: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for linked in &workspace.documents {
        let document = &linked.document;
        if file.is_some_and(|file| file != document.path) {
            continue;
        }
        let anchors = anchors(document);
        for (heading_index, candidate) in document.headings.iter().enumerate() {
            if candidate.text.trim() == heading || anchors[heading_index] == heading {
                spans.push(document.span(section_range(document, heading_index)));
            }
        }
    }
    spans
}

/// The anchor of each heading of [`document`](lynk:rust.argument), as GitHub
/// writes them: lowercased, spaces as hyphens, punctuation dropped, and a
/// repeated one numbered.
fn anchors(document: &Document) -> Vec<String> {
    let mut taken = HashSet::new();
    let mut repeats: HashMap<String, usize> = HashMap::new();
    let mut anchors = Vec::new();
    for heading in &document.headings {
        let base: String = heading
            .text
            .trim()
            .to_lowercase()
            .chars()
            .filter_map(|character| match character {
                ' ' => Some('-'),
                '-' | '_' => Some(character),
                _ if character.is_alphanumeric() => Some(character),
                _ => None,
            })
            .collect();
        let mut anchor = base.clone();
        while taken.contains(&anchor) {
            let repeat = repeats.entry(base.clone()).or_default();
            *repeat += 1;
            anchor = format!("{base}-{repeat}");
        }
        taken.insert(anchor.clone());
        anchors.push(anchor);
    }
    anchors
}

fn resolve_line(workspace: &Workspace, file: Option<&Path>, text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for linked in &workspace.documents {
        let document = &linked.document;
        if document.format != NAME || file.is_some_and(|file| file != document.path) {
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
    use super::*;
    use crate::formats;
    use crate::providers::tests::{assert_query, read_first_link};

    #[test]
    fn the_text_of_a_link_is_its_last_argument() {
        let (query, members) = read_first_link("test.md", "[Two\nwords](lynk:markdown.section\n  (docs/a.md))").unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Section {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Two words".to_owned(),
        });
        assert!(members.is_empty());
    }

    #[test]
    fn a_file_left_out_is_the_one_the_link_is_written_in() {
        let (query, _) = read_first_link("docs/b.md", "[A line](lynk:markdown.line)").unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Line {
            file: Some(PathBuf::from("docs/b.md")),
            text: "A line".to_owned(),
        });
    }

    #[test]
    fn a_section_is_named_by_its_heading_or_its_anchor() {
        let path = "docs/a.md";
        let text = "# Link `syntax`!\n\n## Notes\n\n## Notes\n";
        let documents = vec![formats::of(Path::new(path)).unwrap().read(PathBuf::from(path), text.to_owned())];
        let workspace = Workspace {
            root: PathBuf::new(),
            documents,
        };
        let lines = |heading: &str| -> Vec<usize> {
            resolve_section(&workspace, Some(Path::new(path)), heading)
                .iter()
                .map(|span| span.start.line)
                .collect()
        };
        assert_eq!(lines("Link syntax!"), vec![1]);
        assert_eq!(lines("link-syntax"), vec![1]);
        assert_eq!(lines("notes"), vec![3]);
        assert_eq!(lines("notes-1"), vec![5]);
        assert!(lines("Link-syntax").is_empty());
    }

    #[test]
    fn arguments_spread_over_rust_comment_lines() {
        let source = "//! [Decision](lynk:markdown.section\n//! (docs/a.md))\nfn main() {}\n";
        let (query, _) = read_first_link("test.rs", source).unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Section {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Decision".to_owned(),
        });
    }
}
