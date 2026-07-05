# Spider — Architecture Decision Log

Numbered, append-only record of load-bearing decisions. Each entry is short by
design; full rationale lives in the SRS/LDD sections cited. Status values:
**Proposed** (awaiting spec sign-off) → **Accepted** → (rarely) **Superseded by ADR-xxx**.

---

## ADR-001 — Implementation language: Rust
**Status:** Accepted (2026-07-05) · **Refs:** SRS NG8, LDD §11
The compiler, Silk VM, and toolchain are written in Rust. Memory safety for
the toolchain itself, mature ecosystem (rowan, salsa, cranelift), single-binary
distribution, first-class WASM for the Playground. Self-hosting is explicitly
deferred past 1.0.

## ADR-002 — Indentation-defined blocks; `let`/`var`; zero-option formatter
**Status:** Accepted (2026-07-05) · **Refs:** LDD §2, §3.2, SRS R8
Blocks are defined by indentation (4 spaces). The structure a human sees is
the structure the compiler uses. `let` = immutable binding, `var` = mutable.
`spider fmt` has no configuration. Revisit trigger: if M6 classroom testing
shows copy-paste indentation errors dominating beginner friction.

## ADR-003 — AI never participates in program semantics
**Status:** Accepted (2026-07-05) · **Refs:** SRS §7, NG2, LDD §14
Three tiers: AI in toolchain (optional, labeled, offline-degradable), AI in
language (provider-agnostic runtime constructs returning `Outcome`), AI never
in parsing/typing/codegen. Builds are deterministic given source + lockfile +
toolchain version. This ADR is intended to be permanent; superseding it
requires a majority of the core team and a public RFC.

## ADR-004 — Static, sound, locally-inferred type system; no null; no implicit conversions
**Status:** Accepted (2026-07-05) · **Refs:** LDD §7, §3.7
Hindley–Milner-style inference with row-polymorphic records; annotations
required at `public` boundaries only. `Maybe` replaces null; `Outcome`
replaces exceptions. Gradual typing rejected: it trades away the guarantees
that fund our diagnostics, refactoring, and AOT performance.

## ADR-005 — Tiered execution: Silk VM (dev) + Cranelift AOT (release); ownership-inference + ARC memory with protected GC fallback
**Status:** Accepted (2026-07-05) · **Refs:** LDD §9, §11–12, SRS R1, G2, G5
Development and learning run on the Silk bytecode VM (< 50 ms cold start);
release builds AOT-compile via Cranelift (LLVM post-1.0). Memory: value
semantics at the language level; escape analysis → stack, ownership inference
→ unique heap, ARC+COW for the shared remainder. Because value semantics are
implementation-neutral, the VM tier may fall back to tracing GC without any
language change — this fallback path is architecturally protected: no stdlib
API may observe destruction timing except via explicit `with`/`defer`.

## ADR-006 — Lightweight tasks + structured concurrency; no async/await coloring
**Status:** Accepted (2026-07-05) · **Refs:** LDD §10, §3.9
Any function may block; the runtime multiplexes tasks over OS threads.
`do together` provides structured join/cancel/error semantics; unscoped
`spawn` handles must be awaited or explicitly detached. Data-race freedom
follows from value semantics + move-on-send channels + explicit `shared`
cells. Rejected: async/await (permanent cognitive tax on every user).

## ADR-007 — Capability-based security as a launch feature, enforced by the package manager
**Status:** Accepted (2026-07-05) · **Refs:** SRS FR-16–21, LDD §6, §3.10
Programs and packages declare capabilities (`fs`, `net`, `exec`, `env`, `ai`).
Transitive escalation fails installs; Silk enforces at the syscall boundary;
builds are sandboxed; the registry has no install scripts. Security is a
launch differentiator, not a 2.0 patch.

## ADR-008 — Specification before implementation; milestone gating
**Status:** Accepted (2026-07-05) · **Refs:** SRS §11, §14
No implementation until SRS/LDD sign-off. Milestones M0–M10 ship strictly in
order; each requires green CI on all Tier-1 platforms plus architecture notes,
tests, docs, and examples. The milestone table is the only source of truth
for scope; USPs not in a milestone are not "in progress."

## ADR-009 — Open design questions OQ-1…OQ-8 resolved as recommended
**Status:** Accepted (2026-07-05) · **Refs:** LDD §18
Ratified with spec sign-off: generics use the `of`/`to` spine (`List of Int`,
`Map of Text to Int`); failure constructor is `Fail`; structural interfaces
use `shape`; `Int` is 64-bit checked with overflow panic (+ `BigInt` in std);
`web.toml` is TOML; strings have no direct indexing (slice/iterator API);
the package manager keeps the name `web`; `do together` is the structured
concurrency block pending M7 classroom testing.

## ADR-010 — Losslessness and no-panic are load-bearing invariants of spider-syntax
**Status:** Accepted (2026-07-05) · **Refs:** M1 notes (doc 0004)
For any input whatsoever: `parse(src).root.text() == src`, and the parser
never panics (recursion depth capped, per-line error recovery). Enforced by
corpus assertions and fuzz tests on every CI run. Every future stage (IDE,
refactoring, formatter) may rely on these without re-checking.

## ADR-011 — Contextual (soft) keywords
**Status:** Accepted (2026-07-05) · **Refs:** M1 notes §3 finding 1, M2 notes §2
Spider's English-word keywords collide with natural identifiers (the LDD
itself wrote `fn area(shape: Shape)`, which M1 could not parse). Resolution:
`record`, `choice`, `shape`, `test`, `times`, `together`, `where` are
keywords only in their grammatical position; wherever a *name* is expected
they are ordinary identifiers (the parser remaps the token to `Ident`, text
untouched, losslessness preserved). Operators stay hard forever: `to`, `of`,
`is`, `in`. Structural words stay hard: `let var fn if else for while repeat
match try use return say ask spawn do and or not true false public`.
Disambiguation at statement head: `record X` is a declaration iff followed by
a name and a line end; `test` iff followed by a string.

## ADR-012 — M2 semantic rulings
**Status:** Accepted (2026-07-05) · **Refs:** M2 notes §2–3
(1) No paren-less method arguments; LDD §3.11 amended to `helper.ask("…")`.
(2) Match arms: expression or `say` line; value-matches must agree in type.
(3) Trailing expression/match is the implicit return of a non-`Nothing`
function. (4) Unannotated non-public params are unchecked `Any` until
cross-function inference (post-M3); public boundaries require annotations.
(5) Record/variant construction is call syntax, provisional until M4.
(6) `Any` is the error type: one mistake, one diagnostic — cascades are a
compiler bug by definition. (7) Interpolated `{name}` segments are
marked-as-used but not checked until M3 tokenizes interpolation; a warning
that can be wrong is treated as a defect (W0001 false-positive found and
fixed via the corpus).

## ADR-013 — M3 execution rulings
**Status:** Accepted (2026-07-05) · **Refs:** M3 notes (doc 0006)
(1) `spider run` executes top-level statements, then auto-calls zero-arity
`fn main()`; the REPL never auto-calls main. (2) The VM keeps call frames on
an explicit heap stack — Spider recursion cannot overflow the host stack;
the limit is a language rule (1000, E0307). (3) Value semantics are
implemented copy-on-write; maps are insertion-ordered vectors (deterministic
iteration is a guarantee, honest O(n) lookup until M8). (4) Int is 64-bit
checked (overflow panics E0302, per ADR-009); Float follows IEEE and never
panics. (5) Interpolation holes are parsed by the real parser and checked in
scope via a shared scanner; token-level interpolation in the lexer is M4
work (holes cannot contain text literals until then). (6) Bare `try` on a
`Maybe` propagates `Fail("the value was missing")`. (7) `test` blocks run
under `spider test` (M4), never under `spider run`. (8) Concurrency
constructs run sequentially until M7 and the checker says so (W0003).

---

*Add new entries below; never edit accepted entries — supersede them.*
