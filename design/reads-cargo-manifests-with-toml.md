# Lynk reads Cargo manifests with toml

## Decision

The `cargo` provider of [lynk](lynk:lynk.self) parses a Cargo manifest
with the [`toml`](lynk:cargo.dependency (Cargo.toml)) crate's spanned
tables ([`lynk::providers::cargo`](lynk:rust.module)), without asking
Cargo to resolve it. A dependency is named by the key it is declared
under, as the code using it writes it
([`lynk::providers::cargo::find_dependency`](lynk:rust.item)), and is
looked up in every table Cargo reads dependencies from: the package's
own, those of each platform under `target`, and the workspace's
([`lynk::providers::cargo::dependency_tables`](lynk:rust.item)). A name declared in more than one of
them, once for the code and once for its tests, or in the workspace and
inherited from it, matches each declaration, so a link to it is
ambiguous and fails the lint.

A dependency spans from its key, or the header of its own table, to the
end of its last value, so it covers the whole declaration whichever way
TOML writes the table: inline, under a header, or with dotted keys
([`lynk::providers::cargo::extent`](lynk:rust.item)).

## Why

[`lynk lint`](lynk:lynk.lint) runs before every change, so reading a
manifest must not need Cargo, the network or a lock file. `toml` is
already what the `rust` provider reads a crate's name with
([`lynk::providers::rust::items::crate_name`](lynk:rust.item)), and its
spanned tables keep where every key and value is written, so no other
parser is needed. A table's own span is only its header or first key,
which is why a dependency's span is grown over the keys inside it.

## Alternatives

- **`cargo metadata`**, run as a process: resolves the manifest as Cargo
  does, but needs Cargo installed, may fetch the registry, and gives no
  positions in the manifest.
- **`toml_edit`**: keeps formatting and spans too, but is a second TOML
  parser next to the one already used.
- **Naming a dependency by its package**, the `package` key of a renamed
  dependency: what the registry calls it, but not what the code and the
  rest of the manifest call it.
- **A selector per table**: would tell a test's dependency from the
  code's, and could link a name declared twice, but needs the table in
  every link, though most names are declared once.
