# Lynk leaves what .lynkignore lists unscanned

## Decision

A file named
[`.lynkignore`](lynk:rust.literal (lynk::workspace::IGNORE_FILE_NAME))
lists, in the syntax of `.gitignore` files, what of the directory holding
it, and of those below it, is not scanned for links. It is read by the
[`ignore`](lynk:cargo.dependency (Cargo.toml)) crate, which already walks
the repository, as a custom ignore file
([`lynk::workspace::scanned_files`](lynk:rust.item)).

A file it lists is still read by its format, but as if it held no links
([`lynk::workspace::read`](lynk:rust.item)): its links are neither
checked, nor listed as backlinks, nor stripped when exported, while its
sections, lines and comments may still be linked to, and it is still
searched for context. The file itself is hidden, so it is never scanned.

## Why

Some files hold link-shaped text that is not meant for lynk, such as
vendored documentation or generated code, yet the repository's own text
may still need to point into them. Leaving them out of the repository's
files altogether would break those links; leaving out only their links
keeps them working.

The `ignore` crate reads custom ignore files with the same rules as
`.gitignore` ones, nested files and negations included, so no new
dependency, format or parser is needed, and the syntax is one everyone
knows. Kept next to what it excludes, it is versioned with it, and unlike
`.gitignore` it leaves the files tracked by version control.

## Alternatives

- **Leaving listed files out entirely**, as ignored files are: one walk
  instead of two, but links into them would fail the lint.
- **An `--exclude` option**: not kept with the repository, so every
  caller, from CI to agents, would have to repeat it.
- **A configuration file or a `[package.metadata.lynk]` table**: room
  for other settings later, but a format and parser of its own for what
  is only a list of paths, and the latter only in Rust repositories.
