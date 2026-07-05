//! Hand-written lexer.
//!
//! Produces every byte of the input as a token (trivia included) so the
//! parser can build a lossless CST. Indentation is turned into zero-width
//! `Indent`/`Dedent` tokens with a Python-style indent stack; newlines inside
//! brackets are trivia, so expressions may span lines without continuation
//! characters.

use crate::diagnostics::Diagnostic;
use crate::kind::SyntaxKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
}

pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    Lexer::new(src).run()
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
    indents: Vec<usize>,
    brackets: usize,
    at_line_start: bool,
}

impl Lexer {
    fn new(src: &str) -> Self {
        Lexer {
            chars: src.chars().collect(),
            pos: 0,
            tokens: Vec::new(),
            diags: Vec::new(),
            indents: vec![0],
            brackets: 0,
            at_line_start: true,
        }
    }

    fn peek(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn cur(&self) -> Option<char> {
        self.peek(0)
    }

    fn text_from(&self, start: usize) -> String {
        self.chars[start..self.pos].iter().collect()
    }

    fn push(&mut self, kind: SyntaxKind, start: usize) {
        let text = self.text_from(start);
        self.tokens.push(Token { kind, text });
    }

    fn push_empty(&mut self, kind: SyntaxKind) {
        self.tokens.push(Token {
            kind,
            text: String::new(),
        });
    }

    fn diag(&mut self, code: &'static str, message: impl Into<String>, offset: usize, len: usize) {
        self.diags.push(Diagnostic::error(code, message, offset, len));
    }

    fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        while self.pos < self.chars.len() {
            if self.at_line_start && self.brackets == 0 {
                self.line_start();
                continue;
            }
            self.next_token();
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push_empty(SyntaxKind::Dedent);
        }
        self.push_empty(SyntaxKind::Eof);
        (self.tokens, self.diags)
    }

    fn line_start(&mut self) {
        let start = self.pos;
        let mut width = 0usize;
        let mut saw_tab = false;
        while let Some(c) = self.cur() {
            match c {
                ' ' => {
                    width += 1;
                    self.pos += 1;
                }
                '\t' => {
                    saw_tab = true;
                    width += 4;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        if saw_tab {
            self.diag(
                "E0001",
                "tabs are not allowed in indentation — use spaces",
                start,
                self.pos - start,
            );
        }
        if self.pos > start {
            self.push(SyntaxKind::Whitespace, start);
        }
        self.at_line_start = false;

        // Blank lines and comment-only lines do not change indentation.
        match self.cur() {
            None | Some('\n') | Some('\r') | Some('#') => return,
            _ => {}
        }

        let top = *self.indents.last().unwrap();
        if width > top {
            self.indents.push(width);
            self.push_empty(SyntaxKind::Indent);
        } else if width < top {
            while width < *self.indents.last().unwrap() {
                self.indents.pop();
                self.push_empty(SyntaxKind::Dedent);
            }
            if *self.indents.last().unwrap() != width {
                self.diag(
                    "E0004",
                    "this line's indentation does not line up with any earlier line",
                    start,
                    (self.pos - start).max(1),
                );
            }
        }
    }

    fn next_token(&mut self) {
        let start = self.pos;
        let c = self.cur().unwrap();
        match c {
            '\n' => {
                self.pos += 1;
                self.newline(start);
            }
            '\r' => {
                self.pos += 1;
                if self.cur() == Some('\n') {
                    self.pos += 1;
                }
                self.newline(start);
            }
            ' ' | '\t' => {
                while matches!(self.cur(), Some(' ') | Some('\t')) {
                    self.pos += 1;
                }
                self.push(SyntaxKind::Whitespace, start);
            }
            '#' => {
                let doc = self.peek(1) == Some('#');
                while !matches!(self.cur(), None | Some('\n') | Some('\r')) {
                    self.pos += 1;
                }
                self.push(
                    if doc {
                        SyntaxKind::DocComment
                    } else {
                        SyntaxKind::Comment
                    },
                    start,
                );
            }
            '"' => self.string(start),
            '0'..='9' => self.number(start),
            c if c == '_' || c.is_ascii_alphabetic() => self.ident(start),
            _ => self.punct(start),
        }
    }

    fn newline(&mut self, start: usize) {
        if self.brackets > 0 {
            // Inside brackets a newline is just spacing.
            self.push(SyntaxKind::Whitespace, start);
        } else {
            self.push(SyntaxKind::Newline, start);
            self.at_line_start = true;
        }
    }

    fn string(&mut self, start: usize) {
        self.pos += 1;
        loop {
            match self.cur() {
                None | Some('\n') | Some('\r') => {
                    self.diag(
                        "E0003",
                        "this text never ends — add a closing quote \"",
                        start,
                        self.pos - start,
                    );
                    break;
                }
                Some('\\') => {
                    self.pos += 1;
                    if self.cur().is_some() {
                        self.pos += 1;
                    }
                }
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => self.pos += 1,
            }
        }
        self.push(SyntaxKind::StrLit, start);
    }

    fn number(&mut self, start: usize) {
        let mut float = false;
        self.digits();
        if self.cur() == Some('.') && matches!(self.peek(1), Some(d) if d.is_ascii_digit()) {
            float = true;
            self.pos += 1;
            self.digits();
        }
        if matches!(self.cur(), Some('e') | Some('E')) {
            let a = self.peek(1);
            let b = self.peek(2);
            let exp_ok = matches!(a, Some(d) if d.is_ascii_digit())
                || (matches!(a, Some('+') | Some('-'))
                    && matches!(b, Some(d) if d.is_ascii_digit()));
            if exp_ok {
                float = true;
                self.pos += 1;
                if matches!(self.cur(), Some('+') | Some('-')) {
                    self.pos += 1;
                }
                self.digits();
            }
        }
        self.push(
            if float {
                SyntaxKind::FloatLit
            } else {
                SyntaxKind::IntLit
            },
            start,
        );
    }

    fn digits(&mut self) {
        while matches!(self.cur(), Some(d) if d.is_ascii_digit() || d == '_') {
            self.pos += 1;
        }
    }

    fn ident(&mut self, start: usize) {
        while matches!(self.cur(), Some(ch) if ch == '_' || ch.is_ascii_alphanumeric()) {
            self.pos += 1;
        }
        let text = self.text_from(start);
        let kind = SyntaxKind::keyword(&text).unwrap_or(SyntaxKind::Ident);
        self.tokens.push(Token { kind, text });
    }

    fn punct(&mut self, start: usize) {
        use SyntaxKind::*;
        let c = self.cur().unwrap();
        let n = self.peek(1);
        let (kind, len): (SyntaxKind, usize) = match (c, n) {
            ('-', Some('>')) => (Arrow, 2),
            ('=', Some('=')) => (EqEq, 2),
            ('!', Some('=')) => (NotEq, 2),
            ('<', Some('=')) => (LtEq, 2),
            ('>', Some('=')) => (GtEq, 2),
            ('+', Some('=')) => (PlusAssign, 2),
            ('-', Some('=')) => (MinusAssign, 2),
            ('*', Some('=')) => (StarAssign, 2),
            ('/', Some('=')) => (SlashAssign, 2),
            ('(', _) => {
                self.brackets += 1;
                (LParen, 1)
            }
            (')', _) => {
                self.brackets = self.brackets.saturating_sub(1);
                (RParen, 1)
            }
            ('[', _) => {
                self.brackets += 1;
                (LBracket, 1)
            }
            (']', _) => {
                self.brackets = self.brackets.saturating_sub(1);
                (RBracket, 1)
            }
            ('{', _) => {
                self.brackets += 1;
                (LBrace, 1)
            }
            ('}', _) => {
                self.brackets = self.brackets.saturating_sub(1);
                (RBrace, 1)
            }
            (',', _) => (Comma, 1),
            (':', _) => (Colon, 1),
            ('.', _) => (Dot, 1),
            ('+', _) => (Plus, 1),
            ('-', _) => (Minus, 1),
            ('*', _) => (Star, 1),
            ('/', _) => (Slash, 1),
            ('%', _) => (Percent, 1),
            ('=', _) => (Assign, 1),
            ('<', _) => (Lt, 1),
            ('>', _) => (Gt, 1),
            ('!', _) => {
                self.pos += 1;
                self.diag(
                    "E0002",
                    "unexpected character `!` — use `not` for negation, or `!=` for not-equal",
                    start,
                    1,
                );
                self.push(ErrorToken, start);
                return;
            }
            _ => {
                self.pos += 1;
                self.diag("E0002", format!("unexpected character `{c}`"), start, 1);
                self.push(ErrorToken, start);
                return;
            }
        };
        self.pos += len;
        self.push(kind, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SyntaxKind as K;

    fn kinds(src: &str) -> Vec<K> {
        lex(src)
            .0
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| !k.is_trivia())
            .collect()
    }

    fn rejoin(src: &str) -> String {
        lex(src).0.into_iter().map(|t| t.text).collect()
    }

    #[test]
    fn lossless_simple() {
        for src in [
            "say \"hi\"\n",
            "let x = 1  # comment\n",
            "if a\n    b\n",
            "a\r\nb\r\n",
            "f(1,\n  2)\n",
            "\"unterminated\nnext\n",
            "weird ~ char\n",
        ] {
            assert_eq!(rejoin(src), src);
        }
    }

    #[test]
    fn indent_dedent_pairing() {
        let ks = kinds("if a\n    b\n        c\nd\n");
        let indents = ks.iter().filter(|k| **k == K::Indent).count();
        let dedents = ks.iter().filter(|k| **k == K::Dedent).count();
        assert_eq!(indents, 2);
        assert_eq!(dedents, 2);
    }

    #[test]
    fn newline_in_brackets_is_trivia() {
        let ks = kinds("f(\n1\n)\n");
        assert_eq!(
            ks,
            vec![K::Ident, K::LParen, K::IntLit, K::RParen, K::Newline, K::Eof]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(kinds("1_000")[0], K::IntLit);
        assert_eq!(kinds("3.14")[0], K::FloatLit);
        assert_eq!(kinds("2.5e-3")[0], K::FloatLit);
        assert_eq!(kinds("1e9")[0], K::FloatLit);
        // `1.to` must not lex `.` into the number
        assert_eq!(kinds("1.field")[0..2], [K::IntLit, K::Dot]);
    }

    #[test]
    fn keywords_and_idents() {
        assert_eq!(kinds("let letter")[0..2], [K::LetKw, K::Ident]);
    }

    #[test]
    fn tab_indentation_reports_e0001() {
        let (_, diags) = lex("if a\n\tb\n");
        assert!(diags.iter().any(|d| d.code == "E0001"));
    }

    #[test]
    fn unterminated_string_reports_e0003() {
        let (_, diags) = lex("let s = \"oops\n");
        assert!(diags.iter().any(|d| d.code == "E0003"));
    }
}
