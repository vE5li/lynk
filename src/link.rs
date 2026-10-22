use std::ops::Range;

use crate::document::Document;

/// What a Markdown link's destination starts with to be a link to check.
const SCHEME: &str = "nexus:";
const COMMENT_OPENING: &str = "<!--";
const COMMENT_CLOSING: &str = "-->";
/// The word opening a map's comment.
const MAP_KEYWORD: &str = "nexus";
const MAP_CLOSING: &str = "<!-- /nexus -->";

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
    pub content: Content,
    /// The links inside it, at any depth.
    pub nested: Vec<Range<usize>>,
    /// The whole link: from its `[` to its end, or from a map's opening
    /// comment to its closing one.
    pub range: Range<usize>,
}

/// What a link holds besides its selector name, for its selector to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// A Markdown link's title, if any, and its text.
    Inline { title: Option<String>, text: String },
    /// A map's arguments, in its opening comment, and its body, the lines
    /// between its comments.
    Map { arguments: Range<usize>, body: Range<usize> },
}

/// How a selector reads a link's content.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// Whether it is a map selector, taking a body.
    pub map: bool,
    /// Whether it takes any argument, its last one being a link's text.
    pub takes_arguments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub offset: usize,
}

/// What reading a document's links found.
#[derive(Debug, Default)]
pub struct Found {
    /// Ordered by where they start, a link holding others first.
    pub links: Vec<Link>,
    pub errors: Vec<SyntaxError>,
}

/// Every link of the document's prose outside code, plus what failed to
/// parse.
pub fn find_links(document: &Document) -> Found {
    let mut found = Found::default();
    find_markdown_links(document, &mut found);
    find_maps(document, &mut found);
    found
        .links
        .sort_by_key(|link| (link.range.start, std::cmp::Reverse(link.range.end)));
    let ranges: Vec<Range<usize>> = found.links.iter().map(|link| link.range.clone()).collect();
    for link in &mut found.links {
        link.nested = ranges
            .iter()
            .filter(|range| **range != link.range && link.range.start <= range.start && range.end <= link.range.end)
            .cloned()
            .collect();
    }
    found
}

/// The Markdown links whose destination is a selector after `nexus:`.
fn find_markdown_links(document: &Document, found: &mut Found) {
    for markdown_link in &document.markdown_links {
        let Some(selector_name) = markdown_link.destination.strip_prefix(SCHEME) else {
            continue;
        };
        if !selector_name.contains('.') {
            found.errors.push(SyntaxError {
                message: format!("a link's destination is `{SCHEME}<provider>.<selector>`"),
                offset: markdown_link.range.start,
            });
            continue;
        }
        found.links.push(Link {
            selector_name: selector_name.to_owned(),
            content: Content::Inline {
                title: markdown_link.title.clone(),
                text: markdown_link.text.clone(),
            },
            nested: Vec::new(),
            range: markdown_link.range.clone(),
        });
    }
}

/// The maps written between an opening `<!-- nexus <provider>.<selector>
/// <arguments> -->` and a closing `<!-- /nexus -->`, each starting a line of
/// prose outside code and ending it; nested maps close innermost first.
fn find_maps(document: &Document, found: &mut Found) {
    struct OpenMap {
        start: usize,
        /// `None` for one reported already, closed without a link.
        selector_name: Option<String>,
        arguments: Range<usize>,
        body_start: usize,
    }
    let text = &document.prose;
    let mut open: Vec<OpenMap> = Vec::new();
    let mut line_start = 0;
    while line_start < text.len() {
        let line_end = text[line_start..].find('\n').map_or(text.len(), |found| line_start + found);
        let line = &text[line_start..line_end];
        let start = line_start + line.len() - line.trim_start().len();
        let current_start = line_start;
        line_start = line_end + 1;
        if document.is_code(start) {
            continue;
        }
        let trimmed = line.trim();
        if trimmed == MAP_CLOSING {
            match open.pop() {
                Some(map) => found.links.extend(map.selector_name.map(|selector_name| Link {
                    selector_name,
                    content: Content::Map {
                        arguments: map.arguments,
                        body: map.body_start.min(current_start)..current_start,
                    },
                    nested: Vec::new(),
                    range: map.start..start + trimmed.len(),
                })),
                None => found.errors.push(SyntaxError {
                    message: format!("`{MAP_CLOSING}` closes no map"),
                    offset: start,
                }),
            }
            continue;
        }
        let Some(after_opening) = trimmed.strip_prefix(COMMENT_OPENING) else {
            continue;
        };
        let after_spaces = after_opening.trim_start();
        let opens_map = after_spaces
            .strip_prefix(MAP_KEYWORD)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace));
        if !opens_map {
            continue;
        }
        let keyword_end = start + trimmed.len() - after_spaces.len() + MAP_KEYWORD.len();
        let Some(comment_end) = text[keyword_end..].find(COMMENT_CLOSING).map(|found| keyword_end + found) else {
            found.errors.push(SyntaxError {
                message: format!("a map's opening comment is never closed with `{COMMENT_CLOSING}`"),
                offset: start,
            });
            return;
        };
        let after_comment = comment_end + COMMENT_CLOSING.len();
        let comment_line_end = text[after_comment..].find('\n').map_or(text.len(), |found| after_comment + found);
        line_start = comment_line_end + 1;
        // Still opened, so its closing comment does not fail too.
        if !text[after_comment..comment_line_end].trim().is_empty() {
            found.errors.push(SyntaxError {
                message: "a map's opening comment ends its line".to_owned(),
                offset: after_comment,
            });
        }
        let inside = &text[keyword_end..comment_end];
        let name_start = keyword_end + inside.len() - inside.trim_start().len();
        let name_end = text[name_start..comment_end]
            .find(char::is_whitespace)
            .map_or(comment_end, |found| name_start + found);
        let selector_name = &text[name_start..name_end];
        let valid = selector_name.contains('.');
        if !valid {
            found.errors.push(SyntaxError {
                message: format!("a map opens with `{COMMENT_OPENING} {MAP_KEYWORD} <provider>.<selector>`"),
                offset: start,
            });
        }
        open.push(OpenMap {
            start,
            selector_name: valid.then(|| selector_name.to_owned()),
            arguments: name_end..comment_end,
            body_start: line_start.min(text.len()),
        });
    }
    for map in open {
        found.errors.push(SyntaxError {
            message: format!("map is never closed with `{MAP_CLOSING}`"),
            offset: map.start,
        });
    }
}

impl Link {
    /// The link's arguments and, for a map, the members of its body, as a
    /// selector of [`shape`](nexus:rust.argument) reads them.
    /// [`prose`](nexus:rust.argument) is that of the document the link was
    /// found in.
    pub fn read(&self, prose: &str, shape: Shape) -> Result<(Vec<Argument>, Vec<Member>), SyntaxError> {
        let error = |message: String| SyntaxError {
            message,
            offset: self.range.start,
        };
        let name = &self.selector_name;
        match &self.content {
            Content::Inline { .. } if shape.map => Err(error(format!(
                "`{name}` is a map, written between `{COMMENT_OPENING} {MAP_KEYWORD} {name} ... {COMMENT_CLOSING}` and `{MAP_CLOSING}`"
            ))),
            // The title holds the arguments before the text.
            Content::Inline { title, text } => {
                let mut arguments = arguments(title.as_deref().unwrap_or_default(), self.range.start)?;
                if shape.takes_arguments && !text.is_empty() {
                    arguments.push(Argument {
                        text: text.clone(),
                        quoted: false,
                    });
                }
                Ok((arguments, Vec::new()))
            }
            Content::Map { .. } if !shape.map => Err(error(format!("`{name}` takes no body, so it is written as a link"))),
            Content::Map {
                arguments: arguments_range,
                body,
            } => {
                let arguments = arguments(&prose[arguments_range.clone()], arguments_range.start)?;
                Ok((arguments, members(prose, body.clone(), &self.nested)?))
            }
        }
    }
}

/// The arguments written in [`text`](nexus:rust.argument), which starts at
/// [`offset`](nexus:rust.argument) for errors. Arguments are separated by
/// whitespace, line breaks included, and may be quoted with `"`.
fn arguments(text: &str, offset: usize) -> Result<Vec<Argument>, SyntaxError> {
    let mut arguments = Vec::new();
    let mut characters = text.char_indices().peekable();
    while let Some(&(character_offset, character)) = characters.peek() {
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
                offset: offset + character_offset,
            });
        }
        arguments.push(Argument { text, quoted: true });
    }
    Ok(arguments)
}

/// The [`body`](nexus:rust.argument) of a map read as members: each
/// `- <member>: <description>` line no more indented than the first, with the
/// lines indented further below it, and the links nested in it, as part of
/// its description. A body is prose: quotes mean nothing in it.
fn members(text: &str, body: Range<usize>, nested: &[Range<usize>]) -> Result<Vec<Member>, SyntaxError> {
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
        let inside_nested = nested.iter().any(|range| range.start <= offset && offset < range.end);
        if line.trim().is_empty() {
            continue;
        }
        let continues_member = inside_nested || member_indentation.is_some_and(|member_indentation| indentation > member_indentation);
        if let Some(member) = members.last_mut().filter(|_| continues_member) {
            member.range.end = offset + line.trim().len();
            continue;
        }
        let entry = line.trim().strip_prefix("- ").ok_or_else(|| SyntaxError {
            message: "a map's body is a list of `- <member>: <description>`".to_owned(),
            offset,
        })?;
        member_indentation.get_or_insert(indentation);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::document::Format;

    const WITHOUT_BODY: Shape = Shape {
        map: false,
        takes_arguments: true,
    };
    const MAP: Shape = Shape {
        map: true,
        takes_arguments: true,
    };

    fn found_in(text: &str) -> Found {
        find_links(&Document::parse(PathBuf::from("test.md"), Format::Markdown, text.to_owned()))
    }

    fn links_in(text: &str) -> (Vec<Link>, Vec<SyntaxError>) {
        let found = found_in(text);
        (found.links, found.errors)
    }

    fn links_in_rust(text: &str) -> (Vec<Link>, Vec<SyntaxError>) {
        let found = find_links(&Document::parse(PathBuf::from("test.rs"), Format::Rust, text.to_owned()));
        (found.links, found.errors)
    }

    /// The arguments of a link of Markdown [`text`](nexus:rust.argument)
    /// read by a selector without a body.
    fn arguments_of(text: &str, link: &Link) -> Vec<String> {
        let (arguments, _) = link.read(text, WITHOUT_BODY).unwrap();
        arguments.into_iter().map(|argument| argument.text).collect()
    }

    /// The members of the body of a map of Markdown
    /// [`text`](nexus:rust.argument).
    fn members(text: &str, link: &Link) -> Vec<Member> {
        link.read(text, MAP).unwrap().1
    }

    #[test]
    fn reads_the_arguments_of_a_markdown_link_from_its_title_then_its_text() {
        let text = "See [Phase 2](nexus:markdown.section\n  (\"plans/a b.md\")) here.";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links[0].selector_name, "markdown.section");
        assert_eq!(
            &text[links[0].range.clone()],
            "[Phase 2](nexus:markdown.section\n  (\"plans/a b.md\"))"
        );
        let (arguments, _) = links[0].read(text, WITHOUT_BODY).unwrap();
        assert_eq!(arguments, vec![
            Argument {
                text: "plans/a b.md".to_owned(),
                quoted: true
            },
            Argument {
                text: "Phase 2".to_owned(),
                quoted: false
            },
        ]);
    }

    #[test]
    fn line_breaks_separate_arguments_like_any_whitespace() {
        let text = "[c](nexus:a.b (first\n  \"second\n  argument\"))";
        let (links, _) = links_in(text);
        assert_eq!(arguments_of(text, &links[0]), vec!["first", "second\nargument", "c"]);
    }

    #[test]
    fn reports_an_unclosed_quote() {
        let text = "[c](nexus:a.b (\"first))";
        let (links, _) = links_in(text);
        assert!(links[0].read(text, WITHOUT_BODY).is_err());
    }

    #[test]
    fn reads_a_links_text_with_code_spans_unwrapped_unless_its_selector_takes_no_arguments() {
        let text = "[Link\n`syntax`](nexus:markdown.section) and [nexus](nexus:nexus.self)";
        let (links, _) = links_in(text);
        assert_eq!(arguments_of(text, &links[0]), vec!["Link syntax"]);
        let shape = Shape {
            takes_arguments: false,
            ..WITHOUT_BODY
        };
        assert!(links[1].read(text, shape).unwrap().0.is_empty());
    }

    #[test]
    fn reads_reference_links_where_they_are_used() {
        let text = "Run [`nexus lint`][lint].\n\n[lint]: nexus:nexus.lint\n[web]: https://example.com\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(&text[links[0].range.clone()], "[`nexus lint`][lint]");
    }

    #[test]
    fn ignores_markdown_links_elsewhere_and_in_code() {
        let (links, errors) = links_in("[web](https://example.com) `[a](nexus:a.b)`\n\n```\n[c](nexus:c.d)\n```\n");
        assert!(links.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn reports_a_destination_without_a_selector() {
        assert_eq!(found_in("[a](nexus:self)").errors.len(), 1);
    }

    #[test]
    fn reads_a_map_between_comments() {
        let text =
            "Opcodes:\n<!-- nexus rust.map-variant\n  crate::Bar -->\n- Load: LD\n- Read: [RD](nexus:a.b)\n<!-- /nexus -->\nAfter.\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(links[0].selector_name, "rust.map-variant");
        assert!(text[links[0].range.clone()].ends_with("<!-- /nexus -->"));
        assert_eq!(links[0].nested, vec![links[1].range.clone()]);
        let (arguments, members) = links[0].read(text, MAP).unwrap();
        assert_eq!(arguments[0].text, "crate::Bar");
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("Load", "LD"), ("Read", "[RD](nexus:a.b)")]);
    }

    #[test]
    fn nested_maps_close_innermost_first_and_belong_to_the_member_holding_them() {
        let text = "<!-- nexus rust.* -->\n- module:\n  - Inline: [`a`](nexus:rust.module).\n  - Mapped:\n    <!-- nexus \
                    filesystem.map-entry c -->\n    - d: e\n    <!-- /nexus -->\n- unused: not used\n<!-- /nexus -->\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        let selectors: Vec<_> = links.iter().map(|link| link.selector_name.as_str()).collect();
        assert_eq!(selectors, vec!["rust.*", "rust.module", "filesystem.map-entry"]);
        assert_eq!(links[0].range, 0..text.len() - 1);
        let outer = members(text, &links[0]);
        let names: Vec<_> = outer.iter().map(|member| member.name.as_str()).collect();
        assert_eq!(names, vec!["module", "unused"]);
        assert!(text[outer[0].range.clone()].ends_with("<!-- /nexus -->"));
        assert_eq!(members(text, &links[2])[0].name, "d");
    }

    #[test]
    fn a_body_is_prose_where_code_hides_comments_and_quotes_mean_nothing() {
        let text = "<!-- nexus rust.* -->\n- module: `<!-- /nexus -->`, a 12\" ruler\n  ```\n  <!-- /nexus -->\n  \
                    [a](nexus:rust.module)\n  ```\n<!-- /nexus -->\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(links.len(), 1);
        let members = members(text, &links[0]);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].description.lines().next(), Some("`<!-- /nexus -->`, a 12\" ruler"));
    }

    #[test]
    fn a_body_line_neither_a_member_nor_below_one_is_an_error() {
        let text = "<!-- nexus rust.* -->\n  - module: a\nloose\n<!-- /nexus -->\n";
        let (links, _) = links_in(text);
        assert!(links[0].read(text, MAP).is_err());
    }

    #[test]
    fn a_map_is_never_written_as_a_link_nor_a_link_as_a_map() {
        let text = "[a](nexus:rust.*)\n\n<!-- nexus filesystem.file a -->\n- b\n<!-- /nexus -->\n";
        let (links, _) = links_in(text);
        assert!(links[0].read(text, MAP).is_err());
        assert!(links[1].read(text, WITHOUT_BODY).is_err());
    }

    #[test]
    fn reports_unbalanced_and_malformed_map_comments() {
        for text in [
            "<!-- nexus rust.* -->\n- a\n",
            "- a\n<!-- /nexus -->\n",
            "<!-- nexus rust.* --> - a\n<!-- /nexus -->\n",
            "<!-- nexus rust -->\n<!-- /nexus -->\n",
            "<!-- nexus rust.*\n- a\n",
        ] {
            assert_eq!(found_in(text).errors.len(), 1, "{text}");
        }
        let examples = "`<!-- nexus rust.* -->`\n\n```\n<!-- /nexus -->\n```\n<!-- other comment -->\n";
        assert!(found_in(examples).errors.is_empty());
    }

    #[test]
    fn reads_links_in_rust_comments_only() {
        let source = "// [A](nexus:markdown.section)\nfn main() {\n    let text = \"[B](nexus:markdown.section)\";\n}\n";
        let (links, errors) = links_in_rust(source);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(&source[links[0].range.clone()], "[A](nexus:markdown.section)");
    }

    #[test]
    fn reads_a_map_spread_over_rust_comment_lines() {
        let source = "/// <!-- nexus rust.map-variant\n/// crate::Bar -->\n///   - Load: LD\n///     on [a](nexus:a.b)\n/// <!-- /nexus \
                      -->\nenum Bar {}\n";
        let document = Document::parse(PathBuf::from("test.rs"), Format::Rust, source.to_owned());
        let Found { links, errors } = find_links(&document);
        assert!(errors.is_empty(), "{errors:?}");
        let selectors: Vec<_> = links.iter().map(|link| link.selector_name.as_str()).collect();
        assert_eq!(selectors, vec!["rust.map-variant", "a.b"]);
        let names: Vec<_> = members(&document.prose, &links[0]).into_iter().map(|member| member.name).collect();
        assert_eq!(names, vec!["Load"]);
    }

    #[test]
    fn reads_links_in_wit_comments_only() {
        let source = "package a:b;\n\n/// Runs [nexus](nexus:nexus.self).\n/* nested /* [lint](nexus:nexus.lint) */ */\nworld w {\n    \
                      export run: func();\n}\n";
        let document = Document::parse(PathBuf::from("test.wit"), Format::Wit, source.to_owned());
        let Found { links, errors } = find_links(&document);
        assert!(errors.is_empty());
        let texts: Vec<_> = links.iter().map(|link| &source[link.range.clone()]).collect();
        assert_eq!(texts, vec!["[nexus](nexus:nexus.self)", "[lint](nexus:nexus.lint)"]);
        assert_eq!(document.units()[0].title, "world w {");
    }
}
