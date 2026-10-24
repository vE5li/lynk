# Nexus reads Rust comments with rustc's lexer

## Decision

[nexus](nexus:nexus.self) reads the links in every comment of a Rust file,
plain and doc comments alike, and nothing else of it. It finds them with
`ra-ap-rustc_lexer`, rustc's own lexer as published for rust-analyzer
([`nexus::formats::rust`](nexus:rust.module)). The lines of the comments
are then read by the Markdown format, as every format's comments are
([`nexus::formats::markdown::comments`](nexus:rust.module)): each run of
comment lines on consecutive lines is read as Markdown, so links are
Markdown links, inline code and code blocks hold examples, as in Markdown
files, and each run is one of the units [`nexus find`](nexus:nexus.find)
searches, titled by the line of code it is about. WIT files, whose
comments follow Rust's, are lexed the same way (their
[Decision](nexus:markdown.section (design/reads-wit-with-wit-parser.md)));
languages whose comments only look like Rust's are lexed by their own
format, as protobuf's
[Decision](nexus:markdown.section
  (design/reads-protobuf-with-protox-parse.md)) describes.

## Why

The rule against naming the code base in prose holds for every comment,
and most comments are plain ones, so their links need checking too. The
lexer gives every comment token with its length, so a link's position in
the source is exact, and it costs no build. It is rustc's definition of
Rust's tokens, so strings, raw strings, lifetimes and nested block comments
are told apart as the compiler does.

## Alternatives

- **syn or proc-macro2**, already used to find modules
  ([Decision](nexus:markdown.section (design/reads-rust-with-syn.md))): no
  new dependency, but both drop plain comments and keep only doc comments,
  as attributes, so links in plain comments would go unchecked.
- **A lexer of our own**: no dependency, but repeats what rustc's lexer
  already gets right, including the corner cases.

The lexer's crate is versioned with rustc and gives no stability promise;
the part used here, comment tokens, has not changed in years. It requires
`unicode-ident` and `unicode-properties` on the same Unicode version, so
`unicode-ident` is held back in the lock file until `unicode-properties`
catches up.
