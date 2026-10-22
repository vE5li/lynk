# Nexus prefers Markdown links in every format

## Decision

Each format decides how links are written in its files, since it is the
format that finds them
([Decision](nexus:markdown.section
  (design/splits-languages-into-formats-and-providers.md))). A format writes
them as Markdown links and maps, as
[Link syntax](nexus:markdown.section (README.md)) describes, whenever its
prose can be read as Markdown: Markdown files, and comments and other
prose that their own tools already render as Markdown, such as Rust's and
WIT's. Only where Markdown would be foreign to the language, or cannot be
written at all, does a format define a syntax of its own, and that syntax
still yields the same selector name, arguments and body.

## Why

One syntax is one thing to learn, for people and agents alike, and one
section of the README to keep right. Most prose in a repository is
Markdown already, or close enough that its renderers show Markdown links
as links. The command line names links by selector and arguments in
every format anyway, so a syntax of its own would only change how a link
looks in one kind of file, not what it means.

## Alternatives

- **One syntax for every format, always**: the simplest rule, but would
  force Markdown into prose whose tools render something else, where its
  links would show as noise.
- **A syntax per language by default**, each fitting its language best:
  every format would read naturally on its own, but readers moving
  between files would meet a different syntax in each, and nearly all of
  them would end up as Markdown anyway.
