# Control Flow

## if and else

```spider
if age >= 13
    say "Teenager or older"
else if age >= 3
    say "Kid"
else
    say "Toddler"
```

Conditions must be `Bool`.

## while

```spider
var lives = 3
while lives > 0
    say lives
    lives -= 1
```

## for

```spider
for i in 1 to 3
    say i

for item in [1, 2, 3]
    say item * 2
```

## repeat

```spider
repeat 3 times
    say "Hip hip hooray!"
```

The expression before `times` must be an `Int`.

## match

```spider
match parse_age("")
    Ok(value) -> say value
    Fail(problem) -> say "could not parse: {problem}"
```

The checker performs exhaustiveness checks for choices, `Maybe`, `Outcome`, and
`Bool`.

## return

```spider
fn double(n: Int) -> Int
    return n * 2
```

Functions with non-`Nothing` return types may also end with a trailing
expression.

## Not Implemented

There is no implemented `break` or `continue` command in the current grammar.
