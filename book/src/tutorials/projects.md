# Projects

These projects use implemented Spider features. Networking and real HTTP are
not implemented today, so HTTP-related ideas are design exercises rather than
compilable current projects.

## Calculator

Build functions for add, subtract, multiply, divide, and test them.

```spider
fn add(a: Int, b: Int) -> Int
    return a + b

fn sub(a: Int, b: Int) -> Int
    return a - b

test "calculator"
    expect(add(2, 3), 5)
    expect(sub(5, 2), 3)
```

## Todo CLI Data Model

```spider
record Todo
    title: Text
    done: Bool

fn label(todo: Todo) -> Text
    if todo.done
        return "[x] {todo.title}"
    return "[ ] {todo.title}"
```

## JSON Parser Sketch

Current Spider can model parser results with `Outcome`, records, choices, and
lists, but the stdlib has no JSON module yet.

## Package Manager Exercise

Create a library with `src/lib.sp`, publish it locally with `web publish`, then
install it into another project with `web install`.

## Compiler Extension Exercise

Add a new stdlib method by changing the registry, checker dispatch, VM method
dispatch, tests, and this book.
