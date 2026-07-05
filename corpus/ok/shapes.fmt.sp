record Point
    x: Float
    y: Float
choice Shape
    Circle(radius: Float)
    Rect(width: Float, height: Float)
    Dot
shape Drawable
    fn draw(self, canvas: Canvas)
    fn area(self) -> Float
fn area(figure: Shape) -> Float
    match figure
        Circle(r) -> 3.14159 * r * r
        Rect(w, h) -> w * h
        Dot -> 0.0
