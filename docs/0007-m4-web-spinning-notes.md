# Milestone M4 "Web-spinning" — Architecture Notes

| Field    | Value                                      |
|----------|---------------------------------------------|
| Document | 0007 — M4 milestone notes                   |
| Status   | Shipped 2026-07-05                          |
| Scope    | Typed stdlib registry (100% documented), capability model enforced at check time and runtime, `files` module, `spider test` + `expect`, `--allow` flag, `web.toml` capability parsing |

## 1. What shipped

- **The stdlib registry** (`spider_hir::stdlib`) — one table describing every
  module function, constant, method, and builtin: signature, documentation,
  required capability. The checker types module calls against it (closing
  M2's "module members are unchecked" gap — `math.sqrt("nine")` is now
  E0211, `math.sqirt` is E0306 *at check time* with a "did you mean sqrt?"),
  the VM dispatches by it, and doc coverage is a test over it.
  **Exit criterion: ≥ 90% doc coverage → shipped at 100%** (39 entries),
  with a second test pinning the doc list to the dispatch table so they can
  never drift.
- **The capability model is real** (SRS FR-16/FR-21 groundwork):
  - `web.toml` `[capabilities] allow = […]` parsed (minimal, validated
    against the known set; full TOML arrives with `web` in M5).
  - **Check-time**: `use files` without the `fs` grant → E0244, pointing at
    the `use` line, with both fixes named (manifest or `--allow`).
  - **Runtime**: the VM holds its own grant set and re-checks at the
    syscall boundary → E0310. Tested explicitly with a permissive check and
    an ungranted VM — no code path can bypass the declaration.
  - CLI: manifest discovered by walking up from the file; `--allow fs`
    grants per run for scripts and the REPL (which announces its grants).
  - Scripts default to **zero capabilities** (Safe Mode, P7).
- **`files` module** (first capability-gated I/O): `read_text`,
  `write_text`, `exists`, `list` — failures are `Outcome` values, not
  panics, because a missing file is an expected failure (LDD §8.1).
- **`spider test`**: `test "name"` blocks compile to their own protos and
  run each on a **fresh VM** (tests cannot depend on each other); setup
  output is shown only on failure; failures render the panic in full
  Explain form. The global builtin `expect(actual, expected)` (E0311) is
  the assertion core.

## 2. Exit criteria status (SRS §11, M4 row)

- ✅ **Stdlib ≥ 90% doc coverage** — 100%, CI-enforced forever
  (`stdlib_doc_coverage_is_total`).
- ✅ **Capability enforcement demonstrated** — three ways: unit test at
  check time (E0244), unit test at runtime with a permissive check
  (E0310), and the CLI end-to-end (denied without `--allow fs`, works
  with it).

## 3. Notes and honest deferrals

- The M2 semantics test expecting a *runtime* unknown-member panic now
  fails earlier, at check time — the test was updated to assert the better
  behavior. Runtime E0306 remains only for the defense-in-depth layer.
- `console` as a named module was skipped: `say`/`ask` are language-level
  and capability-free by design (they are how Lina's first program talks).
- **Lexer-level interpolation tokens: deferred again**, explicitly. The
  quote-in-hole limitation stands. It is now the top item for M6 (the LSP
  wants token-level holes for completions inside strings); deferring twice
  is a signal, so it is scheduled, not floating.
- Shared-HIR: the registry removed the checker/VM duplication for the
  whole stdlib surface. The remaining duplication (variant tags in match
  compilation) is unchanged and still tracked for the M6 refactor.
- `time`, `json`, `regex`, `net.http`, `env`, `exec`, `ai` remain
  registry-empty; any member use is an honest check-time E0306.

## 4. Test inventory (82 tests)

New in M4: stdlib doc-coverage gate, doc/dispatch sync check, manifest
parsing (4), capability check-time + runtime + files round-trip + typed
module calls + expect/test-block tests (6), semantics grid updated for
check-time E0306 and E0244 (1120 cases).

## 5. Try it

```
spider test file.sp                   # run test blocks, teaching failures
spider run --allow fs script.sp       # grant a capability for one run
spider repl --allow fs                # capabilities announced per session
spider explain E0244                  # why Safe Mode said no
```

## 6. Next: M5 "First Thread"

The module system across files and the `web` package manager MVP:
manifest, lockfile, local registry, capability diffing on install, and
sandboxed builds — the milestone where the capability model starts paying
for itself transitively.
