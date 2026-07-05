//! Error-tolerant recursive-descent parser.
//!
//! Guarantees, in priority order:
//! 1. Never panics, on any input (fuzzed).
//! 2. The produced tree is lossless: `root.text() == source`, always —
//!    including on files full of errors.
//! 3. All independent errors are reported in one pass, in source order.
//! 4. Error recovery is per-statement: a broken line becomes an `ErrorNode`
//!    and parsing resumes at the next line.

use crate::diagnostics::Diagnostic;
use crate::kind::SyntaxKind;
use crate::kind::SyntaxKind as K;
use crate::lexer::{lex, Token};
use crate::tree::{Element, Node};
use std::rc::Rc;

pub struct Parse {
    pub root: Rc<Node>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a standalone expression (used for the `{…}` holes in text and by
/// the REPL). The root is a `SourceFile` whose first child node is the
/// expression.
pub fn parse_expr_source(src: &str) -> Parse {
    let (tokens, lex_diags) = lex(src);
    let mut offsets = Vec::with_capacity(tokens.len());
    let mut acc = 0usize;
    for t in &tokens {
        offsets.push(acc);
        acc += t.text.chars().count();
    }
    let mut p = Parser {
        tokens,
        offsets,
        pos: 0,
        stack: Vec::new(),
        diags: Vec::new(),
        depth: 0,
    };
    p.start(K::SourceFile);
    p.expr();
    while !p.at(K::Eof) {
        if matches!(p.current(), K::Newline | K::Indent | K::Dedent) {
            p.bump();
            continue;
        }
        p.error_at_current("E0120", "expected the expression to end here");
        p.start(K::ErrorNode);
        p.bump();
        p.finish();
    }
    p.bump(); // Eof
    let root = p.finish_root();
    let mut diagnostics = lex_diags;
    diagnostics.extend(p.diags);
    diagnostics.sort_by_key(|d| d.offset);
    Parse { root, diagnostics }
}

pub fn parse(src: &str) -> Parse {
    let (tokens, lex_diags) = lex(src);

    // Character offset of each token's start, for diagnostics.
    let mut offsets = Vec::with_capacity(tokens.len());
    let mut acc = 0usize;
    for t in &tokens {
        offsets.push(acc);
        acc += t.text.chars().count();
    }

    let mut p = Parser {
        tokens,
        offsets,
        pos: 0,
        stack: Vec::new(),
        diags: Vec::new(),
        depth: 0,
    };
    p.source_file();
    let root = p.finish_root();

    let mut diagnostics = lex_diags;
    diagnostics.extend(p.diags);
    diagnostics.sort_by_key(|d| d.offset);
    Parse { root, diagnostics }
}

const MAX_DEPTH: u32 = 128;

/// Tokens an errored expression must never consume, so recovery can
/// resynchronize at line and bracket boundaries.
fn is_recovery_boundary(k: SyntaxKind) -> bool {
    matches!(
        k,
        K::Newline
            | K::Indent
            | K::Dedent
            | K::Eof
            | K::RParen
            | K::RBracket
            | K::RBrace
            | K::Comma
    )
}

struct InProgress {
    kind: SyntaxKind,
    children: Vec<Element>,
}

struct Parser {
    tokens: Vec<Token>,
    offsets: Vec<usize>,
    pos: usize,
    stack: Vec<InProgress>,
    diags: Vec<Diagnostic>,
    depth: u32,
}

impl Parser {
    // ----- tree building -----

    fn start(&mut self, kind: SyntaxKind) {
        self.stack.push(InProgress {
            kind,
            children: Vec::new(),
        });
    }

    fn finish(&mut self) {
        let top = self.stack.pop().expect("finish without start");
        let node = Rc::new(Node {
            kind: top.kind,
            children: top.children,
        });
        match self.stack.last_mut() {
            Some(parent) => parent.children.push(Element::Node(node)),
            None => self.stack.push(InProgress {
                kind: node.kind,
                children: vec![Element::Node(node)],
            }),
        }
    }

    fn finish_root(&mut self) -> Rc<Node> {
        let top = self.stack.pop().expect("no root");
        Rc::new(Node {
            kind: top.kind,
            children: top.children,
        })
    }

    /// Position marker inside the current node, for `wrap_open`.
    fn checkpoint(&self) -> usize {
        self.stack.last().map(|n| n.children.len()).unwrap_or(0)
    }

    /// Retroactively starts a node at `cp`: children added since the
    /// checkpoint move into the new node. Caller must `finish()` it.
    fn wrap_open(&mut self, cp: usize, kind: SyntaxKind) {
        let top = self.stack.last_mut().expect("wrap without node");
        let tail = top.children.split_off(cp.min(top.children.len()));
        self.stack.push(InProgress {
            kind,
            children: tail,
        });
    }

    // ----- token access -----

    fn peek_index(&self, mut n: usize) -> usize {
        let mut i = self.pos;
        loop {
            if i >= self.tokens.len() {
                return self.tokens.len() - 1; // Eof
            }
            if !self.tokens[i].kind.is_trivia() {
                if n == 0 {
                    return i;
                }
                n -= 1;
            }
            i += 1;
        }
    }

    fn current(&self) -> SyntaxKind {
        self.tokens[self.peek_index(0)].kind
    }

    fn nth(&self, n: usize) -> SyntaxKind {
        self.tokens[self.peek_index(n)].kind
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    fn attach_trivia(&mut self) {
        while self.pos < self.tokens.len() && self.tokens[self.pos].kind.is_trivia() {
            let t = self.tokens[self.pos].clone();
            self.stack
                .last_mut()
                .expect("trivia without node")
                .children
                .push(Element::Token(t));
            self.pos += 1;
        }
    }

    /// Consumes the next significant token into the current node.
    fn bump(&mut self) {
        self.attach_trivia();
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.stack
                .last_mut()
                .expect("bump without node")
                .children
                .push(Element::Token(t));
            self.pos += 1;
        }
    }

    fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    // ----- soft keywords (ADR-011) -----
    // Common English words are only keywords in their grammatical position;
    // everywhere a *name* is expected they are ordinary identifiers, so
    // `fn area(shape: Shape)` and `let times = 3` are legal Spider.

    fn kind_is_name(k: SyntaxKind) -> bool {
        matches!(
            k,
            K::Ident
                | K::RecordKw
                | K::ChoiceKw
                | K::ShapeKw
                | K::TestKw
                | K::TimesKw
                | K::TogetherKw
                | K::WhereKw
        )
    }

    fn at_name(&self) -> bool {
        Self::kind_is_name(self.current())
    }

    /// Bumps the next token as a name, remapping soft keywords to `Ident`
    /// so every later stage sees a plain identifier. Text is untouched, so
    /// losslessness holds.
    fn bump_name(&mut self) {
        self.attach_trivia();
        if self.pos < self.tokens.len() {
            let mut t = self.tokens[self.pos].clone();
            if t.kind != K::Ident {
                t.kind = K::Ident;
            }
            self.stack
                .last_mut()
                .expect("bump without node")
                .children
                .push(Element::Token(t));
            self.pos += 1;
        }
    }

    fn expect_name(&mut self, code: &'static str, message: &str) -> bool {
        if self.at_name() {
            self.bump_name();
            return true;
        }
        self.error_at_current(code, message.to_string());
        false
    }

    /// `record`/`choice`/`shape` start a declaration only when followed by a
    /// name and then a line end — otherwise they are ordinary identifiers.
    fn decl_follows(&self) -> bool {
        Self::kind_is_name(self.nth(1)) && matches!(self.nth(2), K::Newline | K::Eof)
    }

    // ----- diagnostics -----

    fn error_at_current(&mut self, code: &'static str, message: impl Into<String>) {
        let i = self.peek_index(0);
        let len = self.tokens[i].text.chars().count().max(1);
        self.diags
            .push(Diagnostic::error(code, message, self.offsets[i], len));
    }

    fn expect(&mut self, kind: SyntaxKind, code: &'static str, message: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error_at_current(code, message.to_string());
        false
    }

    fn expect_kind(&mut self, kind: SyntaxKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        let msg = format!("expected {}", kind.describe());
        self.error_at_current("E0115", msg);
        false
    }

    /// Consumes the rest of the line into an ErrorNode (if there is anything
    /// before the line boundary).
    fn recover_to_line_end(&mut self) {
        if is_recovery_boundary(self.current()) && !matches!(self.current(), K::Comma) {
            return;
        }
        self.start(K::ErrorNode);
        while !matches!(self.current(), K::Newline | K::Indent | K::Dedent | K::Eof) {
            self.bump();
        }
        self.finish();
    }

    /// Consumes a stray indented region (unexpected Indent at statement level).
    fn swallow_indented(&mut self) {
        self.start(K::ErrorNode);
        let mut depth = 0usize;
        loop {
            match self.current() {
                K::Eof => break,
                K::Indent => {
                    depth += 1;
                    self.bump();
                }
                K::Dedent => {
                    self.bump();
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => self.bump(),
            }
        }
        self.finish();
    }

    fn end_of_line(&mut self) {
        if self.at(K::Eof) || self.at(K::Dedent) {
            return;
        }
        if self.eat(K::Newline) {
            return;
        }
        self.error_at_current("E0120", "expected the line to end here");
        self.recover_to_line_end();
        self.eat(K::Newline);
    }

    // ----- entry -----

    fn source_file(&mut self) {
        self.start(K::SourceFile);
        loop {
            match self.current() {
                K::Eof => break,
                K::Newline => {
                    self.bump();
                }
                K::Dedent => {
                    // Should not happen at top level; keep the token, keep going.
                    self.bump();
                }
                _ => {
                    let before = self.pos;
                    self.stmt();
                    if self.pos == before {
                        // Safety net: guarantee progress.
                        self.bump();
                    }
                }
            }
        }
        self.bump(); // Eof (attaches trailing trivia)
    }

    // ----- statements and declarations -----

    fn stmt(&mut self) {
        match self.current() {
            K::UseKw => self.use_decl(),
            K::PublicKw => match self.nth(1) {
                K::FnKw => self.fn_decl(),
                K::RecordKw => self.record_decl(),
                K::ChoiceKw => self.choice_decl(),
                K::ShapeKw => self.shape_decl(),
                _ => {
                    self.error_at_current(
                        "E0170",
                        "expected fn, record, choice, or shape after `public`",
                    );
                    self.start(K::ErrorNode);
                    self.bump();
                    self.finish();
                    self.recover_to_line_end();
                    self.eat(K::Newline);
                }
            },
            K::FnKw => self.fn_decl(),
            K::RecordKw if self.decl_follows() => self.record_decl(),
            K::ChoiceKw if self.decl_follows() => self.choice_decl(),
            K::ShapeKw if self.decl_follows() => self.shape_decl(),
            K::TestKw if self.nth(1) == K::StrLit => self.test_decl(),
            K::LetKw => self.binding(K::LetStmt),
            K::VarKw => self.binding(K::VarStmt),
            K::SayKw => self.kw_expr_stmt(K::SayStmt),
            K::SpawnKw => self.kw_expr_stmt(K::SpawnStmt),
            K::ReturnKw => self.return_stmt(),
            K::IfKw => self.if_stmt(),
            K::ForKw => self.for_stmt(),
            K::WhileKw => self.while_stmt(),
            K::RepeatKw => self.repeat_stmt(),
            K::MatchKw => self.match_stmt(),
            K::DoKw => self.do_stmt(),
            K::Indent => {
                self.error_at_current("E0132", "this line is indented, but nothing above it starts a block");
                self.swallow_indented();
            }
            _ => self.expr_or_assign_stmt(),
        }
    }

    fn use_decl(&mut self) {
        self.start(K::UseDecl);
        self.bump(); // use
        self.expect_name("E0111", "expected a module name after `use`");
        while self.at(K::Dot) {
            self.bump();
            self.expect_name("E0111", "expected a name after `.`");
        }
        self.end_of_line();
        self.finish();
    }

    fn fn_decl(&mut self) {
        self.start(K::FnDecl);
        self.eat(K::PublicKw);
        self.bump(); // fn
        self.expect_name("E0111", "expected the function's name after `fn`");
        self.param_list();
        if self.at(K::Arrow) {
            self.start(K::RetType);
            self.bump();
            self.type_ref();
            self.finish();
        }
        if self.at(K::WhereKw) {
            self.where_clause();
        }
        self.block();
        self.finish();
    }

    /// A function signature without a body — the members of a `shape`.
    fn fn_sig(&mut self) {
        self.start(K::FnSig);
        self.bump(); // fn
        self.expect_name("E0111", "expected the function's name after `fn`");
        self.param_list();
        if self.at(K::Arrow) {
            self.start(K::RetType);
            self.bump();
            self.type_ref();
            self.finish();
        }
        self.end_of_line();
        self.finish();
    }

    fn param_list(&mut self) {
        self.start(K::ParamList);
        self.expect(K::LParen, "E0115", "expected `(` after the function's name");
        while !matches!(self.current(), K::RParen | K::Newline | K::Dedent | K::Eof) {
            self.param();
            if !self.eat(K::Comma) {
                break;
            }
        }
        self.expect_kind(K::RParen);
        self.finish();
    }

    fn param(&mut self) {
        self.start(K::Param);
        if self.at_name() {
            self.bump_name();
        } else {
            self.error_at_current("E0111", "expected a parameter name");
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
        }
        if self.eat(K::Colon) {
            self.type_ref();
        }
        self.finish();
    }

    fn where_clause(&mut self) {
        self.start(K::WhereClause);
        self.bump(); // where
        loop {
            self.expect_name("E0111", "expected a type name in `where`");
            self.expect_kind(K::IsKw);
            self.expect_name("E0111", "expected a capability name after `is`");
            if !self.eat(K::Comma) {
                break;
            }
        }
        self.finish();
    }

    fn type_ref(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at_current("E0150", "this type is nested too deeply");
            self.start(K::TypeRef);
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
            self.finish();
            return;
        }
        self.depth += 1;
        self.start(K::TypeRef);
        if self.eat(K::LParen) {
            self.type_ref();
            self.expect_kind(K::RParen);
        } else if self.at_name() {
            self.bump_name();
            if self.eat(K::OfKw) {
                self.type_ref();
                // `Map of Text to Int` — the `to` arm belongs to the nearest `of`.
                if self.eat(K::ToKw) {
                    self.type_ref();
                }
            }
        } else {
            self.error_at_current("E0112", "expected a type here (like Int, Text, or List of Int)");
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
        }
        self.finish();
        self.depth -= 1;
    }

    fn record_decl(&mut self) {
        self.start(K::RecordDecl);
        self.eat(K::PublicKw);
        self.bump(); // record
        self.expect_name("E0111", "expected the record's name after `record`");
        self.decl_block(|p| {
            p.start(K::FieldDecl);
            p.expect_name("E0111", "expected a field name");
            p.expect(K::Colon, "E0115", "expected `:` between the field's name and its type");
            p.type_ref();
            p.end_of_line();
            p.finish();
        });
        self.finish();
    }

    fn choice_decl(&mut self) {
        self.start(K::ChoiceDecl);
        self.eat(K::PublicKw);
        self.bump(); // choice
        self.expect_name("E0111", "expected the choice's name after `choice`");
        self.decl_block(|p| {
            p.start(K::VariantDecl);
            p.expect_name("E0111", "expected a case name");
            if p.at(K::LParen) {
                p.bump();
                while !matches!(p.current(), K::RParen | K::Newline | K::Dedent | K::Eof) {
                    p.param();
                    if !p.eat(K::Comma) {
                        break;
                    }
                }
                p.expect_kind(K::RParen);
            }
            p.end_of_line();
            p.finish();
        });
        self.finish();
    }

    fn shape_decl(&mut self) {
        self.start(K::ShapeDecl);
        self.eat(K::PublicKw);
        self.bump(); // shape
        self.expect_name("E0111", "expected the shape's name after `shape`");
        self.decl_block(|p| {
            if p.at(K::FnKw) {
                p.fn_sig();
            } else {
                p.error_at_current("E0111", "a shape lists function signatures, starting with `fn`");
                p.recover_to_line_end();
                p.eat(K::Newline);
            }
        });
        self.finish();
    }

    fn test_decl(&mut self) {
        self.start(K::TestDecl);
        self.bump(); // test
        self.expect(K::StrLit, "E0115", "expected the test's name in quotes after `test`");
        self.block();
        self.finish();
    }

    /// Shared body-parser for record/choice/shape blocks.
    fn decl_block(&mut self, mut member: impl FnMut(&mut Parser)) {
        self.start(K::Block);
        if !self.expect(K::Newline, "E0130", "the block starts on the next line") {
            self.recover_to_line_end();
            self.eat(K::Newline);
        }
        while self.at(K::Newline) {
            self.bump();
        }
        if !self.expect(K::Indent, "E0131", "expected an indented body — indent with 4 spaces") {
            self.finish();
            return;
        }
        loop {
            while self.at(K::Newline) {
                self.bump();
            }
            if matches!(self.current(), K::Dedent | K::Eof) {
                break;
            }
            if self.at(K::Indent) {
                self.error_at_current("E0132", "this line is indented too far");
                self.swallow_indented();
                continue;
            }
            let before = self.pos;
            member(self);
            if self.pos == before {
                self.bump();
            }
        }
        self.eat(K::Dedent);
        self.finish();
    }

    fn block(&mut self) {
        self.decl_block_stmts();
    }

    fn decl_block_stmts(&mut self) {
        self.start(K::Block);
        if !self.expect(K::Newline, "E0130", "the block starts on the next line") {
            self.recover_to_line_end();
            self.eat(K::Newline);
        }
        while self.at(K::Newline) {
            self.bump();
        }
        if !self.expect(K::Indent, "E0131", "expected an indented body — indent with 4 spaces") {
            self.finish();
            return;
        }
        loop {
            while self.at(K::Newline) {
                self.bump();
            }
            if matches!(self.current(), K::Dedent | K::Eof) {
                break;
            }
            if self.at(K::Indent) {
                self.error_at_current("E0132", "this line is indented too far");
                self.swallow_indented();
                continue;
            }
            let before = self.pos;
            self.stmt();
            if self.pos == before {
                self.bump();
            }
        }
        self.eat(K::Dedent);
        self.finish();
    }

    fn binding(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.bump(); // let / var
        self.expect_name("E0111", "expected a name after `let`/`var`");
        if self.eat(K::Colon) {
            self.type_ref();
        }
        if self.expect(K::Assign, "E0121", "expected `=` and a value") {
            self.expr();
        } else {
            self.recover_to_line_end();
        }
        self.end_of_line();
        self.finish();
    }

    fn kw_expr_stmt(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.bump(); // say / spawn
        self.expr();
        self.end_of_line();
        self.finish();
    }

    fn return_stmt(&mut self) {
        self.start(K::ReturnStmt);
        self.bump(); // return
        if !matches!(self.current(), K::Newline | K::Dedent | K::Eof) {
            self.expr();
        }
        self.end_of_line();
        self.finish();
    }

    fn if_stmt(&mut self) {
        self.start(K::IfStmt);
        self.bump(); // if
        self.expr();
        self.block();
        if self.at(K::ElseKw) {
            self.start(K::ElseClause);
            self.bump(); // else
            if self.at(K::IfKw) {
                self.if_stmt();
            } else {
                self.block();
            }
            self.finish();
        }
        self.finish();
    }

    fn for_stmt(&mut self) {
        self.start(K::ForStmt);
        self.bump(); // for
        self.expect_name("E0111", "expected the loop item's name after `for`");
        self.expect(K::InKw, "E0127", "expected `in` — for item in collection");
        self.expr();
        self.block();
        self.finish();
    }

    fn while_stmt(&mut self) {
        self.start(K::WhileStmt);
        self.bump(); // while
        self.expr();
        self.block();
        self.finish();
    }

    fn repeat_stmt(&mut self) {
        self.start(K::RepeatStmt);
        self.bump(); // repeat
        self.expr();
        self.expect(K::TimesKw, "E0140", "expected `times` — repeat 3 times");
        self.block();
        self.finish();
    }

    fn do_stmt(&mut self) {
        self.start(K::DoTogetherStmt);
        self.bump(); // do
        self.expect(K::TogetherKw, "E0141", "expected `together` after `do`");
        self.block();
        self.finish();
    }

    fn match_stmt(&mut self) {
        self.start(K::MatchStmt);
        self.bump(); // match
        self.expr();
        self.start(K::Block);
        if !self.expect(K::Newline, "E0130", "the match cases start on the next line") {
            self.recover_to_line_end();
            self.eat(K::Newline);
        }
        while self.at(K::Newline) {
            self.bump();
        }
        if self.expect(K::Indent, "E0131", "expected the match cases, indented by 4 spaces") {
            loop {
                while self.at(K::Newline) {
                    self.bump();
                }
                if matches!(self.current(), K::Dedent | K::Eof) {
                    break;
                }
                if self.at(K::Indent) {
                    self.error_at_current("E0132", "this line is indented too far");
                    self.swallow_indented();
                    continue;
                }
                let before = self.pos;
                self.match_arm();
                if self.pos == before {
                    self.bump();
                }
            }
            self.eat(K::Dedent);
        }
        self.finish(); // Block
        self.finish(); // MatchStmt
    }

    fn match_arm(&mut self) {
        self.start(K::MatchArm);
        self.pattern();
        self.expect(K::Arrow, "E0128", "expected `->` between the pattern and its result");
        // An arm's result is an expression, optionally spoken: `-> say expr`.
        self.eat(K::SayKw);
        self.expr();
        self.end_of_line();
        self.finish();
    }

    fn pattern(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at_current("E0150", "this pattern is nested too deeply");
            self.start(K::Pattern);
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
            self.finish();
            return;
        }
        self.depth += 1;
        self.start(K::Pattern);
        match self.current() {
            K::IntLit | K::FloatLit | K::StrLit | K::TrueKw | K::FalseKw => self.bump(),
            k if Self::kind_is_name(k) => {
                self.bump_name();
                if self.eat(K::LParen) {
                    while !matches!(self.current(), K::RParen | K::Newline | K::Dedent | K::Eof) {
                        self.pattern();
                        if !self.eat(K::Comma) {
                            break;
                        }
                    }
                    self.expect_kind(K::RParen);
                }
            }
            _ => {
                self.error_at_current("E0151", "expected a pattern here");
                if !is_recovery_boundary(self.current()) {
                    self.bump();
                }
            }
        }
        self.finish();
        self.depth -= 1;
    }

    fn expr_or_assign_stmt(&mut self) {
        let cp = self.checkpoint();
        self.expr();
        if matches!(
            self.current(),
            K::Assign | K::PlusAssign | K::MinusAssign | K::StarAssign | K::SlashAssign
        ) {
            self.wrap_open(cp, K::AssignStmt);
            self.bump(); // the assignment operator
            self.expr();
            self.end_of_line();
            self.finish();
        } else {
            self.wrap_open(cp, K::ExprStmt);
            self.end_of_line();
            self.finish();
        }
    }

    // ----- expressions -----
    // Precedence, loosest to tightest:
    //   try/else · or · and · not · comparisons · `to` (range) ·
    //   + - · * / % · unary - · postfix (call, field, index) · atoms

    fn expr(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at_current("E0150", "this expression is nested too deeply");
            self.start(K::ErrorNode);
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
            self.finish();
            return;
        }
        self.depth += 1;
        if self.at(K::TryKw) {
            self.start(K::TryExpr);
            self.bump();
            self.or_expr();
            if self.eat(K::ElseKw) {
                self.expr();
            }
            self.finish();
        } else {
            self.or_expr();
        }
        self.depth -= 1;
    }

    fn or_expr(&mut self) {
        let cp = self.checkpoint();
        self.and_expr();
        while self.at(K::OrKw) {
            self.wrap_open(cp, K::BinaryExpr);
            self.bump();
            self.and_expr();
            self.finish();
        }
    }

    fn and_expr(&mut self) {
        let cp = self.checkpoint();
        self.not_expr();
        while self.at(K::AndKw) {
            self.wrap_open(cp, K::BinaryExpr);
            self.bump();
            self.not_expr();
            self.finish();
        }
    }

    fn not_expr(&mut self) {
        if self.at(K::NotKw) {
            self.start(K::UnaryExpr);
            self.bump();
            self.not_expr();
            self.finish();
        } else {
            self.cmp_expr();
        }
    }

    fn cmp_expr(&mut self) {
        let cp = self.checkpoint();
        self.range_expr();
        while matches!(
            self.current(),
            K::EqEq | K::NotEq | K::Lt | K::LtEq | K::Gt | K::GtEq
        ) {
            self.wrap_open(cp, K::BinaryExpr);
            self.bump();
            self.range_expr();
            self.finish();
        }
    }

    fn range_expr(&mut self) {
        let cp = self.checkpoint();
        self.add_expr();
        if self.at(K::ToKw) {
            self.wrap_open(cp, K::RangeExpr);
            self.bump();
            self.add_expr();
            self.finish();
        }
    }

    fn add_expr(&mut self) {
        let cp = self.checkpoint();
        self.mul_expr();
        while matches!(self.current(), K::Plus | K::Minus) {
            self.wrap_open(cp, K::BinaryExpr);
            self.bump();
            self.mul_expr();
            self.finish();
        }
    }

    fn mul_expr(&mut self) {
        let cp = self.checkpoint();
        self.unary_expr();
        while matches!(self.current(), K::Star | K::Slash | K::Percent) {
            self.wrap_open(cp, K::BinaryExpr);
            self.bump();
            self.unary_expr();
            self.finish();
        }
    }

    fn unary_expr(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at_current("E0150", "this expression is nested too deeply");
            self.start(K::ErrorNode);
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
            self.finish();
            return;
        }
        if self.at(K::Minus) {
            self.depth += 1;
            self.start(K::UnaryExpr);
            self.bump();
            self.unary_expr();
            self.finish();
            self.depth -= 1;
        } else {
            self.postfix_expr();
        }
    }

    fn postfix_expr(&mut self) {
        let cp = self.checkpoint();
        self.atom();
        loop {
            match self.current() {
                K::LParen => {
                    self.wrap_open(cp, K::CallExpr);
                    self.arg_list();
                    self.finish();
                }
                K::Dot => {
                    self.wrap_open(cp, K::FieldExpr);
                    self.bump();
                    self.expect_name("E0111", "expected a name after `.`");
                    self.finish();
                }
                K::LBracket => {
                    self.wrap_open(cp, K::IndexExpr);
                    self.bump();
                    self.expr();
                    self.expect_kind(K::RBracket);
                    self.finish();
                }
                _ => break,
            }
        }
    }

    fn arg_list(&mut self) {
        self.start(K::ArgList);
        self.bump(); // (
        while !matches!(self.current(), K::RParen | K::Eof) {
            let before = self.pos;
            self.expr();
            if !self.eat(K::Comma) {
                break;
            }
            if self.pos == before {
                break;
            }
        }
        self.expect_kind(K::RParen);
        self.finish();
    }

    fn atom(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at_current("E0150", "this expression is nested too deeply");
            self.start(K::ErrorNode);
            if !is_recovery_boundary(self.current()) {
                self.bump();
            }
            self.finish();
            return;
        }
        self.depth += 1;
        match self.current() {
            K::IntLit | K::FloatLit | K::StrLit | K::TrueKw | K::FalseKw => {
                self.start(K::Literal);
                self.bump();
                self.finish();
            }
            k if Self::kind_is_name(k) => {
                self.start(K::NameRef);
                self.bump_name();
                self.finish();
            }
            K::LParen => {
                self.start(K::ParenExpr);
                self.bump();
                self.expr();
                self.expect_kind(K::RParen);
                self.finish();
            }
            K::LBracket => {
                self.start(K::ListExpr);
                self.bump();
                while !matches!(self.current(), K::RBracket | K::Eof) {
                    let before = self.pos;
                    self.expr();
                    if !self.eat(K::Comma) {
                        break;
                    }
                    if self.pos == before {
                        break;
                    }
                }
                self.expect_kind(K::RBracket);
                self.finish();
            }
            K::LBrace => {
                self.start(K::MapExpr);
                self.bump();
                while !matches!(self.current(), K::RBrace | K::Eof) {
                    let before = self.pos;
                    self.start(K::MapEntry);
                    self.expr();
                    self.expect(K::Colon, "E0115", "expected `:` between a key and its value");
                    self.expr();
                    self.finish();
                    if !self.eat(K::Comma) {
                        break;
                    }
                    if self.pos == before {
                        break;
                    }
                }
                self.expect_kind(K::RBrace);
                self.finish();
            }
            K::AskKw => {
                self.start(K::AskExpr);
                self.bump();
                self.expr();
                self.finish();
            }
            _ => {
                self.error_at_current(
                    "E0110",
                    "expected a value here (like a number, some text, or a name)",
                );
                if !is_recovery_boundary(self.current()) {
                    self.start(K::ErrorNode);
                    self.bump();
                    self.finish();
                }
            }
        }
        self.depth -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_on_valid_and_broken_code() {
        for src in [
            "say \"Hello\"\n",
            "let x = 1 + 2 * 3\n",
            "fn f(a: Int) -> Int\n    return a\n",
            "if a and not b\n    say a\nelse\n    say b\n",
            "match x\n    Ok(v) -> v\n    Fail(e) -> e\n",
            "let broken = \nsay \"still here\"\n",
            "if true\nsay \"no indent\"\n",
            "((((((\n",
            "for i in 1 to 10\n    say i\n",
        ] {
            let p = parse(src);
            assert_eq!(p.root.text(), src, "lossless failed on {src:?}");
        }
    }

    #[test]
    fn error_recovery_keeps_parsing() {
        let p = parse("let x = \nlet y = 2\n");
        assert!(p.diagnostics.iter().any(|d| d.code == "E0110"));
        // The second statement still parsed.
        assert!(p.root.dump().matches("LetStmt").count() == 2);
    }

    #[test]
    fn all_errors_in_one_pass_in_order() {
        let p = parse("let a = \nlet b = \n");
        let codes: Vec<_> = p.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["E0110", "E0110"]);
        assert!(p.diagnostics[0].offset < p.diagnostics[1].offset);
    }

    #[test]
    fn precedence_shape() {
        let p = parse("say 1 + 2 * 3\n");
        let dump = p.root.dump();
        // The `*` nests inside the `+`.
        let plus = dump.find("Plus").unwrap();
        let star = dump.find("Star").unwrap();
        assert!(plus < star);
        assert!(p.diagnostics.is_empty());
    }
}
