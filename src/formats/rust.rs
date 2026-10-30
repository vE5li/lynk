//! Rust source, read through its comments, the only part of it that holds
//! links, as are the languages writing comments the same way; its design
//! record's [Decision](lynk:markdown.section
//! (design/reads-rust-comments-with-rustc-lexer.md)) says how and why.

use std::path::PathBuf;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

use super::Format;
use super::markdown::comments::{self, CommentLine, block_comment_lines};
use crate::workspace::LinkedDocument;

pub const NAME: &str = "rust";

pub struct Rust;

impl Format for Rust {
    fn name(&self) -> &'static str {
        NAME
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn read(&self, path: PathBuf, text: String) -> LinkedDocument {
        read(NAME, path, text)
    }
}

/// Reads a file of a [`format`](lynk:rust.argument) writing comments as
/// Rust does, its comments read as Markdown.
pub fn read(format: &'static str, path: PathBuf, text: String) -> LinkedDocument {
    let lines = comment_lines(&text);
    comments::read(format, path, text, &lines)
}

fn comment_lines(source: &str) -> Vec<CommentLine> {
    let mut lines = Vec::new();
    let mut offset = 0;
    let mut line_number = 0;
    for token in tokenize(source, FrontmatterAllowed::No) {
        let token_range = offset..offset + token.len as usize;
        offset = token_range.end;
        let token_text = &source[token_range.clone()];
        let line_count = token_text.matches('\n').count();
        match token.kind {
            TokenKind::LineComment { .. } => lines.push(CommentLine {
                line_number,
                range: token_range.start + line_comment_marker_length(token_text)..token_range.end,
            }),
            TokenKind::BlockComment { terminated, .. } => {
                // `/*` plus the `*` or `!` of a doc comment.
                let opening_length = 2 + usize::from(token_text[2..].starts_with(['*', '!']) && !token_text[2..].starts_with("*/"));
                let closing_length = if terminated { 2 } else { 0 };
                lines.extend(block_comment_lines(
                    token_text,
                    token_range.start,
                    line_number,
                    opening_length,
                    closing_length,
                ));
            }
            _ => {}
        }
        line_number += line_count;
    }
    lines
}

/// `//` plus the `/` or `!` of a doc comment.
fn line_comment_marker_length(comment: &str) -> usize {
    let after_slashes = &comment[2..];
    2 + usize::from(after_slashes.starts_with('/') || after_slashes.starts_with('!'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comments(source: &str) -> Vec<&str> {
        comment_lines(source)
            .into_iter()
            .map(|line| source[line.range].trim())
            .filter(|text| !text.is_empty())
            .collect()
    }

    #[test]
    fn finds_comments_but_not_code() {
        let source = "//! inner\n/// outer\nfn main() { // plain\n    let text = \"// no\"; /* block */\n}\n";
        assert_eq!(comments(source), vec!["inner", "outer", "plain", "block"]);
    }

    #[test]
    fn strips_the_markers_of_doc_block_comments() {
        let source = "/**\n * first\n * second\n */\nfn main() {}\n";
        assert_eq!(comments(source), vec!["first", "second"]);
    }

    #[test]
    fn nests_block_comments() {
        let source = "/* outer /* inner */ still outer */\nfn main() {}\n";
        assert_eq!(comments(source), vec!["outer /* inner */ still outer"]);
    }
}
