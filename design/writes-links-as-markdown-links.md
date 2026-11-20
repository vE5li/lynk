# Lynk writes links as Markdown links

## Decision

A link is a Markdown link whose destination is
[`lynk:`](lynk:rust.literal (lynk::formats::markdown::links::SCHEME))
followed by `<provider>.<selector>`, as the
README's [Link syntax](../README.md#link-syntax) describes.
Markdown decides where a link starts and ends, and what is code, as
[`pulldown-cmark`](lynk:cargo.dependency (Cargo.toml)) parses it
([`lynk::formats::markdown::parse`](lynk:rust.module));
[lynk](lynk:lynk.self) only reads the selector name from the
destination and hands the selector the link's title and text
([`lynk::formats::markdown::links`](lynk:rust.module)). The text is
the selector's last argument and the title holds those before it, so
every argument is written once, and what the reader sees is the name of
the target ([`lynk::link::Link::read`](lynk:rust.item)). A selector
taking no arguments ignores the text.

In Markdown files, a link whose destination is a path rather than a URL
is read as the selector conventional renderers would follow it to: a file,
a directory, or a section by its heading or its anchor, the path relative
to the file the link is written in
([`lynk::formats::markdown::links::find_path_links`](lynk:rust.item)).
Where such a link can name the target, it is preferred over a `lynk:`
link ([Plain links first](../README.md#plain-links-first)). Its syntax
stays when exported, since it works as a link anywhere. Source code
comments are left out: their tools give destinations meanings of
their own, such as rustdoc's paths to items.

A map is a plain Markdown list between two HTML comments, the opening one
holding [`lynk`](lynk:rust.literal (lynk::formats::markdown::links::MAP_KEYWORD)),
the selector and its arguments, the closing one
[`<!-- /lynk -->`](lynk:rust.literal (lynk::formats::markdown::links::MAP_CLOSING))
([`lynk::formats::markdown::links::find_maps`](lynk:rust.item)). Its
body stays prose, read the
same way by every selector that takes one: quotes mean nothing in it, code
hides comments, and nested links are checked like any other. The map below
follows these rules; changing how links are read breaks this record until it
is updated:

<!-- lynk filesystem.map-entry
  resources -->
- lynk-client: a client of the command line; neither
  `<!-- /lynk -->` in code nor a lone " ends or quotes anything here
- lynk.proto: what a link wrapped inside its title, such as
  [`LintResponse.Problem`](lynk:protobuf.message
    (resources/lynk.proto)), checks against
- lynk.wit: what a nested link such as
  [`context`](lynk:wit.function (resources/lynk.wit)) checks against
<!-- /lynk -->

## Why

Links are read far more often rendered than as source. A syntax of our
own showed up verbatim in every renderer, selector names and paths in the
middle of sentences; a Markdown link renders as the word it links, and the
comments around a map disappear. Markdown already knows where a link
ends, across line breaks, inside lists and in source comments read as
Markdown, so wrapping a link like the prose around it, as formatters do,
needs no rule of ours.

Plain path links are what editors complete, renderers follow and other
tools write; checking them too means a repository's existing links go
stale no more silently than lynk's own, without rewriting them.

## Alternatives

- **The former `<[<provider>.<selector> <arguments>]>` syntax**: needed
  only a closing delimiter, but rendered as raw text, and its parser had
  to define its own rules for code, nesting and quotes.
- **Arguments in the destination**, as a path or query string: Markdown
  destinations cannot hold spaces unless wrapped in `<>`, and quoting
  headings into URLs would make them unreadable.
- **A free link text, with every argument in the title**: lets the text
  read naturally in any sentence, but writes the target's name twice
  whenever the text is that name, which is most of the time.
- **Leaving path links unchecked**: keeps one way of writing a link, but
  leaves the links most repositories already have to rot unnoticed.
- **Telling files from directories by the filesystem** when reading a
  link: would accept `[design](design)`, but reading a file would depend
  on others, and the link's meaning on what happens to exist.
- **Maps as links**, their body in the link text: Markdown link text
  cannot hold a list.
