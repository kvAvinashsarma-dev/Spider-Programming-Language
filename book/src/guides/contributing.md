# Contributing

## Repository Structure

| Path | Purpose |
| --- | --- |
| `crates/spider-syntax` | Lexer, parser, CST, AST views, diagnostics. |
| `crates/spider-hir` | Semantic checker, types, stdlib registry, manifest caps. |
| `crates/spider-silk` | Bytecode compiler, VM, runtime values. |
| `crates/spider-fmt` | Formatter. |
| `crates/spider-web` | Project loader and `web` package manager. |
| `crates/spider-lsp` | Language server. |
| `crates/spider-cli` | `spider` command. |
| `corpus` | Golden syntax, format, check, run fixtures. |
| `docs` | SRS, LDD, ADRs, milestone notes. |
| `website` | Current static website assets. |

## Test Commands

```powershell
cargo test --workspace
```

## Standards

- Preserve parser losslessness.
- Add diagnostics with authored explanations.
- Update corpus goldens when syntax or formatting changes intentionally.
- Keep stdlib registry, checker, VM dispatch, tests, and docs in sync.
- Never document future features as implemented.

## Review Checklist

- Does it parse broken input without panic?
- Does it preserve source text?
- Does `spider fmt` stay idempotent?
- Does the checker avoid cascaded diagnostics?
- Does the VM preserve value semantics?
- Are capabilities enforced at check time and runtime?
