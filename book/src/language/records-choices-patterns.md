# Records, Choices, and Patterns

## Records

```spider
record Point
    x: Float
    y: Float

let p = Point(3.0, 4.0)
say p.x
```

Records are constructed with call syntax. Fields are accessed with dot syntax.

## Choices

```spider
choice Shape
    Circle(radius: Float)
    Rect(width: Float, height: Float)
    Dot
```

Choices are tagged alternatives. Constructors also use call syntax when they
carry fields.

## Pattern Matching

```spider
fn area(shape: Shape) -> Float
    match shape
        Circle(r) -> 3.14159 * r * r
        Rect(w, h) -> w * h
        Dot -> 0.0
```

Patterns can match variant tags and bind fields. `_` is the wildcard pattern.

## Maybe and Outcome

```spider
match maybe_name
    Some(name) -> say name
    None -> say "missing"

match result
    Ok(value) -> say value
    Fail(problem) -> say problem
```

## Shapes

`shape` declarations parse and format. Full interface/trait semantics are not
implemented yet.

## Common Mistakes

- Omitting a case in `match`.
- Using a case name that does not belong to the choice.
- Giving a pattern the wrong number of fields.
