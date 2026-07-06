# Exercises and Answers

This chapter collects focused exercises. They are intentionally small enough to
compile against the current compiler.

## Basics

1. Print your name.

```spider
say "Avinash"
```

2. Store a number in `let` and print double.

```spider
let n = 21
say n * 2
```

3. Count from 1 to 3.

```spider
for i in 1 to 3
    say i
```

4. Write a mutable counter.

```spider
var count = 0
count += 1
say count
```

5. Write a function returning text.

```spider
fn greet(name: Text) -> Text
    return "Hello, {name}"
```

## Control Flow

6. Classify an age.

```spider
let age = 12
if age >= 13
    say "teen"
else
    say "kid"
```

7. Sum a range.

```spider
var total = 0
for i in 1 to 10
    total += i
say total
```

8. Use `repeat`.

```spider
repeat 3 times
    say "again"
```

9. Use `while`.

```spider
var n = 3
while n > 0
    say n
    n -= 1
```

10. Match a `Maybe`.

```spider
match [1, 2].first()
    Some(value) -> say value
    None -> say "empty"
```

## Data

11. Define a record.

```spider
record Point
    x: Float
    y: Float

let p = Point(1.0, 2.0)
say p.x
```

12. Define a choice.

```spider
choice Light
    Red
    Green

match Red
    Red -> say "stop"
    Green -> say "go"
```

13. Use a map.

```spider
let ages = {"Ada": 12}
say ages["Ada"]
```

14. Use a list method.

```spider
say [3, 1, 2].sort()
```

15. Use text methods.

```spider
say " Spider ".trim().upper()
```

## Testing

16. Test a function.

```spider
fn double(n: Int) -> Int
    return n * 2

test "double"
    expect(double(4), 8)
```

17. Seed random.

```spider
use random
random.seed(1)
say random.int(1, 6)
```

18. Handle an outcome.

```spider
fn ok() -> Outcome of Int
    return Ok(1)

match ok()
    Ok(value) -> say value
    Fail(problem) -> say problem
```

19. Use fallback `try`.

```spider
fn fail() -> Outcome of Int
    return Fail("no")

let value = try fail() else 0
say value
```

20. Show value semantics.

```spider
var a = [1]
var b = a
b.push(2)
say a
say b
```

## Challenge Problems

21. Write `factorial`.

```spider
fn factorial(n: Int) -> Int
    if n < 2
        return 1
    return n * factorial(n - 1)
```

22. Find the largest list element using `.sort().last()`.

```spider
match [3, 9, 1].sort().last()
    Some(value) -> say value
    None -> say "empty"
```

23. Convert a score to a grade with `Outcome`.

```spider
fn grade(score: Int) -> Outcome of Text
    if score < 0
        return Fail("negative")
    if score >= 90
        return Ok("A")
    return Ok("B")
```

24. Add a test for `grade`.

```spider
test "grade"
    expect(grade(95), Ok("A"))
```

25. Explain why `files.read_text` needs a capability.

Answer: it observes the host filesystem, so Safe Mode requires `fs`.
