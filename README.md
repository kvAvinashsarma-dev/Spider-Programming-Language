# Spider Programming Language

> Programming should be as easy as speaking — without ever becoming a toy.

**🕸️ Website & browser playground: [spider-lang.vercel.app](https://spider-lang.vercel.app)** —
write and run real Spider in your browser (the actual compiler + Silk VM as
WebAssembly), and learn to code at
[spider-lang.vercel.app/learn](https://spider-lang.vercel.app/learn).

**📖 The Book — official documentation: [spider-lang.vercel.app/book](https://spider-lang.vercel.app/book/)** —
40+ chapters: tutorials, full language and standard-library reference,
compiler/VM internals, and a contributor guide. Source in [`book/`](book/).

**Spider** is a new programming language with one uncompromising thesis: the easiest
language in the world to learn, read, and debug can also be a production-grade,
memory-safe, natively-compiled systems platform. Easy surface. Industrial core.

```spider
say "Hello, world!"

let name = ask "What is your name?"
say "Welcome, {name}!"
```

| Component        | Name            |
|------------------|-----------------|
| Language         | Spider          |
| File extension   | `.sp`           |
| Compiler / CLI   | `spider`        |
| Package manager  | `web`           |
| Bytecode VM      | Silk (internal) |
| Future IDE       | Spider Studio   |

## Project status

**Milestone M4 "Web-spinning" — shipped 2026-07-05. Safe Mode is real.**
The standard library is typed against a single registry (**100%
documented**, CI-enforced), and capabilities are enforced twice: at check
time (`use files` without a grant is a teaching error pointing at the
line) and again inside the VM at the operation — no code path around the
declaration. `spider test` runs `test "…"` blocks on a fresh VM each, with
`expect(actual, expected)` failures rendered as full explanations.

```
spider run file.sp               # Safe Mode: zero capabilities by default
spider run --allow fs script.sp  # explicit, per-run capability grant
spider test .                    # run every test block, isolated
spider repl / new / check / fmt  # the rest of the toolchain
cargo test --workspace           # 82 tests incl. the 1120-case suite
```

**M5 "First Thread" also shipped**: multi-file modules (`use helpers`,
public/private boundaries, side-effect-free imports, cycle detection) and
`web` — the package manager: local registry, immutable published versions,
`web.lock` with content fingerprints, `web audit` tamper detection, and
**capability diffing at install**: a dependency that wants more than your
project allows is refused with the escalation spelled out. 89 tests green.

**M6 "Loom" (partial)**: `spider lsp` — live diagnostics, teaching hovers,
and completion in any LSP editor, protocol-tested without one; `spider
learn` turns any file into a lesson plan. Playground/WASM waits on M8.

Shipped so far: M1 Hatchling ([notes](docs/0004-m1-hatchling-notes.md)),
M2 Spinneret ([notes](docs/0005-m2-spinneret-notes.md)),
M3 Silk ([notes](docs/0006-m3-silk-notes.md)),
M4 Web-spinning ([notes](docs/0007-m4-web-spinning-notes.md)),
M5 First Thread ([notes](docs/0008-m5-first-thread-notes.md)),
M6 Loom partial ([notes](docs/0009-m6-loom-notes.md)).

## Documents

Read in this order:

1. [`docs/0001-software-requirements-specification.md`](docs/0001-software-requirements-specification.md)
   — Vision, philosophy, audience, goals, requirements, roadmap, V1 milestones,
   risks, non-goals. **The contract for what we are building and why.**
2. [`docs/0002-language-design-document.md`](docs/0002-language-design-document.md)
   — Syntax principles and surface tour, type system, memory model, error model,
   concurrency model, module system, package manager, compiler and runtime
   architecture, AI integration, quantum readiness, stdlib roadmap, open questions.
   **The contract for how it works.**
3. [`docs/0003-architecture-decision-log.md`](docs/0003-architecture-decision-log.md)
   — Numbered, immutable record of the load-bearing decisions and their rationale.

## The ten-second pitch

- **Reads like thought.** Natural-language-inspired keywords over a strict,
  deterministic grammar. No ambiguity, no magic.
- **Errors that teach.** Every diagnostic says what happened, why, and how to fix
  it — in human language. The compiler is a teacher, not a judge.
- **Safe by default.** Memory safe, type safe, no null, no data races,
  capability-gated packages (a package cannot touch the network or disk unless
  it declares it and you allow it).
- **Fast by default.** Instant-start bytecode VM for development and learning;
  ahead-of-time native compilation for production. One language, both worlds.
- **AI-native, AI-honest.** First-class, provider-agnostic AI constructs in the
  language and an AI-assisted toolchain — but AI is **never** in the compilation
  path. Builds are deterministic, forever.

## How to review

The specification ends with a sign-off checklist ([SRS §14](docs/0001-software-requirements-specification.md))
and a list of open design questions with recommendations
([LDD §18](docs/0002-language-design-document.md)). Implementation of
Milestone M1 begins only after those decisions are ratified.

## Credits

Designed and built by **Avinash** ([@kvAvinashsarma-dev](https://github.com/kvAvinashsarma-dev))
in collaboration with **Claude** (Anthropic) as co-author — specification,
compiler, Silk VM, toolchain, and documentation.

---

*Spider — spin something.*
