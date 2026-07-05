# Spider — Language Design Document (LDD)

| Field    | Value                                    |
|----------|------------------------------------------|
| Document | 0002 — Language Design Document          |
| Version  | 0.1.0                                    |
| Status   | **Ratified 2026-07-05** — OQ-1…OQ-8 resolved as recommended (ADR-009) |
| Date     | 2026-07-05                               |
| Authors  | Spider founding engineering team         |

> All code in this document is **illustrative and non-normative**. The grammar
> is frozen at M1, the semantics at M3, the spec at M10. Until then, syntax is
> a design proposal — argued for, but honestly revisable.

---

## 1. Design pillars

1. **Read out loud.** Spider code should be pronounceable as near-English
   while parsing with a deterministic, unambiguous, LL-style grammar. We take
   natural language's *ergonomics*, never its *ambiguity* (SRS NG1).
2. **One obvious way.** One loop keyword family, one error-handling story,
   one formatter output. Choice is a tax on readers.
3. **The beginner path is the default path.** Everything Lina writes is real
   Spider that Priya would sign off on. Expert power is additive, opt-in, and
   syntactically visible (SRS P5).
4. **Explicit boundaries, inferred interiors.** Public APIs are fully
   annotated; local code is fully inferred. The compiler does the paperwork
   inside functions; humans document the edges.
5. **Values first.** Value semantics by default; sharing and mutation are
   explicit. This is the root of Spider's memory and concurrency stories.

## 2. Syntax principles

- Blocks by indentation (4 spaces, formatter-enforced). No braces, no `end`.
  Rationale: the block structure a human *sees* is the one the compiler uses —
  an entire class of beginner bugs (mismatched braces) cannot exist. (ADR-002)
- No semicolons. One statement per line; explicit line continuation is never
  needed because expressions continue inside open brackets.
- No sigils on identifiers. `snake_case` values/functions, `CapWords` types.
- Keywords are common English words: `let`, `var`, `fn`, `if`, `else`, `for`,
  `in`, `match`, `try`, `use`, `public`, `spawn`, `say`, `ask`. Because
  English words make good variable names too, several keywords are
  *contextual*: `record`, `choice`, `shape`, `test`, `times`, `together`,
  and `where` are only special in their grammatical position, so
  `fn area(shape: Shape)` is legal (ADR-011).
- String interpolation with `{}`: `"Hello, {name}!"` — the one string feature
  every beginner needs on day one.
- Comments: `#` line comments; `##` doc comments (feed `spider doc`).
- Every construct must pass the ten-year-old test **or** be gated to Expert
  Mode lint level (never a different grammar — SRS NG7).

## 3. Surface tour (illustrative)

### 3.1 Hello

```spider
say "Hello, world!"
```

`say` prints a line. `ask` reads one:

```spider
let name = ask "What is your name?"
say "Welcome, {name}!"
```

### 3.2 Bindings

```spider
let pi = 3.14159          # let = does not change
var score = 0             # var = can change
score = score + 10
score += 5
```

Reassigning a `let` is error `E0102`, whose fix suggestion is "change `let` to
`var` if this value should be able to change."

### 3.3 Types (inferred, optionally written)

```spider
let age = 12                      # Int
let price = 4.99                  # Float
let happy = true                  # Bool
let name: Text = "Ada"            # annotation always allowed
let scores: List of Int = [90, 85, 77]
let ages: Map of Text to Int = {"Ada": 12, "Lin": 11}
```

### 3.4 Functions

```spider
fn greet(name: Text)
    say "Hello, {name}!"

fn area(width: Float, height: Float) -> Float
    return width * height

public fn total(prices: List of Float) -> Float
    ## Adds up every price in the list.
    var sum = 0.0
    for price in prices
        sum += price
    return sum
```

Parameter and return types are required on `public` functions (pillar 4);
inside a module the compiler may infer them.

### 3.5 Control flow

```spider
if age >= 13
    say "Teenager or older"
else if age >= 3
    say "Kid"
else
    say "Toddler"

for item in cart
    total += item.price

for i in 1 to 10
    say i

repeat 3 times
    say "Hip hip hooray!"

while lives > 0
    play_round()
```

`repeat N times` costs one grammar rule and eliminates the single most common
beginner loop mistake (off-by-one over an index nobody needed).

### 3.6 Records and choices (product & sum types)

```spider
record Point
    x: Float
    y: Float

choice Shape
    Circle(radius: Float)
    Rect(width: Float, height: Float)

fn area(shape: Shape) -> Float
    match shape
        Circle(r) -> 3.14159 * r * r
        Rect(w, h) -> w * h
```

`match` is exhaustive: a missing case is a compile error listing exactly which
cases are unhandled.

### 3.7 No null — `Maybe`

```spider
fn find_user(id: Int) -> Maybe of User

match find_user(42)
    Some(user) -> say "Found {user.name}"
    None -> say "No such user"
```

There is no null, nil, or undefined anywhere in the language. `E0410`
("you used a Maybe as if it were the value inside") is projected to be our
most-taught error and gets flagship Explain treatment.

### 3.8 Errors as values — `Outcome`

```spider
fn read_settings(path: Text) -> Outcome of Settings

# Handle explicitly:
match read_settings("app.cfg")
    Ok(settings) -> apply(settings)
    Fail(problem) -> say "Could not load settings: {problem}"

# Or use the two forms of try:
let settings = try read_settings("app.cfg") else default_settings()
fn load() -> Outcome of Settings
    let settings = try read_settings("app.cfg")   # Fail passes upward
    return Ok(settings)
```

`try expr else fallback` handles; bare `try expr` propagates (legal only where
the enclosing function returns `Outcome`). Full model in §8.

### 3.9 Concurrency in one screen

```spider
do together
    let weather = fetch_weather("Tokyo")
    let news = fetch_headlines()
say "{weather} — {news}"           # runs after both finish
```

`do together` is a structured-concurrency block: every line starts
concurrently; the block ends when all complete; a failure cancels siblings and
surfaces as one `Fail`. Long-lived tasks and channels in §10.

### 3.10 Capabilities are visible

```spider
use files                      # requires "fs" in web.toml capabilities
use net.http                   # requires "net"

let page = try http.get("https://example.com") else ""
```

A module that performs I/O must import the capability module; the manifest
must declare it; `web install` enforces it transitively (SRS FR-16).

### 3.11 AI as a language citizen (Tier 2)

```spider
use ai                         # requires "ai" capability

let helper = ai.model()        # resolved from project config — provider-agnostic

let summary: Text = try helper.ask("Summarize in one sentence: {report}")
    else "Summary unavailable."

record Sentiment
    mood: Text
    confidence: Float

let s: Sentiment = try helper.extract(review)    # typed structured output
```

Design guarantees: provider-agnostic (config, not code, picks the vendor);
capability-gated; every call returns `Outcome` (models fail; the language is
honest about it); `spider test` records fixtures so AI-using code tests
deterministically. The compiler never calls a model (SRS §7, NG2).

## 4. One language, three lenses

`--kids`, `--learn`, and default/Expert are **lint-and-presentation levels**:

| Lens | What changes | What never changes |
|------|--------------|--------------------|
| Kids | Vocabulary of diagnostics, colorized output, extra "what is a variable?" asides, stricter lints (e.g., warn on shadowing) | Grammar, types, runtime behavior |
| Learn | Diagnostics include concept explanations and links; `spider run --learn` narrates constructs | Same |
| Default | Standard diagnostics | Same |
| Expert | Enables lints *off* (e.g., allows explicit memory hints §9.4), shows MIR/perf annotations | Same |

CI enforces NG7: no grammar production or semantic rule may consult the mode.

## 5. Module system

- **One file = one module.** A directory with a `mod.sp` is a module with
  children. No headers, no manifests-per-module, no circular imports
  (cycle = compile error with the cycle path drawn).
- `use` brings modules in; names stay qualified by default:

```spider
use math
use net.http
use my_project.billing.invoices

say math.sqrt(2.0)
```

- `public` marks exports. Everything else is module-private. There is no
  `protected`/`internal` ladder in V1.
- Project layout is convention-fixed: `src/main.sp` (apps) or `src/lib.sp`
  (libraries); tests in `tests/` and inline `test` blocks.

## 6. Package manager — `web`

Manifest `web.toml` at project root:

```toml
[project]
name = "lemonade-stand"
version = "0.1.0"
spider = "1.0"

[capabilities]
allow = ["fs", "net"]        # this project's total I/O budget

[dependencies]
http = "2"
charts = { version = "1.4", capabilities = ["net"] }   # cap grant is explicit
```

- **Lockfile** `web.lock`: exact versions + content hashes; committed; builds
  are offline-reproducible from cache (SRS NFR-1).
- **Capability algebra:** a dependency's transitive capability set must be a
  subset of what the project grants it. `web install` prints a capability diff
  on every update; escalation requires an explicit manifest edit. This is the
  package-security headline feature (SRS FR-16, FR-19).
- **Registry:** content-addressed, signed publishes, namespace verification,
  static analysis on publish with public results, community ratings (post-1.0
  GA). No install scripts, ever.
- **Semver with teeth:** the registry runs API-diff on publish; a breaking
  change in a non-major version is rejected, not frowned at.

## 7. Type system

- **Static, sound, fully inferred locally.** Hindley–Milner-style inference
  extended with row-polymorphic records; annotations mandatory only at
  `public` boundaries. No `any`, no implicit conversions (not even
  `Int → Float`; `E0210` explains `to_float()`).
- **Nominal types** (`record`, `choice`) plus **structural interfaces**:

```spider
shape Drawable
    fn draw(self, canvas: Canvas)

fn render(items: List of Drawable, canvas: Canvas)
    for item in items
        item.draw(canvas)
```

  Any record with a matching `draw` satisfies `Drawable` — no `implements`
  ceremony. (Keyword `shape` is on-theme and beginner-pronounceable; see OQ-3.)
- **Generics** use the same readable spine: `List of T`, `Map of K to V`,
  `fn first(items: List of T) -> Maybe of T`. Constraints:
  `fn largest(items: List of T) -> Maybe of T where T is Comparable`.
- **Why not gradual typing:** gradual typing (TypeScript/Python-style) trades
  away the guarantees that make diagnostics, refactoring, and AOT performance
  possible, precisely to accommodate untyped code we don't have. Inference
  gives beginners the *feel* of dynamic typing with none of the runtime
  surprises. Decision recorded as ADR-004.

## 8. Error model

### 8.1 Two kinds of failure, never confused

- **Expected failure** (file missing, network down, bad input): a value —
  `Outcome of T` with `Ok(value)` / `Fail(problem)`. Handled with `match` or
  `try` (§3.8). Function signatures are honest: if it can fail, it says so.
- **Bug** (index out of range, integer overflow in checked context, violated
  invariant): a **panic** — the program stops with a full Spider Explain
  report. Panics are not catchable in ordinary code (SRS NG5); task
  boundaries (§10) are the only isolation points, mirroring Rust/Erlang
  practice.

### 8.2 `Problem` values

`Fail` carries a `Problem`: a structured value with a human message, a stable
code, optional cause chain, and optional data. Libraries define `choice`
problem types; applications usually pattern-match or propagate.

### 8.3 The Spider Explain diagnostic format (normative from M2)

```
error[E0102]: this value was made with `let`, so it cannot change
  --> src/main.sp:4:1
   |
 3 |     let score = 0
   |         ----- score was created here, as a value that never changes
 4 |     score = score + 10
   |     ^^^^^^^^^^^^^^^^^^ but this line tries to change it
   |
what happened: `let` makes a name for a value that stays the same.
why it's an error: Spider stops the change here so a value you relied on
    can't be different later without you knowing.
how to fix: if score should change, create it with `var`:
        var score = 0
suggested fix available: run `spider fix`, or apply from your editor.
learn more: spider explain E0102
```

Requirements: stable codes, all-errors-in-one-pass, machine-applicable fixes
where safe, `--json` for editors/AI agents, 100% Explain coverage of top-100
codes at 1.0 (SRS G6).

## 9. Memory model

**The hardest design bet in Spider** (SRS R1). Requirements in tension:
memory safety (non-negotiable), no GC pauses in production binaries
(performance goal), and *no beginner-visible ownership ceremony* (ten-year-old
test). No shipping language satisfies all three; here is our position:

### 9.1 Semantics first: values by default

`Int`, `Float`, `Bool`, `Text`, `record`s, `List`, `Map` behave as **values**:
assignment and argument passing behave as if copied. Beginners get spreadsheet
mental-model correctness; there is no observable aliasing in the default
language.

### 9.2 Implementation: inferred ownership, ARC fallback

The MIR pass pipeline decides, per allocation:

1. **Stack or inline** — escape analysis proves locality (majority of values).
2. **Unique heap** — single-owner move semantics, deterministic free
   (Lobster-style ownership inference; no user-visible lifetimes).
3. **Shared** — provably-shared values fall back to atomic reference counting
   with copy-on-write for the big collections; cycle detection runs on the
   `shared` type subset only (cycles are rare because the default is values).

Beginners see none of this. Priya can see all of it: Expert Mode surfaces the
inference results as editor annotations ("this list is copied here — 2 MB").

### 9.3 Escape hatch preserved (honesty about R1)

If M3–M4 user testing shows COW+ARC costs or semantics surprises we can't
engineer away, the Silk VM tier falls back to a tracing GC **without changing
language semantics** (value semantics are implementation-neutral by design).
The AOT tier keeps ownership+ARC. This fallback is architecturally protected:
nothing in the stdlib may depend on deterministic destruction timing except
via explicit `defer`/`with` resources.

### 9.4 Explicit control (Expert, post-1.0 candidate)

`with` blocks for deterministic resources (files, sockets, locks) ship in V1.
Arena/region hints and `borrowed` parameter annotations are drafted but gated
behind the post-1.0 performance RFC — not in the V1 surface.

## 10. Concurrency model

- **Lightweight tasks, no function coloring.** Any function may block; the
  Silk scheduler (and the AOT runtime) multiplexes tasks over OS threads,
  Go-style. There is no `async`/`await` split — the single worst
  learnability tax in modern mainstream languages. (ADR-006)
- **Structured by default.** `do together` (§3.9) and
  `for each ... together` (bounded parallel map) cover the 90% case with
  automatic joining, cancellation, and error propagation. Unscoped `spawn`
  returns a `Task` handle that *must* be awaited or explicitly detached
  (`E0703` otherwise) — no silent orphan tasks.
- **Data-race freedom by construction.** Value semantics (§9.1) mean tasks
  share nothing by default. Communication is via **channels**; transferring a
  unique value moves it. Shared mutable state requires an explicit
  `shared` cell type with atomic/locked access — visible in the code, checked
  by the compiler.

```spider
let jobs = channel of Text

spawn worker(jobs)
jobs.send("job-1")
```

## 11. Compiler architecture

```
source (.sp)
  → Lexer          hand-written; produces tokens + trivia (comments/whitespace preserved)
  → Parser         hand-written recursive descent; error-tolerant; emits lossless CST
  → AST            typed syntax layer over CST (rowan-style green trees)
  → HIR            desugared; name resolution; type inference; exhaustiveness
  → MIR            SSA control-flow graph; ownership/escape inference (§9.2);
                   optimizations (inline, DCE, const-prop, COW elision)
  → Backends       Silk bytecode (dev/REPL/Playground)
                   Cranelift AOT → native (release)          [M8]
                   WASM/WASI                                  [M8]
                   LLVM (post-1.0, peak optimization)
                   backend trait = extension point (GPU/quantum, §15)
```

- **Query-based incrementality** (Salsa-style): every stage is a memoized
  query keyed on inputs; edits invalidate minimally (SRS FR-5, G4).
- **Diagnostics as data:** every stage emits structured diagnostics consumed
  identically by CLI, LSP, and `--json`.
- **Implementation language: Rust** (ADR-001) — memory safety for the
  toolchain itself, mature ecosystem (rowan, salsa, cranelift), single-binary
  distribution, credible WASM story for the Playground.
- **Determinism:** no stage may read the clock, network, or environment;
  enforced by CI harness that diff-checks double builds (SRS NFR-1).

## 12. Runtime architecture (Silk)

- Register-based bytecode VM, precompiled stdlib image → **< 50 ms cold
  start** budget (SRS G2), measured in CI from M3 on.
- Work-stealing task scheduler (§10); non-blocking I/O under the hood.
- Capability enforcement at the syscall boundary (SRS FR-21).
- Deterministic single-threaded replay mode: instruction-level recording for
  the time-travel debugger (SRS FR-8) — designed in from day one because it
  is impossible to retrofit.
- Embedding API (C ABI) so Spider can script host applications — post-1.0
  surface, pre-1.0 internal (the Playground embeds Silk-on-WASM).
- AOT-compiled binaries link a slim runtime: scheduler, ARC support, panic
  reporting. No JIT in V1 (startup + determinism + supply-chain simplicity).

## 13. Tooling architecture

| Tool | Notes |
|------|-------|
| `spider fmt` | CST-based, lossless, idempotent, **zero options** — ends formatting debates before the community exists |
| `spider lsp` | Same query engine as the compiler — one source of truth for diagnostics; Learn-Mode-aware hovers |
| `spider test` | `test "adding prices"` blocks colocated with code; deterministic AI fixtures (§3.11) |
| `spider doc` | Doc comments + inferred signatures; missing docs on `public` = warning (SRS FR-12) |
| Debugger | DAP server over Silk; breakpoints, values *rendered as Spider literals*, record/replay alpha at M9 |
| Playground | Silk compiled to WASM; fully offline; every project gets `spider playground` |
| Visual tools | Dependency graph & project map generated from HIR (`spider doc --map`); memory visualizer post-1.0, reads Expert-Mode MIR annotations (§9.2) |

## 14. AI integration architecture

Restating the boundary (SRS §7, NG2): **Tier 1** toolchain assistants,
**Tier 2** language constructs, **Tier 3** never-in-semantics.

- **Tier 1 provider interface:** the toolchain talks to a local
  `ai-provider` process over a documented JSON protocol. Ships with: a
  no-network stub (default), an offline small-model provider (post-1.0), and
  adapters for hosted APIs (explicit opt-in + documented data flow). `spider
  explain` works fully offline from the authored database; `--ai` *adds*
  context-specific tutoring on top.
- **AI output is always labeled and never auto-applied.** `spider fix`
  applies only deterministic, compiler-authored fixes. AI suggestions are a
  separate, visually distinct channel (SRS R5).
- **Tier 2 (`use ai`)**: `Model` handles from project config; `ask`
  (text), `extract` (typed structured output via records — the killer
  feature: the type system defines the contract, the runtime validates it),
  `classify` (via `choice` types). All return `Outcome`. Fixture recording
  makes AI code testable and CI-safe.
- **Spider as the best language *for* AI authors:** regular grammar, one
  obvious way, `--json` diagnostics with machine-applicable fixes, and
  explicit capabilities give code-writing agents tight feedback loops and
  bounded blast radius. This is a strategy pillar, not a slogan (SRS §3).

## 15. Quantum readiness (honest scope)

Per SRS NG3, V1 ships exactly two things:

1. The backend trait in §11 — a documented, versioned interface by which a
   future `spider build --target quantum-sim` could lower MIR to a circuit
   IR. Designing MIR to not assume von-Neumann execution ordering *where it
   costs nothing* is cheap now and expensive later.
2. The `quantum` namespace: reserved in the stdlib and registry.

No qubit types, no simulators, no claims. When quantum hardware has a stable
programming model, Spider will have a socket to plug it into — that is the
entire promise, in writing.

## 16. Standard library roadmap

| Tier | Modules | Milestone |
|------|---------|-----------|
| Core (always loaded) | `Text`, `Int/Float/Bool`, `List`, `Map`, `Set`, `Maybe`, `Outcome`, `Range`, console (`say`/`ask`) | M4 |
| Std (import, no capability) | `math`, `time`, `random` (seedable), `json`, `regex` (readable-pattern API), `test` | M4–M6 |
| Capability-gated | `files` (fs), `net.http` client (net), `env` (env), `exec` (exec), `ai` (ai) | M4–M9 |
| Post-1.0 | `net.http` server, `sql`, `ui` (exploration), `crypto` (audited only), archive/compress, i18n | RFC track |

Stdlib rules: every public item documented with a runnable example; capability
modules never re-export each other; `random` is seedable for classroom
determinism; no third-party dependencies inside the stdlib.

## 17. USP triage — where every promised innovation lives

| Feature (from mission brief) | Home | When |
|---|---|---|
| Explain-everything errors | Compiler core (§8.3) | M2 |
| Learn / Kids / Beginner / Expert modes | Lint & presentation layers (§4) | M2–M6 |
| Smart formatter | `spider fmt` | M1 |
| Safe Mode & package security | Capabilities (§6, §10) | M4–M5 |
| Instant docs / doc-driven dev | `spider doc` + warnings | M9 |
| Playground / live preview | Silk-on-WASM | M6 |
| Visual debugger | DAP + Silk | M9 |
| Time-travel debugger | Replay substrate M3, alpha M9, GA post-1.0 | phased |
| Dependency graph / project map | `spider doc --map` | M9 |
| AI tutor / reviewer / test-gen / migration / security analyzer | Tier-1 toolchain | M9 → post-1.0 |
| AI in the language | Tier-2 `use ai` | M9 |
| Code health / readability / simplicity scores | Post-1.0 lint packs — **scores without proven metrics are noise; we ship them only with evidence** | post-1.0 |
| Community package rating | Registry | post-1.0 GA |
| GPU / heterogeneous | Backend extension point | post-1.0 |
| Quantum | §15 — extension point only | post-1.0 |
| Cloud native | AOT static binaries + WASI now; deploy tooling | post-1.0 |
| Accessibility & i18n | NFR-3 baseline now; translated diagnostics | post-1.0 |

## 18. Open design questions (each with a recommendation)

- **OQ-1 — Generics spelling:** `List of Int` (recommended: reads aloud,
  unambiguous, on-mission) vs `List[Int]` (denser, familiar to experts).
  Decide before M1 grammar freeze. *Recommend: `of`/`to` spine.*
- **OQ-2 — `Fail` naming:** `Fail(problem)` vs `Err(e)` (Rust-familiar) vs
  `Failure(...)`. *Recommend `Fail` — one syllable, honest, kid-clear.*
- **OQ-3 — Structural interface keyword:** `shape` (on-theme, but overloaded
  with geometry examples in teaching material) vs `trait` vs `can`.
  *Recommend `shape`; revisit if classroom testing shows confusion.*
- **OQ-4 — Integer semantics:** arbitrary-precision `Int` by default
  (beginner-correct, slower) vs 64-bit checked (fast, overflow panics).
  *Recommend 64-bit checked with panic + Explain; `BigInt` in std. Revisit
  after M3 benchmarks.*
- **OQ-5 — `web` manifest format:** TOML (recommended; comments, boring,
  proven) vs a Spider-syntax config (dogfooding, but chicken-and-egg at M1).
- **OQ-6 — String indexing:** expose code points, grapheme clusters, or no
  direct indexing (iterator/slice API only)? *Recommend no direct indexing —
  the only Unicode-honest answer — with excellent slice ergonomics.*
- **OQ-7 — Package manager name:** keep `web` per brief (SRS R4) or rename
  before the M5 registry makes it permanent. *Product decision; engineering
  flags searchability risk once, here, and complies with the outcome.*
- **OQ-8 — `do together` keyword phrasing:** finalize against alternatives
  (`together`, `all at once`) with classroom testing before M7.

## 19. Alternatives considered and rejected (summary)

| Decision | Rejected alternative | Why rejected |
|---|---|---|
| Static + inference | Gradual/dynamic typing | Destroys diagnostics, refactoring, AOT perf; runtime type errors are the worst beginner experience of all (ADR-004) |
| Errors as values | Exceptions | Invisible control flow fails P3; `try/else` gives the ergonomics without the lies (NG5) |
| Indentation blocks | Braces | Mismatched-delimiter bugs; visual structure ≠ real structure (ADR-002) |
| Tasks without coloring | async/await | Function coloring is a permanent complexity tax on every user (ADR-006) |
| Ownership-inference + ARC | Pure tracing GC everywhere | Gives up deterministic AOT performance story; but kept as protected fallback (§9.3, ADR-005) |
| | Rust-style explicit borrow checker | Fails the ten-year-old test categorically |
| Rust implementation | Self-hosting from day one | Bootstrap romanticism; delays every milestone (NG8) |
| No macros in V1 | Hygienic macro system | Multiplies dialects, breaks diagnostic quality guarantee (NG9) |

---

*Previous: [0001 — SRS](0001-software-requirements-specification.md) ·
Next: [0003 — Architecture Decision Log](0003-architecture-decision-log.md)*
