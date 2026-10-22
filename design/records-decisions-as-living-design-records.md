# Nexus records decisions as living design records

## Decision

Decisions about the architecture or the tooling are recorded in a
top-level [`design`](nexus:filesystem.directory) directory, one short
file per decision, named after it, with three sections: *Decision* (what
was decided), *Why* and *Alternatives* (what was discarded, and why).
Like any text, a record links to what it is about, and states what is
decided now rather than when it was decided.

Records are not append-only. When a decision changes, its record is
updated in the same change, and the choice it replaces moves into
*Alternatives*, saying why it was dropped.

## Why

A record is text like any other, so its links are checked by
[`nexus lint`](nexus:nexus.lint): a superseded record kept around would
keep linking to code that no longer exists, and fail the lint. Updating
the record keeps it true; moving the replaced choice into *Alternatives*
keeps the reason for changing course in the text, where it is read,
rather than only in version control, where it is not.

## Alternatives

- **Append-only architecture decision records**, superseded by new ones
  instead of edited, in an `adrs` directory: keep the whole history in
  the text, but superseded records either fail the lint or have to be
  exempted from it, the one way text goes stale unnoticed. The directory
  was renamed once the records no longer followed that format.
- **Recording decisions in the agent instructions**: one file to read,
  but it grows with every decision, and a decision about the repository
  is not an instruction for working in it.
- **Recording decisions only in commit messages**: nothing to keep up to
  date, but nobody finds them when reading the code they are about.
