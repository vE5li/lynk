//! Links as Markdown writes them, in Markdown files and in every format
//! whose prose is Markdown.

use std::ops::Range;

use super::parse::MarkdownLink;
use crate::document::Document;
use crate::link::{Found, Link, SyntaxError};

/// What a Markdown link's destination starts with to be a link to check.
const SCHEME: &str = "lynk:";
const COMMENT_OPENING: &str = "<!--";
const COMMENT_CLOSING: &str = "-->";
/// The word opening a map's comment.
const MAP_KEYWORD: &str = "lynk";
const MAP_CLOSING: &str = "<!-- /lynk -->";

/// Every link of the document's prose outside code, plus what failed to
/// parse. [`markdown_links`](lynk:rust.argument) are those Markdown found
/// in the prose, whatever their destination.
pub fn find(document: &Document, markdown_links: &[MarkdownLink]) -> Found {
    let mut found = Found::default();
    find_markdown_links(markdown_links, &mut found);
    find_maps(document, &mut found);
    found.finish()
}

/// The Markdown links whose destination is a selector after `lynk:`; the
/// title holds the arguments before the last one, the text.
fn find_markdown_links(markdown_links: &[MarkdownLink], found: &mut Found) {
    for markdown_link in markdown_links {
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
            arguments: markdown_link.title.clone().unwrap_or_default(),
            last_argument: Some(markdown_link.text.clone()),
            body: None,
            nested: Vec::new(),
            range: markdown_link.range.clone(),
        });
    }
}

/// The maps written between an opening `<!-- lynk <provider>.<selector>
/// <arguments> -->` and a closing `<!-- /lynk -->`, each starting a line of
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
                    arguments: text[map.arguments].to_owned(),
                    last_argument: None,
                    body: Some(map.body_start.min(current_start)..current_start),
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::formats;
    use crate::link::{Argument, Link, Member, Shape, SyntaxError};
    use crate::workspace::LinkedDocument;

    const MAP: Shape = Shape {
        map: true,
        takes_arguments: true,
    };

    /// [`text`](lynk:rust.argument) read as a file at
    /// [`path`](lynk:rust.argument).
    fn read(path: &str, text: &str) -> LinkedDocument {
        formats::of(Path::new(path)).unwrap().read(PathBuf::from(path), text.to_owned())
    }

    fn links_in(text: &str) -> (Vec<Link>, Vec<SyntaxError>) {
        let linked = read("test.md", text);
        (linked.links, linked.syntax_errors)
    }

    fn arguments_of(text: &str, link: &Link) -> Vec<String> {
        let (arguments, _) = link.read(text, Shape::LINK).unwrap();
        arguments.into_iter().map(|argument| argument.text).collect()
    }

    fn members(text: &str, link: &Link) -> Vec<Member> {
        link.read(text, MAP).unwrap().1
    }

    #[test]
    fn reads_the_arguments_of_a_link_from_its_title_then_its_text() {
        let text = "See [Phase 2](lynk:markdown.section\n  (\"plans/a b.md\")) here.";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links[0].selector_name, "markdown.section");
        assert_eq!(
            &text[links[0].range.clone()],
            "[Phase 2](lynk:markdown.section\n  (\"plans/a b.md\"))"
        );
        let (arguments, _) = links[0].read(text, Shape::LINK).unwrap();
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
    fn a_title_may_go_on_over_several_lines() {
        let text = "[c](lynk:a.b (first\n  \"second\n  argument\"))";
        let (links, _) = links_in(text);
        assert_eq!(arguments_of(text, &links[0]), vec!["first", "second\nargument", "c"]);
    }

    #[test]
    fn reads_a_links_text_with_code_spans_unwrapped() {
        let text = "[Link\n`syntax`](lynk:markdown.section)";
        let (links, _) = links_in(text);
        assert_eq!(arguments_of(text, &links[0]), vec!["Link syntax"]);
    }

    #[test]
    fn reads_reference_links_where_they_are_used() {
        let text = "Run [`lynk lint`][lint].\n\n[lint]: lynk:lynk.lint\n[web]: https://example.com\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty());
        assert_eq!(links.len(), 1);
        assert_eq!(&text[links[0].range.clone()], "[`lynk lint`][lint]");
    }

    #[test]
    fn ignores_links_elsewhere_and_in_code() {
        let (links, errors) = links_in("[web](https://example.com) `[a](lynk:a.b)`\n\n```\n[c](lynk:c.d)\n```\n");
        assert!(links.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn reports_a_destination_without_a_selector() {
        assert_eq!(links_in("[a](lynk:self)").1.len(), 1);
    }

    #[test]
    fn reads_a_map_between_comments() {
        let text = "Opcodes:\n<!-- lynk rust.map-variant\n  crate::Bar -->\n- Load: LD\n- Read: [RD](lynk:a.b)\n<!-- /lynk -->\nAfter.\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(links[0].selector_name, "rust.map-variant");
        assert!(text[links[0].range.clone()].ends_with("<!-- /lynk -->"));
        assert_eq!(links[0].nested, vec![links[1].range.clone()]);
        let (arguments, members) = links[0].read(text, MAP).unwrap();
        assert_eq!(arguments[0].text, "crate::Bar");
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("Load", "LD"), ("Read", "[RD](lynk:a.b)")]);
    }

    #[test]
    fn nested_maps_close_innermost_first_and_belong_to_the_member_holding_them() {
        let text = "<!-- lynk rust.* -->\n- module:\n  - Inline: [`a`](lynk:rust.module).\n  - Mapped:\n    <!-- lynk \
                    filesystem.map-entry c -->\n    - d: e\n    <!-- /lynk -->\n- unused: not used\n<!-- /lynk -->\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        let selectors: Vec<_> = links.iter().map(|link| link.selector_name.as_str()).collect();
        assert_eq!(selectors, vec!["rust.*", "rust.module", "filesystem.map-entry"]);
        assert_eq!(links[0].range, 0..text.len() - 1);
        let outer = members(text, &links[0]);
        let names: Vec<_> = outer.iter().map(|member| member.name.as_str()).collect();
        assert_eq!(names, vec!["module", "unused"]);
        assert!(text[outer[0].range.clone()].ends_with("<!-- /lynk -->"));
        assert_eq!(members(text, &links[2])[0].name, "d");
    }

    #[test]
    fn a_body_is_prose_where_code_hides_comments_and_quotes_mean_nothing() {
        let text = "<!-- lynk rust.* -->\n- module: `<!-- /lynk -->`, a 12\" ruler\n  ```\n  <!-- /lynk -->\n  [a](lynk:rust.module)\n  \
                    ```\n<!-- /lynk -->\n";
        let (links, errors) = links_in(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(links.len(), 1);
        let members = members(text, &links[0]);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].description.lines().next(), Some("`<!-- /lynk -->`, a 12\" ruler"));
    }

    #[test]
    fn reports_unbalanced_and_malformed_map_comments() {
        for text in [
            "<!-- lynk rust.* -->\n- a\n",
            "- a\n<!-- /lynk -->\n",
            "<!-- lynk rust.* --> - a\n<!-- /lynk -->\n",
            "<!-- lynk rust -->\n<!-- /lynk -->\n",
            "<!-- lynk rust.*\n- a\n",
        ] {
            assert_eq!(links_in(text).1.len(), 1, "{text}");
        }
        let examples = "`<!-- lynk rust.* -->`\n\n```\n<!-- /lynk -->\n```\n<!-- other comment -->\n";
        assert!(links_in(examples).1.is_empty());
    }

    #[test]
    fn reads_links_in_rust_comments_only() {
        let source = "// [A](lynk:markdown.section)\nfn main() {\n    let text = \"[B](lynk:markdown.section)\";\n}\n";
        let linked = read("test.rs", source);
        assert!(linked.syntax_errors.is_empty());
        assert_eq!(linked.links.len(), 1);
        assert_eq!(&source[linked.links[0].range.clone()], "[A](lynk:markdown.section)");
    }

    #[test]
    fn reads_a_map_spread_over_rust_comment_lines() {
        let source = "/// <!-- lynk rust.map-variant\n/// crate::Bar -->\n///   - Load: LD\n///     on [a](lynk:a.b)\n/// <!-- /lynk \
                      -->\nenum Bar {}\n";
        let linked = read("test.rs", source);
        assert!(linked.syntax_errors.is_empty(), "{:?}", linked.syntax_errors);
        let selectors: Vec<_> = linked.links.iter().map(|link| link.selector_name.as_str()).collect();
        assert_eq!(selectors, vec!["rust.map-variant", "a.b"]);
        let names: Vec<_> = members(&linked.document.prose, &linked.links[0])
            .into_iter()
            .map(|member| member.name)
            .collect();
        assert_eq!(names, vec!["Load"]);
    }

    #[test]
    fn reads_links_in_wit_comments_only() {
        let source = "package a:b;\n\n/// Runs [lynk](lynk:lynk.self).\n/* nested /* [lint](lynk:lynk.lint) */ */\nworld w {\n    export \
                      run: func();\n}\n";
        let linked = read("test.wit", source);
        assert!(linked.syntax_errors.is_empty());
        let texts: Vec<_> = linked.links.iter().map(|link| &source[link.range.clone()]).collect();
        assert_eq!(texts, vec!["[lynk](lynk:lynk.self)", "[lint](lynk:lynk.lint)"]);
        assert_eq!(linked.document.units[0].title, "world w {");
    }
}
