# Lynk compares literals by their value

## Decision

A `rust.literal` link names a constant or static by its path and states
its value as the link's text, so the text is what the reader sees and what
the lint checks
([`lynk::providers::rust::items::find_values`](lynk:rust.item)). Only an
item whose value is a literal, maybe negated, has a value to state
([`lynk::providers::rust::literals::Literal::of_expression`](lynk:rust.item)).

Values are compared, not how they are written
([`lynk::providers::rust::literals::Literal::matches`](lynk:rust.item)):
the text is read as a Rust literal when it is one, and as plain text
otherwise
([`lynk::providers::rust::literals::Literal::written`](lynk:rust.item)).
A value is one of these:

<!-- lynk rust.map-variant lynk::providers::rust::literals::Literal -->
- Text: a string, byte string, C string, character or byte, by its
  content, stated as it is or written as a Rust literal, so prose shows
  `TYPESAFE_API_KEY`, not its quotes.
- Integer: by its base-10 digits, so `16` states `0x10` and `1_6usize`.
- Float: by its value, which an integer may state, so `2` states `2.0`.
- Bool: `true` or `false`.
<!-- /lynk -->

[`lynk::providers::rust::literals::tests::matches_the_value_however_it_is_written`](lynk:rust.item)
holds the comparison to these rules. [`lynk where`](lynk:lynk.where)
resolves a literal like an item, to its definition, and only
[`lynk lint`](lynk:lynk.lint) compares the value.

## Why

Prose states values the way a reader thinks of them, in base 10 and
without quotes or suffixes; a link that had to copy the source's notation
would read like code, and would break when the code changes how it writes
a value without changing the value.

A constant has a name to link it by and one value. A literal elsewhere,
inside a function, has neither: a function holds many literals, and any of
them may move. Values computed by an expression would need the compiler
to evaluate them, which the lint does not run.

## Alternatives

- **Comparing the source text**: no parsing, but `0x10` and `16` would
  differ, and every value would have to be written as Rust writes it,
  quotes and escapes included.
- **Any literal inside an item**, such as a key bound in a function:
  reaches values that are not constants, but a link could match any of
  the function's literals, and a value repeated in it is ambiguous.
- **Evaluating constant expressions**: would cover computed values, but
  needs type information and a build, which the lint avoids.
