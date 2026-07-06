# Formatter

`spider fmt` is implemented in `crates/spider-fmt`. It is canonical and has no
configuration.

## Rules

- Refuses to format files with parse errors.
- Normalizes whitespace and indentation.
- Preserves comments.
- Is idempotent: formatting twice produces the same output.
- Formatted output reparses cleanly.

## Motivation

The formatter is part of the language contract. Users should not spend
attention on style settings.
