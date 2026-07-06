# Expressions and Operators

## Arithmetic

```spider
say 1 + 2 * 3
say (1 + 2) * 3
say 10 / 2
say 10 % 3
```

Integer divide-by-zero panics with E0301. Integer overflow panics with E0302.

## Comparison and Logic

```spider
if age > 3 and age < 13 or not false
    say "boolean logic reads like a sentence"
```

Comparison operators are `==`, `!=`, `<`, `<=`, `>`, and `>=`.

## Calls

```spider
say area(3.0, 4.0)
say "hello".upper()
say [3, 1, 2].sort()
```

User functions and methods use parentheses. The built-in `say` and `ask` forms
are prefix forms.

## Indexing and Assignment

```spider
var scores = [90, 85]
scores[1] = 100

var ages = {"Ada": 12}
ages["Sam"] = 13
```

List indexes outside range panic with E0303. Missing map keys panic with E0304.

## Ranges

```spider
for i in 1 to 10
    say i
```

`to` forms an inclusive range in current loop behavior.
