# Grammar Sketch

This is an implementation-oriented grammar sketch, not a formal complete BNF.

```text
file        = item*
item        = use | record | choice | shape | fn | test | stmt
use         = "use" ident ("." ident)*
record      = "record" name newline indent field* dedent
choice      = "choice" name newline indent variant* dedent
shape       = "shape" name newline indent fn-signature* dedent
fn          = ["public"] "fn" name "(" params ")" ["->" type] [where] block
test        = "test" string block
stmt        = let | var | assign | say | return | if | for | while | repeat
            | match | do-together | spawn | expr
block       = newline indent stmt* dedent
type        = name | "List" "of" type | "Maybe" "of" type
            | "Outcome" "of" type | "Map" "of" type "to" type
expr        = or-expr
```

The parser source remains authoritative.
