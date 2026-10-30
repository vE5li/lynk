lynk-client
============

A Rust client of [lynk](lynk:lynk.self): it runs the command line and
reads what it prints, offering in Rust what
[`resources/lynk.wit`](lynk:filesystem.file) offers as a component.

Usage
-----

The crate is not published; depend on it by path:

```sh
# from the root of this repository
cargo add --path resources/lynk-client
```

A workspace is opened by its repository's root directory, and each
command run on it returns what lynk printed, read back into spans and
problems.

## Links ##

A link is passed as its `<provider.selector>` and its arguments, unquoted,
as on the command line lynk's README describes under
[Commands](lynk:markdown.section (README.md)).

### Errors

When lynk cannot run, or its command fails, the error is what it printed
on standard error. A link that points at nothing is not an error: finding
nothing is an empty list.

## The `PATH` it runs lynk from

It runs the first `lynk` on the `PATH`, which must be recent enough to
take a root directory.

## Spans, *as lynk counts them*

Spans are read as lynk prints them, with lines and columns counted as
[`lynk::location`](lynk:rust.module) counts them. A problem's message is the
lint's own, unchanged.
