# Nexus lints its providers against resources

## Decision

Files exercising what [nexus](nexus:nexus.self)'s own source does not live
in a top-level `resources` directory: languages it reads but is not
written in, and the ways of writing its own language that it does not use.
This record links to everything in them, so every run of
[`nexus lint`](nexus:nexus.lint) on this repository checks those providers
against real files:

<!-- nexus filesystem.map-entry resources -->
- nexus.wit: WIT describing nexus as a component
- nexus.proto: protobuf describing nexus as a service
- nexus-client: a Rust crate running nexus's command line, with its
  README, its manifest and the flake of its development environment
<!-- /nexus -->

The WIT file describes nexus itself as a component. Its comments link to
the parts of nexus they describe, so the lint reads links in WIT comments
too, and each of its functions is linked here, one link per way WIT
declares a function:

<!-- nexus wit.* -->
- function:
  - Inside an interface:
    [`find`](nexus:wit.function (resources/nexus.wit)).
  - Inside a world:
    [`run`](nexus:wit.function (resources/nexus.wit)).
  - A resource's constructor:
    [`workspace.constructor`](nexus:wit.function (resources/nexus.wit)).
  - A resource's static function:
    [`workspace.find-root`](nexus:wit.function (resources/nexus.wit)).
  - A resource's methods:
    [`workspace.lint`](nexus:wit.function (resources/nexus.wit)),
    [`workspace.locate`](nexus:wit.function (resources/nexus.wit)) and
    [`workspace.references`](nexus:wit.function (resources/nexus.wit)).
<!-- /nexus -->

The protobuf file describes the same as messages of a service. Its
comments link to nexus too, in both of protobuf's comment forms, and its
messages are linked here, one link per way protobuf declares a message,
along with the fields of one of them:

<!-- nexus protobuf.* -->
- message:
  - At the top level of the file:
    [`LintRequest`](nexus:protobuf.message (resources/nexus.proto)).
  - Nested in another message:
    [`LintResponse.Problem`](nexus:protobuf.message (resources/nexus.proto)).
- map-field:
  <!-- nexus protobuf.map-field resources/nexus.proto Span -->
  - path: relative to the root
  - line: counted from 1
  - column: counted from 1
  <!-- /nexus -->
<!-- /nexus -->

The Rust crate is a client of nexus, offering in Rust what the WIT file
offers as a component. Its comments link to nexus as well, and each of its
modules is linked here, one link per way Rust declares a module, along
with some of its items and arguments of its functions, one link per way
of naming the item or function, the values of its constants, one link per
way of naming the constant, some of its comments, one link per
way of finding them, and the members of an enum and of a struct and a
variant declaring fields by name:

<!-- nexus rust.* -->
- module:
  - In a file of its own, next to its parent:
    [`nexus_client::span`](nexus:rust.module).
  - In a file of its own, in its parent's directory:
    [`nexus_client::span::position`](nexus:rust.module).
  - In a directory, as its `mod.rs`:
    [`nexus_client::run`](nexus:rust.module).
  - At a path given by a `#[path]` attribute:
    [`nexus_client::problem`](nexus:rust.module).
  - Inline, in its parent's file:
    [`nexus_client::workspace`](nexus:rust.module).
- item:
  - In a module:
    [`nexus_client::span::Span`](nexus:rust.item).
  - Of a module declared at a path given by a `#[path]` attribute:
    [`nexus_client::problem::Problem`](nexus:rust.item).
  - A method, through its type:
    [`nexus_client::workspace::Workspace::lint`](nexus:rust.item).
- literal:
  - Of a constant in a module, a string, by its content:
    [`nexus`](nexus:rust.literal (nexus_client::run::PROGRAM)).
  - Of an associated constant, through its type:
    [1](nexus:rust.literal (nexus_client::span::position::Position::FIRST)).
- argument:
  - Of a function in a module:
    [`root`](nexus:rust.argument (nexus_client::run::nexus)).
  - Of a method, through its type:
    [`selector`](nexus:rust.argument
      (nexus_client::workspace::Workspace::locate)).
  - Of the function a link is written on, as the crate's comments link
    to them.
- comment:
  - A plain comment, by a text spread over its lines:
    [module is named after what it
    returns](nexus:rust.comment (resources/nexus-client/src/lib.rs)).
  - In the file a link is written in, as the crate's comments link to
    them.
- map-variant:
  <!-- nexus rust.map-variant nexus_client::run::Error -->
  - Start: a variant holding a value
  - Failed: a variant declaring its fields by name
  <!-- /nexus -->
- map-field:
  - Of a struct:
    <!-- nexus rust.map-field nexus_client::span::position::Position -->
    - line: counted from 1
    - column: counted from 1
    <!-- /nexus -->
  - Of a variant, through its enum:
    <!-- nexus rust.map-field nexus_client::run::Error::Failed -->
    - stderr: what nexus printed on standard error
    <!-- /nexus -->
<!-- /nexus -->

The crate's manifest declares dependencies, and each of them is linked
here, one link per way Cargo declares a dependency:

<!-- nexus cargo.* -->
- dependency:
  - In the workspace's table:
    [`serde`](nexus:cargo.dependency (resources/nexus-client/Cargo.toml)).
  - With an inline table:
    [`serde_json`](nexus:cargo.dependency
      (resources/nexus-client/Cargo.toml)).
  - With dotted keys:
    [`shlex`](nexus:cargo.dependency (resources/nexus-client/Cargo.toml)).
  - With a version alone, in a platform's table:
    [`libc`](nexus:cargo.dependency (resources/nexus-client/Cargo.toml)).
  - Under a header of its own, among the dependencies of its tests:
    [`tempfile`](nexus:cargo.dependency
      (resources/nexus-client/Cargo.toml)).
<!-- /nexus -->

The crate's flake builds commands for its development environment, and
each of them is linked here, one link per way a binding builds a command
and a dev shell lists it:

<!-- nexus nix.* -->
- dev-shell-command:
  - Named by a string argument, in a dev shell's `buildInputs`:
    [`lint`](nexus:nix.dev-shell-command (resources/nexus-client/flake.nix)).
  - Named by the `name` of an attribute set, unlike its binding:
    [`test-client`](nexus:nix.dev-shell-command
      (resources/nexus-client/flake.nix)).
  - In a list joined to another with `++`:
    [`references`](nexus:nix.dev-shell-command
      (resources/nexus-client/flake.nix)).
<!-- /nexus -->

The crate's README is Markdown written as any repository may write it,
unlike nexus's own documentation, which writes headings one way only. Its
links are checked like any other, and each of its sections is linked
here, one link per way Markdown writes a heading, along with a line of it,
one outside the file the link is written in:

<!-- nexus markdown.* -->
- section:
  - Underlined with `=`:
    [nexus-client](nexus:markdown.section (resources/nexus-client/README.md)).
  - Underlined with `-`:
    [Usage](nexus:markdown.section (resources/nexus-client/README.md)).
  - Closed with `#`:
    [Links](nexus:markdown.section (resources/nexus-client/README.md)).
  - Nested in another section:
    [Errors](nexus:markdown.section (resources/nexus-client/README.md)).
  - With inline code, linked by its text:
    [The `PATH` it runs nexus from](nexus:markdown.section
      (resources/nexus-client/README.md)).
  - With emphasis, linked by its text:
    [Spans, *as nexus counts them*](nexus:markdown.section
      (resources/nexus-client/README.md)).
- line:
  [nothing is an empty list](nexus:markdown.line
    (resources/nexus-client/README.md)).
<!-- /nexus -->

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
