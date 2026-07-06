# Testing

Spider has built-in test blocks.

```spider
test "adding works"
    let result = 1 + 1
    expect(result, 2)
```

Run tests with:

```powershell
spider test .
```

Each test runs on a fresh VM. Tests cannot depend on state from another test.
`expect(actual, expected)` stops the test with E0311 if values differ.

`test` blocks do not run under `spider run`.

## Best Practices

- Keep tests beside the code they describe.
- Use deterministic inputs.
- Use `random.seed` when testing random behavior.

