# Nexus lets selectors read their links

## Decision

<[nexus.self]> reads only three things about a link before its selector
gets involved: the selector name, the closing `]>`, and the links nested
in it (<[rust.module nexus::link]>). The closing `]>` is the first one
outside code and outside nested links. Everything between the name and
the closing belongs to the selector (<[rust.module nexus::selector]>),
which reads its arguments, separated by whitespace with line breaks
included, and then a body if it takes one. This is how a link can be
wrapped like the prose around it, as
<[markdown.section README.md
  "Link syntax"]> describes.

Bodies stay prose, read the same way by every selector that takes one:
quotes and code mean nothing in them except that code hides delimiters,
and nested links are checked like any other. The map below follows these
rules; changing how links are read breaks this ADR until it is updated:

<[filesystem.map-entry
  resources
  - nexus-client: a client of the command line; neither `]>` in code nor
    a lone " ends or quotes anything here
  - nexus.wit: what a nested link such as
    <[wit.function resources/nexus.wit find]> checks against
]>

## Why

Links in source code live in comments, and formatters wrap comments. When
a link's arguments had to fit on its first line, with every later line
read as a body, a formatter wrapping a long link broke <[nexus.lint]>
without anyone touching the link. Splitting a link's arguments from its
body is a question only the selector can answer, since only it knows how
many arguments it takes. The file a link is written in decides what is
code, so link delimiters inside code examples, bodies included, are
ignored for every file type alike.

## Alternatives

- **Keeping the head on one line**: a simple grammar, but formatters,
  editors and people wrap lines, and each wrap would fail the lint.
- **Letting the head go on until a line starting with `- `**: keeps
  parsing generic, but makes a list marker part of the link grammar for
  every selector, though only map selectors take a body.
- **Letting quotes hide `]>`**: allows `]>` in an argument, but the
  closing could then only be found by knowing where the arguments end,
  which only the selector knows; no argument has needed it so far.
