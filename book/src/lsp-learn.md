# LSP and Learn Mode

M6 partially shipped `spider lsp` and `spider learn`.

## Language Server

The LSP server communicates over stdio with Content-Length framing. It supports:

- initialize/shutdown lifecycle;
- diagnostics on open and change;
- keyword and stdlib hover text;
- keyword and stdlib completion.

It intentionally reuses the same parsing and checking pipeline as `spider
check`, so editor diagnostics match terminal diagnostics.

## Learn Mode

`spider learn file.sp` scans a file and lists concepts it uses. The concept
database also powers LSP keyword hovers.

## Example

```powershell
spider learn corpus\run\loops.sp
```

## Limitations

- Cross-file LSP project checking is not complete.
- Kids Mode is not implemented as a separate mode.
- In-string completions wait on lexer-level interpolation tokens.

