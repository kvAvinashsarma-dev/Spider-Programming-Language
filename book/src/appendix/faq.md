# FAQ

## Is Spider implemented?

Partially. The parser, checker, formatter, VM, CLI, local package manager, and
LSP are implemented. Native build, public registry, real concurrency, and docs
generation are future work.

## Is Spider English?

No. Spider uses readable words but has a strict grammar.

## Does Spider have null?

No current null value exists. Use `Maybe`.

## Does Spider use exceptions?

Expected failures use `Outcome`. Runtime panics exist for bugs such as
divide-by-zero or out-of-range indexing.

## Can packages touch files by default?

No. File APIs require the `fs` capability.

## Does `spider build` work?

No. It is a roadmap item.
