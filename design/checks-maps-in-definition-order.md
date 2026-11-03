# Lynk checks maps in definition order

## Decision

A complete map lists its target's members in the order the target defines
them, as the README's [Link syntax](lynk:markdown.section (README.md))
states, and [`lynk lint`](lynk:lynk.lint) reports a member listed out of
that order ([`lynk::providers::check_complete_map`](lynk:rust.item)).
Members that are not members or are listed twice are reported as such and
take no part in the order.

Of the members listed, the longest run already in order is kept, and each
other member is reported against the nearest kept member it belongs before
or after ([`lynk::providers::check_order`](lynk:rust.item)), so moving one
member reports that member alone.

The order is the one the source defines them in, except where the target
defines none:

- A directory's entries are ordered by their names' bytes
  ([`lynk::providers::filesystem::directory_entries`](lynk:rust.item)), so
  uppercase names come before lowercase ones.
- A Rust enum or struct defined more than once, its definitions
  disagreeing, has no order of its own;
  [`design/orders-disagreeing-definitions-by-name.md`](lynk:filesystem.file)
  says how its members are ordered.

A partial map, once there is one, keeps the same order.

## Why

A map reads like the code it lists: someone going from one to the other
finds each member in the same place. Reordering members in the code
usually means something, grouping them or ranking them, and the text
describing them should follow; an order the lint does not check drifts
away from the code unnoticed, like any prose about the code.

Reporting only the members outside the longest ordered run points at what
moved. Comparing each member with the one before it would report a whole
run of members when one member moved past it.

Byte order is what sorting names does without further choices; a
directory's entries rarely differ only in case, and the order can change
should that matter.

## Alternatives

- **Ignoring the order**: lists may be grouped as the text likes, but they
  drift from the code, and reordering the code goes unnoticed.
- **Ordered maps as their own selectors**, such as `ordered-map-<member>`:
  keeps unordered maps possible, but doubles the map selectors, and a map
  in any other order is one a reader has to search.
- **Ordering every map by name**: one rule for every target, but it
  ignores the order the code chose, which is usually the one to read in.
- **Ordering a directory's entries case-insensitively**, as many file
  browsers do: closer to how people sort, but needs a rule for names
  equal but for case.
