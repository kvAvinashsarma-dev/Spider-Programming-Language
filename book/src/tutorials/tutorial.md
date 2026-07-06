# Tutorial

## Lesson 1: Hello

```spider
say "Hello, world!"
```

## Lesson 2: Input

```spider
let name = ask "What is your name?"
say "Welcome, {name}!"
```

## Lesson 3: Branches

```spider
let age = 12
if age >= 13
    say "teen"
else
    say "kid"
```

## Lesson 4: Loops

```spider
var total = 0
for i in 1 to 10
    total += i
say total
```

## Lesson 5: Functions

```spider
fn double(n: Int) -> Int
    return n * 2

say double(21)
```

## Lesson 6: Records and Choices

```spider
record Point
    x: Float
    y: Float

choice Shape
    Circle(radius: Float)
    Dot
```

## Lesson 7: Tests

```spider
test "math works"
    expect(1 + 1, 2)
```

## Challenge

Write a program that sums the even numbers from 1 to 20.

Answer:

```spider
var total = 0
for n in 1 to 20
    if n % 2 == 0
        total += n
say total
```
