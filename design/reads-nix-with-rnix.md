# Lynk reads Nix with rnix

## Decision

The `nix` provider of [lynk](lynk:lynk.self) parses a Nix file with
the [`rnix`](lynk:cargo.dependency (Cargo.toml)) crate
([`lynk::providers::nix`](lynk:rust.module)) and reads its syntax
tree, evaluating it without Nix. One selector names a command of a
development environment the way a reader types it, by the name it has on
`PATH`, and resolves to where it is built
([`lynk::providers::nix::find_dev_shell_command`](lynk:rust.item)).
Another names what `nix run` runs by the name it is given. The others
name an attribute of a file by its attribute path, or map the attributes
under one.

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

An attribute's path is read from the syntax alone, never evaluated
([`lynk::providers::nix::definitions`](lynk:rust.item)): the names of
the bindings it is nested in, as values of bindings, through parentheses
and `let` bodies, counted from the outermost attribute set or from the
`let` binding it ([`lynk::providers::nix::container_path`](lynk:rust.item)).
Nested and dotted bindings are the same path, and every binding writing
an attribute's last name defines it. The bindings counted from one
outermost attribute set build one value, as Nix merges them, so an
attribute split over several of them, as modules write them, is one
target spanning them all
([`lynk::providers::nix::find_attribute`](lynk:rust.item)), and maps
as one, in the order its attributes are first defined
([`lynk::providers::nix::attribute_names_under`](lynk:rust.item)).
The same path counted from different attribute sets, such as each one
given to a function merging them, is as many targets, and a link to it
is ambiguous.

What `nix run` runs is an attribute found the same way, under the first
of the flake outputs it tries that holds the name
([`lynk::providers::nix::RUNNABLE_OUTPUTS`](lynk:rust.item),
[`lynk::providers::nix::find_app`](lynk:rust.item)). Its path is read
as a flake writes it, under a system, or as the functions building one
system's outputs write it, without one, either possibly under the
attributes flake-parts writes outputs under
([`lynk::providers::nix::runnable`](lynk:rust.item)). A system is told
apart from a name by the kernel ending it
([`lynk::providers::nix::SYSTEM_KERNELS`](lynk:rust.item)), and a `let`
binds no output.

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

Text about configuration names attributes as they are written: the
variables of an environment, the users of a test realm, the servers of a
client's settings. Those are where a reader edits them, so an attribute
links to where it is written rather than to the option it becomes once
modules merge, which only evaluating every module could tell.

Text telling the reader to `nix run` something names it as they type
it, so the link names it the same way, and fails when it no longer runs
anything, whichever output used to hold it.

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
- **Linking commands with `nix.attribute`**, without the dev shell:
  links anything in a Nix file, but names the binding rather than the
  command, and stays silent when a command leaves the environment.
- **Evaluating attribute paths**, as the dev shells' inputs are: would
  follow attribute sets passed to functions such as `lib.mkMerge`, but
  a path would then name a value rather than where it is written, and
  links could not tell which binding of a merged attribute set they
  point at.
- **Linking apps with `nix.attribute`**: needs the output and the
  system in the path, which the reader never types, and fails when the
  name moves to another output `nix run` still finds it in.
- **Paths counted from the file's root expression only**: one path per
  attribute, but `let` bindings and the attribute sets of a module's
  function body would need prefixes no reader writes.
