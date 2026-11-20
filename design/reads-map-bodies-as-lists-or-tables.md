# Lynk reads map bodies as lists or tables

## Decision

A map's body, as the README's [Link syntax](../README.md#link-syntax)
defines it, is either a Markdown list or a Markdown table
([`lynk::link::members`](lynk:rust.item)). It is a table when its
second line is a table's delimiter row
([`lynk::link::is_delimiter_row`](lynk:rust.item)), and a list otherwise.

In a table ([`lynk::link::table_members`](lynk:rust.item)), the first
column holds the members, one per row, and the rest of the row is the
member's description, the pipes between its cells included. The header
row is free text, not a member, and lynk does not check it. Cells are
split as GitHub Flavored Markdown splits them, on every pipe not escaped
as `\|`, code spans included
([`lynk::link::row_cells`](lynk:rust.item)). A table is the whole body:
a line after the blank line ending it, or a map nested in it, is an
error.

A list ([`lynk::link::list_members`](lynk:rust.item)) reads as it always
has, and is still the only body that can nest maps.

## Why

A table is how documentation usually lays out several facts about each
member, such as a variant with its opcode and its cost. As a list, those
facts would have to be packed into one description. The member is what
the row is about, so it goes in the first column, where readers look for
it.

The first column needs no marking, which keeps the table as a reader
would write it without lynk. The rest of the row is kept as the
description because nothing checks descriptions; splitting it into
columns would give them a structure nothing uses.

The header names the columns for the reader. Requiring a fixed header
would make every table read the same, at the cost of a word that suits
the table, so it is left free for now.

## Alternatives

- **Choosing the members' column by its header**, such as one titled
  `Member`: lets the members sit in any column, but makes the header
  syntax, and a table whose first column is not its subject reads
  oddly anyway.
- **Marking the members' cells**, with a code span or a link: works in
  any column, but forces a style on the table that a list does not have.
- **Only lists**: one syntax to learn and to read, but tables describing
  members then stay unchecked, or get rewritten as lists that read worse.
