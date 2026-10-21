# Nexus find re-ranks with TypeSafe

## Decision

<[nexus.find]> picks the best word matches, then asks TypeSafe's model, in
one request, whether each of them is relevant to the task, and orders them
by that probability (<[rust.module nexus::relevance]>). It does so only when
`TYPESAFE_API_KEY` is set, and keeps the word ranking when it is not or the
request fails.

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
