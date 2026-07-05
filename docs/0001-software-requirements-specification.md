# Spider — Software Requirements Specification (SRS)

| Field    | Value                                        |
|----------|----------------------------------------------|
| Document | 0001 — Software Requirements Specification   |
| Version  | 0.1.0                                        |
| Status   | **Ratified 2026-07-05** — all recommendations approved |
| Date     | 2026-07-05                                   |
| Authors  | Spider founding engineering team             |
| Applies  | Spider language, `spider` toolchain, `web` package manager, Silk runtime |

---

## 1. Vision

Every mainstream language asks: *how do I write code?*
Spider asks: **how do I express an idea?**

Spider is a general-purpose programming language whose surface is simple enough
for a ten-year-old to learn in an afternoon and whose core is sound enough to
run production services for decades. It is not a teaching toy that people
outgrow, and not an industrial tool that beginners bounce off. It is one
language with one semantics that meets people where they are and grows with
them.

In thirty years, we want "I learned to program in Spider" to be as common as
"I learned to type on a keyboard" — and we want a meaningful fraction of the
world's new software to still be written in it.

## 2. Philosophy

1. **Simplicity is engineered, not declared.** Every feature we *don't* add is
   a feature. Every concept a user must hold in their head is a cost we pay
   interest on forever.
2. **The compiler is a teacher.** A compiler error is a conversation, not a
   verdict. Every diagnostic must explain what happened, why, and how to fix it,
   in language a beginner can act on.
3. **One language, one meaning.** Beginner Mode, Kids Mode, and Expert Mode
   change *what the tools show and allow*, never *what a program means*. There
   are no dialects. A program is valid Spider or it is not.
4. **Safe by default, capable by declaration.** Memory safety, type safety,
   data-race freedom, and capability-gated I/O are the defaults. Power is
   opted into explicitly and visibly — never removed, never hidden.
5. **Deterministic forever.** Given the same source and lockfile, a build
   produces the same result on any machine, any year, with the network off.
   Nothing probabilistic — including AI — may ever participate in deciding what
   a program means. (AI assists humans; it does not define semantics. See §7.)
6. **Honesty over hype.** We do not ship claims we cannot keep. "Quantum ready"
   means a designed extension point, not a feature. We say so in writing.

### The ten-year-old test

Every syntax rule and every diagnostic must pass this question before merge:
**"Can a motivated 10-year-old, with the compiler's help, understand it?"**
If not, it is redesigned or it does not ship in the default (non-Expert)
surface of the language.

## 3. Target audience

Design personas — every requirement in this document should serve at least one:

- **Lina, 10, learner.** Has never programmed. Types `say "hi"` and it works.
  The compiler and Learn Mode are her first teacher. She must never see the
  words "segmentation fault," "borrow," or "monad."
- **Ben, 24, career-switcher / beginner.** Builds his first real web tool in a
  weekend. Needs great errors, one obvious way to do things, batteries-included
  stdlib, and a package manager that cannot hurt him.
- **Priya, 34, senior engineer.** Ships production services. Needs performance,
  observability, native compilation, structured concurrency, dependency
  auditing, and no ceiling. Expert Mode is hers.
- **Marcus, 45, educator.** Teaches a classroom of 30. Needs Kids Mode,
  the Playground, deterministic behavior, and offline capability.
- **The AI agent.** By 2030 a large share of code will be written and reviewed
  by AI systems. Spider's regularity, explicitness, and machine-readable
  diagnostics make it the easiest language for AI to write *correctly* — a
  first-class design constraint, not an accident.

## 4. Guiding principles (normative)

- **P1 — One obvious way.** Where two syntaxes could express the same thing,
  we ship one. The formatter has no options.
- **P2 — Read out loud.** Code should be pronounceable as near-English without
  being English. Natural-language-*inspired*, never natural-language-*parsed*.
- **P3 — No hidden magic.** No implicit conversions, no invisible control flow,
  no compile-time behavior a user cannot inspect and predict.
- **P4 — Errors are product.** Diagnostics get the same design rigor as syntax.
  A confusing error message is a P1 bug.
- **P5 — Progressive disclosure.** Advanced power exists but never leaks into
  beginner code paths. You cannot stumble into lifetimes by accident.
- **P6 — Fast feedback.** Cold start of a script under 50 ms. Sub-second
  incremental rebuilds are a feature gate for every milestone.
- **P7 — Capability security.** Code cannot perform I/O it has not declared.
  This applies transitively to every dependency.
- **P8 — Tooling is the language.** Formatter, LSP, test runner, doc generator,
  and package manager ship in the same repo, same release, same version — from
  Milestone 1 onward, not bolted on later.

## 5. Language goals (measurable)

| ID  | Goal | Target (V1) |
|-----|------|-------------|
| G1  | Time from install to running "Hello, world" | < 60 seconds |
| G2  | Cold start, interpreted `hello.sp` | < 50 ms |
| G3  | Full non-incremental compile of a 10 kLOC project (VM bytecode) | < 1 s |
| G4  | Incremental recompile after a one-function edit | < 100 ms |
| G5  | AOT-compiled numeric code vs. equivalent Go | within 1.5× |
| G6  | Diagnostic quality: % of top-100 common errors with a full Explain entry (what/why/fix/example) | 100% |
| G7  | Beginner study: % of first-week users who resolve a type error without external help | > 80% |
| G8  | Memory safety CVEs attributable to safe Spider code | 0, permanently |
| G9  | Toolchain install size (compiler + stdlib + LSP + fmt) | < 150 MB |
| G10 | Platforms at 1.0 | Windows x64/ARM64, Linux x64/ARM64, macOS ARM64/x64, WebAssembly (WASI) |

## 6. Non-goals (V1 — explicit and binding)

- **NG1 — Not English.** Spider is not a natural-language programming system.
  Free-form English is ambiguous and unimplementable as a deterministic
  language; we take NL *inspiration* for keywords and phrasing only. Anyone
  proposing "the compiler should just understand what you meant" is proposing
  a different product.
- **NG2 — No AI in the compilation path.** AI never affects parsing, type
  checking, code generation, or program semantics. (Rationale in §7 and ADR-003.)
- **NG3 — No quantum implementation.** V1 ships a designed backend extension
  point and a reserved namespace — nothing more. No simulated qubits, no claims.
- **NG4 — Not a kernel/embedded systems language in V1.** No bare-metal, no
  `no-runtime` mode, no inline assembly. Extension point reserved post-1.0.
- **NG5 — No exceptions as a control-flow mechanism.** Expected failures are
  values (§ LDD-8). Panics exist only for bugs and are not catchable in
  ordinary code.
- **NG6 — No manual memory management in the safe language.** Ever.
- **NG7 — No language dialects.** Modes affect tooling and lints only (P5).
  There is no "Kids Spider" grammar.
- **NG8 — No self-hosting before 1.0.** The compiler is written in Rust
  (ADR-001). Self-hosting is a romantic distraction until the language is
  stable.
- **NG9 — No plugin/macro system in V1.** Syntax-level extensibility
  (macros, custom operators) multiplies the "one obvious way" surface and
  destroys diagnostic quality. Revisit post-1.0 with extreme prejudice.

## 7. AI position statement (requirement-level)

The brief demands "AI built into the language, not a library." We accept it
with one non-negotiable boundary, because millions of developers will depend
on reproducible builds:

- **Tier 1 — AI in the toolchain** (`spider explain`, `spider tutor`,
  `spider review`, doc generation, test suggestion, migration assistant):
  ships as first-party toolchain features. Optional, offline-degradable,
  privacy-documented. Never required to build or run.
- **Tier 2 — AI in the language**: Spider defines first-class, provider-agnostic
  constructs (`use ai`, model handles, prompt values, typed structured output)
  so AI-using programs are portable across providers and testable via recorded
  fixtures. The *runtime* calls models; the *compiler* never does.
- **Tier 3 — AI never in semantics**: what a program means is defined by the
  written specification and the deterministic compiler alone. This is
  permanent (NG2).

## 8. Functional requirements

### 8.1 Language and compiler

- **FR-1** The `spider` toolchain shall compile `.sp` sources through the full
  pipeline: lexer → CST → AST → HIR (names/types) → MIR (optimization) →
  {Silk bytecode | native code | WASM}.
- **FR-2** The compiler shall be error-tolerant: a file with errors still
  produces a CST/AST suitable for IDE features, and reports *all* independent
  errors in one pass, ordered by source position.
- **FR-3** Every diagnostic shall carry: stable error code, one-line summary,
  what/why/fix sections, a minimal corrected example, and (when applicable) a
  machine-applicable suggested fix. This is the **Spider Explain** format
  (LDD §8.3).
- **FR-4** `--learn` (Learn Mode) shall augment diagnostics and `spider run`
  output with explanations of the constructs in use. `--kids` shall additionally
  simplify vocabulary and enable colorized output. Neither changes semantics.
- **FR-5** The compiler shall support a query-based incremental architecture
  such that edits invalidate only dependent computations (G4).

### 8.2 Execution

- **FR-6** `spider run file.sp` shall execute via the Silk VM with cold start
  < 50 ms (G2). `spider build --release` shall produce a self-contained native
  executable.
- **FR-7** A REPL (`spider repl`) and a browser Playground shall execute the
  same Silk bytecode with the same semantics.
- **FR-8** Silk shall support deterministic record/replay of single-threaded
  program regions, as the substrate for the time-travel debugger (post-M9,
  LDD §14.4).

### 8.3 Tooling (ships with the toolchain, same version — P8)

- **FR-9** `spider fmt` — canonical formatter, zero configuration.
- **FR-10** `spider lsp` — language server: diagnostics, completion, hover
  with Learn-Mode-aware docs, rename, go-to-definition.
- **FR-11** `spider test` — built-in test framework; test blocks live beside
  code; snapshot and property testing post-1.0.
- **FR-12** `spider doc` — documentation generated from code and doc comments;
  every `public` item without a doc comment is a warning (Documentation-Driven
  Development requirement).
- **FR-13** `spider explain E####` — offline explanation database for every
  error code; `--ai` flag optionally enriches with Tier-1 AI.
- **FR-14** `spider new <name>` — project scaffolding with manifest, example,
  test, and README.

### 8.4 Package manager (`web`)

- **FR-15** `web install <pkg>` resolves against a content-addressed registry,
  writes an exact lockfile, and verifies signatures and checksums.
- **FR-16** Every package shall declare **capabilities** (`net`, `fs`, `exec`,
  `env`, `ai`, …). Installing a package whose transitive capability set exceeds
  the project's declared set fails with an explanation and a diff.
- **FR-17** `web audit` reports known vulnerabilities, capability changes
  between versions, and unmaintained dependencies.
- **FR-18** The registry shall support community ratings, download provenance,
  and namespace ownership verification. Static analysis runs on publish;
  results are public.
- **FR-19** Builds are sandboxed: package build steps get no network and no
  filesystem outside their own tree. There are no arbitrary install scripts.

### 8.5 Security

- **FR-20** Safe Spider code shall be memory-safe and data-race-free by
  construction; violations are compile-time errors (LDD §9–10).
- **FR-21** Safe Mode (default): a program's I/O is limited to its manifest's
  declared capabilities at both compile time (visibility) and runtime
  (enforcement in Silk; OS-level sandboxing where available).

## 9. Non-functional requirements

- **NFR-1 Determinism.** Identical inputs (source + lockfile + toolchain
  version) yield bit-identical bytecode. Native output is reproducible per
  target triple.
- **NFR-2 Portability.** Single-binary toolchain per platform; no external
  runtime dependencies for `spider run`.
- **NFR-3 Accessibility.** Diagnostics readable by screen readers; color never
  the sole channel of information; diagnostic text externalized for
  internationalization (top languages post-1.0).
- **NFR-4 Stability.** From 1.0: semver on the language; a written
  compatibility promise; deprecations require one minor version of warnings
  plus an automated `spider fix` migration.
- **NFR-5 Openness.** Apache-2.0 OR MIT dual license; public RFC process for
  all language changes after spec ratification.

## 10. Product surface (CLI contract)

```
spider new <name>        create a project
spider run [file|.]      run via Silk VM (dev)
spider build [--release] AOT-compile to native / WASM
spider test              run tests
spider fmt               format (no options)
spider doc               generate documentation
spider repl              interactive session
spider explain E0301     explain an error code
spider fix               apply machine-applicable migrations/fixes
spider lsp               language server (spawned by editors)

web install <pkg>        add a dependency (capability-checked)
web remove <pkg>         remove a dependency
web audit                security & health report
web publish              publish (signed, statically analyzed)
web search <query>       search the registry
```

Flags common to all: `--learn`, `--kids`, `--json` (machine-readable output
for AI agents and editors).

## 11. Development roadmap and V1 milestones

Rules: milestones ship in order; each must compile, pass CI on all Tier-1
platforms, and include architecture notes, tests, docs, and examples. No
milestone starts until the previous one's exit criteria are met.

| # | Codename | Scope | Exit criteria |
|---|----------|-------|---------------|
| M0 | Foundation | This spec ratified; repo scaffold; CI (Win/Linux/macOS); RFC process; ADR log | Spec signed off; green CI on empty workspace |
| M1 | Hatchling | Lexer, error-tolerant parser, CST/AST, `spider fmt` (idempotent), golden-file test harness | Formats & round-trips full syntax corpus; fuzzing finds no parser panics in 24h |
| M2 | Spinneret | Name resolution, type inference core, diagnostics engine with Explain format, `spider explain` DB | Top 50 error codes fully authored; inference sound on test corpus |
| M3 | Silk | Bytecode compiler + Silk VM; `spider run`, `spider repl`; core semantics (values, functions, control flow, pattern matching) | Cold start < 50 ms; semantics test suite ≥ 1000 cases green |
| M4 | Web-spinning | Stdlib core (`Text`, `List`, `Map`, `Maybe`, `Outcome`, console, files-with-capability); `spider test` | Stdlib 90% doc coverage; capability enforcement demonstrated |
| M5 | First Thread | Module system; `web` MVP: manifest, lockfile, local + alpha registry, capability diffing, sandboxed builds | Install/publish round-trip; capability escalation blocked in test |
| M6 | Loom | LSP, browser Playground (WASM build of Silk), Learn/Kids modes in diagnostics | Editor demo: live errors + completion; Playground runs offline |
| M7 | Many Legs | Concurrency: lightweight tasks, structured `do together`, channels; data-race freedom checks | Race-freedom test corpus green; scheduler benchmarks published |
| M8 | Exoskeleton | AOT native backend (Cranelift), WASM/WASI target, cross-compilation | G5 performance target met on benchmark suite |
| M9 | Spider Sense | Debugger (DAP), record/replay alpha, `spider doc`, Tier-1 AI toolchain (`explain --ai`, tutor) behind explicit opt-in | Step-debug + one recorded replay demo; AI features fully offline-degradable |
| M10 | 1.0-beta | Spec freeze, compatibility promise, `spider fix` migrations, security audit, docs site | External security audit passed; 3 nontrivial real apps built by non-team users |

Post-1.0 tracks (unordered): LLVM backend, GPU/heterogeneous extension point,
embedded profile, macro RFC, i18n of diagnostics, Spider Studio IDE,
time-travel debugger GA, package ratings GA, quantum backend interface.

## 12. Risks

| ID | Risk | Severity | Mitigation |
|----|------|----------|------------|
| R1 | **Memory-model bet fails** — inferred ownership + ARC hybrid (LDD §9) proves confusing or slow | High | M3–M4 gate: real beginner testing; fallback path to tracing GC for the VM tier is architecturally preserved (ADR-005) |
| R2 | **Scope explosion** — the USP list is a decade of work | High | Every USP is tagged core/toolchain/post-1.0 in LDD §17; milestones are the only source of truth for "now" |
| R3 | **Two-modes dialect drift** — Kids/Beginner modes mutate into dialects | High | NG7 is binding; CI rejects any mode-conditional grammar or semantics |
| R4 | **`web` name collision** — "the web package manager" is ambiguous in speech and un-searchable | Medium | Keep per brief; brand as "web, the Spider package manager"; registry domain + `web.toml` manifest give concrete anchors. Flagged to product for reconsideration before M5. |
| R5 | **AI features erode trust** — cost, privacy, hallucinated fixes | Medium | Tier boundaries (§7); AI output always labeled, never auto-applied; offline-first; fixture-based testing for Tier-2 |
| R6 | **Performance vs. simplicity** — no-ceiling promise vs. beginner surface | Medium | Tiered execution (VM + AOT); Expert Mode; continuous public benchmarks from M3 |
| R7 | **Adoption** — languages die of loneliness, not technical flaws | High | Education-first wedge (Playground, Kids Mode, curriculum); killer toolchain out of the box; AI-agent-friendliness as a distribution channel |
| R8 | **Indentation-based blocks** divide opinion, complicate tooling | Low | Proven by Python at planetary scale; CST design handles it; decision recorded (ADR-002) with revisit criteria |

## 13. Success metrics (first two years post-1.0)

- 100k monthly-active toolchain installs; 1k published packages.
- A published third-party curriculum teaching Spider to under-14s.
- ≥ 3 companies running Spider in production and saying so publicly.
- Median "confusing error" report rate trending down release-over-release.

## 14. Review and sign-off checklist

Implementation (M1) begins only when each item is explicitly approved:

- [x] Vision, philosophy, non-goals (§1, §2, §6) — accepted as binding.
- [x] AI boundary: no AI in compilation path, three-tier model (§7 / ADR-003).
- [x] Memory model direction: inferred ownership + ARC hybrid with GC fallback preserved (LDD §9 / ADR-005).
- [x] Static, fully inferred, sound type system; no null; errors as values (LDD §7–8 / ADR-004).
- [x] Concurrency: lightweight tasks + structured concurrency, no function coloring (LDD §10 / ADR-006).
- [x] Tiered execution: Silk VM (dev/learn) + Cranelift AOT (release) (ADR-005).
- [x] Implementation language: Rust (ADR-001).
- [x] Indentation-based blocks, `let`/`var`, canonical formatter (ADR-002).
- [x] Capability-based package security as a launch feature (§8.4–8.5).
- [x] Open design questions in LDD §18 — each resolved or explicitly deferred.

---

*Next document: [0002 — Language Design Document](0002-language-design-document.md).*
