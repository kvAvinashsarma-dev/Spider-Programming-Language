# Spider Programming Language

> Programming should be as easy as speaking — without ever becoming a toy.

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

**Milestone M2 "Spinneret" — shipped 2026-07-05.** The compiler now has a
semantic brain: name resolution, unification-based type inference (no
annotations needed on locals, generics instantiated per call site),
exhaustive-match analysis, `try`/`Outcome` rules, "did you mean" suggestions
on every unknown name, contextual keywords (`fn area(shape: Shape)` is
legal — ADR-011), and **59 authored error codes** with what/why/fix
explanations, the ≥50 gate CI-enforced. 39 tests green.

```
cargo build --workspace          # build the toolchain
cargo test  --workspace          # 39 tests: goldens, fuzz, inference corpus
spider check file.sp             # parse + names + types, teaching diagnostics
spider fmt  file.sp              # canonical formatting (no options)
spider tree / tokens / explain   # debugging & the error database
```

Shipped so far: M1 Hatchling ([notes](docs/0004-m1-hatchling-notes.md)),
M2 Spinneret ([notes](docs/0005-m2-spinneret-notes.md)).
Next: **M3 "Silk"** — the bytecode VM: `spider run`, the REPL, tokenized
string interpolation, and the < 50 ms cold-start budget measured in CI.

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

---

*Spider — spin something.*
