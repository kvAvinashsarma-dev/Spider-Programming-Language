# Welcome

Spider is a programming language created by Kv Avinash Sarma with a simple surface and an industrial core.
Its repository currently contains the parser, formatter, semantic checker,
Silk bytecode VM, command-line tool, local package manager, LSP, browser
playground facade, corpus examples, milestone notes, and website.

This book is official documentation, not a marketing README. It documents what
the implementation in this repository can parse, check, format, compile, and
run today. When the roadmap and implementation disagree, this book follows the
implementation and names the discrepancy.

## A First Program

```spider
say "Hello, world!"

let name = ask "What is your name?"
say "Welcome, {name}!"
```

`say` prints a value. `ask` prints a prompt and returns text typed by the user.
String interpolation uses `{...}` holes parsed as Spider expressions, with one
current limitation: text literals inside interpolation holes are not supported
because interpolation is still scanned inside string tokens.

## The Shape of Spider Today

Spider files use the `.sp` extension. Blocks are indentation based. The
formatter has no options. The parser is lossless and error tolerant. The
checker performs name resolution, type checking, exhaustiveness checks, and
capability checks. The compiler lowers checked programs to Silk bytecode. The
VM executes with value semantics implemented by copy-on-write containers.

## What This Book Includes

This book includes a tutorial, language reference, standard-library reference,
diagnostics guide, CLI reference, package-manager guide, compiler internals
guide, VM bytecode reference, contributor guide, projects, cookbook, exercises,
answers, and a validation report.

## Exercise

1. Write a file named `hello.sp` that asks for a name and prints a greeting.
2. Run it with `spider run hello.sp`.
3. Format it with `spider fmt hello.sp`.

Answer:

```spider
let name = ask "Name?"
say "Hello, {name}!"
```
