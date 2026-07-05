//! Diagnostics in the Spider Explain format (LDD §8.3).
//!
//! Every diagnostic carries a stable code; `explain()` returns the authored
//! what/why/fix entry for a code. Offsets and lengths are in characters.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    /// Character offset into the source.
    pub offset: usize,
    /// Length in characters (always >= 1).
    pub len: usize,
}

pub struct Explain {
    pub what: &'static str,
    pub why: &'static str,
    pub fix: &'static str,
}

/// 1-based (line, column) for a character offset.
pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in src.chars().enumerate() {
        if i == offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn source_line(src: &str, line: usize) -> String {
    src.lines().nth(line - 1).unwrap_or("").to_string()
}

/// Renders one diagnostic in the Spider Explain style.
pub fn render(src: &str, file: &str, d: &Diagnostic) -> String {
    let (line, col) = line_col(src, d.offset);
    let text = source_line(src, line);
    let caret_len = d.len.min(text.chars().count().saturating_sub(col - 1)).max(1);
    let mut out = String::new();
    out.push_str(&format!("error[{}]: {}\n", d.code, d.message));
    out.push_str(&format!("  --> {file}:{line}:{col}\n"));
    out.push_str("   |\n");
    out.push_str(&format!("{line:>3}| {text}\n"));
    out.push_str(&format!(
        "   | {}{}\n",
        " ".repeat(col - 1),
        "^".repeat(caret_len)
    ));
    if let Some(e) = explain(d.code) {
        out.push_str(&format!("what happened: {}\n", e.what));
        out.push_str(&format!("why it's an error: {}\n", e.why));
        out.push_str(&format!("how to fix: {}\n", e.fix));
    }
    out.push_str(&format!("learn more: spider explain {}\n", d.code));
    out
}

/// The authored explanation database. Grows every milestone; 100% coverage
/// of the top-100 codes is a 1.0 gate (SRS G6).
pub fn explain(code: &str) -> Option<Explain> {
    let (what, why, fix) = match code {
        "E0001" => (
            "this line starts with a tab character.",
            "Spider measures blocks by spaces, and tabs look different in every editor, so the compiler can't be sure how deep the line is.",
            "replace each tab with 4 spaces. `spider fmt` can do this for you once the file parses.",
        ),
        "E0002" => (
            "there is a character here that Spider does not use.",
            "every symbol in Spider has one clear meaning; an unknown symbol usually means a typo.",
            "delete the character, or check the line for a typo.",
        ),
        "E0003" => (
            "a piece of text in quotes was started but never closed.",
            "without the closing quote, Spider can't tell where the text ends and the code continues.",
            "add a closing \" at the end of the text, before the line ends.",
        ),
        "E0004" => (
            "this line is indented to a depth that no surrounding block uses.",
            "each block's lines must start at exactly the same column, so Spider knows which block the line belongs to.",
            "line it up with the block above it — blocks step in by exactly 4 spaces.",
        ),
        "E0110" => (
            "Spider expected a value here (like a number, some text, or a name) but found something else.",
            "this spot in the line must contain something that produces a value.",
            "write a value or a name here. For example: `let x = 5`.",
        ),
        "E0111" => (
            "Spider expected a name here.",
            "this is the spot where the thing being created or used gets its name.",
            "add a name made of letters, digits and _, starting with a letter. For example: `fn greet(name)`.",
        ),
        "E0112" => (
            "Spider expected a type here (like Int, Text, or List of Int).",
            "after `:` or `->` Spider needs to know what kind of value this is.",
            "write a type, for example: `let age: Int = 12`.",
        ),
        "E0115" => (
            "a specific symbol was expected at this spot.",
            "Spider's grammar needs this symbol to understand the line.",
            "add the symbol shown in the message just before this spot.",
        ),
        "E0120" => (
            "the statement was complete, but more code follows on the same line.",
            "Spider keeps one statement per line so code always reads top-to-bottom.",
            "move the extra code to its own line.",
        ),
        "E0121" => (
            "`let` and `var` create a name for a value, but the `=` and the value are missing.",
            "a name without a value would be empty, and Spider has no empty values.",
            "write `= value` after the name. For example: `let score = 0`.",
        ),
        "E0127" => (
            "`for` needs the word `in` between the name and the collection.",
            "the loop reads like a sentence: for each item in the collection.",
            "write it like: `for item in cart`.",
        ),
        "E0128" => (
            "each match case needs `->` between the pattern and the result.",
            "the arrow separates what you're matching from what happens when it matches.",
            "write it like: `Circle(r) -> 3.14 * r * r`.",
        ),
        "E0130" => (
            "a block was about to start, but the rest of this line has extra code.",
            "the body of if/for/fn always starts on the next line, indented.",
            "press Enter after this line, then indent the body by 4 spaces.",
        ),
        "E0131" => (
            "this line needs an indented body under it.",
            "the lines that belong to if/for/fn are shown by indenting them 4 spaces.",
            "indent the next line by 4 spaces to put it inside the block.",
        ),
        "E0132" => (
            "this line is indented, but nothing above it starts a block.",
            "only lines under if/for/fn/record (and friends) may be indented.",
            "remove the extra indentation, or add the missing block header above.",
        ),
        "E0140" => (
            "`repeat` needs the word `times` after the count.",
            "the loop reads like a sentence: repeat 3 times.",
            "write it like: `repeat 3 times`.",
        ),
        "E0141" => (
            "`do` must be followed by `together`.",
            "`do together` is one phrase — it starts a block whose lines run at the same time.",
            "write `do together` and indent the lines below it.",
        ),
        "E0150" => (
            "the code here is nested too deeply for Spider to follow.",
            "Spider limits nesting so mistakes like a runaway `(((((` are caught early.",
            "check for unclosed brackets, or break the expression into smaller named pieces.",
        ),
        "E0151" => (
            "Spider expected a pattern here (a name, a value, or Choice(parts)).",
            "each match case starts with the pattern it should match.",
            "write a pattern like `Circle(r)`, `0`, or a name to catch everything.",
        ),
        "E0170" => (
            "`public` must be followed by fn, record, choice, or shape.",
            "`public` marks a declaration that other modules may use, so it needs a declaration after it.",
            "write for example: `public fn total(prices: List of Float) -> Float`.",
        ),
        _ => return None,
    };
    Some(Explain { what, why, fix })
}
