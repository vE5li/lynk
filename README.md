# nexus

Documentation and code comments that name parts of a code base go stale
the moment someone changes the code without updating the text, and nothing
notices. [nexus](nexus:nexus.self) replaces such names with *links* it
checks against the code, so moving, renaming, adding or removing what a
link refers to fails [`nexus lint`](nexus:nexus.lint) until the text is
updated:


```
Every variant maps to a single opcode in the VM:

<!-- nexus rust.map-variant cratename::foo::Bar -->
- Load: LD
- Read: RD
<!-- /nexus -->
```

It reads the file types [`nexus::format`](nexus:rust.module) lists:
documentation whole, source code only in its comments.

## Commands

`nexus [--root <directory>] <command>`; without `--root`, the repository
is the nearest directory up holding `.jj` or `.git`.

- [`lint`](nexus:nexus.lint) checks every link of the repository.
- [`where`](nexus:nexus.where) resolves a link to the spans it points at,
  with their first line.
- [`ref`](nexus:nexus.ref) lists every place linking to the spans a link
  points at.
- [`find`](nexus:nexus.find) lists the parts of the documentation and the
  comments matching a few words, best first. Unlike links, it matches
  words, so it also finds what nobody linked yet. With `TYPESAFE_API_KEY`
  set, its best word matches are ordered by how relevant they are, shown
  as a probability.

[`where`](nexus:nexus.where) and [`ref`](nexus:nexus.ref) take a
`<provider.selector>` and its arguments. On the command line nothing is
quoted, and a file left out matches every file: `nexus ref markdown.section
Legacy` lists everything referring to any section titled "Legacy".

## Link syntax

A link is a Markdown link whose destination is `nexus:`, a *provider* (the
file type or language its target lives in), a dot and a *selector* (what
to pick out of it). The link text, what the reader sees, is the selector's
last argument, its code spans unwrapped; the arguments before it, if any,
go in the link's title, in parentheses, which renderers show at most as a
tooltip. A selector taking no arguments ignores the text. Arguments in the
title are separated by whitespace, line breaks included, so a link may be
wrapped like the prose around it; they may be quoted with `"`. Each
selector defines its own arguments and their defaults. Links inside code,
inline or fenced, are examples and are not checked; in source code, where
only comments are read, each run of comment lines is read as Markdown,
links included:

```
[Commands](nexus:markdown.section)
[Layering](nexus:markdown.section (docs/design.md))
[`OpenWindowOk`](nexus:protobuf.message (protobuf/window.proto))
[`view_id`](nexus:protobuf.message-field
  (protobuf/window.proto OpenWindowOk))
```

Links used often may be written in reference style, the definition
holding the selector and arguments:

```
[nexus][self] checks links; run [`nexus lint`][lint] before pushing.

[self]: nexus:nexus.self
[lint]: nexus:nexus.lint
```

*Map* selectors list the members of their target in a body, a Markdown
list of `- <member>: <description>` items between an opening
`<!-- nexus <provider.selector> <arguments> -->` comment and a closing
`<!-- /nexus -->`, each starting a line and ending one. Both comments are
hidden once rendered, leaving the list. A description may go on over the
lines indented below its item, nested lists and links included; a link
nested in a body is checked like any other, and nested maps close
innermost first:

- **`map-<member>`** is complete: every member appears exactly once, so
  adding, removing or renaming one fails the lint until the list follows.
- **`partial-map-<member>`** names some members: every entry must exist,
  but members may be left out. Use it only when the text is genuinely
  about a few members, never to dodge updating a complete map.

Written in place of a selector, `*` makes a complete map of the provider's
selectors, named without the provider. It takes no arguments and promises
nothing about its descriptions; it only makes adding a selector to the
provider fail the lint until the text describing it follows. On the
command line, quote it from the shell, which would expand it:

```
<!-- nexus wit.* -->
- function: written as [`find`](nexus:wit.function (nexus.wit))
<!-- /nexus -->
```

## Selectors

The selectors [nexus](nexus:nexus.self) implements; a link with any other
selector fails the lint. Arguments left out default to where the link is
written: without a file, the `markdown` selectors mean the file the link
is written in, and without a function, `rust.argument` means the function
the link is written on, the innermost whose definition, doc comment
included, holds the link. Rust items are named by their paths as Rust
writes them, starting with the crate:

<!-- nexus nexus.selectors -->
- markdown.section: `[file] "<heading>"`, the section under a heading
- markdown.line: `[file] "<text>"`, a line containing the text
- filesystem.file: `<path>`
- filesystem.directory: `<path>`
- filesystem.map-entry: `<directory>`, its entries except hidden and ignored ones
- rust.module: `<module path>`, the crate alone being its root
- rust.argument: `[function path] <argument>`, an argument of a function, `self` for its receiver; a method's path goes through its type or trait
- wit.function: `<file> <function>`, the functions with that name in its interfaces and worlds; a resource's as `<resource>.<function>`, its constructor as `<resource>.constructor`
- nexus.selectors: no arguments; a complete map of these selectors
- nexus.self: no arguments; the tool itself
- nexus.lint: no arguments; its command checking every link
- nexus.where: no arguments; its command resolving a link
- nexus.ref: no arguments; its command listing the links to a target
- nexus.find: no arguments; its command searching the documentation by words
<!-- /nexus -->

Selectors planned so far:

- `rust.item <crate path>`; `rust.map-variant` / `rust.partial-map-variant
  <enum>`; `rust.map-field` / `rust.partial-map-field <struct>`.
- `protobuf.message <file> <message>`; `protobuf.message-field <file>
  <message> <field>`; `protobuf.map-field` / `protobuf.partial-map-field
  <file> <message>`; `protobuf.map-message` / `protobuf.partial-map-message
  <file>`.
- `filesystem.partial-map-entry <directory>`.
