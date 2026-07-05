# Milestone M2 "Spinneret" — Architecture Notes

| Field    | Value                                    |
|----------|-------------------------------------------|
| Document | 0005 — M2 milestone notes                 |
| Status   | Shipped 2026-07-05                        |
| Scope    | Name resolution, type-inference core, contextual keywords (ADR-011), semantic diagnostics, 59 authored error codes, semantic golden corpus |

## 1. What shipped

- **`spider-hir`** — the new semantic crate:
  - **Span map**: the lossless CST stores no positions; one walk derives every
    node's character range, so semantic diagnostics point exactly at code.
  - **Name resolution**: two passes (names, then signatures) so declaration
    order never matters at the top level; block scopes with `let`/`var`
    mutability, shadowing rules, and unused-name warnings (`W0001`).
  - **Type inference**: unification-based (Hindley–Milner core) with
    `List/Map/Maybe/Outcome`, records, choices, function types, and signature
    type parameters (`T`) instantiated fresh at each call site. `let` needs no
    annotations anywhere; annotations are checked when written.
  - **Match analysis**: exhaustiveness for choices, `Maybe`, `Outcome`, and
    `Bool` (`E0230` lists exactly the missing cases), unknown-case detection
    with suggestions (`E0231`), arity checks (`E0232`), unreachable-arm
    detection (`E0234`).
  - **`try` rules**: only on `Outcome`/`Maybe` (`E0235`); a bare `try`
    requires an `Outcome`-returning function (`E0236`).
  - **"Did you mean"**: every unknown name/field/case/module suggests the
    closest known candidate (edit distance, stricter for short names).
- **Contextual keywords (ADR-011)** in the parser: `record`, `choice`,
  `shape`, `test`, `times`, `together`, `where` are keywords only in their
  grammatical position — everywhere a *name* is expected they are ordinary
  identifiers. The M1 finding case `fn area(shape: Shape)` — written in our
  own LDD and unparseable in M1 — now parses, checks, and is a regression
  test in both corpora.
- **Severity** on diagnostics (error/warning), and **59 authored Explain
  codes** (`spider explain` covers all of them; a unit test enforces the
  ≥50 exit gate and that every listed code is authored).
- `spider check` now runs the full front half: parse → (if clean) resolve +
  infer. Exit code reflects errors only; warnings don't fail builds.

## 2. Design rulings recorded this milestone (ADR-011, ADR-012)

1. **Contextual keywords** — hard keywords are the small structural set
   (`let var fn if else for in to while repeat match try use return say ask
   spawn do and or not true false of is public`); the seven soft keywords
   above are positional. `to`, `of`, `is`, `in` stay hard: they are operators.
2. **Paren-less method arguments rejected** — the LDD's `helper.ask "text"`
   sketch is amended to `helper.ask("text")`; the built-in `ask expr` prefix
   form is unaffected. Grammar ambiguity is not worth one pair of parentheses.
3. **Match arms** accept an expression or a `say` line; arms of a
   value-producing match must agree in type (`E0240`); any `say` arm makes
   the match a statement.
4. **Trailing expression is the implicit return** for non-`Nothing`
   functions (this is how the LDD's `fn area` body works with no `return`).
5. **Unannotated non-public parameters are unchecked (`Any`)** — honest gap;
   cross-function inference is deliberately deferred rather than half-shipped.
   Public boundaries must be annotated (`E0208`), per LDD pillar 4.
6. **Record/variant constructors** are call-syntax (`Point(1.0, 2.0)`),
   provisional until the M4 stdlib design revisits construction syntax.
7. **`_`-prefixed names** (and `_`) opt out of unused warnings.

## 3. Honest limitations (tracked for M3/M4)

- String interpolation is still one token. Names inside `{…}` are **marked
  used** (so `W0001` can never fire falsely — this bug was caught by the
  corpus and fixed) but not resolved or type-checked. M3 tokenizes
  interpolation properly.
- No definite-return analysis: a non-`Nothing` function that can fall off the
  end without returning is not yet an error (no false positives this way;
  real analysis lands with MIR in M3).
- Module members (`math.sqrt`) are unchecked until the stdlib is typed (M4).
- Top-level script variables are not visible inside functions (script-mode
  capture rules are an M3 design question alongside the Silk VM).

## 4. Test inventory (39 tests)

| Suite | Proves |
|---|---|
| spider-syntax units (13) + goldens (3) + fuzz (3) | M1 invariants still hold under the contextual-keyword grammar change |
| spider-hir units (13) | every major E02xx/W code fires on a minimal case; clean programs stay clean; one-mistake-one-diagnostic |
| spider-hir ty units (3) | unifier laws incl. occurs check |
| golden_check (2) | check_ok corpus is semantically clean (inference sound — the M2 exit criterion); check_err diagnostics snapshot-frozen with positions |
| spider-fmt units (4) + goldens (1) | formatter laws over the extended corpus (incl. soft_names.sp) |
| diagnostics unit (1) | all 59 codes authored; ≥50 gate enforced in CI forever |

## 5. Exit criteria status (SRS §11, M2 row)

- ✅ **Top 50 error codes fully authored** — 59 codes with what/why/fix, all
  reachable from real checks, coverage CI-enforced.
- ✅ **Inference sound on test corpus** — `check_ok` corpus (scripts,
  records/choices/match, Outcome/try, generics with constraints) checks
  clean; unsoundness would fail `check_ok_corpus_is_semantically_clean`.

## 6. Next: M3 "Silk"

Bytecode compiler + the Silk VM: `spider run`, the REPL, real string
interpolation in the lexer, and the < 50 ms cold-start budget measured in CI.
