fn largest(items: List of T) -> Maybe of T where T is Comparable
    return items.first()

fn main()
    match largest([3, 1, 2])
        Some(value) -> say value + 1
        None -> say "empty list"
    match largest(["b", "a"])
        Some(word) -> say word.upper()
        None -> say "no words"
