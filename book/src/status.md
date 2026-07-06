# Project Status

The repository is versioned as `0.1.0` and contains milestones M1 through M5
shipped, with M6 partially shipped.

## Implemented

- Lossless lexer, parser, CST, typed AST views, and formatter.
- Semantic checker with name resolution, type inference, choices, records,
  generics, pattern matching, diagnostics, capabilities, and project modules.
- Silk register bytecode compiler and VM.
- `spider run`, `spider test`, `spider new`, `spider repl`, `spider fmt`,
  `spider check`, `spider tree`, `spider tokens`, `spider explain`,
  `spider learn`, and `spider lsp`.
- Local `web` package manager with `install`, `publish`, `audit`, and `remove`.
- Safe Mode capability model for `fs`, `net`, `env`, `exec`, and `ai`, with
  `files` implemented behind `fs`.
- LSP diagnostics, hover, and completion.

## Not Implemented Yet

- `spider build` native compilation.
- `spider doc`.
- `spider package`.
- Public network registry, signing, namespaces, and transitive package
  dependency resolution.
- Full concurrency semantics. `do together` and `spawn` parse; runtime behavior
  is sequential until M7.
- AOT backend and WASM target as production outputs.
- Lexer-level interpolation tokens. Current interpolation is scanner based.
- Complete website documentation generated from this book.

## Source of Truth

The code paths used for this book are:

- `crates/spider-syntax`
- `crates/spider-hir`
- `crates/spider-silk`
- `crates/spider-fmt`
- `crates/spider-web`
- `crates/spider-lsp`
- `crates/spider-playground`
- `crates/spider-cli`
- `corpus`
- `docs`

