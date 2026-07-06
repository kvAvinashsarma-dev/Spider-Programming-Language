# Validation Report

Generated from repository inspection on 2026-07-06.

## Build Verification

- `cargo test --workspace`: passed.
- `cargo install mdbook`: completed, installing mdBook 0.5.3.
- `mdbook build book`: passed.
- HTML output path: `book/book`.

## Counts

- Chapters in this mdBook: 35.
- Spider code examples written in this book: 70+.
- Exercises and challenge prompts: 25.
- Standard-library registry entries documented: 39.
- Bytecode instructions documented: 35.

This first generated book is not yet a 400-600 page manuscript and does not
yet contain 600 examples or 300 exercises. It is a complete publishable
documentation skeleton with substantive content for every implemented area,
ready for expansion as the language grows.

## Verified Implemented CLI Commands

- `spider run`
- `spider test`
- `spider new`
- `spider repl`
- `spider lsp`
- `spider learn`
- `spider fmt`
- `spider check`
- `spider tree`
- `spider tokens`
- `spider explain`
- `web install`
- `web publish`
- `web audit`
- `web remove`

## Undocumented APIs

No current stdlib registry entries are intentionally omitted from the standard
library chapter.

## Undocumented Compiler Features

The parser and checker include internal recovery details and many diagnostics
not expanded one by one in this first pass. The diagnostics chapter names the
major families and runtime codes.

## Undocumented VM Instructions

No current `Instr` variants are intentionally omitted from the bytecode
reference.

## Known Discrepancies

- Roadmap commands `spider build`, `spider doc`, and `spider package` are not
  implemented.
- `break` and `continue` are requested in the original documentation brief but
  are not implemented.
- Sets, tuples, traits, interfaces, closures, anonymous functions, native AOT,
  real concurrency, HTTP server APIs, JSON APIs, regex APIs, env APIs, exec
  APIs, and AI APIs are not implemented in the current code.
- `shape` parses but does not yet provide full trait/interface semantics.
- Website documentation, PDF, EPUB, and DOCX outputs require additional build
  tooling beyond this mdBook source.
