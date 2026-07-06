# Glossary

Capability: A named permission such as `fs` that a program or package must
declare before using gated APIs.

CST: Concrete syntax tree. Spider's CST is lossless.

HIR: Higher-level semantic representation. The repository uses `spider-hir`
for checking even though the public structure is still close to syntax.

Outcome: A typed expected-failure value, either `Ok(value)` or `Fail(problem)`.

Maybe: An optional value, either `Some(value)` or `None`.

Silk: Spider's bytecode virtual machine.

Soft keyword: A word that is only a keyword in specific grammar positions.

Value semantics: Assignment behaves like a copy.
