# Lynk defaults to where a link is written

## Decision

A selector may leave out the arguments naming what holds its target when
the link is written inside it: a Markdown file for a section, a Rust file
for a comment, a Rust function for one of its arguments (the README's
[Selectors](lynk:markdown.section (README.md))). The link's
file and first line are all a selector gets to decide
([`lynk::providers::Place`](lynk:rust.item)); a Rust function holds a
link when its definition, doc comment included, does, and the innermost
one wins
([`lynk::providers::rust::items::find_enclosing_argument`](lynk:rust.item)).
On the command line, where nothing holds the link, those arguments are
required.

## Why

A comment describing a function names its arguments far more often than
anything else in it; repeating the function's path in each link would
make the comment harder to read than the backticks it replaces, and go
stale when the function moves. Links are compared by the spans they
resolve to ([`lynk::providers::Query::resolve`](lynk:rust.item)), so [`lynk ref`](lynk:lynk.ref) finds a link written with
the default from the same target written in full.

## Alternatives

- **Always naming the function**: one form only, but heavy enough that
  comments would keep naming arguments in backticks instead.
- **Taking the function from the item after the comment**, for plain
  comments too: would cover a `//` comment above a function, but the item
  after a comment is not always what it describes, while doc comments are
  part of their function's definition.
