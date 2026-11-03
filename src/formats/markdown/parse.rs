//! The parts of a Markdown text links need, as `pulldown-cmark` parses
//! them.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::document::Heading;

/// What links need of a Markdown text.
pub struct Parsed {
    /// Inline code and code blocks.
    pub code_ranges: Vec<Range<usize>>,
    pub headings: Vec<Heading>,
    pub links: Vec<MarkdownLink>,
    /// The definitions of reference links, used or not.
    pub definitions: Vec<Definition>,
}

/// A reference link's definition, `[<label>]: <destination>`.
pub struct Definition {
    pub range: Range<usize>,
    pub destination: String,
}

/// A Markdown link, inline or by reference, with its destination and title
/// resolved.
pub struct MarkdownLink {
    /// The link where it is used, from its `[` to its end.
    pub range: Range<usize>,
    pub destination: String,
    /// `None` without one.
    pub title: Option<String>,
    /// What the reader sees, code spans unwrapped and whitespace collapsed.
    pub text: String,
    /// Where the text is written, as it is written, inside the brackets.
    pub text_range: Range<usize>,
}

pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed {
        code_ranges: Vec::new(),
        headings: Vec::new(),
        links: Vec::new(),
        definitions: Vec::new(),
    };
    let mut open_heading: Option<Heading> = None;
    let mut open_link: Option<MarkdownLink> = None;
    // The events inside the open link, which make up its text.
    let mut link_text_range: Option<Range<usize>> = None;
    let mut code_block_start = None;
    let mut events = Parser::new(text).into_offset_iter();
    for (event, range) in events.by_ref() {
        let inside_link = open_link.is_some() && !matches!(event, Event::End(TagEnd::Link));
        if inside_link {
            link_text_range = Some(match link_text_range.take() {
                Some(text_range) => text_range.start.min(range.start)..text_range.end.max(range.end),
                None => range.clone(),
            });
        }
        let content = match &event {
            Event::Text(content) | Event::Code(content) => Some(content.as_ref()),
            Event::SoftBreak | Event::HardBreak => Some(" "),
            _ => None,
        };
        if let Some(content) = content {
            if let Some(heading) = &mut open_heading {
                heading.text.push_str(content);
            }
            if let Some(link) = &mut open_link {
                link.text.push_str(content);
            }
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                open_heading = Some(Heading {
                    level: level as usize,
                    text: String::new(),
                    range,
                });
            }
            Event::End(TagEnd::Heading(_)) => parsed.headings.extend(open_heading.take()),
            Event::Start(Tag::Link { dest_url, title, .. }) => {
                open_link = Some(MarkdownLink {
                    text_range: range.start + 1..range.start + 1,
                    range,
                    destination: dest_url.into_string(),
                    title: Some(title.into_string()).filter(|title| !title.is_empty()),
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Link) => {
                if let Some(mut link) = open_link.take() {
                    link.text = link.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    link.text_range = link_text_range.take().unwrap_or(link.text_range);
                    parsed.links.push(link);
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    parsed.code_ranges.push(start..range.end);
                }
            }
            Event::Code(_) => parsed.code_ranges.push(range),
            _ => {}
        }
    }
    parsed.definitions = events
        .reference_definitions()
        .iter()
        .map(|(_, definition)| Definition {
            range: definition.span.clone(),
            destination: definition.dest.to_string(),
        })
        .collect();
    parsed
}
