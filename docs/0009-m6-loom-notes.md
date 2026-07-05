# Milestone M6 "Loom" — Architecture Notes (PARTIAL)

| Field    | Value                                     |
|----------|--------------------------------------------|
| Document | 0009 — M6 milestone notes                  |
| Status   | Partially shipped 2026-07-05 — see §3      |
| Scope    | Language server (diagnostics, hover, completion), Learn Mode concept database, `spider learn` |

## 1. What shipped

- **`spider lsp`** — the language server, stdio + Content-Length framing,
  zero dependencies (hand-rolled JSON, ~200 lines, tested):
  - **Live diagnostics** on open/change: the same parse → resolve → infer
    pipeline as `spider check`, published per keystroke with stable codes
    and the full teaching messages ("did you mean `total`?" appears inside
    the editor squiggle).
  - **Teaching hovers**: hovering any keyword explains it in one plain
    sentence (the Learn Mode concept database, 34 entries); hovering a
    stdlib name shows its registry doc — the same single source of truth
    as `spider explain` and the doc-coverage gate.
  - **Completion**: keywords (each carrying its concept doc) and stdlib
    members.
  - The protocol is tested without an editor: `handle_message` is pure;
    tests drive initialize → didOpen (diagnostics with suggestion) →
    didChange (diagnostics clear) → hover → completion → shutdown/exit.
  - Wiring: any LSP-capable editor → command `spider lsp`, language id
    `spider`, extension `.sp`.
- **Learn Mode** (`spider learn file.sp`): lists every construct a file
  uses with its one-line explanation — a lesson plan generated from code.
  The same database powers hovers, so the terminal and the editor teach
  identically.

## 2. Exit criteria status (SRS §11, M6 row)

- ✅ *Editor demo: live errors + completion* — live errors and completion
  are protocol-tested end to end; hover exceeds the criterion.
- ❌ *Playground runs offline* — **not shipped.** Requires the
  wasm32-unknown-unknown build of Silk plus a web shell; deferred with the
  M8 WASM target where that toolchain work lands once, not twice.

## 3. Honestly deferred (M6 remains open)

- **Playground (WASM)** — with M8, as above.
- **Kids Mode** — our diagnostics are already written to the ten-year-old
  test; a separate mode without classroom evidence would be a flag, not a
  feature. Needs the M6 classroom testing round (OQ-8 also waits on it).
- **Lexer-level interpolation tokens** (third deferral, tracked): the
  quote-in-hole limitation stands; the LSP's in-string completions want
  this too. It is the first work item when M6 reopens.
- Cross-file LSP: the server checks single files today; project-aware
  diagnostics reuse `check_project` when the loader learns incremental
  reloading.

## 4. Tests: 95 green

New: 6 spider-lsp tests (JSON round-trip; initialize capabilities;
didOpen/didChange diagnostics cycle; hover teaching; completion content;
shutdown/exit protocol).
