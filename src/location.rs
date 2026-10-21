use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Line and column of a byte offset, both counted from 1; columns count
/// characters, not bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Maps byte offsets of one text to positions.
pub struct LineIndex<'text> {
    text: &'text str,
    line_starts: Vec<usize>,
}

impl<'text> LineIndex<'text> {
    pub fn new(text: &'text str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        Self { text, line_starts }
    }

    pub fn position(&self, offset: usize) -> Position {
        let line_index = self.line_starts.partition_point(|line_start| *line_start <= offset) - 1;
        let line_start = self.line_starts[line_index];
        Position {
            line: line_index + 1,
            column: self.text[line_start..offset].chars().count() + 1,
        }
    }

    /// The span of a byte range of the text, which is the file at
    /// <[rust.argument path]>.
    pub fn span(&self, path: &Path, range: Range<usize>) -> Span {
        // The end position names the last character, not the one after it.
        let last = self.text[range.clone()]
            .char_indices()
            .next_back()
            .map_or(range.start, |(offset, _)| range.start + offset);
        Span {
            path: path.to_owned(),
            start: self.position(range.start),
            end: self.position(last),
        }
    }

    /// The byte range of every line, without its line break.
    pub fn lines(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.line_starts.iter().enumerate().map(|(line_index, line_start)| {
            let end = self
                .line_starts
                .get(line_index + 1)
                .map_or(self.text.len(), |next_line_start| next_line_start - 1);
            *line_start..end
        })
    }
}

/// A range of a file, with paths relative to the repository root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub path: PathBuf,
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn whole_file(path: &Path) -> Self {
        let start = Position { line: 1, column: 1 };
        Self {
            path: path.to_owned(),
            start,
            end: start,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.path.display(), self.start.line, self.start.column)?;
        if self.end != self.start {
            write!(formatter, "-{}:{}", self.end.line, self.end.column)?;
        }
        Ok(())
    }
}
