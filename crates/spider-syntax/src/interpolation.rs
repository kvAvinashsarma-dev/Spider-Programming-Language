//! String segmentation: text pieces, escapes, and `{expression}` holes.
//!
//! One scanner shared by the checker (types the holes) and the compiler
//! (compiles them), so the two can never disagree about where a hole is.
//!
//! Escapes: \n \t \" \\ \{ \} — anything else keeps the backslash literally
//! (a diagnostic for unknown escapes is future work, tracked in M3 notes).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text, escapes already applied.
    Text(String),
    /// Source code between `{` and `}`, verbatim.
    Expr(String),
}

/// Splits a raw `StrLit` token (quotes included) into segments.
/// Tolerates unterminated strings — the lexer has already diagnosed those.
pub fn segments(token_text: &str) -> Vec<Segment> {
    let chars: Vec<char> = token_text.chars().collect();
    let mut i = 0;
    // Skip the opening quote.
    if chars.first() == Some(&'"') {
        i = 1;
    }
    let mut out = Vec::new();
    let mut text = String::new();
    while i < chars.len() {
        match chars[i] {
            '"' => break,
            '\\' if i + 1 < chars.len() => {
                let c = chars[i + 1];
                match c {
                    'n' => text.push('\n'),
                    't' => text.push('\t'),
                    '"' => text.push('"'),
                    '\\' => text.push('\\'),
                    '{' => text.push('{'),
                    '}' => text.push('}'),
                    other => {
                        text.push('\\');
                        text.push(other);
                    }
                }
                i += 2;
            }
            '{' => {
                if !text.is_empty() {
                    out.push(Segment::Text(std::mem::take(&mut text)));
                }
                let mut depth = 1;
                let mut expr = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    match chars[i] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    expr.push(chars[i]);
                    i += 1;
                }
                out.push(Segment::Expr(expr));
                i += 1; // past the closing `}` (or quote/end on malformed input)
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        out.push(Segment::Text(text));
    }
    out
}

/// True if the literal contains at least one `{…}` hole.
pub fn has_holes(token_text: &str) -> bool {
    segments(token_text)
        .iter()
        .any(|s| matches!(s, Segment::Expr(_)))
}

/// The fully unescaped text of a literal with no holes.
pub fn plain_text(token_text: &str) -> String {
    let mut out = String::new();
    for seg in segments(token_text) {
        match seg {
            Segment::Text(t) => out.push_str(&t),
            Segment::Expr(e) => {
                // Defensive: caller should have checked has_holes.
                out.push('{');
                out.push_str(&e);
                out.push('}');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_text_and_holes() {
        assert_eq!(
            segments("\"Welcome, {name}!\""),
            vec![
                Segment::Text("Welcome, ".into()),
                Segment::Expr("name".into()),
                Segment::Text("!".into()),
            ]
        );
    }

    #[test]
    fn escapes_apply() {
        assert_eq!(
            segments("\"a\\nb \\{not a hole\\} \\\"q\\\"\""),
            vec![Segment::Text("a\nb {not a hole} \"q\"".into())]
        );
    }

    #[test]
    fn nested_braces_stay_in_one_hole() {
        // Note: holes cannot contain string literals until interpolation is
        // tokenized in the lexer (the outer StrLit token would end at the
        // inner quote). Brace nesting itself works.
        assert_eq!(
            segments("\"m: {{1: 2}.length()}\""),
            vec![
                Segment::Text("m: ".into()),
                Segment::Expr("{1: 2}.length()".into()),
            ]
        );
    }

    #[test]
    fn plain_and_holes() {
        assert!(has_holes("\"x {y}\""));
        assert!(!has_holes("\"x \\{y\\}\""));
        assert_eq!(plain_text("\"a\\tb\""), "a\tb");
    }
}
