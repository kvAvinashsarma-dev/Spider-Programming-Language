# Compiler Architecture

Spider's implemented pipeline is:

```mermaid
flowchart LR
    Source[".sp source"] --> Lexer["Lexer"]
    Lexer --> CST["Lossless CST"]
    CST --> AST["Typed AST views"]
    AST --> Check["Name and type checker"]
    Check --> Project["Project merge and module visibility"]
    Project --> Bytecode["Silk bytecode compiler"]
    Bytecode --> VM["Silk VM"]
```

## Lexer

The lexer produces tokens and diagnostics. It also inserts zero-width
`Indent`/`Dedent` tokens. Newlines inside brackets are trivia.

## Parser

The parser is recursive descent and error tolerant.

```mermaid
flowchart TD
    Tokens --> SourceFile
    SourceFile --> Statements
    Statements --> Declarations
    Statements --> Expressions
    Expressions --> Recovery["line-based recovery"]
```

Guarantees:

- never panic on any input;
- preserve every byte of input in the tree;
- report independent errors in source order;
- cap nesting depth.

## Semantic Checker

The checker resolves names, checks mutability, infers types, checks module
members against the stdlib registry, verifies match exhaustiveness, validates
`try`, and applies capability policy.

```mermaid
flowchart LR
    Parse --> Names["collect names"]
    Names --> Signatures["collect signatures"]
    Signatures --> Bodies["check bodies"]
    Bodies --> Diagnostics
```

## Project Checker

M5 adds `check_project`, which merges type names, per-module signatures, exports
and constructors, then checks each module body with the merged world.

## Bytecode Generator

The compiler lowers CST-shaped typed programs to register instructions. It
assumes the checker has already accepted the program; inconsistencies are
toolchain bugs, not user diagnostics.

