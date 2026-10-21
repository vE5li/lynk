# Working with a nexus repository

A repository whose documentation and code comments link to the code with
<[nexus.self]>, this one included, follows these rules.

## Referring to the code base

Neither documentation nor code comments enumerate or name parts of the
code base in prose: no lists of messages, types, variants, files or
functions, and no "see `foo`". Such lists go stale the moment someone
changes the code without updating the text, and nothing notices.

The rule holds for everyday words too, not only identifiers. "It reads
Markdown, Rust and WIT files" lists the variants of an enum as surely as
naming them would, and "its four commands" goes stale just the same.
Whenever the items of a list or a count come from the code (languages,
formats, commands, options, directories), the prose either states the
rule they follow instead, or links to them.

Prose describes ideas, rules and reasons. When it has to say something
about specific parts of the code, it uses a *link* that <[nexus.self]>
checks against the code, so moving, renaming, adding or removing what it
refers to fails the lint until the text is updated. The link syntax and
every selector are defined in
<[markdown.section README.md "Link syntax"]> and
<[markdown.section README.md "Selectors"]>.

## Following links

Run as `nexus` from anywhere inside the repository. Besides
<[nexus.lint]>, which checks every link, it navigates links in both
directions, each command taking a `<provider.selector>` and its arguments:

- <[nexus.ref]> lists every place linking to that target.
- <[nexus.where]> resolves a link to the spans it points at, with their
  first line.

On the command line nothing is quoted, and a file left out matches every
file. For example, <[nexus.ref]> with `filesystem.file README.md` lists
everything that refers to the README.

Before starting a task, run <[nexus.find]> with a few words describing it
(`nexus find comment parsing`) and read what it lists: the documentation
sections, decisions and comments most related to them, best first. Unlike
links, it matches words, so it also finds what nobody linked yet. With
`TYPESAFE_API_KEY` set, its best word matches are ordered by how relevant
they are to the task, shown as a probability; keep the key out of version
control.

Before changing, moving or removing anything, run <[nexus.ref]> on it and
update the text at every place it lists, in the same change. Follow links
with <[nexus.where]> instead of searching for names. Run <[nexus.lint]>
before finishing any change.

## Adding selectors

Add a selector only when text being written needs it and no existing
selector fits: implement it in <[nexus.self]> and define it in its
<[markdown.section README.md "Selectors"]> in the same change, and
mention it when reporting the change so it gets reviewed. Never add
selectors in advance.

## Decisions

Decisions about the architecture or the tooling are recorded in a
top-level `adrs` directory (here <[filesystem.directory adrs]>), one short
file per decision, named after it, with three sections: *Decision* (what
was decided), *Why* and *Alternatives* (what was discarded, and why). Like
any text, an ADR links to what it is about. When a decision changes,
update its ADR in the same change; the history lives in version control.
