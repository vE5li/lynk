use std::ops::Range;
use std::path::PathBuf;

use crate::location::{LineIndex, Span};

/// A part of a document's prose searched as one, such as a Markdown
/// heading with its text or a run of comment lines.
pub struct Unit {
    /// Empty when nothing better than the file's name titles it.
    pub title: String,
    pub range: Range<usize>,
}

pub struct Heading {
    pub level: usize,
    pub text: String,
    pub range: Range<usize>,
}

/// A file of the repository that may contain links, read by its format as
/// far as links need it.
pub struct Document {
    pub path: PathBuf,
    /// The name of its format.
    pub format: &'static str,
    pub text: String,
    /// The text links are read from: `text` with everything that is not
    /// prose blanked out, byte for byte, so offsets into it are offsets
    /// into `text`.
    pub prose: String,
    /// Inline code and code blocks: links inside them are examples.
    pub code_ranges: Vec<Range<usize>>,
    /// Empty in formats without headings.
    pub headings: Vec<Heading>,
    /// What [`nexus find`](nexus:nexus.find) searches.
    pub units: Vec<Unit>,
}

impl Document {
    pub fn is_code(&self, offset: usize) -> bool {
        self.code_ranges.iter().any(|range| range.contains(&offset))
    }

    pub fn span(&self, range: Range<usize>) -> Span {
        LineIndex::new(&self.text).span(&self.path, range)
    }
}
