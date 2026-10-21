# nexus

Documentation and code comments that name parts of a code base go stale
the moment someone changes the code without updating the text, and nothing
notices. <[nexus.self]> replaces such names with *links* it checks against
the code, so moving, renaming, adding or removing what a link refers to
fails <[nexus.lint]> until the text is updated:


```
Every variant maps to a single opcode in the VM:

<[rust.map-variant cratename::foo::Bar
- Load: LD
- Read: RD
]>
```

It reads the file types <[rust.module nexus::document]> defines: documentation
whole, source code only in its comments.

## Commands

`nexus [--root <directory>] <command>`; without `--root`, the repository
is the nearest directory up holding `.jj` or `.git`.

- <[nexus.lint]> checks every link of the repository.
- <[nexus.where]> resolves a link to the spans it points at, with their
  first line.
- <[nexus.ref]> lists every place linking to the spans a link points at.
- <[nexus.find]> lists the parts of the documentation and the comments
  matching a few words, best first. Unlike links, it matches words, so it
  also finds what nobody linked yet. With `TYPESAFE_API_KEY` set, its best
  word matches are ordered by how relevant they are, shown as a
  probability.

<[nexus.where]> and <[nexus.ref]> take a `<provider.selector>` and its
arguments. On the command line nothing is quoted, and a file left out
matches every file: `nexus ref markdown.section Legacy` lists everything
referring to any section titled "Legacy".

## Link syntax

A link is `<[`, a *provider* (the file type or language its target lives
in), a dot, a *selector* (what to pick out of it), the selector's
arguments and `]>`. Arguments are separated by whitespace, line breaks
included, so a link may be wrapped like the prose around it; they may be
quoted with `"`. Each selector defines its own arguments, their defaults
and whether it takes a body, and reads everything up to the `]>` closing
its link. That `]>` is the first one outside code and outside nested
links; quotes do not hide it. Links inside code, inline or fenced, are
examples and are not checked; in source code, where only comments are
read, each run of comment lines is read as Markdown to find that code:

```
<[markdown.section "Commands"]>
<[markdown.section docs/design.md "Layering"]>
<[protobuf.message protobuf/window.proto OpenWindowOk]>
<[protobuf.message-field protobuf/window.proto OpenWindowOk view_id]>
```

*Map* selectors list the members of their target in a body, after their
arguments and before the closing `]>`, as a Markdown list of `- <member>: <description>` items. A
description may go on over the lines indented below its item, nested lists
and links included; a link nested in a body is checked like any other.
A body is prose, where quotes mean nothing and delimiters inside code do
not count:

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
<[wit.*
- function: written as <[wit.function nexus.wit find]>
]>
```

## Selectors

The selectors <[nexus.self]> implements; a link with any other selector
fails the lint. Arguments left out default to where the link is written:
without a file, the `markdown` selectors mean the file the link is written
in, and without a function, `rust.argument` means the function the link is
written on, the innermost whose definition, doc comment included, holds
the link. Rust items are named by their paths as Rust writes them,
starting with the crate:

<[nexus.selectors
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
]>

Selectors planned so far:

- `rust.item <crate path>`; `rust.map-variant` / `rust.partial-map-variant
  <enum>`; `rust.map-field` / `rust.partial-map-field <struct>`.
- `protobuf.message <file> <message>`; `protobuf.message-field <file>
  <message> <field>`; `protobuf.map-field` / `protobuf.partial-map-field
  <file> <message>`; `protobuf.map-message` / `protobuf.partial-map-message
  <file>`.
- `filesystem.partial-map-entry <directory>`.
