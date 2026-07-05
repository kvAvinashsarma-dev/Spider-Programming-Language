fn parse_age(text: Text) -> Outcome of Int
    if text.length() > 0
        return Ok(21)
    return Fail("empty input")

fn describe(age: Maybe of Int) -> Text
    match age
        Some(value) -> "age {value}"
        None -> "unknown"

fn main()
    let age = try parse_age("21") else 0
    say age
    match parse_age("")
        Ok(value) -> say value
        Fail(problem) -> say "could not parse: {problem}"
    say describe(Some(12))
