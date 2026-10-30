//! Links as every format finds them, whatever their syntax, and how a
//! selector reads one.

use std::ops::Range;

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
    /// Its arguments as written: separated by whitespace, line breaks
    /// included, and maybe quoted with `"`.
    pub arguments: String,
    /// An argument after them taken whole, such as a Markdown link's text,
    /// for selectors taking any.
    pub last_argument: Option<String>,
    /// A map's body, in the document's prose.
    pub body: Option<Range<usize>>,
    /// The links inside it, at any depth.
    pub nested: Vec<Range<usize>>,
    /// The whole link, in the document's prose.
    pub range: Range<usize>,
}

/// How a selector reads a link's content.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// Whether it is a map selector, taking a body.
    pub map: bool,
    /// Whether it takes any argument.
    pub takes_arguments: bool,
}

impl Shape {
    pub const LINK: Self = Self {
        map: false,
        takes_arguments: true,
    };
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

impl Found {
    /// Orders the links and tells each which links it holds.
    pub fn finish(mut self) -> Self {
        self.links.sort_by_key(|link| (link.range.start, std::cmp::Reverse(link.range.end)));
        let ranges: Vec<Range<usize>> = self.links.iter().map(|link| link.range.clone()).collect();
        for link in &mut self.links {
            link.nested = ranges
                .iter()
                .filter(|range| **range != link.range && link.range.start <= range.start && range.end <= link.range.end)
                .cloned()
                .collect();
        }
        self
    }
}

impl Link {
    /// The link's arguments and, for a map, the members of its body, as a
    /// selector of [`shape`](lynk:rust.argument) reads them.
    /// [`prose`](lynk:rust.argument) is that of the document the link was
    /// found in.
    pub fn read(&self, prose: &str, shape: Shape) -> Result<(Vec<Argument>, Vec<Member>), SyntaxError> {
        let error = |message: String| SyntaxError {
            message,
            offset: self.range.start,
        };
        let name = &self.selector_name;
        let members = match (shape.map, &self.body) {
            (true, Some(body)) => members(prose, body.clone(), &self.nested)?,
            (false, None) => Vec::new(),
            (true, None) => {
                return Err(error(format!("`{name}` is a map, so it is written with a body")));
            }
            (false, Some(_)) => return Err(error(format!("`{name}` takes no body"))),
        };
        let mut arguments = arguments(&self.arguments, self.range.start)?;
        let last_argument = self.last_argument.as_ref().filter(|text| shape.takes_arguments && !text.is_empty());
        arguments.extend(last_argument.map(|text| Argument {
            text: text.clone(),
            quoted: false,
        }));
        Ok((arguments, members))
    }
}

/// The arguments written in [`text`](lynk:rust.argument), whose errors are
/// reported at [`offset`](lynk:rust.argument).
fn arguments(text: &str, offset: usize) -> Result<Vec<Argument>, SyntaxError> {
    let mut arguments = Vec::new();
    let mut characters = text.chars().peekable();
    while let Some(&character) = characters.peek() {
        if character.is_whitespace() {
            characters.next();
            continue;
        }
        if character != '"' {
            let mut text = String::new();
            while let Some(&character) = characters.peek() {
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
        while let Some(character) = characters.next() {
            match character {
                '"' => {
                    closed = true;
                    break;
                }
                '\\' => text.extend(characters.next()),
                _ => text.push(character),
            }
        }
        if !closed {
            return Err(SyntaxError {
                message: "quoted argument is never closed".to_owned(),
                offset,
            });
        }
        arguments.push(Argument { text, quoted: true });
    }
    Ok(arguments)
}

/// The [`body`](lynk:rust.argument) of a map read as members: each
/// `- <member>: <description>` line no more indented than the first, with
/// the lines indented further below it, and the links nested in it, as part
/// of its description. A body is prose: quotes mean nothing in it.
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
    use super::*;

    fn link(arguments: &str, last_argument: Option<&str>, body: Option<Range<usize>>) -> Link {
        Link {
            selector_name: "a.b".to_owned(),
            arguments: arguments.to_owned(),
            last_argument: last_argument.map(str::to_owned),
            body,
            nested: Vec::new(),
            range: 0..0,
        }
    }

    const MAP: Shape = Shape {
        map: true,
        takes_arguments: true,
    };

    #[test]
    fn reads_bare_and_quoted_arguments_then_the_last_one_whole() {
        let (arguments, _) = link("a \"b \\\"c\\\"\"\n d", Some("e f"), None).read("", Shape::LINK).unwrap();
        assert_eq!(arguments, vec![
            Argument {
                text: "a".to_owned(),
                quoted: false
            },
            Argument {
                text: "b \"c\"".to_owned(),
                quoted: true
            },
            Argument {
                text: "d".to_owned(),
                quoted: false
            },
            Argument {
                text: "e f".to_owned(),
                quoted: false
            },
        ]);
    }

    #[test]
    fn a_selector_without_arguments_ignores_the_last_one() {
        let shape = Shape {
            takes_arguments: false,
            ..Shape::LINK
        };
        assert!(link("", Some("text"), None).read("", shape).unwrap().0.is_empty());
    }

    #[test]
    fn reports_an_unclosed_quote() {
        assert!(link("\"a", None, None).read("", Shape::LINK).is_err());
    }

    #[test]
    fn a_map_has_a_body_and_a_link_has_none() {
        assert!(link("", None, None).read("", MAP).is_err());
        assert!(link("", None, Some(0..0)).read("", Shape::LINK).is_err());
    }

    #[test]
    fn reads_a_body_of_members_with_their_descriptions() {
        let prose = "- a: b\n  more\n- c\n";
        let (_, members) = link("", None, Some(0..prose.len())).read(prose, MAP).unwrap();
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("a", "b\n  more"), ("c", "")]);
    }

    #[test]
    fn a_body_line_neither_a_member_nor_below_one_is_an_error() {
        let prose = "  - module: a\nloose\n";
        assert!(link("", None, Some(0..prose.len())).read(prose, MAP).is_err());
    }
}
