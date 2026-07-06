# Cookbook

## Print a List

```spider
say [1, 2, 3]
```

## Sort Text

```spider
let names = ["Grace", "Ada", "Lin"]
say names.sort()
```

## Safely Read a File

Requires `fs`.

```spider
use files

match files.read_text("notes.txt")
    Ok(text) -> say text
    Fail(problem) -> say "Could not read: {problem}"
```

Run:

```powershell
spider run --allow fs read.sp
```

## Deterministic Random

```spider
use random

random.seed(7)
say random.int(1, 6)
```

## A Simple Parser Shape

```spider
fn parse_age(text: Text) -> Outcome of Int
    if text.length() == 0
        return Fail("empty input")
    return Ok(21)
```

## Test a Function

```spider
fn inc(n: Int) -> Int
    return n + 1

test "inc"
    expect(inc(1), 2)
```
