//! A place in a file, counted as [`lynk::location`](lynk:rust.module) counts it.

/// Line and column, both from 1; columns count characters, not bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    /// What the first line and the first column count as.
    pub const FIRST: u32 = 1;

    /// Reads `line:column`.
    pub fn parse(text: &str) -> Option<Self> {
        let (line, column) = text.split_once(':')?;
        Some(Self {
            line: line.parse().ok()?,
            column: column.parse().ok()?,
        })
    }
}
