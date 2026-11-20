# Working with a lynk repository

A repository whose documentation and code comments link to the code with
[lynk](lynk:lynk.self), this one included, follows these rules.

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
about specific parts of the code, it uses a *link* that
[lynk](lynk:lynk.self) checks against the code, so moving, renaming,
adding or removing what it refers to fails the lint until the text is
updated. The link syntax and every selector are defined in the README's
[Link syntax](README.md#link-syntax) and
[Selectors](README.md#selectors) sections. In Markdown files, a plain
path link is preferred over a `lynk:` link wherever it can name the
target, as [Plain links first](README.md#plain-links-first) describes.

## Following links

Run as `lynk` from anywhere inside the repository. Besides
[`lynk lint`](lynk:lynk.lint), which checks every link, it navigates
links in both directions, each command taking a `<provider.selector>` and
its arguments:

- [`lynk resolve`](lynk:lynk.resolve) resolves a link to the spans it
  points at, with their first line.
- [`lynk backlinks`](lynk:lynk.backlinks) lists every place linking to
  that target.

On the command line nothing is quoted, and a file left out matches every
file. For example, [`lynk backlinks`](lynk:lynk.backlinks) with
`filesystem.file README.md` lists everything that refers to the README.

Before starting a task, run [`lynk context`](lynk:lynk.context) with a few
words describing it (`lynk context comment parsing`) and read what it lists:
the documentation sections, decisions and comments most related to them,
best first. Unlike links, it matches words, so it also finds what nobody
linked yet. With `TYPESAFE_API_KEY` set, its best word matches are ordered
by how relevant they are to the task, shown as a probability; keep the key
out of version control.

Before changing, moving or removing anything, run
[`lynk backlinks`](lynk:lynk.backlinks) on it and update the text at
every place it lists, in the same change. Follow links with
[`lynk resolve`](lynk:lynk.resolve) instead of searching for names. Run
[`lynk lint`](lynk:lynk.lint) before finishing any change. Then pipe
`jj diff --git` into [`lynk affected`](lynk:lynk.affected), reread
the text at every link it lists and update what the change made wrong,
before starting the next change with `jj new`.

## Adding selectors

Add a selector only when text being written needs it and no existing
selector fits: implement it in [lynk](lynk:lynk.self) and define it in
its README's [Selectors](README.md#selectors)
in the same change, and mention it when reporting the change so it gets
reviewed. Never add selectors in advance.

## Decisions

Decisions about the architecture or the tooling are recorded in
[`design`](design/), following the
[Decision](design/records-decisions-as-living-design-records.md#decision)
of the record about how they are recorded. When a decision changes,
update its record in the same change.
