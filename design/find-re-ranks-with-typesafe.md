# Lynk find re-ranks with TypeSafe

## Decision

[`lynk find`](lynk:lynk.find) picks the
[`30`](lynk:rust.literal (lynk::relevance::SHORTLIST_LENGTH)) best word
matches, then asks TypeSafe's model
[`jev-latest`](lynk:rust.literal (lynk::relevance::MODEL)), in one
request, whether each of them is relevant to the
task ([`lynk::relevance::relevance`](lynk:rust.item)), and orders them
by that probability ([`lynk::relevance::re_rank`](lynk:rust.item)). It
does so only when
[`TYPESAFE_API_KEY`](lynk:rust.literal (lynk::relevance::API_KEY_VARIABLE))
is set, and keeps the
word ranking when it is not or the request fails. The request is made with
[`ureq`](lynk:cargo.dependency (Cargo.toml)).

## Why

Words alone rank a section high for sharing words with the task, even when
it is about something else, and long plans share words with anything. On
the first tasks tried, the relevant sections were among the word matches
but badly ordered; judged by relevance, they came first and the sections
merely sharing words dropped to the bottom. One request for thirty
sections took under half a second.

## Alternatives

- **Words only**: no service and no key, but the order is often wrong. It
  remains the fallback.
- **Embeddings**: they find sections meaning the same as the task without
  sharing its words, which no re-ranking of word matches can, but they rank
  by similarity rather than relevance and need a model and an index kept in
  sync with the documentation. Missing sections was not the problem seen;
  if it shows up, embeddings join the word matches as a second source of
  candidates, still re-ranked.
- **A general language model**: could judge relevance too, but is slower
  and costlier per section, and answers in text to be parsed instead of a
  probability.
