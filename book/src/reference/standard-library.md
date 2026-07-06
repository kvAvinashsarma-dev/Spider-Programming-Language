# Standard Library Reference

The standard library registry is `spider_hir::stdlib`. It is the single source
of truth for signatures, docs, checker behavior, VM dispatch, and doc coverage.

## Capabilities

Known capability names:

| Capability | Meaning | Implemented module today |
| --- | --- | --- |
| `fs` | Filesystem access. | `files` |
| `net` | Network access. | reserved |
| `env` | Environment access. | reserved |
| `exec` | Process execution. | reserved |
| `ai` | AI provider access. | reserved |

## Modules

### math

| Member | Signature | Description |
| --- | --- | --- |
| `math.sqrt` | `Float -> Float` | Square root. |
| `math.pow` | `Float, Float -> Float` | Power. |
| `math.abs` | `T -> T` | Distance from zero. |
| `math.min` | `T, T -> T` | Smaller comparable value. |
| `math.max` | `T, T -> T` | Larger comparable value. |
| `math.floor` | `Float -> Int` | Round down. |
| `math.round` | `Float -> Int` | Round to nearest whole number. |
| `math.pi` | `Float` | Circle constant. |

### random

| Member | Signature | Description |
| --- | --- | --- |
| `random.seed` | `Int -> Nothing` | Set deterministic sequence seed. |
| `random.int` | `Int, Int -> Int` | Random integer between inclusive bounds. |
| `random.float` | `() -> Float` | Random float in `[0, 1)`. |

### files

Requires `fs`.

| Member | Signature | Description |
| --- | --- | --- |
| `files.read_text` | `Text -> Outcome of Text` | Read a file. |
| `files.write_text` | `Text, Text -> Outcome of Bool` | Write a file. |
| `files.exists` | `Text -> Bool` | Test path existence. |
| `files.list` | `Text -> Outcome of List of Text` | List directory names, sorted. |

## Methods

### Int

- `to_float() -> Float`
- `abs() -> Int`

### Float

- `to_int() -> Int`
- `round() -> Int`
- `floor() -> Int`
- `abs() -> Float`

### Text

- `length() -> Int`
- `upper() -> Text`
- `lower() -> Text`
- `trim() -> Text`
- `contains(Text) -> Bool`
- `split(Text) -> List of Text`

### List

- `length() -> Int`
- `first() -> Maybe of T`
- `last() -> Maybe of T`
- `sort() -> List of T`
- `reverse() -> List of T`
- `push(T) -> Nothing`
- `contains(T) -> Bool`

### Map

- `length() -> Int`
- `keys() -> List of K`
- `values() -> List of V`
- `has(K) -> Bool`

## Builtins

`expect(actual, expected)` is available everywhere. It is intended for
`spider test`.
