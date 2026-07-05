## Greets one person by name.
fn greet(name: Text)
    say "Hello, {name}!"
fn area(width: Float, height: Float) -> Float
    return width * height
public fn total(prices: List of Float) -> Float
    ## Adds up every price in the list.
    var sum = 0.0
    for price in prices
        sum += price
    return sum
fn largest(items: List of T) -> Maybe of T where T is Comparable
    return items.first()
fn main()
    greet("Ada")
    say total([1.5, 2.5])
