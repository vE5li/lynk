//! The problems [`nexus lint`](nexus:nexus.lint) prints, one `span: message` line each.

use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub at: Span,
    pub message: String,
}

impl Problem {
    /// Reads one line of the lint's output. Its other lines, like the
    /// places an ambiguous link points at and the final count, give `None`.
    pub fn parse(line: &str) -> Option<Self> {
        if line.starts_with(char::is_whitespace) {
            return None;
        }
        let (at, message) = line.split_once(": ")?;
        Some(Self {
            at: Span::parse(at)?,
            message: message.to_owned(),
        })
    }
}
