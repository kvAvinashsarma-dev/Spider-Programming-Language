# Adding Language Features

## Adding Syntax

1. Add token kinds in `spider-syntax`.
2. Extend the lexer if new spelling is required.
3. Extend the parser with recovery behavior.
4. Add corpus examples and tree snapshots.
5. Update the formatter.
6. Add diagnostics and explanations.
7. Update this book.

## Adding Type Rules

1. Extend `Ty` if needed.
2. Add inference/checking rules.
3. Add positive and negative corpus examples.
4. Prevent diagnostic cascades.

## Adding Bytecode

1. Add an `Instr` variant.
2. Lower from the compiler.
3. Execute it in the VM loop.
4. Add runtime tests and semantics cases.
5. Document the instruction in the bytecode reference.

## Adding Standard Library APIs

1. Add the signature and doc to `spider_hir::stdlib`.
2. Add VM dispatch.
3. Add checker or method dispatch tests.
4. Add examples.
5. Update the standard library chapter.
