//! The lines a unified diff changes, counted in the files as it leaves
//! them, which is how the repository has them when the diff is of its
//! working copy.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::location::Span;

/// The lines a diff changes, by file.
#[derive(Debug, Default)]
pub struct Changes {
    lines: HashMap<PathBuf, BTreeSet<usize>>,
}

impl Changes {
    /// Reads the unified diff [`text`](lynk:rust.argument), as `git diff`
    /// and `jj diff --git` write it, paths relative to the repository root.
    /// An added line changes itself; lines removed with none added in their
    /// place change the lines on either side of where they were, since what
    /// held them now holds those instead. Files the diff deletes change
    /// nothing left to point at.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut changes = Self::default();
        let mut lines = text.lines().enumerate();
        let mut old_path: Option<&str> = None;
        let mut new_path: Option<PathBuf> = None;
        while let Some((index, line)) = lines.next() {
            if let Some(path) = line.strip_prefix("--- ") {
                old_path = Some(path_of(path));
                continue;
            }
            if let Some(path) = line.strip_prefix("+++ ") {
                new_path = new_file(old_path, path_of(path));
                if let Some(path) = &new_path {
                    changes.lines.entry(path.clone()).or_default();
                }
                continue;
            }
            let Some(hunk) = line.strip_prefix("@@ ") else {
                continue;
            };
            let header = hunk_header(hunk).ok_or_else(|| format!("line {}: not a hunk header: {line}", index + 1))?;
            let changed = hunk_changes(header, &mut lines)?;
            let Some(path) = &new_path else {
                continue;
            };
            changes.lines.entry(path.clone()).or_default().extend(changed);
        }
        Ok(changes)
    }

    /// Whether the diff changes any line of [`span`](lynk:rust.argument),
    /// or, for a whole file or directory, any file inside it.
    pub fn touch(&self, span: &Span) -> bool {
        if span.is_whole() {
            return self.lines.keys().any(|path| path.starts_with(&span.path));
        }
        let Some(lines) = self.lines.get(&span.path) else {
            return false;
        };
        lines.range(span.start.line..=span.end.line).next().is_some()
    }
}

/// The lines a hunk changes, read from [`lines`](lynk:rust.argument) after
/// its [`header`](lynk:rust.argument). They are counted rather than read up
/// to the next header, since a removed line may itself look like one.
fn hunk_changes<'text>(
    header: (usize, usize, usize),
    lines: &mut impl Iterator<Item = (usize, &'text str)>,
) -> Result<BTreeSet<usize>, String> {
    let (mut old_count, mut new_line, mut new_count) = header;
    let mut changed = BTreeSet::new();
    // The run of removed and added lines being read, which only touches the
    // lines around it when it adds none.
    let mut removed = false;
    let mut added = false;
    loop {
        let ended = old_count == 0 && new_count == 0;
        let line = if ended { None } else { lines.next().map(|(_, line)| line) };
        if !ended && line.is_none() {
            return Err("the diff ends inside a hunk".to_owned());
        }
        match line.and_then(|line| line.chars().next()) {
            Some('+') => {
                changed.insert(new_line);
                added = true;
                new_line += 1;
                new_count = new_count.saturating_sub(1);
                continue;
            }
            Some('-') => {
                removed = true;
                old_count = old_count.saturating_sub(1);
                continue;
            }
            Some('\\') => continue,
            _ => {}
        }
        if removed && !added {
            changed.extend([new_line - 1, new_line]);
        }
        (removed, added) = (false, false);
        if ended {
            break;
        }
        // Context, its leading space possibly trimmed off an empty line.
        new_line += 1;
        old_count = old_count.saturating_sub(1);
        new_count = new_count.saturating_sub(1);
    }
    changed.remove(&0);
    Ok(changed)
}

/// A path of a `---` or `+++` line, without the timestamp `diff` may add.
fn path_of(text: &str) -> &str {
    text.split('\t').next().unwrap_or_default().trim_end()
}

/// The file a diff leads to, without the `b/` prefix git gives it when the
/// file it comes from has the `a/` one; none if the file is deleted.
fn new_file(old_path: Option<&str>, new_path: &str) -> Option<PathBuf> {
    if new_path == "/dev/null" {
        return None;
    }
    let prefixed = old_path.is_none_or(|old_path| old_path == "/dev/null" || old_path.starts_with("a/"));
    let path = match new_path.strip_prefix("b/") {
        Some(path) if prefixed => path,
        _ => new_path,
    };
    Some(Path::new(path).to_owned())
}

/// The lines a hunk removes, the first line it leads to and the lines it
/// leads to, from its header after the `@@ `.
fn hunk_header(header: &str) -> Option<(usize, usize, usize)> {
    let mut ranges = header.split_whitespace();
    let (_, old_count) = hunk_range(ranges.next()?.strip_prefix('-')?)?;
    let (new_start, new_count) = hunk_range(ranges.next()?.strip_prefix('+')?)?;
    // An empty range starts at the line before it.
    let new_line = if new_count == 0 { new_start + 1 } else { new_start };
    Some((old_count, new_line, new_count))
}

/// The start and length of a `<start>[,<length>]` range of a hunk header.
fn hunk_range(range: &str) -> Option<(usize, usize)> {
    let (start, count) = range.split_once(',').unwrap_or((range, "1"));
    Some((start.parse().ok()?, count.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Position;

    fn span(path: &str, start: usize, end: usize) -> Span {
        Span {
            path: PathBuf::from(path),
            start: Position { line: start, column: 1 },
            end: Position { line: end, column: 1 },
        }
    }

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -2,3 +2,3 @@ fn a() {
 one
-two
+deux
 three
@@ -10,2 +10,1 @@
 ten
--- not a header
";

    #[test]
    fn changes_the_added_lines_and_those_around_removed_ones() {
        let changes = Changes::parse(DIFF).unwrap();
        assert!(!changes.touch(&span("src/a.rs", 1, 2)));
        assert!(changes.touch(&span("src/a.rs", 3, 3)));
        assert!(!changes.touch(&span("src/a.rs", 4, 9)));
        assert!(changes.touch(&span("src/a.rs", 11, 11)));
        assert!(changes.touch(&span("src/a.rs", 10, 10)));
        assert!(!changes.touch(&span("src/b.rs", 3, 3)));
    }

    #[test]
    fn changes_the_files_and_directories_holding_changed_files() {
        let changes = Changes::parse(DIFF).unwrap();
        assert!(changes.touch(&Span::whole_file(Path::new("src/a.rs"))));
        assert!(changes.touch(&Span::whole_file(Path::new("src"))));
        assert!(!changes.touch(&Span::whole_file(Path::new("sr"))));
    }

    #[test]
    fn reads_new_and_deleted_files() {
        let diff = "--- /dev/null\n+++ b/new.md\n@@ -0,0 +1,2 @@\n+a\n+b\n--- a/old.md\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n";
        let changes = Changes::parse(diff).unwrap();
        assert!(changes.touch(&span("new.md", 2, 2)));
        assert!(!changes.touch(&Span::whole_file(Path::new("old.md"))));
    }

    #[test]
    fn keeps_paths_without_git_prefixes() {
        let diff = "--- b/x.md\t2024-01-01\n+++ b/x.md\t2024-01-02\n@@ -1 +1 @@\n-a\n+b\n";
        let changes = Changes::parse(diff).unwrap();
        assert!(changes.touch(&span("b/x.md", 1, 1)));
    }

    #[test]
    fn refuses_a_truncated_hunk() {
        assert!(Changes::parse("+++ b/x.md\n@@ -1,2 +1,2 @@\n a\n").is_err());
    }
}
