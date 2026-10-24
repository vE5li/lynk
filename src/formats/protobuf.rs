//! Protobuf files, read through their comments, as Markdown; their design
//! record's [Decision](nexus:markdown.section
//! (design/reads-protobuf-with-protox-parse.md)) says how and why.

use std::path::PathBuf;

use super::Format;
use super::markdown::comments::{self, CommentLine, block_comment_lines};
use crate::workspace::LinkedDocument;

pub const NAME: &str = "protobuf";

pub struct Protobuf;

impl Format for Protobuf {
    fn name(&self) -> &'static str {
        NAME
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["proto"]
    }

    fn read(&self, path: PathBuf, text: String) -> LinkedDocument {
        let lines = comment_lines(&text);
        comments::read(NAME, path, text, &lines)
    }
}

/// The lines of every comment, found as protobuf's tokenizer finds them:
/// `//` up to the end of the line, `/*` up to the first `*/`, without
/// nesting, and neither inside a string, quoted with `"` or `'`, which
/// escapes with `\` and ends at the end of its line at the latest.
fn comment_lines(source: &str) -> Vec<CommentLine> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut line_number = 0;
    let mut offset = 0;
    while offset < bytes.len() {
        // Bytes, not characters: every byte a comment or a string starts or
        // ends at is ASCII, so ranges between them are whole characters.
        let rest = &bytes[offset..];
        if rest.starts_with(b"//") {
            let end = rest
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(source.len(), |found| offset + found);
            lines.push(CommentLine {
                line_number,
                range: offset + 2..end,
            });
            offset = end;
        } else if rest.starts_with(b"/*") {
            let closing = rest[2..].windows(2).position(|pair| pair == b"*/");
            let end = closing.map_or(source.len(), |found| offset + 2 + found + 2);
            let comment = &source[offset..end];
            // `/*` plus the `*` of a `/**` comment.
            let opening_length = 2 + usize::from(comment[2..].starts_with('*') && !comment[2..].starts_with("*/"));
            let closing_length = if closing.is_some() { 2 } else { 0 };
            lines.extend(block_comment_lines(
                comment,
                offset,
                line_number,
                opening_length,
                closing_length,
            ));
            line_number += comment.matches('\n').count();
            offset = end;
        } else if let quote @ (b'"' | b'\'') = bytes[offset] {
            offset += 1;
            while offset < bytes.len() && bytes[offset] != quote && bytes[offset] != b'\n' {
                offset += if bytes[offset] == b'\\' && bytes.get(offset + 1).is_some_and(|byte| *byte != b'\n') {
                    2
                } else {
                    1
                };
            }
            if bytes.get(offset) == Some(&quote) {
                offset += 1;
            }
        } else {
            if bytes[offset] == b'\n' {
                line_number += 1;
            }
            offset += 1;
        }
    }
    lines
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

    fn line_numbers(source: &str) -> Vec<usize> {
        comment_lines(source).into_iter().map(|line| line.line_number).collect()
    }

    #[test]
    fn finds_line_and_block_comments() {
        let source = "// first\nmessage A { // second\n  /* third */ string b = 1;\n}\n";
        assert_eq!(comments(source), vec!["first", "second", "third"]);
        assert_eq!(line_numbers(source), vec![0, 1, 2]);
    }

    #[test]
    fn strips_the_markers_of_block_comments() {
        let source = "/**\n * first\n * second\n */\nmessage A {}\n// after\n";
        assert_eq!(comments(source), vec!["first", "second", "after"]);
        assert_eq!(line_numbers(source).last(), Some(&5));
    }

    #[test]
    fn does_not_nest_block_comments() {
        let source = "/* outer /* inner */ string a = 1; // after\n";
        assert_eq!(comments(source), vec!["outer /* inner", "after"]);
    }

    #[test]
    fn skips_strings_quoted_either_way() {
        let source = "option a = \"// no \\\" /* no\";\noption b = 'it''s // no';\n// yes\n";
        assert_eq!(comments(source), vec!["yes"]);
    }

    #[test]
    fn reads_comments_among_characters_of_any_width() {
        let source = "option a = \"é\"; // ü /* ß */\nmessage Ä {}\n";
        assert_eq!(comments(source), vec!["ü /* ß */"]);
    }

    #[test]
    fn ends_an_unterminated_string_at_the_end_of_its_line() {
        let source = "option a = \"open\n// yes\n";
        assert_eq!(comments(source), vec!["yes"]);
        assert_eq!(line_numbers(source), vec![1]);
    }

    #[test]
    fn reads_an_unterminated_block_comment_to_the_end() {
        let source = "/* open\nstill open";
        assert_eq!(comments(source), vec!["open", "still open"]);
    }
}
