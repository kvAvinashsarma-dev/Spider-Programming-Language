# Milestone M5 "First Thread" — Architecture Notes

| Field    | Value                                    |
|----------|-------------------------------------------|
| Document | 0008 — M5 milestone notes                 |
| Status   | Shipped 2026-07-05                        |
| Scope    | Multi-file modules; `web` package manager MVP (local registry, lockfile, capability diffing, audit); project-aware checker & compiler |

## 1. What shipped

- **Multi-file modules** (ADR-015): `use helpers` loads `src/helpers.sp`;
  `use shop.cart` loads `src/shop/cart.sp`; `use <pkg>` loads
  `web_modules/<pkg>/src/lib.sp`. Stdlib names always win; cycles are an
  error that spells out the circle. Rules: functions cross module
  boundaries only via `alias.fn(...)` and only when `public` (a private
  call teaches: *"add `public` to its `fn` to share it"*); types
  (records/choices) are project-global; **only the main file runs
  top-level code** (E0246) — importing a module can never have surprise
  side effects.
- **Project-aware pipeline**: `check_project` (4-stage: merge type names →
  per-module signatures → merge exports/ctors → body-check with the merged
  world) and `compile_project` (functions qualified per module, one
  Program). Single-file remains a special case of the same path.
- **`web`, the package manager MVP** (SRS FR-15/16/19 slice):
  - Local registry (`~/.spider/registry`, `SPIDER_REGISTRY` overridable);
    published versions are immutable.
  - `web install`: **capability diffing** — a dependency demanding more
    than the project allows is refused, with the exact escalation listed.
    Blocked-then-granted is the M5 exit-criterion test.
  - `web.lock`: exact versions + content fingerprints (FNV-64,
    line-ending-normalized); `web audit` verifies integrity and lists each
    dependency's capabilities — tamper detection tested.
  - **No install scripts, ever** — a package is data (SRS FR-19).
  - `web publish / install / audit / remove`; installing also records the
    dependency in `web.toml`.
- `spider run/check/test` all route through the loader; diagnostics render
  against the correct file.

## 2. Exit criteria status (SRS §11, M5 row)

- ✅ **Install/publish round-trip** — tested end to end, including running
  the installed package's code and lockfile contents.
- ✅ **Capability escalation blocked in test** — `sneaky` (needs fs) into a
  no-capability project: refused naming `fs`; granted after the project
  allows it.

## 3. Honest limitations

- The registry is local (a directory). Networked registry, signing, and
  namespace ownership are post-1.0 by roadmap; the fingerprint is FNV-64,
  a integrity check against accidents and casual tampering, not a
  cryptographic guarantee — stated in the code and here.
- Transitive package dependencies (a package importing another package)
  are not resolved in M5; packages must be self-contained. M6+.
- Variant names share one global namespace across a project (as they
  already did within a file). The shared-HIR refactor tracks it.
- `web.toml` dependency versions are recorded but not yet range-resolved
  (latest wins at install). Semver-with-teeth is an M10/registry feature.

## 4. Tests: 89 green

New: 7 spider-web tests — multi-file run, privacy, E0246, cycle
explanation, cross-module types, publish/install/lockfile/tamper, and the
capability-escalation exit criterion.

## 5. Next: M6

LSP over stdio (diagnostics + hover), Learn Mode, and the twice-deferred
lexer-level interpolation tokens.
