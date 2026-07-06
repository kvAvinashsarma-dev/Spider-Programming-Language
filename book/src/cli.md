# Spider CLI

The `spider` command is implemented in `crates/spider-cli/src/main.rs`.

## Commands Implemented Today

| Command | Purpose |
| --- | --- |
| `spider run <file.sp|project-dir>` | Check, compile, and run through Silk. |
| `spider test [path]` | Run every `test "..."` block under a path. |
| `spider new <name>` | Create a project with `web.toml`, `src/main.sp`, and README. |
| `spider repl [--allow cap]` | Interactive Spider session. |
| `spider fmt <paths...>` | Format `.sp` files in place. |
| `spider fmt --check <paths...>` | Report files that would be reformatted. |
| `spider check <file.sp>` | Parse, resolve, type-check, and explain problems. |
| `spider tree <file.sp>` | Print the syntax tree for debugging. |
| `spider tokens <file.sp>` | Print the token stream for debugging. |
| `spider explain <CODE>` | Print an explanation for a diagnostic code. |
| `spider learn <file.sp>` | List concepts used by a file. |
| `spider lsp` | Run the language server over stdio. |
| `spider --version` | Show the toolchain version. |

## Safe Mode

Programs start with zero capabilities. Use `web.toml` or `--allow` to grant a
capability for a run:

```powershell
spider run --allow fs script.sp
spider repl --allow fs
```

Known capability names are `fs`, `net`, `env`, `exec`, and `ai`. Only `fs` has
implemented standard-library operations today, through the `files` module.

## Roadmap Commands Not Implemented

The public roadmap mentions `spider build`, `spider doc`, and package-oriented
commands. In this repository:

- `spider build` is explicitly reported as coming later.
- `spider doc` is not implemented.
- Package operations live in the separate `web` executable.

## Common Mistakes

- Running `spider check src` currently expects a file path, not a whole source
  directory.
- Running file I/O without `--allow fs` or a manifest grant produces E0244 at
  check time or E0310 at runtime.

## Exercise

Create a project and run it.

Answer:

```powershell
spider new hello-spider
cd hello-spider
spider run .
```
