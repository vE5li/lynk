# Lynk maps a provider's selectors with a star

## Decision

Any provider of [lynk](lynk:lynk.self) can be linked as `<provider>.`
followed by
[`*`](lynk:rust.literal (lynk::providers::EVERY_SELECTOR)),
a complete map of its selectors (the README's
[Link syntax](lynk:markdown.section (README.md))), checked as every
complete map is
([`lynk::providers::check_complete_map`](lynk:rust.item)). It is
syntax rather than a selector: it is not listed among the selectors, and
it asks nothing of its descriptions. Text exercising or describing every
selector of a provider, like
[`design/lints-its-providers-against-resources.md`](lynk:filesystem.file),
uses it so that adding a selector fails [`lynk lint`](lynk:lynk.lint) until the text follows.
Nothing requires a provider to be mapped anywhere.

Bodies may nest lists and links, so the links exercising a selector sit
under its entry.

## Why

The map of every selector in the README fails when a selector is added,
but only there; the text that exercises a provider's selectors stayed
silent. A star reads as "all selectors" where it is used most, in
documentation.

## Alternatives

- **A `map-selector` selector per provider**: follows the naming of map
  selectors and needs no quoting from the shell, but adds a selector to
  every provider, each of which the maps of selectors would then have to
  list.
- **Requiring each entry to link to its selector**, or every provider to be
  mapped: guarantees more, but turns a reminder into a rule about how
  documentation is written.
