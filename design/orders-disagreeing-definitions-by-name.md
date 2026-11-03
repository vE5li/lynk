# Lynk orders disagreeing definitions by name

## Decision

A Rust enum or struct may be defined more than once at the same path, each
definition under its own `cfg`. A map of its members checks them against
every definition found, and lists them in the order of the definitions
when all of them list the same members in the same order. When they do
not, the members of all of them are ordered by their names' bytes
([`lynk::providers::rust::expected_members`](lynk:rust.item)), as a
directory's entries are, which the README's
[Selectors](lynk:markdown.section (README.md)) state for both Rust map
selectors.

This is the exception to the definition order of maps that
[`design/checks-maps-in-definition-order.md`](lynk:filesystem.file)
records.

## Why

Definitions disagreeing on their members, or on their order, have no order
in common: a member only one of them defines has no place among the others,
and two orders cannot both be followed. Ordering by name is a rule the
text can follow without knowing which definitions exist, and it changes
only when a member is added, removed or renamed.

## Alternatives

- **Following each definition, failing only when the list follows none**:
  keeps the source's order, but a list naming members of several
  definitions follows none of them, so it only helps when the definitions
  agree on their members anyway.
- **Following the members in the order they first appear across the
  definitions**: needs no new rule, but that order depends on which
  definition is read first, which nobody writing the text sees.
- **Not checking the order when definitions disagree**: never wrong, but
  the lint stays silent about exactly the targets hardest to keep in step.
