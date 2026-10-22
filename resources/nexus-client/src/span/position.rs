//! A place in a file, counted as [`nexus::location`](nexus:rust.module) counts it.

/// Line and column, both from 1; columns count characters, not bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    /// Reads `line:column`.
    pub fn parse(text: &str) -> Option<Self> {
        let (line, column) = text.split_once(':')?;
        Some(Self {
            line: line.parse().ok()?,
            column: column.parse().ok()?,
        })
    }
}
