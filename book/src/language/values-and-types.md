# Values, Variables, and Types

Spider has immutable bindings, mutable variables, primitive values, collections,
records, choices, and typed failure values.

## Bindings

```spider
let pi = 3.14159
var score = 0
score += 5
```

Use `let` by default. Use `var` only when the name must be reassigned or
mutated.

## Type Annotations

```spider
let title: Text = "Spider"
let scores: List of Int = [90, 85, 77]
let ages: Map of Text to Int = {"Ada": 12, "Lin": 11}
```

The checker infers local `let` types. Public function boundaries must be
annotated.

## Primitive Types

- `Int`
- `Float`
- `Bool`
- `Text`
- `Nothing`

## Generic Types

```spider
List of Int
Map of Text to Int
Maybe of Text
Outcome of Int
```

## Value Semantics

Assignment behaves like a copy. The VM implements this with copy-on-write.

```spider
var a = [1, 2]
var b = a
b.push(3)
say a
say b
```

Output:

```text
[1, 2]
[1, 2, 3]
```

## Common Mistakes

- Assigning to a `let` binding.
- Mixing `Int` and `Float` without conversion.
- Leaving public function parameters unannotated.

