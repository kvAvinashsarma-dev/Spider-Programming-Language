# Diagnostics Reference

Spider diagnostics have stable codes. The implementation includes authored
explanations in `spider_syntax::diagnostics`.

## Syntax Diagnostics

Examples include:

- E0001: tabs in indentation.
- E0002: unexpected character.
- E0003: unterminated text literal.
- E0004: indentation does not line up.
- E0110: expected a value.
- E0115: expected a specific syntax kind.
- E0120: expected expression to end.
- E0150: nesting too deep.

## Semantic Diagnostics

Examples include:

- unknown names and did-you-mean suggestions;
- duplicate names;
- assigning through immutable bindings;
- type mismatches;
- non-`Bool` conditions;
- non-exhaustive `match`;
- wrong pattern arity;
- invalid `try`;
- missing capability E0244;
- imported module top-level side effect E0246.

## Runtime Panics

| Code | Meaning |
| --- | --- |
| E0301 | Integer divide by zero. |
| E0302 | Integer overflow. |
| E0303 | List position out of range. |
| E0304 | Missing map key. |
| E0305 | Sorting values with no ordering. |
| E0306 | Unknown module member at runtime defense layer. |
| E0307 | Recursion limit exceeded. |
| E0310 | Runtime capability denied. |
| E0311 | `expect` failed. |

## Using explain

```powershell
spider explain E0303
```

## Best Practices

- Treat diagnostics as part of the language.
- Prefer one root error over cascades.
- Use suggestions only when they point to valid code.
