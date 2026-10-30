//! Rust comments, found by their text.

use std::ops::Range;
use std::path::Path;

use crate::formats::rust::NAME;
use crate::location::Span;
use crate::workspace::{LinkedDocument, Workspace};

/// Every comment of the Rust files, or of [`file`](lynk:rust.argument)
/// alone, holding [`text`](lynk:rust.argument). A comment is a run of
/// comment lines on consecutive lines, read as one text, so the text may
/// spread over its lines, as a link's text may.
pub fn find_comments(workspace: &Workspace, file: Option<&Path>, text: &str) -> Vec<Span> {
    let text = collapse_whitespace(text);
    workspace
        .documents
        .iter()
        .filter(|linked| linked.document.format == NAME && file.is_none_or(|file| file == linked.document.path))
        .flat_map(|linked| {
            let document = &linked.document;
            document
                .units
                .iter()
                .filter(|unit| outside_links(linked, unit.range.clone()).iter().any(|part| part.contains(&text)))
                .map(|unit| document.span(unit.range.clone()))
        })
        .collect()
}

/// The parts of a comment's prose between the links in it, its whitespace
/// collapsed; a link naming the text would otherwise match its own comment.
fn outside_links(linked: &LinkedDocument, range: Range<usize>) -> Vec<String> {
    let mut links: Vec<_> = linked
        .links
        .iter()
        .map(|link| link.range.clone())
        .filter(|link| range.start <= link.start && link.end <= range.end)
        .collect();
    links.sort_by_key(|link| link.start);
    let starts = std::iter::once(range.start).chain(links.iter().map(|link| link.end));
    let ends = links.iter().map(|link| link.start).chain(std::iter::once(range.end));
    starts
        .zip(ends)
        .map(|(start, end)| collapse_whitespace(&linked.document.prose[start..end]))
        .collect()
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::formats;

    fn workspace(files: &[(&str, &str)]) -> Workspace {
        let documents = files
            .iter()
            .map(|(path, text)| formats::of(Path::new(path)).unwrap().read(PathBuf::from(path), (*text).to_owned()))
            .collect();
        Workspace {
            root: PathBuf::new(),
            documents,
        }
    }

    fn comments(workspace: &Workspace, file: Option<&str>, text: &str) -> Vec<String> {
        find_comments(workspace, file.map(Path::new), text)
            .iter()
            .map(|span| format!("{span}"))
            .collect()
    }

    #[test]
    fn finds_the_comments_holding_the_text_across_their_lines() {
        let workspace = workspace(&[
            (
                "src/a.rs",
                "// Kept sorted,\n// by name.\nconst A: u8 = 0;\n\n/* by name */\nfn b() {}\n",
            ),
            ("src/b.rs", "fn c() {} // sorted, by name\n"),
            ("b.wit", "// Kept sorted, by name.\n"),
        ]);
        assert_eq!(comments(&workspace, None, "sorted,\nby name"), vec![
            "src/a.rs:1:3-2:11",
            "src/b.rs:1:13-1:28"
        ]);
        assert_eq!(comments(&workspace, Some("src/a.rs"), "by name"), vec![
            "src/a.rs:1:3-2:11",
            "src/a.rs:5:3-5:11"
        ]);
        assert!(comments(&workspace, None, "const").is_empty());
    }

    #[test]
    fn a_link_does_not_match_its_own_comment() {
        let workspace = workspace(&[(
            "src/a.rs",
            "// Kept sorted.\nconst A: u8 = 0;\n\n/// As [kept sorted](lynk:rust.comment), above.\nfn b() {}\n",
        )]);
        assert_eq!(comments(&workspace, None, "sorted"), vec!["src/a.rs:1:3-1:15"]);
    }
}
