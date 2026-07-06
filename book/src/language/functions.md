# Functions

## Declaration

```spider
fn greet(name: Text)
    say "Hello, {name}!"
```

## Return Types

```spider
fn area(width: Float, height: Float) -> Float
    return width * height
```

## Public Functions

```spider
public fn total(prices: List of Float) -> Float
    var sum = 0.0
    for price in prices
        sum += price
    return sum
```

Public functions cross module boundaries. Public parameters must have
annotations.

## Generics

```spider
fn largest(items: List of T) -> Maybe of T where T is Comparable
    return items.first()
```

Implemented built-in constraints are `Comparable`, `Equatable`, and
`Printable`.

## Recursion

```spider
fn fib(n: Int) -> Int
    if n < 2
        return n
    return fib(n - 1) + fib(n - 2)
```

The VM uses explicit frames. Recursion is limited to 1000 calls and reports
E0307 when exceeded.

## Closures

Closures and anonymous functions are not implemented in the current grammar.
