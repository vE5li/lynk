# Nexus names Rust items by their paths

## Decision

The `rust` selectors of [nexus](nexus:nexus.self) name modules and
functions by their paths as Rust writes them, starting with the crate's
name as code using it writes it ([`nexus::providers::rust`](nexus:rust.module)): its
library's name when the
manifest gives one, otherwise its package's with dashes turned into
underscores. A method's path goes through its type or trait, as calling
it by its full path does.

## Why

A path is what a Rust programmer already reads and writes, and copies from
`use` declarations, error messages and rustdoc. One argument for the whole
path also leaves room for selectors taking more after it, like an argument
after its function.

## Alternatives

- **The package, then the path from the crate root**, as two arguments:
  names the package as its manifest does, but splits one name in two and
  reads like no Rust code.
- **Paths starting with `crate`**: as written inside the crate, but means
  a different crate depending on where the link is, and nothing on the
  command line.
