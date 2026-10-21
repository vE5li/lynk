nexus-client
============

A Rust client of <[nexus.self]>: it runs the command line and reads what
it prints, offering in Rust what <[filesystem.file resources/nexus.wit]>
offers as a component.

Usage
-----

The crate is not published; depend on it by path:

```sh
# from the root of this repository
cargo add --path resources/nexus-client
```

A workspace is opened by its repository's root directory, and each
command run on it returns what nexus printed, read back into spans and
problems.

## Links ##

A link is passed as its `<provider.selector>` and its arguments, unquoted, as
on the command line <[markdown.section README.md "Commands"]> describes.

### Errors

When nexus cannot run, or its command fails, the error is what it printed
on standard error. A link that points at nothing is not an error: finding
nothing is an empty list.

## The `PATH` it runs nexus from

It runs the first `nexus` on the `PATH`, which must be recent enough to
take a root directory.

## Spans, *as nexus counts them*

Spans are read as nexus prints them, with lines and columns counted as
<[rust.module nexus::location]> counts them. A problem's message is the
lint's own, unchanged.
