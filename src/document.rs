use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::location::{LineIndex, Span};
use crate::{markdown, rust_comment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    /// Only the comments are read.
    Rust,
    /// Only the comments are read; they are written as in Rust.
    Wit,
}

impl Format {
    pub fn of(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "md" => Some(Self::Markdown),
            "rs" => Some(Self::Rust),
            "wit" => Some(Self::Wit),
            _ => None,
        }
    }
}

/// A part of a document's prose searched as one: a Markdown heading with
/// its text, a run of comment lines.
pub struct Unit {
    /// Empty for the text before a Markdown file's first heading.
    pub title: String,
    pub range: Range<usize>,
}

pub struct Heading {
    pub level: usize,
    pub text: String,
    pub range: Range<usize>,
}

/// A file of the repository that may contain links, parsed as far as links
/// need it.
pub struct Document {
    pub path: PathBuf,
    pub format: Format,
    pub text: String,
    /// The text links are read from: `text` with everything that is not
    /// prose blanked out, byte for byte, so offsets into it are offsets
    /// into `text`.
    pub prose: String,
    /// Inline code and code blocks: links inside them are examples.
    pub code_ranges: Vec<Range<usize>>,
    pub headings: Vec<Heading>,
    /// The runs of comment lines of source code.
    pub comment_blocks: Vec<Range<usize>>,
}

/// Titles taken from source code are cut, since a line of code may be long.
const TITLE_CHARACTERS: usize = 80;

impl Document {
    pub fn parse(path: PathBuf, format: Format, text: String) -> Self {
        let (prose, code_ranges, headings, comment_blocks) = match format {
            Format::Markdown => {
                let (code_ranges, headings) = markdown::parse(&text);
                (text.clone(), code_ranges, headings, Vec::new())
            }
            Format::Rust | Format::Wit => {
                let comments = rust_comment::comment_text(&text);
                (comments.text, comments.code_ranges, Vec::new(), comments.blocks)
            }
        };
        Self {
            path,
            format,
            text,
            prose,
            code_ranges,
            headings,
            comment_blocks,
        }
    }

    pub fn units(&self) -> Vec<Unit> {
        match self.format {
            Format::Markdown => self.markdown_units(),
            Format::Rust | Format::Wit => self.comment_blocks.iter().map(|block| self.comment_unit(block.clone())).collect(),
        }
    }

    /// Every heading with its own text, plus the text before the first
    /// heading. Headings without text of their own are left out: a title
    /// would only repeat its file name.
    fn markdown_units(&self) -> Vec<Unit> {
        let mut units = Vec::new();
        let first_heading_start = self.headings.first().map_or(self.text.len(), |heading| heading.range.start);
        if !self.text[..first_heading_start].trim().is_empty() {
            units.push(Unit {
                title: String::new(),
                range: 0..first_heading_start,
            });
        }
        for (index, heading) in self.headings.iter().enumerate() {
            let end = self.headings.get(index + 1).map_or(self.text.len(), |next| next.range.start);
            if self.text[heading.range.end.min(end)..end].trim().is_empty() {
                continue;
            }
            units.push(Unit {
                title: heading.text.trim().to_owned(),
                range: heading.range.start..end,
            });
        }
        units
    }

    /// Titled by the line of code the comment is about: the line after it,
    /// or its own line for a comment behind code.
    fn comment_unit(&self, block: Range<usize>) -> Unit {
        let line_start = self.text[..block.start].rfind('\n').map_or(0, |found| found + 1);
        let code_before = self.text[line_start..block.start].trim_end_matches(['/', '*', '!']).trim();
        let next_line = self.text[block.end..].lines().nth(1).unwrap_or_default().trim();
        let title = match code_before {
            code if !code.is_empty() => code,
            _ if !next_line.is_empty() => next_line,
            _ => self.prose[block.clone()].lines().next().unwrap_or_default().trim(),
        };
        Unit {
            title: title.chars().take(TITLE_CHARACTERS).collect(),
            range: block,
        }
    }

    pub fn is_code(&self, offset: usize) -> bool {
        self.code_ranges.iter().any(|range| range.contains(&offset))
    }

    /// From the heading to the next heading of the same or a higher level.
    pub fn section_range(&self, heading_index: usize) -> Range<usize> {
        let heading = &self.headings[heading_index];
        let end = self.headings[heading_index + 1..]
            .iter()
            .find(|later| later.level <= heading.level)
            .map_or(self.text.len(), |later| later.range.start);
        let trimmed_length = self.text[heading.range.start..end].trim_end().len();
        heading.range.start..heading.range.start + trimmed_length
    }

    pub fn span(&self, range: Range<usize>) -> Span {
        LineIndex::new(&self.text).span(&self.path, range)
    }
}
