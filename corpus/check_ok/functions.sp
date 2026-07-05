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

public fn distance(a: Point, b: Point) -> Float
    let dx = a.x - b.x
    let dy = a.y - b.y
    return dx * dx + dy * dy

fn main()
    let p = Point(1.0, 2.0)
    let q = Point(4.0, 6.0)
    say distance(p, q)
    say area(Circle(2.0))
