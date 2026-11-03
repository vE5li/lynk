//! Documents as the readers of their rendered text see them: without the
//! syntax of their links, their text and bodies left in place.

use std::path::Path;

use crate::formats;
use crate::location::LineIndex;
use crate::workspace::{self, LinkedDocument};

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
    // Listed before writing anything.
    let files = workspace::files(&source).map_err(|error| format!("{}: {error}", source.display()))?;
    for path in files {
        let target = output.join(path.strip_prefix(&source).unwrap_or(&path));
        export_file(&root, &path, &target).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

/// Writes the file at [`path`](lynk:rust.argument) to
/// [`target`](lynk:rust.argument), exported if a format reads it.
fn export_file(root: &Path, path: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let Some(format) = formats::of(path) else {
        std::fs::copy(path, target)?;
        return Ok(());
    };
    let text = std::fs::read_to_string(path)?;
    let relative_path = path.strip_prefix(root).unwrap_or(path).to_owned();
    std::fs::write(target, export(&format.read(relative_path, text)))
}

/// What may be left of a line once the syntax of its links is gone for the
/// line to go too, besides whitespace: the markers of the comment it was
/// written in.
const COMMENT_MARKERS: &[char] = &['/', '*', '!'];

/// The document's text without the syntax of its links, and without the
/// lines holding nothing else.
pub fn export(linked: &LinkedDocument) -> String {
    let text = &linked.document.text;
    let mut hidden = vec![false; text.len()];
    for range in &linked.link_syntax {
        hidden[range.clone()].fill(true);
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
    let kept: Vec<u8> = text
        .bytes()
        .zip(&hidden)
        .filter(|(_, hidden)| !**hidden)
        .map(|(byte, _)| byte)
        .collect();
    // Links' syntax starts and ends at ASCII characters, as do lines.
    String::from_utf8(kept).expect("hiding whole characters keeps the text valid UTF-8")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn exported(path: &str, text: &str) -> String {
        let format = formats::of(Path::new(path)).unwrap();
        export(&format.read(PathBuf::from(path), text.to_owned()))
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
    fn refuses_a_relative_output_or_one_inside_the_directory() {
        let root = scratch_directory("refuses-root");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        assert!(export_directory(&root, Path::new("docs"), Path::new("out")).is_err());
        assert!(export_directory(&root, Path::new("docs"), &root.join("docs/out")).is_err());
    }
}
