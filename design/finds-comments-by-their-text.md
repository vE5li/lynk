# Nexus finds comments by their text

## Decision

A `rust.comment` link names a comment by text it holds, the link's text,
in one Rust file or in every Rust file
([`nexus::providers::rust::comments::find_comments`](nexus:rust.item)). A
comment is a run of comment lines on consecutive lines, the unit the Rust
format already reads as Markdown, and is read as one text: its whitespace
is collapsed, so the text may spread over its lines as a link's text may
([`nexus::providers::rust::comments::collapse_whitespace`](nexus:rust.item)).
The links in a comment are left out of its text
([`nexus::providers::rust::comments::outside_links`](nexus:rust.item)), so
a link never matches the comment it is written in.

[`nexus::providers::rust::comments::tests::finds_the_comments_holding_the_text_across_their_lines`](nexus:rust.item)
and
[`nexus::providers::rust::comments::tests::a_link_does_not_match_its_own_comment`](nexus:rust.item)
hold the matching to these rules.

## Why

Some decisions are carried out inside a function, by one branch or one
statement, and the comment explaining that spot is the only thing there
to link to. A comment has no name, but its words are what a reader would
quote, so the link reads as a quotation, and rewording or deleting the
comment, usually a sign the code under it changed, fails the lint.

Collapsing whitespace keeps reflowing a comment from breaking links to
it. A link written in a comment states the text it names, so without
leaving links out, every such link would also match itself.

## Alternatives

- **Line numbers**: precise, but stale with every edit above them.
- **Anchors written into comments**, a name for the link to use: survive
  rewording, but are one more name to keep, and a comment can be reworded
  until it no longer explains what its anchor promises without failing
  the lint.
- **Matching a comment's whole text**: stricter, but any edit to the
  comment, even one keeping the part a link relies on, would fail every
  link to it.
