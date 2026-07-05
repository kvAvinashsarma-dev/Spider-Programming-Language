fn parse_grade(score: Int) -> Outcome of Text
    if score < 0
        return Fail("scores start at zero")
    if score >= 90
        return Ok("A")
    if score >= 75
        return Ok("B")
    return Ok("keep going")

for score in [95, 80, 12, -3]
    match parse_grade(score)
        Ok(grade) -> say "{score}: {grade}"
        Fail(problem) -> say "{score}: oops — {problem}"

let fallback = try parse_grade(-1) else "?"
say "with fallback: {fallback}"
