# Lynk reads Nix with rnix

## Decision

The `nix` provider of [lynk](lynk:lynk.self) parses a Nix file with
the [`rnix`](lynk:cargo.dependency (Cargo.toml)) crate
([`lynk::providers::nix`](lynk:rust.module)) and reads its syntax
tree, evaluating it without Nix. Its one selector names a command of a
development environment the way a reader types it, by the name it has on
`PATH`, and resolves to where it is built
([`lynk::providers::nix::find_dev_shell_command`](lynk:rust.item)).

A dev shell is either a call to one of the functions building one
([`lynk::providers::nix::DEV_SHELL_FUNCTIONS`](lynk:rust.item)),
selected under any attribute path, or an attribute of one of the
attributes holding dev shells
([`lynk::providers::nix::DEV_SHELL_ATTRIBUTES`](lynk:rust.item)), as
flake-parts modules declare them, written nested or with dotted paths
([`lynk::providers::nix::is_dev_shell_input`](lynk:rust.item)). What it
puts on `PATH` is the value of its inputs
([`lynk::providers::nix::DEV_SHELL_INPUTS`](lynk:rust.item)).

Those values are evaluated by a small evaluator of the file alone
([`lynk::providers::nix::evaluate`](lynk:rust.module)): names are
followed to the `let` bindings, recursive attribute sets and function
arguments binding them, functions written in the file are applied, lists
joined with `++` are joined, strings are interpolated, and the list
functions of `builtins` and nixpkgs' `lib` that commands are usually
built with are applied as Nix applies them
([`lynk::providers::nix::evaluate::Builtin`](lynk:rust.item)). So a
command named `"check-${target}"` in a function mapped over a list of
names written in the file is found under each of its names. Anything else,
such as what a file imports or what a flake's inputs hold, is unknown, and
what is unknown builds no command.

A command is built where a function the file does not define is applied
to its name, given as a string or as the `name` of an attribute set,
however many arguments the function takes
([`lynk::providers::nix::evaluate::Value::Built`](lynk:rust.item)). That
covers the shell script builders of nixpkgs without naming them. A
command spans from the name of the binding holding it to the end of its
value, or covers the application alone when it is written inline, such
as in a list.

## Why

[`lynk lint`](lynk:lynk.lint) runs before every change, so reading a
flake must not need Nix, the network or a lock file; evaluating with Nix
would need all three, and gives values rather than where they are
written, which the spans need. `rnix` keeps where every node is written.
Text naming a command is about what the reader runs, so the link names
the command, and checking that a dev shell lists it makes removing a
command from the environment fail the lint even when its binding stays
behind.

Development environments generate their commands: one per machine, per
target, per environment, built by a function mapped over a list. Reading
only bindings listed by name misses them, so the provider evaluates as
much as the file itself decides. Stopping at the file keeps the
evaluation fast and its answer the same everywhere, and what it cannot
know it leaves out rather than guessing.

## Alternatives

- **`nix eval`** of the dev shell's inputs: exact, but needs Nix, fetches
  the flake's inputs, is slow, and gives no positions in the file.
- **Reading only bindings a dev shell lists by name**, as the provider
  first did: no evaluator to maintain, but misses commands built inline
  in the list or generated from a list of names, and flake-parts dev
  shells, which are attributes rather than calls.
- **Naming the binding instead of the command**: simpler, but the text
  names what is typed, which only usually equals the binding's name.
- **Recognising the script builders by name**, such as
  `writeShellScriptBin`: stricter, but every other builder taking a name
  would need adding.
- **A selector for any binding**, without the dev shell: links anything
  in a Nix file, but would stay silent when a command leaves the
  environment.
