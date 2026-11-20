//! Documents as the readers of their rendered text see them: without the
//! syntax of their links, their text and bodies left in place, and in
//! Markdown files the links renderers can follow written as plain links.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use crate::formats;
use crate::location::LineIndex;
use crate::providers::{self, PlainLink};
use crate::workspace::{self, LinkedDocument, Workspace};

/// Writes every file of [`directory`](lynk:rust.argument), relative to
/// [`root`](lynk:rust.argument), to the same place inside
/// [`output`](lynk:rust.argument): those of a format exported, the others
/// copied as they are. The output must be absolute and outside the
/// directory, which would otherwise end up exporting its own output.
pub fn export_directory(root: &Path, directory: &Path, output: &Path) -> Result<(), String> {
    if !output.is_absolute() {
        return Err(format!("{}: the output path must be absolute", output.display()));
    }
    let root = root.canonicalize().map_err(|error| format!("{}: {error}", root.display()))?;
    let source = root.join(directory);
    let source = source.canonicalize().map_err(|error| format!("{}: {error}", source.display()))?;
    if !source.is_dir() {
        return Err(format!("{}: not a directory", source.display()));
    }
    let resolved_output = output.canonicalize().unwrap_or_else(|_| output.to_owned());
    if resolved_output.starts_with(&source) {
        return Err(format!("{}: the output is inside {}", output.display(), source.display()));
    }
    // Listed and read before writing anything, so that links may lead to
    // any of them.
    let files = workspace::files(&source).map_err(|error| format!("{}: {error}", source.display()))?;
    let relative = |path: &Path| path.strip_prefix(&root).unwrap_or(path).to_owned();
    let mut documents = Vec::new();
    for path in &files {
        let Some(format) = formats::of(path) else {
            continue;
        };
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        documents.push(format.read(relative(path), text));
    }
    let exported: Vec<PathBuf> = files.iter().map(|path| relative(path)).collect();
    let workspace = Workspace {
        root: root.clone(),
        documents,
    };
    for path in &files {
        let target = output.join(path.strip_prefix(&source).unwrap_or(path));
        let text = workspace
            .document(&relative(path))
            .map(|linked| export(linked, &plain_links(linked, &workspace, &exported)));
        write_file(path, &target, text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

/// Writes [`text`](lynk:rust.argument) to [`target`](lynk:rust.argument),
/// or without any, copies the file at [`path`](lynk:rust.argument) there.
fn write_file(path: &Path, target: &Path, text: Option<String>) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let Some(text) = text else {
        std::fs::copy(path, target)?;
        return Ok(());
    };
    std::fs::write(target, text)
}

/// The destinations of the plain links replacing those of
/// [`linked`](lynk:rust.argument) in its export, by the links' ranges: in a
/// Markdown file, every link to `lynk:` whose target a plain link can lead
/// to, among the [`exported`](lynk:rust.argument) paths. Source code
/// comments are left out, as their tools read destinations their own way.
fn plain_links(linked: &LinkedDocument, workspace: &Workspace, exported: &[PathBuf]) -> HashMap<Range<usize>, String> {
    let document = &linked.document;
    if document.format != formats::markdown::NAME {
        return HashMap::new();
    }
    linked
        .links
        .iter()
        .filter(|link| link.text.is_some())
        .filter_map(|link| {
            let (query, _) = providers::of_link(link, document).ok()?;
            let plain_link = query.plain_link(workspace)?;
            let target = without_current_directory(&plain_link.path);
            exported.iter().any(|path| path.starts_with(&target)).then_some(())?;
            Some((link.range.clone(), destination(&document.path, &plain_link)))
        })
        .collect()
}

/// [`path`](lynk:rust.argument) without its `.` components, the root being
/// empty.
fn without_current_directory(path: &Path) -> PathBuf {
    path.components().filter(|component| *component != Component::CurDir).collect()
}

/// The destination of [`plain_link`](lynk:rust.argument) written in the
/// file at [`from`](lynk:rust.argument): its path relative to the file,
/// none for a section of the file itself, escaped as a URL.
fn destination(from: &Path, plain_link: &PlainLink) -> String {
    let target = without_current_directory(&plain_link.path);
    let base = without_current_directory(from.parent().unwrap_or(Path::new("")));
    let target: Vec<_> = target.components().collect();
    let base: Vec<_> = base.components().collect();
    let common = target.iter().zip(&base).take_while(|(first, second)| first == second).count();
    let parts: Vec<String> = std::iter::repeat_n("..".to_owned(), base.len() - common)
        .chain(target[common..].iter().map(|part| escape(&part.as_os_str().to_string_lossy())))
        .collect();
    let mut destination = parts.join("/");
    if plain_link.directory {
        if destination.is_empty() {
            destination.push('.');
        }
        destination.push('/');
    }
    let Some(anchor) = &plain_link.anchor else {
        return destination;
    };
    if plain_link.path == from {
        destination.clear();
    }
    format!("{destination}#{}", escape(anchor))
}

/// [`text`](lynk:rust.argument) with every byte a URL's path or fragment
/// may not hold as it is written as a `%XX` escape.
fn escape(text: &str) -> String {
    const ALLOWED: &str = "-._~!$&'*+,;=:@";
    let mut escaped = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || ALLOWED.as_bytes().contains(&byte) {
            escaped.push(byte as char);
            continue;
        }
        escaped.push_str(&format!("%{byte:02X}"));
    }
    escaped
}

/// What may be left of a line once the syntax of its links is gone for the
/// line to go too, besides whitespace: the markers of the comment it was
/// written in.
const COMMENT_MARKERS: &[char] = &['/', '*', '!'];

/// The document's text without the syntax of its links, and without the
/// lines holding nothing else. The links given a destination in
/// [`plain_links`](lynk:rust.argument), by their ranges, are written as
/// plain links to it instead, around their text.
pub fn export(linked: &LinkedDocument, plain_links: &HashMap<Range<usize>, String>) -> String {
    let text = &linked.document.text;
    let mut hidden = vec![false; text.len()];
    for range in &linked.link_syntax {
        hidden[range.clone()].fill(true);
    }
    // What closes a plain link's text, by where it is inserted; the syntax
    // after the text stays hidden.
    let mut closings = HashMap::new();
    for link in &linked.links {
        let (Some(text_range), Some(destination)) = (&link.text, plain_links.get(&link.range)) else {
            continue;
        };
        hidden[link.range.start..text_range.start].fill(false);
        closings.insert(text_range.end, format!("]({destination})"));
    }
    for line in LineIndex::new(text).lines() {
        if !hidden[line.clone()].contains(&true) {
            continue;
        }
        let left_only_markers = text[line.clone()]
            .char_indices()
            .filter(|(offset, _)| !hidden[line.start + offset])
            .all(|(_, character)| character.is_whitespace() || COMMENT_MARKERS.contains(&character));
        if !left_only_markers {
            continue;
        }
        let with_line_break = line.start..(line.end + 1).min(text.len());
        hidden[with_line_break].fill(true);
    }
    let mut kept = Vec::with_capacity(text.len());
    for (offset, byte) in text.bytes().enumerate() {
        if let Some(closing) = closings.get(&offset) {
            kept.extend(closing.bytes());
        }
        if !hidden[offset] {
            kept.push(byte);
        }
    }
    // Links' syntax starts and ends at ASCII characters, as do lines.
    String::from_utf8(kept).expect("hiding whole characters keeps the text valid UTF-8")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn exported(path: &str, text: &str) -> String {
        let format = formats::of(Path::new(path)).unwrap();
        export(&format.read(PathBuf::from(path), text.to_owned()), &HashMap::new())
    }

    #[test]
    fn keeps_the_text_of_links_as_written() {
        let text =
            "Run [`lynk lint`](lynk:lynk.lint) or [a\n  section](lynk:markdown.section\n  (README.md)), see [web](https://example.com).\n";
        assert_eq!(
            exported("test.md", text),
            "Run `lynk lint` or a\n  section, see [web](https://example.com).\n"
        );
    }

    #[test]
    fn leaves_links_in_code_alone() {
        let text = "`[a](lynk:a.b)`\n\n```\n<!-- lynk rust.* -->\n```\n";
        assert_eq!(exported("test.md", text), text);
    }

    #[test]
    fn drops_the_definitions_of_reference_links() {
        let text = "Run [`lynk lint`][lint] [x][web].\n\n[lint]: lynk:lynk.lint\n[web]: https://example.com\n";
        assert_eq!(
            exported("test.md", text),
            "Run `lynk lint` [x][web].\n\n[web]: https://example.com\n"
        );
    }

    #[test]
    fn keeps_the_body_of_maps_without_their_comments() {
        let text = "Opcodes:\n<!-- lynk rust.map-variant\n  crate::Bar -->\n- Load: LD\n  - Nested:\n    <!-- lynk a.b -->\n    - c: \
                    [d](lynk:a.b)\n    <!-- /lynk -->\n<!-- /lynk -->\nAfter.\n";
        assert_eq!(
            exported("test.md", text),
            "Opcodes:\n- Load: LD\n  - Nested:\n    - c: d\nAfter.\n"
        );
    }

    #[test]
    fn drops_comment_lines_left_empty_but_keeps_code() {
        let source =
            "/// See [it](lynk:a.b\n///   (c d)).\n/// <!-- lynk a.b -->\n/// - e: f\n/// <!-- /lynk -->\nfn g() {} // [](lynk:a.b)\n";
        assert_eq!(exported("test.rs", source), "/// See it.\n/// - e: f\nfn g() {} // \n");
    }

    /// An empty directory of its own for the test called
    /// [`name`](lynk:rust.argument).
    fn scratch_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("lynk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn exports_the_files_it_reads_and_copies_the_others() {
        let root = scratch_directory("exports-root");
        let output = scratch_directory("exports-output").join("out");
        std::fs::create_dir_all(root.join("docs/nested")).unwrap();
        std::fs::write(root.join("docs/a.md"), "See [lint](lynk:lynk.lint).\n").unwrap();
        std::fs::write(root.join("docs/nested/b.txt"), "[c](lynk:a.b)\n").unwrap();
        std::fs::write(root.join("elsewhere.md"), "").unwrap();
        export_directory(&root, Path::new("docs"), &output).unwrap();
        assert_eq!(std::fs::read_to_string(output.join("a.md")).unwrap(), "See lint.\n");
        assert_eq!(std::fs::read_to_string(output.join("nested/b.txt")).unwrap(), "[c](lynk:a.b)\n");
        assert!(!output.join("elsewhere.md").exists());
    }

    #[test]
    fn writes_links_renderers_follow_as_plain_links_to_what_it_exports() {
        let root = scratch_directory("plain-root");
        let output = scratch_directory("plain-output").join("out");
        std::fs::create_dir_all(root.join("docs/sub dir")).unwrap();
        let a = "# Here\n\nSee [docs/b.md](lynk:filesystem.file), [docs/sub dir](lynk:filesystem.directory),\n[Two \
                 words](lynk:markdown.section\n  (\"docs/sub dir/c.md\")), [Here](lynk:markdown.section),\n[`elsewhere.md`][e] and \
                 [lint](lynk:lynk.lint).\n\n[e]: lynk:filesystem.file\n";
        std::fs::write(root.join("docs/a.md"), a).unwrap();
        std::fs::write(root.join("docs/b.md"), "").unwrap();
        std::fs::write(
            root.join("docs/sub dir/c.md"),
            "# Two words\n\n[docs/b.md](lynk:filesystem.file)\n",
        )
        .unwrap();
        std::fs::write(root.join("docs/d.rs"), "/// [docs/b.md](lynk:filesystem.file)\nfn d() {}\n").unwrap();
        std::fs::write(root.join("elsewhere.md"), "").unwrap();
        export_directory(&root, Path::new("docs"), &output).unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join("a.md")).unwrap(),
            "# Here\n\nSee [docs/b.md](b.md), [docs/sub dir](sub%20dir/),\n[Two words](sub%20dir/c.md#two-words), \
             [Here](#here),\n`elsewhere.md` and lint.\n\n"
        );
        assert_eq!(
            std::fs::read_to_string(output.join("sub dir/c.md")).unwrap(),
            "# Two words\n\n[docs/b.md](../b.md)\n"
        );
        assert_eq!(
            std::fs::read_to_string(output.join("d.rs")).unwrap(),
            "/// docs/b.md\nfn d() {}\n"
        );
    }

    #[test]
    fn refuses_a_relative_output_or_one_inside_the_directory() {
        let root = scratch_directory("refuses-root");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        assert!(export_directory(&root, Path::new("docs"), Path::new("out")).is_err());
        assert!(export_directory(&root, Path::new("docs"), &root.join("docs/out")).is_err());
    }
}
