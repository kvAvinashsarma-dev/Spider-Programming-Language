record Point
    x: Float
    y: Float

choice Shape
    Circle(radius: Float)
    Rect(width: Float, height: Float)
    Dot

fn area(shape: Shape) -> Float
    match shape
        Circle(r) -> 3.14159 * r * r
        Rect(w, h) -> w * h
        Dot -> 0.0

fn describe(shape: Shape) -> Text
    match shape
        Circle(r) -> "a circle of radius {r}"
        Rect(w, h) -> "a {w} by {h} rectangle"
        Dot -> "a dot"

fn main()
    let shapes = [Circle(1.0), Rect(2.0, 3.0), Dot]
    for shape in shapes
        say "{describe(shape)} — area {area(shape)}"
    let p = Point(3.0, 4.0)
    say "point: {p}"
