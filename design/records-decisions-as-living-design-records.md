# Lynk records decisions as living design records

## Decision

Decisions about the architecture or the tooling are recorded in a
top-level [`design`](lynk:filesystem.directory) directory, one short
file per decision, named after it, with three sections: *Decision* (what
was decided), *Why* and *Alternatives* (what was discarded, and why).
Like any text, a record links to what it is about, and states what is
decided now rather than when it was decided.

A record links to every part of the code it talks about, as precisely as
the selectors allow: the functions, modules and files that carry out the
decision, rather than only the directory they live in. The aim is to tie
the record so tightly to the code that changing either without the other
fails the lint. A record choosing a dependency links to where the
manifest declares it, so replacing or dropping the dependency fails the
lint as well. When no selector reaches what a record needs to name, the
record links to the nearest thing one does reach.

Records are not append-only. When a decision changes, its record is
updated in the same change, and the choice it replaces moves into
*Alternatives*, saying why it was dropped.

## Why

A record is text like any other, so its links are checked by
[`lynk lint`](lynk:lynk.lint): a superseded record kept around would
keep linking to code that no longer exists, and fail the lint. Updating
the record keeps it true; moving the replaced choice into *Alternatives*
keeps the reason for changing course in the text, where it is read,
rather than only in version control, where it is not.

The lint only notices what is linked. A record that describes code in
prose stays plausible while the code moves away from it; every link
turns a change to the code it names into a prompt to reread the record.

## Alternatives

- **Append-only architecture decision records**, superseded by new ones
  instead of edited, in an `adrs` directory: keep the whole history in
  the text, but superseded records either fail the lint or have to be
  exempted from it, the one way text goes stale unnoticed. The directory
  was renamed once the records no longer followed that format.
- **Linking only the directory or module a decision lives in**: fewer
  links to keep up to date, but the lint stays silent while the functions
  the record describes are renamed, removed or rewritten.
- **Recording decisions in the agent instructions**: one file to read,
  but it grows with every decision, and a decision about the repository
  is not an instruction for working in it.
- **Recording decisions only in commit messages**: nothing to keep up to
  date, but nobody finds them when reading the code they are about.
