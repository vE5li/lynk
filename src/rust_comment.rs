//! Reads the comments of Rust source, the only part of it that holds
//! links, and of the languages writing comments the same way;
//! <[markdown.section adrs/reads-rust-comments-with-rustc-lexer.md "Decision"]>
//! says how and why.

use std::ops::Range;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

use crate::markdown;

pub struct CommentText {
    /// The source with everything but the comments' text blanked out, byte
    /// for byte; line breaks are kept.
    pub text: String,
    /// Inline code and code blocks inside the comments.
    pub code_ranges: Vec<Range<usize>>,
    /// Each run of comment lines on consecutive lines, from the start of
    /// its first line's text to the end of its last.
    pub blocks: Vec<Range<usize>>,
}

pub fn comment_text(source: &str) -> CommentText {
    let lines = comment_lines(source);
    let mut text: Vec<u8> = source.bytes().map(|byte| if byte == b'\n' { b'\n' } else { b' ' }).collect();
    for line in &lines {
        text[line.range.clone()].copy_from_slice(&source.as_bytes()[line.range.clone()]);
    }
    let blocks = blocks(&lines);
    let code_ranges = blocks.iter().flat_map(|block| code_ranges(source, block)).collect();
    CommentText {
        // Only whole characters were copied and only ASCII blanked out.
        text: String::from_utf8(text).expect("blanking keeps the text valid UTF-8"),
        code_ranges,
        blocks: blocks
            .iter()
            .map(|block| block[0].range.start..block[block.len() - 1].range.end)
            .collect(),
    }
}

/// The text of one line of a comment, without its comment markers.
struct CommentLine {
    line_number: usize,
    range: Range<usize>,
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
                lines.extend(block_comment_lines(token_text, token_range.start, line_number, terminated));
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

/// Each line of a block comment without `/*` (plus the `*` or `!` of a doc
/// comment), `*/` and a leading `*` lining the comment up.
fn block_comment_lines(comment: &str, start: usize, first_line_number: usize, terminated: bool) -> Vec<CommentLine> {
    let opening_length = 2 + usize::from(comment[2..].starts_with(['*', '!']) && !comment[2..].starts_with("*/"));
    let closing_length = if terminated { 2 } else { 0 };
    let inner_range = opening_length.min(comment.len() - closing_length)..comment.len() - closing_length;
    let mut lines = Vec::new();
    let mut line_start = inner_range.start;
    for (index, line) in comment[inner_range.clone()].split('\n').enumerate() {
        let mut content_start = line_start;
        if index > 0 {
            let indentation = line.len() - line.trim_start().len();
            let rest = &line[indentation..];
            if rest.starts_with('*') && !rest.starts_with("*/") {
                content_start += indentation + 1;
            }
        }
        lines.push(CommentLine {
            line_number: first_line_number + index,
            range: start + content_start..start + line_start + line.len(),
        });
        line_start += line.len() + 1;
    }
    lines
}

/// Runs of comment lines on consecutive source lines, each read as one
/// Markdown text.
fn blocks(lines: &[CommentLine]) -> Vec<&[CommentLine]> {
    let mut blocks = Vec::new();
    let mut block_start = 0;
    for index in 1..=lines.len() {
        let continues = index < lines.len() && lines[index].line_number == lines[index - 1].line_number + 1;
        if !continues {
            blocks.push(&lines[block_start..index]);
            block_start = index;
        }
    }
    blocks.retain(|block| !block.is_empty());
    blocks
}

/// The block's code ranges in the source: its lines, stripped of the
/// indentation they share, are parsed as Markdown, and the ranges found
/// are mapped back line by line.
fn code_ranges(source: &str, block: &[CommentLine]) -> Vec<Range<usize>> {
    let indentation = block
        .iter()
        .map(|line| &source[line.range.clone()])
        .filter(|text| !text.trim().is_empty())
        .map(|text| text.len() - text.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut markdown_text = String::new();
    // Where each line starts, in the Markdown text and in the source.
    let mut line_starts = Vec::new();
    for line in block {
        let text = &source[line.range.clone()];
        let skipped = indentation.min(text.len() - text.trim_start().len());
        line_starts.push((markdown_text.len(), line.range.start + skipped));
        markdown_text.push_str(&text[skipped..]);
        markdown_text.push('\n');
    }
    let to_source = |markdown_offset: usize| {
        let line_index = line_starts.partition_point(|(start, _)| *start <= markdown_offset) - 1;
        let (markdown_start, source_start) = line_starts[line_index];
        source_start + markdown_offset - markdown_start
    };
    let (ranges, _) = markdown::parse(&markdown_text);
    ranges
        .into_iter()
        .map(|range| to_source(range.start)..to_source(range.end.saturating_sub(1)) + 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comments(source: &str) -> String {
        comment_text(source).text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn code(source: &str) -> Vec<&str> {
        comment_text(source)
            .code_ranges
            .into_iter()
            .map(|range| source[range].trim())
            .collect()
    }

    #[test]
    fn keeps_comments_and_blanks_out_code() {
        let source = "//! inner\n/// outer\nfn main() { // plain\n    let text = \"// no\"; /* block */\n}\n";
        assert_eq!(comments(source), "inner outer plain block");
        assert_eq!(comment_text(source).text.len(), source.len());
    }

    #[test]
    fn strips_the_markers_of_block_comments() {
        let source = "/**\n * first\n * second\n */\nfn main() {}\n";
        assert_eq!(comments(source), "first second");
    }

    #[test]
    fn finds_inline_code_and_code_blocks() {
        let source = "/// See `<[a.b]>`.\n///\n/// ```\n/// <[c.d]>\n/// ```\nfn main() {}\n";
        assert_eq!(code(source), vec!["`<[a.b]>`", "```\n/// <[c.d]>\n/// ```"]);
    }

    #[test]
    fn groups_comments_on_consecutive_lines() {
        let source = "// first\n// second\nfn main() {}\n\n// third\n";
        let blocks: Vec<_> = comment_text(source).blocks.into_iter().map(|range| &source[range]).collect();
        assert_eq!(blocks, vec![" first\n// second", " third"]);
    }

    #[test]
    fn does_not_mistake_indentation_for_a_code_block() {
        let source = "fn main() {\n        // <[a.b]>\n}\n";
        assert!(code(source).is_empty());
    }
}
