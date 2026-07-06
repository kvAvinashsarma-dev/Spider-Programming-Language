# Collections and Outcomes

## Lists

```spider
let names = ["Ada", "Grace", "Alan"]
say names[0]
say names.length()
say names.first()
```

Lists support `length`, `first`, `last`, `sort`, `reverse`, `push`, and
`contains`.

## Maps

```spider
let ages = {"Ada": 12, "Lin": 11}
say ages["Ada"]
say ages.keys()
say ages.has("Lin")
```

Maps preserve insertion order in the VM.

## Maybe

`Maybe of T` is either `Some(T)` or `None`.

```spider
match [1, 2].first()
    Some(value) -> say value
    None -> say "empty"
```

## Outcome

`Outcome of T` is either `Ok(T)` or `Fail(Text-like problem)`.

```spider
fn parse_grade(score: Int) -> Outcome of Text
    if score < 0
        return Fail("scores start at zero")
    return Ok("pass")
```

## try

```spider
let fallback = try parse_grade(-1) else "?"
```

Bare `try` inside an `Outcome`-returning function propagates failure. A bare
`try` on `Maybe` propagates `Fail("the value was missing")`.
