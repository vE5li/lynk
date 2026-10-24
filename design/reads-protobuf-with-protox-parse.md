# Nexus reads protobuf with protox-parse

## Decision

The `protobuf` provider of [nexus](nexus:nexus.self) parses protobuf files
with [`protox-parse`](nexus:cargo.dependency (Cargo.toml))
([`nexus::providers::protobuf`](nexus:rust.module)), the parser of
`protox`, one file at a time, without resolving imports or type names. A
message is found through the source code info the parser fills in, by the
location of its declaration, from its keyword to its closing brace
([`nexus::providers::protobuf::find_message`](nexus:rust.item)), its
lines and columns turned into bytes
([`nexus::providers::protobuf::byte_range`](nexus:rust.item)). A nested
message, like
[`LintResponse.Problem`](nexus:protobuf.message (resources/nexus.proto)),
is named after the messages it is nested in, joined with dots as protobuf
writes it, but without the package: the file already scopes it
([`nexus::providers::protobuf::collect_messages`](nexus:rust.item)).

The links in a protobuf file's comments are found by a scanner of the
protobuf format's own
([`nexus::formats::protobuf::comment_lines`](nexus:rust.item)), following protobuf's tokenizer: `//` comments up to the end of the line,
`/* */` comments up to the first `*/`, without nesting, and neither inside
a string, quoted with `"` or `'`. The lines it finds are read as Markdown,
as every format's comments are
([`nexus::formats::markdown::comments::read`](nexus:rust.item)).

## Why

[`nexus lint`](nexus:nexus.lint) runs before every change, so reading a
protobuf file must not need `protoc` installed or its imports found.
`protox-parse` is written in Rust, parses a single file's syntax on its
own, and records the span of every declaration in the descriptor's source
code info, typed by [`prost-types`](nexus:cargo.dependency (Cargo.toml)), as `protoc` does, so the parser settles what is a message and
where it ends.

Comments need their exact positions, so links in them can be reported and
followed. The parser keeps comments only as text attached to the
declarations around them, without positions, and does not expose its
lexer; what tells a comment apart in protobuf is
small enough to scan directly: two comment forms and two quotes.

## Alternatives

- **`protoc`**, run as a process: the reference compiler, but an external
  tool every repository would have to install, and it fails on any
  missing import, which says nothing about whether a link is stale.
- **`protobuf-parse`**, the parser of the `protobuf` crate: written in
  Rust too, but its pure parser leaves out the source code info, so it
  says what a file declares but not where.
- **`protox`** with its file resolution: would check imports and types
  across files, at the same cost as `protoc` when one is missing.
- **Matching lines by hand**: no dependency, but would have to tell
  messages from comments, strings and options, and follow nesting, all of
  which the grammar already settles.
- **Naming messages with their package**: what generated code and other
  files use, but repeats what the file already says in every link.
- **Reading comments with rustc's lexer**, as WIT's are: protobuf's
  comments look like Rust's, but its block comments do not nest and its
  strings may be quoted with `'`, which Rust lexes as a character or a
  lifetime, so a comment could be missed or code read as one.
- **Reading comments from the source code info**: no scanner of our own,
  but gives comments as text without positions, so a link in one could
  be neither reported nor followed.
