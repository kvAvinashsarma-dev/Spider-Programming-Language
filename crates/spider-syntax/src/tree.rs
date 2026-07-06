//! The lossless concrete syntax tree.
//!
//! Every token of the source — including whitespace and comments — is a leaf
//! in this tree, so `Node::text()` of the root reproduces the input exactly.
//! Typed AST accessors live in `ast.rs`; tools that need full fidelity (the
//! formatter, the future IDE) walk this tree directly.

use crate::kind::SyntaxKind;
use crate::lexer::Token;
use std::fmt::Write as _;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Element {
    Node(Rc<Node>),
    Token(Token),
}

#[derive(Debug)]
pub struct Node {
    pub kind: SyntaxKind,
    pub children: Vec<Element>,
}

impl Node {
    /// Reconstructs the exact source text covered by this node.
    pub fn text(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }

    fn collect_text(&self, out: &mut String) {
        for child in &self.children {
            match child {
                Element::Token(t) => out.push_str(&t.text),
                Element::Node(n) => n.collect_text(out),
            }
        }
    }

    /// Debug/golden dump: node structure plus significant tokens.
    /// Whitespace and layout tokens are omitted for readability; comments are
    /// kept because their placement is part of what we test.
    pub fn dump(&self) -> String {
        let mut s = String::new();
        self.dump_into(&mut s, 0);
        s
    }

    fn dump_into(&self, out: &mut String, depth: usize) {
        let _ = writeln!(out, "{}{:?}", "  ".repeat(depth), self.kind);
        for child in &self.children {
            match child {
                Element::Node(n) => n.dump_into(out, depth + 1),
                Element::Token(t) => match t.kind {
                    SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::Indent
                    | SyntaxKind::Dedent
                    | SyntaxKind::Eof => {}
                    _ => {
                        let _ =
                            writeln!(out, "{}{:?} {:?}", "  ".repeat(depth + 1), t.kind, t.text);
                    }
                },
            }
        }
    }

    // ----- direct-child accessors -----

    pub fn child_nodes(&self) -> Vec<&Rc<Node>> {
        self.children
            .iter()
            .filter_map(|e| match e {
                Element::Node(n) => Some(n),
                _ => None,
            })
            .collect()
    }

    pub fn child_tokens(&self) -> Vec<&Token> {
        self.children
            .iter()
            .filter_map(|e| match e {
                Element::Token(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    pub fn find_node(&self, kind: SyntaxKind) -> Option<&Rc<Node>> {
        self.child_nodes().into_iter().find(|n| n.kind == kind)
    }

    pub fn nodes_of(&self, kind: SyntaxKind) -> Vec<&Rc<Node>> {
        self.child_nodes()
            .into_iter()
            .filter(|n| n.kind == kind)
            .collect()
    }

    pub fn find_token(&self, kind: SyntaxKind) -> Option<&Token> {
        self.child_tokens().into_iter().find(|t| t.kind == kind)
    }
}
