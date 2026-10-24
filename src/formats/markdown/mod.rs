//! Markdown files, prose whole, and the Markdown way of writing links,
//! which every format whose prose is Markdown reads with it.

pub mod comments;
mod links;
pub mod parse;

use std::path::PathBuf;

pub use links::find as find_links;

use super::Format;
use crate::document::{Document, Unit};
use crate::workspace::LinkedDocument;

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
