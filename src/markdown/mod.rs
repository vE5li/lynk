//! Markdown: a format whose files are prose whole, and a provider of their
//! sections and lines.

mod links;
pub mod parse;

use std::ops::Range;
use std::path::{Path, PathBuf};

pub use links::find as find_links;

use crate::document::{Document, Unit};
use crate::format::Format;
use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::provider::{Place, Provider, Query, normalize};
use crate::workspace::{LinkedDocument, Workspace};

pub const NAME: &str = "markdown";

pub struct Markdown;

impl Format for Markdown {
    fn name(&self) -> &'static str {
        NAME
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md"]
    }

    fn read(&self, path: PathBuf, text: String) -> LinkedDocument {
        let parsed = parse::parse(&text);
        let mut document = Document {
            path,
            format: NAME,
            prose: text.clone(),
            text,
            code_ranges: parsed.code_ranges,
            headings: parsed.headings,
            units: Vec::new(),
        };
        document.units = units(&document);
        LinkedDocument::new(document, |document| find_links(document, &parsed.links))
    }
}

/// Every heading with its own text, plus the text before the first heading.
/// Headings without text of their own are left out: a title would only
/// repeat its file name.
fn units(document: &Document) -> Vec<Unit> {
    let text = &document.text;
    let headings = &document.headings;
    let mut units = Vec::new();
    let first_heading_start = headings.first().map_or(text.len(), |heading| heading.range.start);
    if !text[..first_heading_start].trim().is_empty() {
        units.push(Unit {
            title: String::new(),
            range: 0..first_heading_start,
        });
    }
    for (index, heading) in headings.iter().enumerate() {
        let end = headings.get(index + 1).map_or(text.len(), |next| next.range.start);
        if text[heading.range.end.min(end)..end].trim().is_empty() {
            continue;
        }
        units.push(Unit {
            title: heading.text.trim().to_owned(),
            range: heading.range.start..end,
        });
    }
    units
}

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
            _ => return Err(format!("`{NAME}.{selector}` takes an optional file and a name")),
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
        for (heading_index, candidate) in document.headings.iter().enumerate() {
            if candidate.text.trim() == heading {
                spans.push(document.span(section_range(document, heading_index)));
            }
        }
    }
    spans
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
    use crate::provider::tests::{assert_query, read_first_link};

    #[test]
    fn the_text_of_a_link_is_its_last_argument() {
        let (query, members) = read_first_link("test.md", "[Two\nwords](nexus:markdown.section\n  (docs/a.md))").unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Section {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Two words".to_owned(),
        });
        assert!(members.is_empty());
    }

    #[test]
    fn a_file_left_out_is_the_one_the_link_is_written_in() {
        let (query, _) = read_first_link("docs/b.md", "[A line](nexus:markdown.line)").unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Line {
            file: Some(PathBuf::from("docs/b.md")),
            text: "A line".to_owned(),
        });
    }

    #[test]
    fn arguments_spread_over_rust_comment_lines() {
        let source = "//! [Decision](nexus:markdown.section\n//! (docs/a.md))\nfn main() {}\n";
        let (query, _) = read_first_link("test.rs", source).unwrap();
        assert_query(query.as_ref(), &MarkdownQuery::Section {
            file: Some(PathBuf::from("docs/a.md")),
            heading: "Decision".to_owned(),
        });
    }
}
