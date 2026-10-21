use std::ops::Range;

use crate::document::Document;

const OPENING: &str = "<[";
const CLOSING: &str = "]>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    pub text: String,
    pub quoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub description: String,
    pub range: Range<usize>,
}

/// A link as written, before its selector reads what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub selector_name: String,
    /// From the end of the selector name to the closing delimiter.
    pub content: Range<usize>,
    /// The links nested in the content, at any depth.
    pub nested: Vec<Range<usize>>,
    /// From the opening to the closing delimiter, both included.
    pub range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub offset: usize,
}

/// Every link of the document's prose outside code, those nested in another
/// right after the link holding them, plus what failed to parse.
pub fn find_links(document: &Document) -> (Vec<Link>, Vec<SyntaxError>) {
    let mut links = Vec::new();
    let mut errors = Vec::new();
    let mut search_from = 0;
    while let Some(found) = document.prose[search_from..].find(OPENING) {
        let start = search_from + found;
        search_from = start + OPENING.len();
        if document.is_code(start) {
            continue;
        }
        match parse_link(document, start, &mut links) {
            Ok(end) => search_from = end,
            Err(error) => errors.push(error),
        }
    }
    (links, errors)
}

/// Parses the link starting at <[rust.argument start]> into
/// <[rust.argument links]>, followed by the links nested in it, and returns
/// where it ends. On failure <[rust.argument links]> is left as it was.
///
/// Only the selector name and the link's extent are read here: how the rest
/// splits into arguments and a body is up to the selector, so line breaks
/// mean nothing to the link.
fn parse_link(document: &Document, start: usize, links: &mut Vec<Link>) -> Result<usize, SyntaxError> {
    let text = &document.prose;
    let name_start = start + OPENING.len();
    let name_end = text[name_start..]
        .char_indices()
        .find(|&(offset, character)| character.is_whitespace() || text[name_start + offset..].starts_with(CLOSING))
        .map_or(text.len(), |(offset, _)| name_start + offset);
    let selector_name = &text[name_start..name_end];
    if !selector_name.contains('.') || selector_name.contains('"') {
        return Err(SyntaxError {
            message: "a link starts with `<provider>.<selector>`".to_owned(),
            offset: start,
        });
    }
    let mut nested = Vec::new();
    let content_end = find_closing(document, name_end, &mut nested).ok_or_else(|| SyntaxError {
        message: format!("link is never closed with `{CLOSING}`"),
        offset: start,
    })??;
    links.push(Link {
        selector_name: selector_name.to_owned(),
        content: name_end..content_end,
        nested: nested.iter().map(|link: &Link| link.range.clone()).collect(),
        range: start..content_end + CLOSING.len(),
    });
    links.append(&mut nested);
    Ok(content_end + CLOSING.len())
}

/// The closing delimiter of a link whose content starts at
/// <[rust.argument from]>, parsing the links nested in it into
/// <[rust.argument nested]>. Delimiters inside code do not
/// count, nor do those of nested links; quotes mean nothing here.
fn find_closing(document: &Document, from: usize, nested: &mut Vec<Link>) -> Option<Result<usize, SyntaxError>> {
    let text = &document.prose;
    let mut offset = from;
    while offset < text.len() {
        let rest = &text[offset..];
        let in_code = document.is_code(offset);
        if !in_code && rest.starts_with(CLOSING) {
            return Some(Ok(offset));
        }
        if !in_code && rest.starts_with(OPENING) {
            match parse_link(document, offset, nested) {
                Ok(end) => {
                    offset = end;
                    continue;
                }
                Err(error) => return Some(Err(error)),
            }
        }
        offset += rest.chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// What a link holds after its selector name, for its selector to read:
/// arguments first, then, for selectors taking one, a body.
#[derive(Debug, Clone)]
pub struct Content<'a> {
    prose: &'a str,
    range: Range<usize>,
    nested: &'a [Range<usize>],
}

impl Link {
    /// <[rust.argument prose]> is that of the document the link was found
    /// in.
    pub fn content<'a>(&'a self, prose: &'a str) -> Content<'a> {
        Content {
            prose,
            range: self.content.clone(),
            nested: &self.nested,
        }
    }
}

impl<'a> Content<'a> {
    /// The first <[rust.argument count]> arguments, every one when `None`,
    /// and the content
    /// left after them. Arguments are separated by whitespace, line breaks
    /// included, and may be quoted with `"`.
    pub fn arguments(&self, count: Option<usize>) -> Result<(Vec<Argument>, Content<'a>), SyntaxError> {
        let text = &self.prose[self.range.clone()];
        let mut arguments = Vec::new();
        let mut characters = text.char_indices().peekable();
        while let Some(&(offset, character)) = characters.peek() {
            if count.is_some_and(|count| arguments.len() == count) {
                break;
            }
            if character.is_whitespace() {
                characters.next();
                continue;
            }
            if character != '"' {
                let mut text = String::new();
                while let Some(&(_, character)) = characters.peek() {
                    if character.is_whitespace() {
                        break;
                    }
                    text.push(character);
                    characters.next();
                }
                arguments.push(Argument { text, quoted: false });
                continue;
            }
            characters.next();
            let mut text = String::new();
            let mut closed = false;
            while let Some((_, character)) = characters.next() {
                match character {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => text.extend(characters.next().map(|(_, escaped)| escaped)),
                    _ => text.push(character),
                }
            }
            if !closed {
                return Err(SyntaxError {
                    message: "quoted argument is never closed".to_owned(),
                    offset: self.range.start + offset,
                });
            }
            arguments.push(Argument { text, quoted: true });
        }
        let rest_start = characters.peek().map_or(self.range.end, |&(offset, _)| self.range.start + offset);
        let rest = Content {
            range: rest_start..self.range.end,
            ..self.clone()
        };
        Ok((arguments, rest))
    }

    /// The content read as a body of members: each `- <member>:
    /// <description>` line no more indented than the first, with the lines
    /// indented further below it, and the links nested in them, as part of
    /// its description. A body is prose: quotes mean nothing in it.
    pub fn members(&self) -> Result<Vec<Member>, SyntaxError> {
        let text = self.prose;
        let body = self.range.clone();
        let mut members: Vec<Member> = Vec::new();
        let mut member_indentation = None;
        let mut line_start = body.start;
        while line_start < body.end {
            let line_end = text[line_start..body.end].find('\n').map_or(body.end, |found| line_start + found);
            let line = &text[line_start..line_end];
            let current_start = line_start;
            line_start = line_end + 1;
            let indentation = line.len() - line.trim_start().len();
            let offset = current_start + indentation;
            let inside_nested = self.nested.iter().any(|range| range.start < offset && offset < range.end);
            if line.trim().is_empty() {
                continue;
            }
            // The body may start on the line holding the arguments, after
            // them; only lines of its own set the members' indentation.
            let own_line = text[..current_start].ends_with('\n');
            let continues_member = inside_nested || member_indentation.is_some_and(|member_indentation| indentation > member_indentation);
            if let Some(member) = members.last_mut().filter(|_| continues_member) {
                member.range.end = offset + line.trim().len();
                continue;
            }
            let entry = line.trim().strip_prefix("- ").ok_or_else(|| SyntaxError {
                message: "a link's body is a list of `- <member>: <description>`".to_owned(),
                offset,
            })?;
            if own_line {
                member_indentation.get_or_insert(indentation);
            }
            let name = entry.split_once(':').map_or(entry, |(name, _)| name);
            members.push(Member {
                name: name.trim().to_owned(),
                description: String::new(),
                range: offset..offset + line.trim().len(),
            });
        }
        for member in &mut members {
            let entry = &text[member.range.start + 2..member.range.end];
            member.description = entry.split_once(':').map_or("", |(_, description)| description).trim().to_owned();
        }
        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::document::Format;

    fn links_in(text: &str) -> (Vec<Link>, Vec<SyntaxError>) {
        find_links(&Document::parse(PathBuf::from("test.md"), Format::Markdown, text.to_owned()))
    }

    fn links_in_rust(text: &str) -> (Vec<Link>, Vec<SyntaxError>) {
        find_links(&Document::parse(PathBuf::from("test.rs"), Format::Rust, text.to_owned()))
    }

    /// The members of the body of a link of Markdown <[rust.argument text]>
    /// after <[rust.argument arguments]> arguments.
    fn members(text: &str, link: &Link, arguments: usize) -> Vec<Member> {
        let (_, body) = link.content(text).arguments(Some(arguments)).unwrap();
        body.members().unwrap()
    }

    #[test]
    fn parses_bare_and_quoted_arguments() {
        let text = "See <[markdown.section plans/lsp.md \"Phase \\\"2\\\"\"]> here.";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].selector_name, "markdown.section");
        let (arguments, rest) = links[0].content(text).arguments(None).unwrap();
        assert_eq!(arguments, vec![
            Argument {
                text: "plans/lsp.md".to_owned(),
                quoted: false
            },
            Argument {
                text: "Phase \"2\"".to_owned(),
                quoted: true
            },
        ]);
        assert!(rest.arguments(None).unwrap().0.is_empty());
    }

    #[test]
    fn line_breaks_separate_arguments_like_any_whitespace() {
        let text = "<[markdown.section plans/lsp.md\n  \"Phase\n  2\"]>";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        let (arguments, _) = links[0].content(text).arguments(None).unwrap();
        let texts: Vec<_> = arguments.iter().map(|argument| argument.text.as_str()).collect();
        assert_eq!(texts, vec!["plans/lsp.md", "Phase\n  2"]);
    }

    #[test]
    fn parses_a_body_of_members() {
        let text = "Opcodes:\n<[rust.map-variant crate::Bar\n  - Load: LD\n  - Read: RD\n]>\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        let members: Vec<_> = members(text, &links[0], 1)
            .iter()
            .map(|member| (member.name.clone(), member.description.clone()))
            .collect();
        assert_eq!(members, vec![
            ("Load".to_owned(), "LD".to_owned()),
            ("Read".to_owned(), "RD".to_owned())
        ]);
    }

    #[test]
    fn parses_links_nested_in_a_body_as_links_of_their_own() {
        let text = "<[rust.*\n- module:\n  - Inline: <[rust.module a b]>.\n  - Mapped: <[filesystem.map-entry c\n    - d: e\n  ]>\n- \
                    unused: not used\n]>\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        let selectors: Vec<_> = links.iter().map(|link| link.selector_name.as_str()).collect();
        assert_eq!(selectors, vec!["rust.*", "rust.module", "filesystem.map-entry"]);
        let names: Vec<_> = members(text, &links[0], 0).into_iter().map(|member| member.name).collect();
        assert_eq!(names, vec!["module", "unused"]);
        assert_eq!(links[0].range, 0..text.len() - 1);
        assert_eq!(members(text, &links[2], 1)[0].name, "d");
    }

    #[test]
    fn a_member_spans_the_nested_link_it_holds() {
        let text = "<[rust.*\n- module: <[filesystem.map-entry c\n  - d: e\n]> held\n- other\n]>";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        let members = members(text, &links[0], 0);
        assert_eq!(
            &text[members[0].range.clone()],
            "- module: <[filesystem.map-entry c\n  - d: e\n]> held"
        );
        assert_eq!(members[1].name, "other");
    }

    #[test]
    fn a_body_is_prose_where_code_hides_delimiters_and_quotes_mean_nothing() {
        let text = "<[rust.*\n- module: `]>` and `<[`, a 12\" ruler\n]>";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(members(text, &links[0], 0)[0].description, "`]>` and `<[`, a 12\" ruler");
    }

    #[test]
    fn code_blocks_in_a_body_hide_links() {
        let text = "<[rust.*\n- module: like\n  ```\n  <[rust.module a b]>\n  ```\n]>";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(members(text, &links[0], 0).len(), 1);
    }

    #[test]
    fn a_body_line_neither_a_member_nor_below_one_is_an_error() {
        let text = "<[rust.*\n  - module: a\nloose\n]>";
        let (links, _) = links_in(text);
        assert!(links[0].content(text).members().is_err());
    }

    #[test]
    fn ignores_links_in_code() {
        let (links, errors) = links_in("`<[markdown.section \"A\"]>`\n\n```\n<[markdown.section \"B\"]>\n```\n");
        assert!(links.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn quotes_do_not_hide_a_closing_delimiter() {
        let text = "<[markdown.line \"a ]> b\"]>";
        let (links, _) = links_in(text);
        assert_eq!(&text[links[0].range.clone()], "<[markdown.line \"a ]>");
        assert!(links[0].content(text).arguments(None).is_err());
    }

    #[test]
    fn reads_links_in_rust_comments_only() {
        let source = "// <[markdown.section \"A\"]>\nfn main() {\n    let text = \"<[markdown.section \\\"B\\\"]>\";\n}\n";
        let (links, errors) = links_in_rust(source);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(&source[links[0].range.clone()], "<[markdown.section \"A\"]>");
    }

    #[test]
    fn parses_a_body_spread_over_rust_comment_lines() {
        let source = "/// <[rust.map-variant crate::Bar\n///   - Load: LD\n/// ]>\nenum Bar {}\n";
        let document = Document::parse(PathBuf::from("test.rs"), Format::Rust, source.to_owned());
        let (links, errors) = find_links(&document);
        assert!(errors.is_empty());
        let names: Vec<_> = members(&document.prose, &links[0], 1)
            .into_iter()
            .map(|member| member.name)
            .collect();
        assert_eq!(names, vec!["Load"]);
    }

    #[test]
    fn reads_links_in_wit_comments_only() {
        let source = "package a:b;\n\n/// Runs <[nexus.self]>.\n/* nested /* <[nexus.lint]> */ */\nworld w {\n    export run: func();\n}\n";
        let document = Document::parse(PathBuf::from("test.wit"), Format::Wit, source.to_owned());
        let (links, errors) = find_links(&document);
        assert!(errors.is_empty());
        let texts: Vec<_> = links.iter().map(|link| &source[link.range.clone()]).collect();
        assert_eq!(texts, vec!["<[nexus.self]>", "<[nexus.lint]>"]);
        assert_eq!(document.units()[0].title, "world w {");
    }

    #[test]
    fn reports_an_unclosed_link() {
        let (links, errors) = links_in("<[markdown.section \"A\"");
        assert!(links.is_empty());
        assert_eq!(errors.len(), 1);
    }
}
