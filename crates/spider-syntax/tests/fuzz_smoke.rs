//! Deterministic fuzz smoke test.
//!
//! Not a substitute for the 24-hour fuzz run in the M1 exit criteria (that
//! runs in CI, longer), but a fast local guarantee of the two invariants:
//! never panic, always lossless — on random garbage and on random token soup.

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

const CHARSET: &[char] = &[
    'a', 'b', 'z', 'A', 'Z', '_', '0', '9', ' ', ' ', '\n', '\n', '\r', '\t', '"', '#', '(', ')',
    '[', ']', '{', '}', ',', ':', '.', '+', '-', '*', '/', '%', '=', '<', '>', '!', '\\', '~',
    'é', '🕷', '中',
];

const SOUP: &[&str] = &[
    "let", "var", "fn", "if", "else", "for", "in", "to", "while", "repeat", "times", "match",
    "try", "use", "return", "say", "ask", "record", "choice", "shape", "test", "spawn", "do",
    "together", "and", "or", "not", "true", "false", "of", "where", "is", "public", "name", "x",
    "1", "3.14", "\"text\"", "\"open", "(", ")", "[", "]", "{", "}", ",", ":", ".", "->", "+",
    "-", "*", "/", "==", "!=", "<=", ">=", "+=", "=", "\n", "\n", "\n", "    ", "        ", " ",
    "# comment", "## doc",
];

fn assert_invariants(src: &str) {
    let p = spider_syntax::parse(src);
    assert_eq!(
        p.root.text(),
        src,
        "losslessness violated for input {src:?}"
    );
}

#[test]
fn random_garbage_never_panics_and_stays_lossless() {
    let mut state = 0xC0FFEE_u64;
    for _ in 0..3000 {
        let len = (xorshift(&mut state) % 120) as usize;
        let s: String = (0..len)
            .map(|_| CHARSET[(xorshift(&mut state) as usize) % CHARSET.len()])
            .collect();
        assert_invariants(&s);
    }
}

#[test]
fn random_token_soup_never_panics_and_stays_lossless() {
    let mut state = 0x5EED_u64;
    for _ in 0..2000 {
        let len = (xorshift(&mut state) % 60) as usize;
        let s: String = (0..len)
            .map(|_| SOUP[(xorshift(&mut state) as usize) % SOUP.len()])
            .collect::<Vec<_>>()
            .join("");
        assert_invariants(&s);
    }
}

#[test]
fn pathological_nesting_is_rejected_not_crashed() {
    for pat in ["(", "[", "{", "f(", "not ", "-"] {
        let src = pat.repeat(600);
        let p = spider_syntax::parse(&src);
        assert_eq!(p.root.text(), src);
        assert!(!p.diagnostics.is_empty());
    }
}
