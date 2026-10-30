//! A range of a file as lynk prints it: `path:line:column`, followed by
//! `-line:column` when it ends elsewhere.

pub mod position;

use std::path::PathBuf;

use position::Position;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Relative to the repository root.
    pub path: PathBuf,
    pub start: Position,
    /// The last character, equal to the start for a single position.
    pub end: Position,
}

impl Span {
    pub fn parse(text: &str) -> Option<Self> {
        // Paths may hold dashes too, so the end is only taken when both
        // sides of the last dash read as positions.
        if let Some((head, end)) = text.rsplit_once('-')
            && let Some(end) = Position::parse(end)
            && let Some((path, start)) = parse_start(head)
        {
            return Some(Self { path, start, end });
        }
        let (path, start) = parse_start(text)?;
        Some(Self { path, start, end: start })
    }
}

/// The path and start of a span, all of it but what follows
/// [the last dash](lynk:rust.comment) when it ends elsewhere.
fn parse_start(text: &str) -> Option<(PathBuf, Position)> {
    let (rest, column) = text.rsplit_once(':')?;
    let (path, line) = rest.rsplit_once(':')?;
    let position = Position::parse(&format!("{line}:{column}"))?;
    Some((PathBuf::from(path), position))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(line: u32, column: u32) -> Position {
        Position { line, column }
    }

    #[test]
    fn reads_single_positions_and_ranges() {
        let single = Span::parse("src/location.rs:1:1").unwrap();
        assert_eq!(single.path, PathBuf::from("src/location.rs"));
        assert_eq!((single.start, single.end), (position(1, 1), position(1, 1)));

        let range = Span::parse("design/reads-rust-with-syn.md:3:1-31:2").unwrap();
        assert_eq!(range.path, PathBuf::from("design/reads-rust-with-syn.md"));
        assert_eq!((range.start, range.end), (position(3, 1), position(31, 2)));

        let dashed = Span::parse("design/reads-rust-with-syn.md:3:1").unwrap();
        assert_eq!(dashed.path, PathBuf::from("design/reads-rust-with-syn.md"));

        assert_eq!(Span::parse("3 error(s)"), None);
    }
}
