choice Color
    Red
    Green
    Blue

match Red
    Red -> say "warm"
    Green -> say "calm"

match Green
    Purple -> say "?"
    _ -> say "something else"

let answer = try 5 else 0
say answer
repeat "three" times
    say "nope"
