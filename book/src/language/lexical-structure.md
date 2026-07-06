# Lexical Structure

Spider source is UTF-8 text in `.sp` files. The current lexer accepts ASCII
identifiers beginning with a letter or `_`, followed by letters, digits, or `_`.

## Comments

```spider
# A normal comment.
## A documentation comment.
```

## Strings

```spider
let greeting = "Hello"
let message = "Hello, {name}!"
```

Escapes supported by the interpolation scanner include `\n`, `\t`, `\"`, `\\`,
`\{`, and `\}`.

## Numbers

```spider
let whole = 1_000_000
let decimal = 3.14
let tiny = 2.5e-3
let negative = -42
```

`Int` is 64-bit checked. `Float` follows IEEE behavior and does not panic for
divide-by-zero.

## Indentation

Blocks are indentation based. Tabs in indentation produce E0001. Indentation
must line up with an earlier indentation level.

```spider
if true
    say "indented"
```

## Soft Keywords

`record`, `choice`, `shape`, `test`, `times`, `together`, and `where` are
contextual. They can be names when a name is expected:

```spider
let times = 3
repeat times times
    say "again"
```

Hard structural words such as `let`, `var`, `fn`, `if`, `else`, `for`, `while`,
`match`, `try`, `use`, `return`, `say`, `ask`, `spawn`, `do`, `and`, `or`,
`not`, `true`, `false`, `public`, `to`, `of`, `is`, and `in` are reserved.
