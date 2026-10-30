# Lynk splits languages into formats and providers

## Decision

[lynk](lynk:lynk.self) knows a language through two roles, each a
trait with its own list, in a module of its own:

- A *format* ([`lynk::formats::Format`](lynk:rust.item), listed in
  [`lynk::formats::FORMATS`](lynk:rust.item)) reads the files links
  are written in: what of them is prose, what is code, what
  [`lynk find`](lynk:lynk.find) searches, and the links themselves.
- A *provider* ([`lynk::providers::Provider`](lynk:rust.item), listed
  in [`lynk::providers::PROVIDERS`](lynk:rust.item)) names the targets
  links point at: its selectors, how each reads its arguments, what it
  resolves to and how it checks a map's body.

A language has a submodule in each module whose role it plays, like
[`lynk::formats::rust`](lynk:rust.module) and
[`lynk::providers::rust`](lynk:rust.module). Formats share what they
have in common by calling each other: a format whose comments are
Markdown finds them itself, as only it knows its language's tokens, and
reads them with the Markdown format's reader of comments
([`lynk::formats::markdown::comments`](lynk:rust.module)). A
provider may ask which format a file has
([`lynk::formats::of`](lynk:rust.item)), but no format depends on a
provider. What is common to every link, its selector name, arguments and
body, and how a selector reads them, belongs to neither role
([`lynk::link`](lynk:rust.module)).

For now the lists hold trait objects, a provider matches its selectors by
name, and each query is a type of its provider's own
([`lynk::providers::Query`](lynk:rust.item)); this is the
simplest form, not a settled one.

## Why

Both roles used to be spread over the code by kind: one enum for every
file type, another for every selector, and a match over each wherever
they differed. Adding a language meant touching all of them; now it is
adding a submodule per role and listing it. The roles are split because
they do not always come together: the filesystem and lynk itself are
targets without files of their own to read, and a language may be read
before anything in it can be linked. The two halves of a language share
next to no code, Rust's format being its lexer
([`ra-ap-rustc_lexer`](lynk:cargo.dependency (Cargo.toml))) and its
provider [`syn`](lynk:cargo.dependency (Cargo.toml)), so
grouping by role keeps each trait next to everything implementing it,
and keeps the one direction they depend on each other in plain sight.

## Alternatives

- **One trait per language**, with both roles: simpler to list, but
  providers without files would implement reading as nothing, and a
  format without selectors would implement selectors as nothing.
- **One module per language**, holding both its format and its
  provider: everything about a language in one place, but each such
  module is really two unrelated ones in one file, and the traits end up
  apart from what implements them.
- **Keeping the enums**, grouped by language: exhaustive matches check
  that every selector is handled, but each enum still lists every
  language, so adding one still touches them all. Completeness comes
  from the lists instead, and the maps of selectors in the documentation
  still fail the lint when a selector is added.
