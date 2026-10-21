# Nexus lints its providers against resources

## Decision

Files exercising what <[nexus.self]>'s own source does not live in a
top-level `resources` directory: languages it reads but is not written in,
and the ways of writing its own language that it does not use. This ADR
links to everything in them, so every run of <[nexus.lint]> on this
repository checks those providers against real files:

<[filesystem.map-entry resources
  - nexus.wit: WIT describing nexus as a component
  - nexus-client: a Rust crate running nexus's command line, with its README
]>

The WIT file describes nexus itself as a component. Its comments link to
the parts of nexus they describe, so the lint reads links in WIT comments
too, and each of its functions is linked here, one link per way WIT
declares a function:

<[wit.*
- function:
  - Inside an interface:
    <[wit.function resources/nexus.wit find]>.
  - Inside a world:
    <[wit.function resources/nexus.wit run]>.
  - A resource's constructor:
    <[wit.function resources/nexus.wit workspace.constructor]>.
  - A resource's static function:
    <[wit.function resources/nexus.wit workspace.find-root]>.
  - A resource's methods:
    <[wit.function resources/nexus.wit workspace.lint]>,
    <[wit.function resources/nexus.wit workspace.locate]> and
    <[wit.function resources/nexus.wit workspace.references]>.
]>

The Rust crate is a client of nexus, offering in Rust what the WIT file
offers as a component. Its comments link to nexus as well, and each of its
modules is linked here, one link per way Rust declares a module, along
with arguments of its functions, one link per way of naming the function:

<[rust.*
- module:
  - In a file of its own, next to its parent:
    <[rust.module nexus_client::span]>.
  - In a file of its own, in its parent's directory:
    <[rust.module nexus_client::span::position]>.
  - In a directory, as its `mod.rs`:
    <[rust.module nexus_client::run]>.
  - At a path given by a `#[path]` attribute:
    <[rust.module nexus_client::problem]>.
  - Inline, in its parent's file:
    <[rust.module nexus_client::workspace]>.
- argument:
  - Of a function in a module:
    <[rust.argument nexus_client::run::nexus root]>.
  - Of a method, through its type:
    <[rust.argument nexus_client::workspace::Workspace::locate selector]>.
  - Of the function a link is written on, as the crate's comments link
    to them.
]>

The crate's README is Markdown written as any repository may write it,
unlike nexus's own documentation, which writes headings one way only. Its
links are checked like any other, and each of its sections is linked
here, one link per way Markdown writes a heading, along with a line of it,
one outside the file the link is written in:

<[markdown.*
- section:
  - Underlined with `=`:
    <[markdown.section resources/nexus-client/README.md "nexus-client"]>.
  - Underlined with `-`:
    <[markdown.section resources/nexus-client/README.md "Usage"]>.
  - Closed with `#`:
    <[markdown.section resources/nexus-client/README.md "Links"]>.
  - Nested in another section:
    <[markdown.section resources/nexus-client/README.md "Errors"]>.
  - With inline code, linked by its text:
    <[markdown.section resources/nexus-client/README.md "The PATH it runs nexus from"]>.
  - With emphasis, linked by its text:
    <[markdown.section resources/nexus-client/README.md "Spans, as nexus counts them"]>.
- line:
  <[markdown.line resources/nexus-client/README.md "nothing is an empty list"]>.
]>

Each of these lists maps all of its provider's selectors, so a selector
added to one of them fails the lint until it is exercised here too, or
the list says why it is not.

## Why

Unit tests cover a provider against files written in a temporary directory,
which nothing but the test reads. A resource is read by the same lint any
repository runs, through the same links, so a change to a provider or to
the parser behind it that stops finding what these links point at fails
the lint here before it fails elsewhere. Describing nexus keeps the
resources about this repository rather than arbitrary samples.

## Alternatives

- **Unit tests only**: they check the provider's results but not the path
  from a written link to them.
- **Resources next to the code reading them**, under `src`: mixes files
  that are not Rust into the crate, and hides them from anyone looking for
  what the repository describes.
