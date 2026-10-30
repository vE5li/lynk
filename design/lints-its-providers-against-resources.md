# Lynk lints its providers against resources

## Decision

Files exercising what [lynk](lynk:lynk.self)'s own source does not live
in a top-level `resources` directory: languages it reads but is not
written in, and the ways of writing its own language that it does not use.
This record links to everything in them, so every run of
[`lynk lint`](lynk:lynk.lint) on this repository checks those providers
against real files:

<!-- lynk filesystem.map-entry resources -->
- lynk.wit: WIT describing lynk as a component
- lynk.proto: protobuf describing lynk as a service
- lynk-client: a Rust crate running lynk's command line, with its
  README, its manifest and the flake of its development environment
<!-- /lynk -->

The WIT file describes lynk itself as a component. Its comments link to
the parts of lynk they describe, so the lint reads links in WIT comments
too, and each of its functions is linked here, one link per way WIT
declares a function:

<!-- lynk wit.* -->
- function:
  - Inside an interface:
    [`find`](lynk:wit.function (resources/lynk.wit)).
  - Inside a world:
    [`run`](lynk:wit.function (resources/lynk.wit)).
  - A resource's constructor:
    [`workspace.constructor`](lynk:wit.function (resources/lynk.wit)).
  - A resource's static function:
    [`workspace.find-root`](lynk:wit.function (resources/lynk.wit)).
  - A resource's methods:
    [`workspace.lint`](lynk:wit.function (resources/lynk.wit)),
    [`workspace.locate`](lynk:wit.function (resources/lynk.wit)) and
    [`workspace.references`](lynk:wit.function (resources/lynk.wit)).
<!-- /lynk -->

The protobuf file describes the same as messages of a service. Its
comments link to lynk too, in both of protobuf's comment forms, and its
messages are linked here, one link per way protobuf declares a message,
along with the fields of one of them:

<!-- lynk protobuf.* -->
- message:
  - At the top level of the file:
    [`LintRequest`](lynk:protobuf.message (resources/lynk.proto)).
  - Nested in another message:
    [`LintResponse.Problem`](lynk:protobuf.message (resources/lynk.proto)).
- map-field:
  <!-- lynk protobuf.map-field resources/lynk.proto Span -->
  - path: relative to the root
  - line: counted from 1
  - column: counted from 1
  <!-- /lynk -->
<!-- /lynk -->

The Rust crate is a client of lynk, offering in Rust what the WIT file
offers as a component. Its comments link to lynk as well, and each of its
modules is linked here, one link per way Rust declares a module, along
with some of its items and arguments of its functions, one link per way
of naming the item or function, the values of its constants, one link per
way of naming the constant, some of its comments, one link per
way of finding them, and the members of an enum and of a struct and a
variant declaring fields by name:

<!-- lynk rust.* -->
- module:
  - In a file of its own, next to its parent:
    [`lynk_client::span`](lynk:rust.module).
  - In a file of its own, in its parent's directory:
    [`lynk_client::span::position`](lynk:rust.module).
  - In a directory, as its `mod.rs`:
    [`lynk_client::run`](lynk:rust.module).
  - At a path given by a `#[path]` attribute:
    [`lynk_client::problem`](lynk:rust.module).
  - Inline, in its parent's file:
    [`lynk_client::workspace`](lynk:rust.module).
- item:
  - In a module:
    [`lynk_client::span::Span`](lynk:rust.item).
  - Of a module declared at a path given by a `#[path]` attribute:
    [`lynk_client::problem::Problem`](lynk:rust.item).
  - A method, through its type:
    [`lynk_client::workspace::Workspace::lint`](lynk:rust.item).
- literal:
  - Of a constant in a module, a string, by its content:
    [`lynk`](lynk:rust.literal (lynk_client::run::PROGRAM)).
  - Of an associated constant, through its type:
    [1](lynk:rust.literal (lynk_client::span::position::Position::FIRST)).
- argument:
  - Of a function in a module:
    [`root`](lynk:rust.argument (lynk_client::run::lynk)).
  - Of a method, through its type:
    [`selector`](lynk:rust.argument
      (lynk_client::workspace::Workspace::locate)).
  - Of the function a link is written on, as the crate's comments link
    to them.
- comment:
  - A plain comment, by a text spread over its lines:
    [module is named after what it
    returns](lynk:rust.comment (resources/lynk-client/src/lib.rs)).
  - In the file a link is written in, as the crate's comments link to
    them.
- map-variant:
  <!-- lynk rust.map-variant lynk_client::run::Error -->
  - Start: a variant holding a value
  - Failed: a variant declaring its fields by name
  <!-- /lynk -->
- map-field:
  - Of a struct:
    <!-- lynk rust.map-field lynk_client::span::position::Position -->
    - line: counted from 1
    - column: counted from 1
    <!-- /lynk -->
  - Of a variant, through its enum:
    <!-- lynk rust.map-field lynk_client::run::Error::Failed -->
    - stderr: what lynk printed on standard error
    <!-- /lynk -->
<!-- /lynk -->

The crate's manifest declares dependencies, and each of them is linked
here, one link per way Cargo declares a dependency:

<!-- lynk cargo.* -->
- dependency:
  - In the workspace's table:
    [`serde`](lynk:cargo.dependency (resources/lynk-client/Cargo.toml)).
  - With an inline table:
    [`serde_json`](lynk:cargo.dependency
      (resources/lynk-client/Cargo.toml)).
  - With dotted keys:
    [`shlex`](lynk:cargo.dependency (resources/lynk-client/Cargo.toml)).
  - With a version alone, in a platform's table:
    [`libc`](lynk:cargo.dependency (resources/lynk-client/Cargo.toml)).
  - Under a header of its own, among the dependencies of its tests:
    [`tempfile`](lynk:cargo.dependency
      (resources/lynk-client/Cargo.toml)).
<!-- /lynk -->

The crate's flake builds commands for its development environment, and
each of them is linked here, one link per way a binding builds a command
and a dev shell lists it:

<!-- lynk nix.* -->
- dev-shell-command:
  - Named by a string argument, in a dev shell's `buildInputs`:
    [`lint`](lynk:nix.dev-shell-command (resources/lynk-client/flake.nix)).
  - Named by the `name` of an attribute set, unlike its binding:
    [`test-client`](lynk:nix.dev-shell-command
      (resources/lynk-client/flake.nix)).
  - In a list joined to another with `++`:
    [`references`](lynk:nix.dev-shell-command
      (resources/lynk-client/flake.nix)).
<!-- /lynk -->

The crate's README is Markdown written as any repository may write it,
unlike lynk's own documentation, which writes headings one way only. Its
links are checked like any other, and each of its sections is linked
here, one link per way Markdown writes a heading, along with a line of it,
one outside the file the link is written in:

<!-- lynk markdown.* -->
- section:
  - Underlined with `=`:
    [lynk-client](lynk:markdown.section (resources/lynk-client/README.md)).
  - Underlined with `-`:
    [Usage](lynk:markdown.section (resources/lynk-client/README.md)).
  - Closed with `#`:
    [Links](lynk:markdown.section (resources/lynk-client/README.md)).
  - Nested in another section:
    [Errors](lynk:markdown.section (resources/lynk-client/README.md)).
  - With inline code, linked by its text:
    [The `PATH` it runs lynk from](lynk:markdown.section
      (resources/lynk-client/README.md)).
  - With emphasis, linked by its text:
    [Spans, *as lynk counts them*](lynk:markdown.section
      (resources/lynk-client/README.md)).
- line:
  [nothing is an empty list](lynk:markdown.line
    (resources/lynk-client/README.md)).
<!-- /lynk -->

Each of these lists maps all of its provider's selectors, so a selector
added to one of them fails the lint until it is exercised here too, or
the list says why it is not.

## Why

Unit tests cover a provider against files written in a temporary directory,
which nothing but the test reads. A resource is read by the same lint any
repository runs, through the same links, so a change to a provider or to
the parser behind it that stops finding what these links point at fails
the lint here before it fails elsewhere. Describing lynk keeps the
resources about this repository rather than arbitrary samples.

## Alternatives

- **Unit tests only**: they check the provider's results but not the path
  from a written link to them.
- **Resources next to the code reading them**, under `src`: mixes files
  that are not Rust into the crate, and hides them from anyone looking for
  what the repository describes.
