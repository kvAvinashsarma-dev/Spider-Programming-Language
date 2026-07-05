//! spider-syntax — lexer, lossless CST, and error-tolerant parser for Spider.
//!
//! Invariants this crate promises to every other part of the toolchain:
//! - `parse(src).root.text() == src` for **any** input (losslessness).
//! - Parsing never panics (fuzzed in `tests/fuzz_smoke.rs`).
//! - All diagnostics carry stable codes with authored Explain entries.

pub mod ast;
pub mod concepts;
pub mod diagnostics;
pub mod interpolation;
pub mod kind;
pub mod lexer;
pub mod parser;
pub mod tree;

pub use diagnostics::{explain, line_col, render, Diagnostic, Explain, Severity};
pub use kind::SyntaxKind;
pub use lexer::{lex, Token};
pub use parser::{parse, parse_expr_source, Parse};
pub use tree::{Element, Node};

/// Strips a UTF-8 byte-order mark, which some Windows editors prepend.
pub fn strip_bom(src: &str) -> &str {
    src.strip_prefix('\u{feff}').unwrap_or(src)
}
