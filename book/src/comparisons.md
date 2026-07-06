# Spider Compared

This chapter compares Spider's implemented design with familiar languages.

## Python

Spider shares indentation-based blocks and beginner-friendly readability with
Python. Unlike Python, Spider is statically checked before execution and has
explicit `Outcome` and `Maybe` values instead of exceptions and null.

## Rust

Spider shares safety ambitions with Rust but does not expose borrow checking in
the current user surface. Value semantics are implemented in the VM with
copy-on-write containers. Spider's capability model is also part of the
language and package toolchain.

## Go

Spider shares simple tooling and a preference for readable code with Go. Go's
current implementation has goroutines; Spider's concurrency syntax exists but
executes sequentially until M7.

## Java

Spider avoids mandatory classes for simple programs. It has records and choices
today, not class inheritance.

## C#

C# has a broad industrial runtime and many language constructs. Spider is much
smaller today. Its implemented surface favors few keywords, no nullable values,
and deterministic package capability checks.

## JavaScript

Spider is not dynamically typed JavaScript. It checks names, types, module
visibility, match exhaustiveness, and capabilities before running.

## Summary

Spider's current identity is closest to "Python-like readability with a
static checker, Rust-like honesty about safety, and a small integrated toolchain."
