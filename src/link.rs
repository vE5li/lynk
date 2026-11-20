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
    /// Where its text is written, in the document's prose, for a link
    /// written as a Markdown link to `lynk:`: what stays of it once
    /// exported, unless a plain link can replace it.
    pub text: Option<Range<usize>>,
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
    /// What only the links are written in, in the document's prose: the
    /// text without it is what a reader of the rendered text sees.
    pub syntax: Vec<Range<usize>>,
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

/// The [`body`](lynk:rust.argument) of a map read as members, from a table
/// when its second line is a table's delimiter row, from a list otherwise.
/// A body is prose: quotes mean nothing in it, but a member may be written
/// as a code span.
fn members(text: &str, body: Range<usize>, nested: &[Range<usize>]) -> Result<Vec<Member>, SyntaxError> {
    let is_table = lines(text, body.clone())
        .filter(|(_, line)| !line.trim().is_empty())
        .nth(1)
        .is_some_and(|(_, line)| is_delimiter_row(line.trim()));
    if is_table {
        return table_members(text, body, nested);
    }
    list_members(text, body, nested)
}

/// The lines of [`body`](lynk:rust.argument) in
/// [`text`](lynk:rust.argument), each with where it starts.
fn lines(text: &str, body: Range<usize>) -> impl Iterator<Item = (usize, &str)> {
    text[body.clone()].split('\n').scan(body.start, |start, line| {
        let line_start = *start;
        *start += line.len() + 1;
        Some((line_start, line))
    })
}

/// The members of a body written as a list: each `- <member>:
/// <description>` line no more indented than the first, with the lines
/// indented further below it, and the links nested in it, as part of its
/// description.
fn list_members(text: &str, body: Range<usize>, nested: &[Range<usize>]) -> Result<Vec<Member>, SyntaxError> {
    let mut members: Vec<Member> = Vec::new();
    let mut member_indentation = None;
    for (line_start, line) in lines(text, body) {
        let indentation = line.len() - line.trim_start().len();
        let offset = line_start + indentation;
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
            message: BODY_SYNTAX.to_owned(),
            offset,
        })?;
        member_indentation.get_or_insert(indentation);
        let name = entry.split_once(':').map_or(entry, |(name, _)| name).trim();
        members.push(Member {
            name: unwrap_code_span(name).to_owned(),
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

/// The members of a body written as a table: after its header and
/// delimiter rows, each row's first cell is a member and the rest of the
/// row its description. A blank line ends the table, and with it the body.
fn table_members(text: &str, body: Range<usize>, nested: &[Range<usize>]) -> Result<Vec<Member>, SyntaxError> {
    let mut members = Vec::new();
    let mut rows_seen = 0;
    let mut ended = false;
    for (line_start, line) in lines(text, body) {
        let row = line.trim();
        let offset = line_start + line.len() - line.trim_start().len();
        let error = |message: &str| SyntaxError {
            message: message.to_owned(),
            offset,
        };
        if row.is_empty() {
            ended = rows_seen > 0;
            continue;
        }
        rows_seen += 1;
        if ended {
            return Err(error("a table ends a map's body"));
        }
        if nested.iter().any(|range| range.start <= offset && offset < range.end) {
            return Err(error("a map's table holds no map"));
        }
        // Its header and delimiter rows.
        if rows_seen <= 2 {
            continue;
        }
        let cells = row_cells(row);
        let name = cells[0].trim().replace("\\|", "|");
        if name.is_empty() {
            return Err(error("a row of a map's table starts with a member"));
        }
        let inner = row_inner(row);
        let description = inner.get(cells[0].len() + 1..).unwrap_or("").trim();
        members.push(Member {
            name: unwrap_code_span(&name).to_owned(),
            description: description.to_owned(),
            range: offset..offset + row.len(),
        });
    }
    Ok(members)
}

const BODY_SYNTAX: &str = "a map's body is a list of `- <member>: <description>` or a table whose first column holds the members";

/// [`name`](lynk:rust.argument) without the backticks of a code span
/// around it.
fn unwrap_code_span(name: &str) -> &str {
    name.strip_prefix('`').and_then(|name| name.strip_suffix('`')).unwrap_or(name)
}

/// Whether [`row`](lynk:rust.argument), trimmed, is a table's delimiter
/// row: cells such as `---`, `:--`, `--:` or `:-:`, separated by pipes.
fn is_delimiter_row(row: &str) -> bool {
    row.contains('|')
        && row_cells(row).iter().all(|cell| {
            let cell = cell.trim();
            let dashes = cell.strip_prefix(':').unwrap_or(cell);
            let dashes = dashes.strip_suffix(':').unwrap_or(dashes);
            !dashes.is_empty() && dashes.chars().all(|character| character == '-')
        })
}

/// [`row`](lynk:rust.argument), trimmed, without the pipes it may start and
/// end with.
fn row_inner(row: &str) -> &str {
    let row = row.strip_prefix('|').unwrap_or(row);
    match row.strip_suffix('|') {
        Some(inner) if !inner.ends_with('\\') => inner,
        _ => row,
    }
}

/// The cells of [`row`](lynk:rust.argument), trimmed, separated by the
/// pipes not escaped as `\|`, untrimmed.
fn row_cells(row: &str) -> Vec<&str> {
    let inner = row_inner(row);
    let mut cells = Vec::new();
    let mut cell_start = 0;
    let mut escaped = false;
    for (index, character) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '|' => {
                cells.push(&inner[cell_start..index]);
                cell_start = index + 1;
            }
            _ => {}
        }
    }
    cells.push(&inner[cell_start..]);
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(arguments: &str, last_argument: Option<&str>, body: Option<Range<usize>>) -> Link {
        Link {
            selector_name: "a.b".to_owned(),
            arguments: arguments.to_owned(),
            last_argument: last_argument.map(str::to_owned),
            text: None,
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
    fn reads_members_written_as_code_spans() {
        let prose = "- `a`: `b`\n- `c`\n";
        let (_, members) = link("", None, Some(0..prose.len())).read(prose, MAP).unwrap();
        let members: Vec<_> = members
            .iter()
            .map(|member| (member.name.as_str(), member.description.as_str()))
            .collect();
        assert_eq!(members, vec![("a", "`b`"), ("c", "")]);
    }

    fn names_and_descriptions(prose: &str) -> Result<Vec<(String, String)>, SyntaxError> {
        let (_, members) = link("", None, Some(0..prose.len())).read(prose, MAP)?;
        Ok(members.into_iter().map(|member| (member.name, member.description)).collect())
    }

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
    }

    #[test]
    fn reads_a_table_of_members_from_its_first_column() {
        let prose = "| Variant | Opcode | Note |\n| --- | :-: | --- |\n| `Load` | LD | [a](lynk:a.b) |\n| Read | RD |  |\n";
        assert_eq!(
            names_and_descriptions(prose).unwrap(),
            pairs(&[("Load", "LD | [a](lynk:a.b)"), ("Read", "RD |")])
        );
    }

    #[test]
    fn reads_table_rows_without_outer_pipes_and_with_escaped_ones() {
        let prose = "Member | Description\n--- | ---\na\\|b | c \\| d\ne\n";
        assert_eq!(names_and_descriptions(prose).unwrap(), pairs(&[("a|b", "c \\| d"), ("e", "")]));
    }

    #[test]
    fn a_table_is_the_whole_body() {
        for prose in [
            "| a | b |\n| - | - |\n| c | d |\n\n| e | f |\n",
            "| a | b |\n| - | - |\n| c | d |\n\n- e: f\n",
            "| a | b |\n| - | - |\n|  | d |\n",
        ] {
            assert!(names_and_descriptions(prose).is_err(), "{prose}");
        }
        assert!(names_and_descriptions("- a: b\n| c | d |\n").is_err());
    }

    #[test]
    fn a_body_line_neither_a_member_nor_below_one_is_an_error() {
        let prose = "  - module: a\nloose\n";
        assert!(link("", None, Some(0..prose.len())).read(prose, MAP).is_err());
    }
}
