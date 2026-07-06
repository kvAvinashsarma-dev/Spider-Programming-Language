//! Diagnostics in the Spider Explain format (LDD §8.3).
//!
//! Every diagnostic carries a stable code; `explain()` returns the authored
//! what/why/fix entry for a code. Offsets and lengths are in characters.
//! Codes: E00xx lexer · E01xx parser · E02xx names & types · W00xx warnings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    /// Character offset into the source.
    pub offset: usize,
    /// Length in characters (always >= 1).
    pub len: usize,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        message: impl Into<String>,
        offset: usize,
        len: usize,
    ) -> Self {
        Diagnostic {
            code,
            message: message.into(),
            offset,
            len: len.max(1),
            severity: Severity::Error,
        }
    }

    pub fn warning(
        code: &'static str,
        message: impl Into<String>,
        offset: usize,
        len: usize,
    ) -> Self {
        Diagnostic {
            code,
            message: message.into(),
            offset,
            len: len.max(1),
            severity: Severity::Warning,
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
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
    let caret_len = d
        .len
        .min(text.chars().count().saturating_sub(col - 1))
        .max(1);
    let head = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let mut out = String::new();
    out.push_str(&format!("{head}[{}]: {}\n", d.code, d.message));
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
        out.push_str(&format!("why: {}\n", e.why));
        out.push_str(&format!("how to fix: {}\n", e.fix));
    }
    out.push_str(&format!("learn more: spider explain {}\n", d.code));
    out
}

/// The authored explanation database — every code the toolchain can emit.
/// 100% coverage of emitted codes is CI-enforced; the top-100 list is a 1.0
/// gate (SRS G6).
pub fn explain(code: &str) -> Option<Explain> {
    let (what, why, fix) = match code {
        // ----- lexer -----
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
        // ----- parser -----
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
        // ----- names -----
        "E0102" => (
            "this value was made with `let`, so it cannot change.",
            "`let` makes a name for a value that stays the same; Spider stops the change here so a value you relied on can't be different later without you knowing.",
            "if this value should change, create it with `var` instead of `let`.",
        ),
        "E0201" => (
            "this name is not known here.",
            "every name must be created (with let, var, fn, record…) before it is used.",
            "check the spelling — the message suggests the closest known name if there is one.",
        ),
        "E0203" => (
            "this name already exists in the same block.",
            "two things with the same name in one place would be impossible to tell apart.",
            "pick a different name — or, if you meant to change the existing value, assign with `=` instead of creating it again.",
        ),
        "E0204" => (
            "this type name is not known.",
            "types must be built-in (Int, Float, Bool, Text, List, Map, Maybe, Outcome) or declared with record/choice.",
            "check the spelling, or declare the type first.",
        ),
        "E0208" => (
            "a public function must write out its parameter types.",
            "public functions are promises to other modules; a promise needs to say exactly what it accepts.",
            "add a type to each parameter: `public fn total(prices: List of Float)`.",
        ),
        "E0209" => (
            "this container type needs `of` and an item type.",
            "`List` alone doesn't say what's inside; `List of Int` does.",
            "write `List of Int`, `Map of Text to Int`, `Maybe of Text`, and so on.",
        ),
        // ----- types -----
        "E0210" => (
            "whole numbers (Int) and decimal numbers (Float) got mixed in one calculation.",
            "Spider never converts numbers silently, because silent conversions cause surprising bugs.",
            "convert one side yourself: `count.to_float()` or write `10.0` instead of `10`.",
        ),
        "E0211" => (
            "the value here has a different type than this spot needs.",
            "every spot in Spider code expects one type of value, so mistakes are caught before the program runs.",
            "the message shows what was expected and what was found — change one side to match the other.",
        ),
        "E0212" => (
            "the condition of if/while must be a yes-or-no value (Bool).",
            "`if` needs to decide, and only true/false can decide.",
            "compare something: `if age >= 13`, or use a Bool value directly.",
        ),
        "E0213" => (
            "`and`, `or`, and `not` work only on yes-or-no values (Bool).",
            "combining anything else with and/or has no clear meaning.",
            "make both sides comparisons or Bool values: `age > 3 and age < 13`.",
        ),
        "E0214" => (
            "this math operation needs numbers on both sides.",
            "`+ - * / %` are for numbers; other values can't be added or multiplied.",
            "to build text from pieces, use interpolation instead: \"total: {count}\".",
        ),
        "E0215" => (
            "these two values cannot be compared.",
            "comparisons only make sense between two values of the same, comparable type.",
            "make both sides the same type — the message shows what each side is.",
        ),
        "E0216" => (
            "this call has the wrong number of arguments.",
            "a function needs exactly the ingredients its recipe lists.",
            "the message shows how many the function takes — add or remove arguments to match.",
        ),
        "E0217" => (
            "this is being called like a function, but it is not one.",
            "only functions (and record/choice constructors) can be called with (…).",
            "remove the parentheses, or check the name — maybe you meant a function with a similar name.",
        ),
        "E0218" => (
            "this value cannot be indexed with [ ].",
            "only List (by position) and Map (by key) support [ ]. Text has no direct indexing because human characters are more complicated than positions.",
            "for Text, use methods like .length() and slices; for other values, check the type.",
        ),
        "E0219" => (
            "a list is indexed by position, so the index must be an Int.",
            "positions are whole numbers: names[0] is the first item.",
            "use an Int index: `names[0]`, `names[i]`.",
        ),
        "E0220" => (
            "this map key has the wrong type.",
            "a Map of Text to Int can only be looked up with Text keys.",
            "use a key of the map's key type — the message shows both types.",
        ),
        "E0221" => (
            "this record has no field with that name.",
            "records only contain the fields written in their declaration.",
            "check the spelling — the message suggests the closest field if there is one.",
        ),
        "E0222" => (
            "this value has no fields to access with a dot.",
            "only records (and modules) have named parts.",
            "check the value's type — the message shows what it actually is.",
        ),
        "E0223" => (
            "`repeat` needs a whole number (Int) for the count.",
            "you can repeat 3 times, but not \"three\" times or 2.5 times.",
            "write a whole number: `repeat 3 times`.",
        ),
        "E0224" => (
            "`for` can only walk through a List or a range.",
            "the loop needs a collection of items to visit one by one.",
            "loop over a list `for item in items` or a range `for i in 1 to 10`.",
        ),
        "E0225" => (
            "this function promises one return type but returns another.",
            "the `->` in the signature is a promise; every return must keep it.",
            "make the returned value match the promised type, or change the signature.",
        ),
        "E0226" => (
            "`return` only works inside a function.",
            "at the top of a file there is no function to return from.",
            "delete the `return`, or move this code into a function.",
        ),
        "E0227" => (
            "the items in this list have different types.",
            "a List holds items of one type, so every item can be treated the same way.",
            "make all items the same type, or split them into separate lists.",
        ),
        "E0228" => (
            "the keys or values in this map are not all the same type.",
            "a Map has one key type and one value type throughout.",
            "make all keys one type and all values one type.",
        ),
        "E0229" => (
            "both ends of a range must be whole numbers (Int).",
            "`1 to 10` counts whole steps; decimals and other values can't count steps.",
            "use Int on both sides: `for i in 1 to 10`.",
        ),
        "E0230" => (
            "this match does not cover every possible case.",
            "if a value arrived that no case matches, the program would have nowhere to go — Spider catches this before running.",
            "add the missing cases listed in the message, or add a final name pattern to catch everything else.",
        ),
        "E0231" => (
            "this pattern names a case that the matched choice does not have.",
            "patterns can only match cases that the choice declares.",
            "check the spelling against the choice's declaration — the message suggests the closest case.",
        ),
        "E0232" => (
            "this pattern has the wrong number of parts for the case it matches.",
            "a pattern unpacks exactly the pieces the case carries.",
            "match the declaration: `Circle(radius: Float)` unpacks as `Circle(r)`.",
        ),
        "E0233" => (
            "this pattern unpacks parts, but the matched value is not a choice.",
            "only choice cases (and Maybe/Outcome) carry parts to unpack.",
            "match plain values with literals or a name: `0 -> …` or `other -> …`.",
        ),
        "E0234" => (
            "this case is already handled by an earlier line of the match.",
            "the earlier case wins every time, so this line can never run.",
            "remove the duplicate line, or reorder the cases.",
        ),
        "E0235" => (
            "`try` only works on values that can fail — Outcome or Maybe.",
            "`try` means \"unwrap this or handle the failure\"; a plain value has no failure to handle.",
            "remove the `try`, or make the function return an Outcome.",
        ),
        "E0236" => (
            "a bare `try` passes failure upward, so it needs a function that returns Outcome.",
            "when the tried thing fails, the failure must have somewhere to go.",
            "add `else fallback` to handle it here, or change this function to return `Outcome of …`.",
        ),
        "E0237" => (
            "only a name, a field, or an index position can be assigned to.",
            "an assignment needs a place to store the value.",
            "put a name on the left: `score = 10`, `point.x = 1.0`, `items[0] = 5`.",
        ),
        "E0240" => (
            "the cases of this match produce different types of values.",
            "when a match's result is used, every case must produce the same type.",
            "make every case after `->` produce the same type — the message shows the two that disagree.",
        ),
        "E0241" => (
            "this `where` constraint is not a known capability.",
            "constraints must be built-in (Comparable, Equatable, Printable) or a declared shape.",
            "check the spelling, or declare a shape with that name.",
        ),
        "E0246" => (
            "only the main file runs top-level code; this file is a module.",
            "modules are boxes of definitions other files borrow from — if they also ran code on their own, importing a module could have surprise side effects.",
            "move this code into a function, or into the project's main file.",
        ),
        "E0244" => (
            "this module needs a capability the program was not given.",
            "Spider programs can only touch the outside world (files, network…) with capabilities declared up front — that's what keeps every Spider program and package safe to run.",
            "add the capability to `allow` in web.toml, or run a script with `--allow fs` (etc.) to grant it for one run.",
        ),
        "E0243" => (
            "the code inside { } in this text is not a valid expression.",
            "everything between { and } in text is real Spider code whose value gets woven into the text.",
            "fix the code inside the braces — or, for a literal brace, write \\{ and \\}.",
        ),
        "E0242" => (
            "functions, records, choices, and shapes live at the top level of a file.",
            "nested declarations arrive in a later Spider version; keeping them top-level keeps files easy to navigate.",
            "move this declaration out of the block, to the left margin.",
        ),
        // ----- runtime panics (the program stopped; these are not compile errors) -----
        "E0301" => (
            "the program divided a number by zero and stopped.",
            "no number times zero gives the left side back, so the answer doesn't exist.",
            "check the divisor before dividing: `if count > 0`.",
        ),
        "E0302" => (
            "a whole-number calculation grew past what Int can hold, and the program stopped.",
            "Int holds numbers up to about 9.2 quintillion; going past that would silently wrap to a wrong value in many languages — Spider stops instead.",
            "use Float for huge approximate values, or restructure the calculation.",
        ),
        "E0303" => (
            "the program asked a list for a position it doesn't have, and stopped.",
            "positions count from 0, so a list of 3 items has positions 0, 1, and 2.",
            "check `.length()` first, or loop with `for item in items` which can never miss.",
        ),
        "E0304" => (
            "the program asked a map for a key it doesn't hold, and stopped.",
            "reading a missing key has no honest answer, so Spider stops rather than invent one.",
            "check with `.has(key)` first.",
        ),
        "E0305" => (
            "the items in this list have no smaller-or-larger order, so it cannot be sorted.",
            "sorting needs to compare items; numbers and text compare, records and choices don't (yet).",
            "sort a list of numbers or text, or transform the items into something comparable first.",
        ),
        "E0306" => (
            "this module has no member with that name (in this Spider version).",
            "the standard library grows milestone by milestone — right now math, random, and files exist; the message suggests the closest member if there is one.",
            "check the spelling against the module's members, or check which modules exist yet.",
        ),
        "E0307" => (
            "the program called functions too deeply and stopped.",
            "each unfinished call needs memory; endless recursion would eat it all — Spider stops at 1000 nested calls in this version.",
            "check that the recursion has a base case that is actually reached.",
        ),
        "E0310" => (
            "the program tried to use a capability it was not given, and stopped.",
            "capabilities are enforced twice: when checking (E0244) and again while running, so no code path can sneak around the declaration.",
            "grant the capability in web.toml or with --allow, and only if you trust what the program does with it.",
        ),
        "E0311" => (
            "a test expectation failed: the two values were not equal.",
            "`expect(actual, expected)` is how a test says what must be true; the message shows both values.",
            "look at which value is wrong — the code being tested, or the expectation itself.",
        ),
        // ----- warnings -----
        "W0001" => (
            "this name is created but never used.",
            "unused names are usually leftovers or typos, and they make code harder to read.",
            "use the value, delete the line, or name it `_` to say \"on purpose\".",
        ),
        "W0002" => (
            "this module is not part of Spider's standard library.",
            "multi-file projects and packages arrive in Milestone M5; until then only standard modules resolve.",
            "check the spelling against the standard library list, or wait for M5 for your own modules.",
        ),
        "W0003" => (
            "this code will run, but one line at a time for now.",
            "the concurrent scheduler ships in Milestone M7; until then `do together` and `spawn` run in order, which gives the same results for code without timing dependencies.",
            "nothing to fix — this note disappears in M7.",
        ),
        _ => return None,
    };
    Some(Explain { what, why, fix })
}

/// Number of authored explanation entries — asserted in tests so the count
/// in documentation can never silently drift.
pub fn authored_code_count() -> usize {
    ALL_CODES.iter().filter(|c| explain(c).is_some()).count()
}

pub const ALL_CODES: &[&str] = &[
    "E0001", "E0002", "E0003", "E0004", "E0102", "E0110", "E0111", "E0112", "E0115", "E0120",
    "E0121", "E0127", "E0128", "E0130", "E0131", "E0132", "E0140", "E0141", "E0150", "E0151",
    "E0170", "E0201", "E0203", "E0204", "E0208", "E0209", "E0210", "E0211", "E0212", "E0213",
    "E0214", "E0215", "E0216", "E0217", "E0218", "E0219", "E0220", "E0221", "E0222", "E0223",
    "E0224", "E0225", "E0226", "E0227", "E0228", "E0229", "E0230", "E0231", "E0232", "E0233",
    "E0234", "E0235", "E0236", "E0237", "E0240", "E0241", "E0242", "E0243", "E0244", "E0246",
    "E0301", "E0302", "E0303", "E0304", "E0305", "E0306", "E0307", "E0310", "E0311", "W0001",
    "W0002", "W0003",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_code_is_authored_and_top_50_gate_met() {
        for code in ALL_CODES {
            assert!(
                explain(code).is_some(),
                "code {code} listed but not authored"
            );
        }
        assert!(
            authored_code_count() >= 50,
            "M2 exit gate: top-50 error codes must be authored (have {})",
            authored_code_count()
        );
    }
}
