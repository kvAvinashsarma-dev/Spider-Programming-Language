# Vision and Philosophy

Spider's thesis is that code can read plainly without becoming vague. It uses
small, pronounceable keywords such as `say`, `ask`, `let`, `var`, `fn`, `match`,
`repeat`, and `try`, but it is not natural-language programming. The grammar is
deterministic and implemented by a recursive-descent parser.

## Motivation

Spider is designed for learners, educators, production engineers, and tools.
The compiler tries to teach. Diagnostics include stable codes and explanation
entries. The formatter removes style debate. Safe Mode makes ambient power
visible by requiring explicit capabilities.

## Principles

- One obvious spelling for common ideas.
- Readable syntax with strict semantics.
- Errors as a product surface.
- Safe by default, capable by declaration.
- Deterministic builds and deterministic runtime behavior where possible.
- Tooling is part of the language, not an afterthought.

## History

The repository records its early history through milestone notes:

- M1 Hatchling: syntax, parser, formatter.
- M2 Spinneret: HIR-style checking, inference, diagnostics.
- M3 Silk: bytecode compiler, VM, run, repl, new.
- M4 Web-spinning: stdlib registry, capabilities, tests.
- M5 First Thread: modules and local package manager.
- M6 Loom: partially shipped LSP and Learn Mode.

## Behind the Scenes

The parser is lossless: every token, including comments and whitespace, remains
in the tree. That is why the formatter, LSP, and future refactoring tools can
operate without destroying user context.

## Common Mistakes

- Treating Spider as English. It is not; it only borrows readable words.
- Assuming roadmap features exist. The compiler decides what is real.
- Expecting invisible package permissions. Capabilities are explicit.

## Best Practices

- Run `spider check` before `spider run` when learning.
- Prefer `let` until mutation is required.
- Keep public functions annotated.
- Use `Outcome` for expected failure and let panics represent bugs.

## Exercise

Explain why `say "hi"` is valid Spider but "print hi" is not.

Answer: Spider has a fixed grammar. `say` is the implemented output keyword;
free English commands are not parsed.
