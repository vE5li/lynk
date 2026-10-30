# Lynk reads WIT with wit-parser

## Decision

The `wit` provider of [lynk](lynk:lynk.self) parses WIT files with
[`wit-parser`](lynk:cargo.dependency (Cargo.toml))
([`lynk::providers::wit`](lynk:rust.module)), the parser of the
Bytecode Alliance's `wasm-tools`, without its default features. A file is
parsed together with the other WIT files of its directory, as they may
form one package, or alone if they do not parse; dependencies are not
resolved ([`lynk::providers::wit::parse_package`](lynk:rust.item)). A
function is found by the span of its name
([`lynk::providers::wit::find_function`](lynk:rust.item)), a resource's
named after it
([`lynk::providers::wit::link_name`](lynk:rust.item)).

The links in a WIT file's comments, plain and doc comments alike, are read
with the reader of Rust comments
([`lynk::formats::rust::read`](lynk:rust.item)), not with `wit-parser`,
as its
[Decision](lynk:markdown.section
  (design/reads-rust-comments-with-rustc-lexer.md)) describes.

## Why

[`lynk lint`](lynk:lynk.lint) runs before every change, so reading a
WIT file must not need its dependencies fetched or a component built. `wit-parser` is the
reference definition of the language, keeps the span of every function,
and names a resource's functions after it. Without default features it
leaves out `wasmparser` and `serde`.

WIT writes comments as Rust does: `//` and `///` lines, and `/* */` blocks
that nest. Its other tokens have no quotes, so rustc's lexer cannot
mistake any of them for a comment or a comment for one of them.
`wit-parser` keeps only doc comments, without their positions, and does
not expose its lexer.

## Alternatives

- **Matching lines by hand**: no dependency, but would have to tell
  functions from types, comments and strings, and follow resources, all of
  which the grammar already settles.
- **Resolving the whole package with its dependencies** (`Resolve`): would
  check `use` across packages too, but fails whenever a dependency is
  missing from the `deps` directory, which says nothing about whether a
  link is stale.
- **Reading comments with a lexer of our own**: WIT's comment syntax is
  small, but nested block comments are its corner case, and the Rust
  reader already gets them right.
