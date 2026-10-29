# Nexus reads Nix with rnix

## Decision

The `nix` provider of [nexus](nexus:nexus.self) parses a Nix file with
the [`rnix`](nexus:cargo.dependency (Cargo.toml)) crate
([`nexus::providers::nix`](nexus:rust.module)) and reads its syntax
tree, evaluating nothing. Its one selector names a command of a
development environment the way a reader types it, by the name it has on
`PATH`, and resolves to the binding that builds it
([`nexus::providers::nix::find_dev_shell_command`](nexus:rust.item)).

A binding builds a command when its value applies a function to the
command's name, given as a plain string or as the `name` of an attribute
set, however many arguments the function takes
([`nexus::providers::nix::builds_command`](nexus:rust.item)). That covers
the shell script builders of nixpkgs without naming them. It counts only
when a dev shell lists the binding by name in what it puts on `PATH`
([`nexus::providers::nix::DEV_SHELL_INPUTS`](nexus:rust.item)), a dev
shell being a call to one of the functions building one
([`nexus::providers::nix::DEV_SHELL_FUNCTIONS`](nexus:rust.item)),
selected under any attribute path. Lists joined with `++` are read one by
one ([`nexus::providers::nix::dev_shell_inputs`](nexus:rust.item)).

A command spans from its binding's name to the end of its value.

## Why

[`nexus lint`](nexus:nexus.lint) runs before every change, so reading a
flake must not need Nix, the network or a lock file; evaluating would
need all three. Text naming a command is about what the reader runs, so
the link names the command, and checking that a dev shell lists it makes
removing a command from the environment fail the lint even when its
binding stays behind. `rnix` keeps where every node is written, which
the spans need.

## Alternatives

- **`nix eval`** of the dev shell's inputs: exact, but needs Nix, fetches
  the flake's inputs, and gives no positions in the file.
- **Naming the binding instead of the command**: simpler, but the text
  names what is typed, which only usually equals the binding's name.
- **Recognising the script builders by name**, such as
  `writeShellScriptBin`: stricter, but every other builder taking a name
  would need adding.
- **A selector for any binding**, without the dev shell: links anything
  in a Nix file, but no text needs that yet, and it would stay silent
  when a command leaves the environment.
