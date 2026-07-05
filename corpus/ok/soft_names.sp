# Soft keywords (ADR-011): common English words are only special in their
# grammatical position. Everywhere a name is expected, they are plain names.
let times = 3
say times + 1
var record = 1
record += 1
let test = record * times
say test

record Inventory
    shape: Text
    where: Text

fn describe(shape: Inventory) -> Text
    return shape.where

repeat times times
    say "spinning"
