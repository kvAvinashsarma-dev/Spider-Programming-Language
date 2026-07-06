# Language Reference

This chapter collects the implemented user-facing grammar in reference form.

## Top-Level Forms

```text
use path
record Name
choice Name
shape Name
fn name(params) -> Type
public fn name(params) -> Type
test "name"
statement
```

## Statements

```text
let name [: Type] = expr
var name [: Type] = expr
target = expr
target += expr
target -= expr
target *= expr
target /= expr
say expr
return [expr]
if expr block [else ...]
for name in expr block
while expr block
repeat expr times block
match expr arms
do together block
spawn expr
expr
```

`break` and `continue` are not implemented.

## Types

```text
Int
Float
Bool
Text
Nothing
List of T
Map of K to V
Maybe of T
Outcome of T
RecordName
ChoiceName
```

## Expressions

Expressions include literals, names, calls, method calls, field access,
indexing, lists, maps, ranges, unary operators, binary operators, `ask`,
`try`, and `match` as value expression where arm results agree.

## Operator Precedence

From tightest to loosest:

1. call, method call, indexing, field access
2. unary `-`
3. `*`, `/`, `%`
4. `+`, `-`
5. `to`
6. comparisons
7. `not`
8. `and`
9. `or`
