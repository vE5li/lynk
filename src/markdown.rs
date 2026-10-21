use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::document::Heading;

/// The code ranges (inline code and code blocks) and headings of a
/// Markdown text.
pub fn parse(text: &str) -> (Vec<Range<usize>>, Vec<Heading>) {
    let mut code_ranges = Vec::new();
    let mut headings = Vec::new();
    let mut open_heading: Option<Heading> = None;
    let mut code_block_start = None;
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                open_heading = Some(Heading {
                    level: level as usize,
                    text: String::new(),
                    range,
                });
            }
            Event::End(TagEnd::Heading(_)) => headings.extend(open_heading.take()),
            Event::Start(Tag::CodeBlock(_)) => code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    code_ranges.push(start..range.end);
                }
            }
            Event::Code(code) => {
                if let Some(heading) = &mut open_heading {
                    heading.text.push_str(&code);
                }
                code_ranges.push(range);
            }
            Event::Text(content) => {
                if let Some(heading) = &mut open_heading {
                    heading.text.push_str(&content);
                }
            }
            _ => {}
        }
    }
    (code_ranges, headings)
}
