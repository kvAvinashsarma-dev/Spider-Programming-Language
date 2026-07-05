//! spider-fmt — the canonical Spider formatter.
//!
//! Rules (there are no options — SRS P1):
//! - indentation is exactly 4 spaces per block level
//! - one space around binary operators, `=`, and `->`; none inside brackets
//! - `, ` after commas; `: ` in annotations and map entries
//! - runs of blank lines collapse to one; files end with exactly one newline
//! - line endings normalize to `\n`
//! - comments are preserved; a comment trailing code is separated by two spaces
//!
//! The formatter refuses files that do not parse: formatting must never guess.

use spider_syntax::{parse, Diagnostic, Element, Node, SyntaxKind as K};
use std::rc::Rc;

/// Formats Spider source. Returns `Err(diagnostics)` if the file has parse
/// errors — a formatter that edits broken code destroys the user's context
/// for fixing it.
pub fn format_source(src: &str) -> Result<String, Vec<Diagnostic>> {
    let src = spider_syntax::strip_bom(src);
    let normalized = src.replace("\r\n", "\n").replace('\r', "\n");
    let p = parse(&normalized);
    if !p.diagnostics.is_empty() {
        return Err(p.diagnostics);
    }
    let mut f = Fmt {
        out: String::new(),
        indent: 0,
        pending: Vec::new(),
    };
    f.container(&p.root);
    if f.out.trim().is_empty() {
        return Ok(String::new());
    }
    let mut out = f.out.trim_end().to_string();
    out.push('\n');
    Ok(out)
}

struct Fmt {
    out: String,
    indent: usize,
    /// Comments collected from inside the current line, emitted at its end.
    pending: Vec<String>,
}

impl Fmt {
    fn ind(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
    }

    fn line_end(&mut self) {
        if !self.pending.is_empty() {
            self.out.push_str("  ");
            let joined = self.pending.join("  ");
            self.out.push_str(&joined);
            self.pending.clear();
        }
        self.out.push('\n');
    }

    /// Walks the statement-bearing children of SourceFile or Block, handling
    /// blank-line collapsing and standalone comments.
    fn container(&mut self, node: &Node) {
        let mut saw_content = false;
        let mut pending_blank = false;
        let mut skip_newline = false;
        for el in &node.children {
            match el {
                Element::Token(t) => match t.kind {
                    K::Newline => {
                        if skip_newline {
                            skip_newline = false;
                        } else if saw_content {
                            pending_blank = true;
                        }
                    }
                    K::Comment | K::DocComment => {
                        if pending_blank {
                            self.out.push('\n');
                            pending_blank = false;
                        }
                        self.ind();
                        self.out.push_str(t.text.trim_end());
                        self.out.push('\n');
                        saw_content = true;
                        skip_newline = true;
                    }
                    _ => {}
                },
                Element::Node(n) => {
                    if pending_blank {
                        self.out.push('\n');
                        pending_blank = false;
                    }
                    skip_newline = false;
                    self.stmt(n);
                    saw_content = true;
                }
            }
        }
    }

    fn block(&mut self, node: Option<&Rc<Node>>) {
        self.line_end();
        if let Some(b) = node {
            self.indent += 1;
            self.container(b);
            self.indent -= 1;
        }
    }

    /// Collects comments buried inside a statement's expression parts so they
    /// can be re-emitted at the end of the line. Does not descend into nested
    /// blocks — those handle their own comments.
    fn collect_comments(&mut self, node: &Node) {
        for el in &node.children {
            match el {
                Element::Token(t) if matches!(t.kind, K::Comment | K::DocComment) => {
                    self.pending.push(t.text.trim_end().to_string());
                }
                Element::Node(n) if !matches!(n.kind, K::Block | K::ElseClause) => {
                    self.collect_comments(n);
                }
                _ => {}
            }
        }
    }

    fn stmt(&mut self, n: &Rc<Node>) {
        self.collect_comments(n);
        self.ind();
        match n.kind {
            K::LetStmt | K::VarStmt => self.binding(n),
            K::AssignStmt => self.assign(n),
            K::ExprStmt => {
                let e = self.exprs_of(n);
                if let Some(x) = e.first() {
                    self.expr(x);
                }
                self.line_end();
            }
            K::SayStmt => self.kw_expr("say", n),
            K::SpawnStmt => self.kw_expr("spawn", n),
            K::ReturnStmt => {
                self.out.push_str("return");
                let e = self.exprs_of(n);
                if let Some(x) = e.first() {
                    self.out.push(' ');
                    self.expr(x);
                }
                self.line_end();
            }
            K::UseDecl => self.use_decl(n),
            K::IfStmt => self.if_stmt(n),
            K::ForStmt => self.for_stmt(n),
            K::WhileStmt => self.header_expr_block("while", n),
            K::RepeatStmt => self.repeat_stmt(n),
            K::DoTogetherStmt => {
                self.out.push_str("do together");
                self.block(n.find_node(K::Block));
            }
            K::MatchStmt => self.header_expr_block("match", n),
            K::MatchArm => self.match_arm(n),
            K::FnDecl => self.fn_decl(n, false),
            K::FnSig => self.fn_decl(n, true),
            K::RecordDecl => self.named_decl("record", n),
            K::ChoiceDecl => self.named_decl("choice", n),
            K::ShapeDecl => self.named_decl("shape", n),
            K::TestDecl => self.test_decl(n),
            K::FieldDecl => self.field_decl(n),
            K::VariantDecl => self.variant_decl(n),
            _ => {
                // Unknown statement kind: emit source text as-is (safety net;
                // unreachable on a clean parse).
                let raw = n.text();
                self.out.push_str(raw.trim());
                self.line_end();
            }
        }
    }

    // ----- statement writers -----

    fn binding(&mut self, n: &Node) {
        self.out
            .push_str(if n.kind == K::LetStmt { "let " } else { "var " });
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        if let Some(t) = n.find_node(K::TypeRef) {
            self.out.push_str(": ");
            self.type_ref(t);
        }
        self.out.push_str(" = ");
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.line_end();
    }

    fn assign(&mut self, n: &Node) {
        let e = self.exprs_of(n);
        let op = n
            .child_tokens()
            .into_iter()
            .find(|t| {
                matches!(
                    t.kind,
                    K::Assign | K::PlusAssign | K::MinusAssign | K::StarAssign | K::SlashAssign
                )
            })
            .map(|t| t.text.clone())
            .unwrap_or_else(|| "=".into());
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.out.push(' ');
        self.out.push_str(&op);
        self.out.push(' ');
        if let Some(x) = e.get(1) {
            self.expr(x);
        }
        self.line_end();
    }

    fn kw_expr(&mut self, kw: &str, n: &Node) {
        self.out.push_str(kw);
        self.out.push(' ');
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.line_end();
    }

    fn use_decl(&mut self, n: &Node) {
        self.out.push_str("use ");
        for t in n.child_tokens() {
            match t.kind {
                K::Ident => self.out.push_str(&t.text),
                K::Dot => self.out.push('.'),
                _ => {}
            }
        }
        self.line_end();
    }

    fn if_stmt(&mut self, n: &Node) {
        self.out.push_str("if ");
        let e = self.exprs_of(n);
        if let Some(c) = e.first() {
            self.expr(c);
        }
        self.block(n.find_node(K::Block));
        if let Some(else_clause) = n.find_node(K::ElseClause) {
            self.collect_comments(else_clause);
            self.ind();
            if let Some(nested_if) = else_clause.find_node(K::IfStmt) {
                self.out.push_str("else ");
                self.if_stmt(nested_if);
            } else {
                self.out.push_str("else");
                self.block(else_clause.find_node(K::Block));
            }
        }
    }

    fn for_stmt(&mut self, n: &Node) {
        self.out.push_str("for ");
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        self.out.push_str(" in ");
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.block(n.find_node(K::Block));
    }

    fn header_expr_block(&mut self, kw: &str, n: &Node) {
        self.out.push_str(kw);
        self.out.push(' ');
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.block(n.find_node(K::Block));
    }

    fn repeat_stmt(&mut self, n: &Node) {
        self.out.push_str("repeat ");
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.out.push_str(" times");
        self.block(n.find_node(K::Block));
    }

    fn match_arm(&mut self, n: &Node) {
        if let Some(p) = n.find_node(K::Pattern) {
            self.pattern(p);
        }
        self.out.push_str(" -> ");
        if n.find_token(K::SayKw).is_some() {
            self.out.push_str("say ");
        }
        let e = self.exprs_of(n);
        if let Some(x) = e.first() {
            self.expr(x);
        }
        self.line_end();
    }

    fn fn_decl(&mut self, n: &Node, sig_only: bool) {
        if n.find_token(K::PublicKw).is_some() {
            self.out.push_str("public ");
        }
        self.out.push_str("fn ");
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        self.out.push('(');
        if let Some(pl) = n.find_node(K::ParamList) {
            let params = pl.nodes_of(K::Param);
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.param(p);
            }
        }
        self.out.push(')');
        if let Some(rt) = n.find_node(K::RetType) {
            self.out.push_str(" -> ");
            if let Some(t) = rt.find_node(K::TypeRef) {
                self.type_ref(t);
            }
        }
        if let Some(w) = n.find_node(K::WhereClause) {
            self.out.push_str(" where ");
            for t in w.child_tokens() {
                match t.kind {
                    K::Ident => self.out.push_str(&t.text),
                    K::IsKw => self.out.push_str(" is "),
                    K::Comma => self.out.push_str(", "),
                    _ => {}
                }
            }
        }
        if sig_only {
            self.line_end();
        } else {
            self.block(n.find_node(K::Block));
        }
    }

    fn param(&mut self, p: &Node) {
        if let Some(name) = p.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        if let Some(t) = p.find_node(K::TypeRef) {
            self.out.push_str(": ");
            self.type_ref(t);
        }
    }

    fn named_decl(&mut self, kw: &str, n: &Node) {
        if n.find_token(K::PublicKw).is_some() {
            self.out.push_str("public ");
        }
        self.out.push_str(kw);
        self.out.push(' ');
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        self.block(n.find_node(K::Block));
    }

    fn test_decl(&mut self, n: &Node) {
        self.out.push_str("test ");
        if let Some(name) = n.find_token(K::StrLit) {
            self.out.push_str(&name.text);
        }
        self.block(n.find_node(K::Block));
    }

    fn field_decl(&mut self, n: &Node) {
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        self.out.push_str(": ");
        if let Some(t) = n.find_node(K::TypeRef) {
            self.type_ref(t);
        }
        self.line_end();
    }

    fn variant_decl(&mut self, n: &Node) {
        if let Some(name) = n.find_token(K::Ident) {
            self.out.push_str(&name.text);
        }
        let params = n.nodes_of(K::Param);
        if !params.is_empty() {
            self.out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(", ");
                }
                self.param(p);
            }
            self.out.push(')');
        }
        self.line_end();
    }

    // ----- expressions, types, patterns -----

    fn exprs_of<'a>(&self, n: &'a Node) -> Vec<&'a Rc<Node>> {
        n.child_nodes()
            .into_iter()
            .filter(|c| c.kind.is_expr())
            .collect()
    }

    fn expr(&mut self, n: &Rc<Node>) {
        match n.kind {
            K::Literal | K::NameRef => {
                if let Some(t) = n.child_tokens().into_iter().find(|t| !t.kind.is_trivia()) {
                    self.out.push_str(&t.text);
                }
            }
            K::BinaryExpr | K::RangeExpr => {
                let parts = self.exprs_of(n);
                let op = n
                    .child_tokens()
                    .into_iter()
                    .find(|t| !t.kind.is_trivia())
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                if let Some(a) = parts.first() {
                    self.expr(a);
                }
                self.out.push(' ');
                self.out.push_str(&op);
                self.out.push(' ');
                if let Some(b) = parts.get(1) {
                    self.expr(b);
                }
            }
            K::UnaryExpr => {
                let parts = self.exprs_of(n);
                let is_not = n.find_token(K::NotKw).is_some();
                self.out.push_str(if is_not { "not " } else { "-" });
                if let Some(x) = parts.first() {
                    self.expr(x);
                }
            }
            K::CallExpr => {
                let parts = self.exprs_of(n);
                if let Some(callee) = parts.first() {
                    self.expr(callee);
                }
                self.out.push('(');
                if let Some(args) = n.find_node(K::ArgList) {
                    let items = self.exprs_of(args);
                    for (i, a) in items.iter().enumerate() {
                        if i > 0 {
                            self.out.push_str(", ");
                        }
                        self.expr(a);
                    }
                }
                self.out.push(')');
            }
            K::FieldExpr => {
                let parts = self.exprs_of(n);
                if let Some(base) = parts.first() {
                    self.expr(base);
                }
                self.out.push('.');
                if let Some(name) = n.find_token(K::Ident) {
                    self.out.push_str(&name.text);
                }
            }
            K::IndexExpr => {
                let parts = self.exprs_of(n);
                if let Some(base) = parts.first() {
                    self.expr(base);
                }
                self.out.push('[');
                if let Some(idx) = parts.get(1) {
                    self.expr(idx);
                }
                self.out.push(']');
            }
            K::ParenExpr => {
                self.out.push('(');
                let parts = self.exprs_of(n);
                if let Some(x) = parts.first() {
                    self.expr(x);
                }
                self.out.push(')');
            }
            K::ListExpr => {
                self.out.push('[');
                let items = self.exprs_of(n);
                for (i, x) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expr(x);
                }
                self.out.push(']');
            }
            K::MapExpr => {
                self.out.push('{');
                let entries = n.nodes_of(K::MapEntry);
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    let kv = self.exprs_of(entry);
                    if let Some(k) = kv.first() {
                        self.expr(k);
                    }
                    self.out.push_str(": ");
                    if let Some(v) = kv.get(1) {
                        self.expr(v);
                    }
                }
                self.out.push('}');
            }
            K::AskExpr => {
                self.out.push_str("ask ");
                let parts = self.exprs_of(n);
                if let Some(x) = parts.first() {
                    self.expr(x);
                }
            }
            K::TryExpr => {
                self.out.push_str("try ");
                let parts = self.exprs_of(n);
                if let Some(x) = parts.first() {
                    self.expr(x);
                }
                if let Some(fallback) = parts.get(1) {
                    self.out.push_str(" else ");
                    self.expr(fallback);
                }
            }
            _ => {
                let raw = n.text();
                self.out.push_str(raw.trim());
            }
        }
    }

    fn type_ref(&mut self, n: &Node) {
        for el in &n.children {
            match el {
                Element::Token(t) => match t.kind {
                    K::Ident => self.out.push_str(&t.text),
                    K::OfKw => self.out.push_str(" of "),
                    K::ToKw => self.out.push_str(" to "),
                    K::LParen => self.out.push('('),
                    K::RParen => self.out.push(')'),
                    _ => {}
                },
                Element::Node(inner) if inner.kind == K::TypeRef => self.type_ref(inner),
                _ => {}
            }
        }
    }

    fn pattern(&mut self, n: &Node) {
        for el in &n.children {
            match el {
                Element::Token(t) => match t.kind {
                    K::Ident | K::IntLit | K::FloatLit | K::StrLit | K::TrueKw | K::FalseKw => {
                        self.out.push_str(&t.text)
                    }
                    K::LParen => self.out.push('('),
                    K::Comma => self.out.push_str(", "),
                    K::RParen => self.out.push(')'),
                    _ => {}
                },
                Element::Node(sub) if sub.kind == K::Pattern => self.pattern(sub),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_spacing_and_indentation() {
        let ugly = "let   x=1+2*3\nif x>2\n        say   \"big\"\n";
        let pretty = format_source(ugly).unwrap();
        assert_eq!(pretty, "let x = 1 + 2 * 3\nif x > 2\n    say \"big\"\n");
    }

    #[test]
    fn refuses_broken_code() {
        assert!(format_source("let x = \n").is_err());
    }

    #[test]
    fn empty_file_stays_empty() {
        assert_eq!(format_source("").unwrap(), "");
        assert_eq!(format_source("\n\n\n").unwrap(), "");
    }

    #[test]
    fn idempotent_on_basics() {
        let once = format_source("say 1 to 10\nlet m = {\"a\": 1}\n").unwrap();
        let twice = format_source(&once).unwrap();
        assert_eq!(once, twice);
    }
}
