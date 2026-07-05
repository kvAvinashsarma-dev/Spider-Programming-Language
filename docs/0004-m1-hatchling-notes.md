# Milestone M1 "Hatchling" — Architecture Notes

| Field    | Value                                   |
|----------|------------------------------------------|
| Document | 0004 — M1 milestone notes                |
| Status   | Shipped 2026-07-05                       |
| Scope    | Lexer, error-tolerant parser, lossless CST, typed AST views, `spider fmt`, golden-file harness, fuzz smoke tests, CLI (`fmt`/`check`/`tree`/`tokens`/`explain`) |

## 1. What shipped

```
crates/
├── spider-syntax    lexer → tokens (trivia included) → error-tolerant
│                    recursive-descent parser → lossless CST → typed AST views;
│                    diagnostics with the Spider Explain database
├── spider-fmt       canonical formatter (zero options), refuses broken files
└── spider-cli       the `spider` binary: fmt, check, tree, tokens, explain
corpus/
├── ok/              11 valid files covering the LDD §3 tour; .tree.txt and
│                    .fmt.sp golden snapshots
└── err/             3 broken files; .diag.txt and .tree.txt snapshots
```

Zero external dependencies, by decision: the syntax crate must build offline
and keep the toolchain supply chain auditable (SRS NFR-1/2). We accept
re-implementing small things (green trees, a test harness) as the price.

## 2. Architecture decisions made during implementation

- **Trivia-in-tree losslessness.** Every byte of input — whitespace, comments,
  broken tokens — is a token in the CST. `root.text() == source` is asserted
  for *every* corpus file and *every* fuzz input, including garbage. This is
  the foundation the IDE (M6) and refactoring engine stand on.
- **Indentation via zero-width `Indent`/`Dedent` tokens**, Python-style stack
  in the lexer. Newlines inside brackets are trivia, so multi-line calls and
  literals need no continuation syntax.
- **Error recovery is per-line.** A broken statement becomes an `ErrorNode`
  ending at the line boundary; parsing resumes on the next line. All
  independent errors are reported in one pass, sorted by position (FR-2).
- **Recursion depth is capped (128)** in expressions, types, and patterns, so
  `((((((…` is a friendly `E0150` instead of a stack overflow. Verified by
  the pathological-nesting fuzz test.
- **The formatter refuses files that don't parse.** A formatter that rewrites
  broken code destroys the context the user needs to fix it. `spider check`
  first, then `spider fmt`.
- **Formatter laws are tests, not intentions:** idempotence
  (`fmt(fmt(x)) == fmt(x)`) and reparse-cleanliness are asserted over the
  whole corpus on every CI run.

## 3. Design findings — feed into M2 (honest report)

Implementing the grammar surfaced four real issues in the LDD sketches:

1. **Reserved-word collisions.** LDD §3.6 itself wrote `fn area(shape: Shape)`
   — but `shape` is a keyword, so that line does not parse. The same trap
   exists for `times`, `to`, `of`, `is`, `do`, `together`, `test`, `record`,
   `choice` — all common English identifiers. **M2 must decide:** contextual
   keywords (recommended: they are only special in statement-head or
   type-operator position) or a shorter reserved list. Corpus uses `figure`
   for now. This is exactly the kind of finding milestone-gated development
   exists to catch early.
2. **Paren-less method arguments.** LDD §3.11 sketched `helper.ask "text"`.
   Allowing paren-less arguments on arbitrary methods creates real grammar
   ambiguity. M1 requires parens on method calls; the built-in `ask expr`
   prefix form is unaffected. Revisit at M2 with the `ai` design (or amend
   the LDD example to `helper.ask("…")`).
3. **Match arms and statements.** `Fail(problem) -> say "…"` from LDD §3.8
   requires arms to accept a `say` result, not just expressions. M1 grammar:
   arm result = expression, optionally prefixed with `say`. M2 should decide
   whether arms may take full statements or indented blocks.
4. **`channel of Text` as an expression** (LDD §10) uses a type inside
   expression position. Deferred to the M7 concurrency design; not in the M1
   grammar.

Also deferred, tracked for M2: tokenizing `{…}` string interpolation
(currently one `StrLit` token — lossless, but M2's name resolution needs the
inner expressions), Unicode identifiers, negative-literal patterns.

## 4. Test inventory

| Suite | What it proves |
|---|---|
| `spider-syntax` unit tests (12) | lexer edge cases (indent pairing, bracket-newline trivia, numbers, tabs, unterminated strings), parser losslessness, recovery, one-pass error ordering, precedence |
| `tests/golden.rs` (3) | tree snapshots for ok+err corpus; diagnostic code/position snapshots; **every emitted code has an Explain entry** |
| `tests/fuzz_smoke.rs` (3) | 5 000 random inputs + pathological nesting: never panics, always lossless |
| `spider-fmt` unit tests (4) | spacing/indent normalization, refusal of broken files, empty-file behavior, idempotence |
| `tests/golden_fmt.rs` (1×11 files) | formatter goldens + idempotence law + reparse-clean law |

Regenerate snapshots with `SPIDER_UPDATE_GOLDEN=1 cargo test`.

## 5. Exit criteria status (SRS §11, M1 row)

- ✅ Formats & round-trips full syntax corpus — 11 ok-files, goldens frozen.
- ⏳ "Fuzzing finds no parser panics in 24 h" — the deterministic 5 000-case
  smoke suite passes locally and in CI; the 24-hour continuous run needs CI
  infrastructure (cargo-fuzz on a nightly runner) that lands with the M2 CI
  hardening. Flagged, not forgotten.

## 6. Try it

```
cargo build --workspace
.\target\debug\spider.exe check corpus\err\err_expr.sp   # Spider Explain errors
.\target\debug\spider.exe fmt <file.sp>                  # canonical formatting
.\target\debug\spider.exe tree corpus\ok\shapes.sp       # CST dump
.\target\debug\spider.exe explain E0121                  # error database
```

## 7. Next: M2 "Spinneret"

Name resolution, type inference core, the diagnostics engine generalized, the
top-50 error codes authored, and rulings on the four design findings above.
