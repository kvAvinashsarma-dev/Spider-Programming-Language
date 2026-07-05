# Milestone M3 "Silk" — Architecture Notes

| Field    | Value                                     |
|----------|--------------------------------------------|
| Document | 0006 — M3 milestone notes                  |
| Status   | Shipped 2026-07-05                         |
| Scope    | Silk VM + bytecode compiler; `spider run`, `spider repl`, `spider new`; string interpolation checked & executed; runtime panics in Explain format; 1119-case semantics suite; cold start measured |

## 1. What shipped

- **`spider-silk`** — the Silk register-based bytecode VM (ADR-005) and its
  compiler:
  - **Values with real value semantics** (LDD §9.1): assignment behaves as a
    copy, implemented copy-on-write (`Rc` + make-on-mutate). `var b = a;
    b.push(3)` leaves `a` untouched — tested. Maps are insertion-ordered:
    deterministic iteration is a language guarantee, not luck.
  - **Register-based compiler**: each function gets a statically-allocated
    frame; l-value paths (`grid[0][1] = 9`, `point.x = 1.0`, `xs.push(2)`
    through nested paths) compile to read/mutate/write-back chains so value
    semantics hold at any depth.
  - **Explicit heap frame stack**: Spider recursion never touches the host
    stack. The first implementation recursed in Rust and overflowed in
    testing at depth ~1000 — the fix was architectural (an interpreter loop
    over `Vec<Frame>`), not a smaller limit. The recursion limit (1000,
    Python-precedent, E0307) is now a language rule, not a crash.
  - **Runtime panics in Spider Explain format**: E0301 divide-by-zero,
    E0302 Int overflow (64-bit checked, per ADR-009), E0303 list position,
    E0304 missing map key, E0305 unsortable items, E0306 unknown module
    member, E0307 recursion limit. Floats follow IEEE (no panic; documented).
  - Builtin methods (Text/List/Map/Int/Float), `math` and `random` modules —
    `random` is seedable and deterministic (classroom requirement).
- **String interpolation is real**: one shared scanner
  (`spider_syntax::interpolation`) feeds both the checker — every `{…}` hole
  is parsed with the real parser and type-checked in scope (E0243 for broken
  holes) — and the compiler, which lowers holes to concatenation. Escapes:
  `\n \t \" \\ \{ \}`.
- **`spider run`** (script + auto-`main`, ADR-013), **`spider new`**
  (manifest with an empty capability list, per the security model), and
  **`spider repl`** — declarations and bindings persist across entries,
  values live on; incomplete blocks continue on the next line; a trailing
  expression echoes its value.

## 2. Exit criteria (SRS §11, M3 row) — both measured, not claimed

- ✅ **Semantics suite ≥ 1000 cases**: **1119 cases**, most generated
  combinatorially with expected values computed by an independent Rust
  implementation (differential testing). The count is asserted by the suite
  itself on a thread-local counter, so parallel test scheduling can never
  inflate it — the first run of the gate honestly failed at 921 and the grid
  was widened.
- ✅ **Cold start < 50 ms**: release `spider run hello.sp`, 15 process
  spawns on Windows 11: **min 18.9 ms, median 20.4 ms, max 43.3 ms** —
  including process start, parse, check, compile, and execution. CI now has
  a `cold-start` job that fails the build if the average crosses 50 ms.

## 3. Design rulings and honest limitations

Rulings (recorded as ADR-013):
1. `spider run` executes top-level statements, then auto-calls a zero-arity
   `fn main()` if present. The REPL never auto-calls `main`.
2. Recursion limit 1000 (E0307). An explicit-frame design already means the
   limit is memory-cheap; raising it is an M8 tuning question.
3. A bare `try` on a `Maybe` propagates `Fail("the value was missing")`.
4. `test` blocks parse and check but do not run under `spider run` — they
   are `spider test`'s job (M4).
5. `do together` / `spawn` execute sequentially until M7; the checker says
   so honestly on every use (W0003).

Limitations, tracked:
- **Holes cannot contain text literals** (`"{m["k"]}"`) — the M1 lexer ends
  the string token at the inner quote. Fixing this requires tokenizing
  interpolation in the lexer; scheduled with the M4 stdlib work since typed
  string APIs want it too.
- Compound assignment through an index (`a[f()] += 1`) evaluates the path
  once (correct), but the checker/compiler pair resolves match-pattern tags
  independently — a lowercase choice case name could theoretically be
  misread as a binding by one and a tag by the other. Convention (CapWords
  cases) makes this unreachable in practice; unifying on shared HIR tables
  is the M4 refactor that removes the duplication entirely.
- Instruction operands use boxed vectors and per-proto const tables — clean,
  not fast. Benchmarks and packing belong to M8 (Exoskeleton) by design.

## 4. Test inventory (71 tests)

| Suite | Proves |
|---|---|
| spider-syntax (19 + interpolation 4) | M1 invariants + segment scanner (escapes, nesting) |
| spider-hir (18) | M2 checks + interpolation holes typed in scope |
| spider-silk lib (7) | hello/interpolation/value-semantics/panics/ask/REPL session/incompleteness |
| `tests/semantics.rs` (20 tests, **1119 cases**) | the language does what the LDD says: arithmetic/comparison grids, short-circuit, loops, functions, generics, records/choices, Outcome/try, panics, value semantics |
| `tests/run_golden.rs` | 4 real programs, byte-exact output snapshots |
| spider-fmt (5) | formatter laws still hold |

## 5. Try it

```
spider run corpus\run\showcase.sp     # records, choices, match, interpolation
spider repl                           # persistent interactive session
spider new my-project                 # scaffold (empty capability list)
spider explain E0303                  # runtime panic database
```

## 6. Next: M4 "Web-spinning"

The typed standard library core (`Text`, `List`, `Map`, `Maybe`, `Outcome`,
console, capability-gated `files`), `spider test`, lexer-level interpolation
tokens, and the shared-HIR refactor that unifies checker and compiler tables.
