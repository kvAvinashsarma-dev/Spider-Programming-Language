//! spider-hir — name resolution and the type-inference core (Milestone M2).
//!
//! `check_source` runs the full front half of the pipeline: parse, then —
//! only if the syntax is clean — resolve names and infer/check types.
//! Semantic analysis never runs over syntax errors: half-parsed trees produce
//! confusing second-hand diagnostics, and one clear message beats three
//! murky ones.

pub mod check;
pub mod manifest;
pub mod span;
pub mod stdlib;
pub mod ty;

pub use check::{check_parse, check_parse_caps, check_project, ExportedFn, ProjectModule};
pub use manifest::{parse_manifest, Manifest};
pub use stdlib::CapPolicy;
pub use ty::{Ty, Unifier};

use spider_syntax::Diagnostic;

pub fn check_source(src: &str) -> Vec<Diagnostic> {
    let src = spider_syntax::strip_bom(src);
    let parse = spider_syntax::parse(src);
    if !parse.diagnostics.is_empty() {
        return parse.diagnostics;
    }
    check_parse(&parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<&'static str> {
        check_source(src).into_iter().map(|d| d.code).collect()
    }

    fn assert_has(src: &str, code: &str) {
        let got = codes(src);
        assert!(
            got.contains(&code),
            "expected {code} for {src:?}, got {got:?}"
        );
    }

    fn assert_clean(src: &str) {
        let got = check_source(src);
        assert!(got.is_empty(), "expected clean for {src:?}, got {got:#?}");
    }

    // ----- clean programs -----

    #[test]
    fn clean_inference_end_to_end() {
        assert_clean(
            "fn double(x: Int) -> Int\n    return x * 2\n\nsay double(21)\n",
        );
        assert_clean("let xs = [1, 2, 3]\nfor x in xs\n    say x + 1\n");
        assert_clean("let empty: List of Text = []\nsay empty.length()\n");
        assert_clean("let m = {\"a\": 1}\nsay m[\"a\"] + 1\n");
    }

    #[test]
    fn soft_keywords_are_names() {
        assert_clean("let times = 3\nsay times + 1\n");
        assert_clean("var record = 1\nrecord += 1\nsay record\n");
        assert_clean(
            "record Shape\n    kind: Text\n\nfn describe(shape: Shape) -> Text\n    return shape.kind\n\nsay describe(Shape(\"round\"))\n",
        );
    }

    #[test]
    fn generics_instantiate_at_call_sites() {
        assert_clean(
            "fn largest(items: List of T) -> Maybe of T where T is Comparable\n    return items.first()\n\nmatch largest([3, 1, 2])\n    Some(value) -> value + 1\n    None -> 0\n",
        );
    }

    // ----- names -----

    #[test]
    fn name_errors() {
        assert_has("let total = 1\nsay totl\n", "E0201");
        assert_has("let x = 1\nx = 2\nsay x\n", "E0102");
        assert_has("let x = 1\nlet x = 2\nsay x\n", "E0203");
        assert_has("fn f()\n    say 1\n\nfn f()\n    say 2\n", "E0203");
        assert_has("let p: Wibble = 1\nsay p\n", "E0204");
        assert_has("let xs: List = []\nsay xs\n", "E0209");
        assert_has("public fn f(x)\n    say x\n", "E0208");
        assert_has("use wibble\n", "W0002");
        assert_has("let unused = 1\n", "W0001");
    }

    #[test]
    fn suggestion_appears_in_message() {
        let d = check_source("let total = 1\nsay totl\n");
        let msg = &d.iter().find(|d| d.code == "E0201").unwrap().message;
        assert!(msg.contains("did you mean `total`?"), "message: {msg}");
    }

    // ----- types -----

    #[test]
    fn type_errors() {
        assert_has("say 1 + 2.5\n", "E0210");
        assert_has("let x: Int = \"hi\"\nsay x\n", "E0211");
        assert_has("if 5\n    say 1\n", "E0212");
        assert_has("say true and 2\n", "E0213");
        assert_has("say \"a\" + \"b\"\n", "E0214");
        assert_has("say 1 < \"a\"\n", "E0215");
        assert_has("let x = 3\nsay x(1)\n", "E0217");
        assert_has("say \"abc\"[0]\n", "E0218");
        assert_has("let xs = [1]\nsay xs[\"a\"]\n", "E0219");
        assert_has("let m = {\"a\": 1}\nsay m[2]\n", "E0220");
        assert_has("say 5.upper()\n", "E0221");
        assert_has("let n = 5\nsay n.field\n", "E0222");
        assert_has("repeat \"three\" times\n    say 1\n", "E0223");
        assert_has("for i in 5\n    say i\n", "E0224");
        assert_has("fn f() -> Int\n    return \"hi\"\n", "E0225");
        assert_has("return 5\n", "E0226");
        assert_has("say [1, \"a\"]\n", "E0227");
        assert_has("say {\"a\": 1, 2: 3}\n", "E0228");
        assert_has("for i in 1 to 2.5\n    say i\n", "E0229");
        assert_has("1 = 2\n", "E0237");
        assert_has(
            "fn f(x: Int) -> Int\n    return x\n\nsay f(1, 2)\n",
            "E0216",
        );
    }

    // ----- match -----

    #[test]
    fn match_errors() {
        let choice = "choice Color\n    Red\n    Green\n    Blue\n\n";
        assert_has(
            &format!("{choice}match Red\n    Red -> say \"r\"\n    Green -> say \"g\"\n"),
            "E0230",
        );
        assert_has(
            &format!("{choice}match Red\n    Purple -> say \"?\"\n    other -> say \"ok\"\n"),
            "E0231",
        );
        assert_has(
            &format!("{choice}match Red\n    Red(x) -> say x\n    other -> say \"ok\"\n"),
            "E0232",
        );
        assert_has(
            "match 5\n    Circle(x) -> say x\n    other -> say \"ok\"\n",
            "E0233",
        );
        assert_has(
            &format!(
                "{choice}match Red\n    Red -> say \"a\"\n    Red -> say \"b\"\n    other -> say \"c\"\n"
            ),
            "E0234",
        );
        assert_has(
            "match 1\n    1 -> 2\n    other -> \"x\"\n",
            "E0240",
        );
    }

    #[test]
    fn match_exhaustive_clean() {
        assert_clean(
            "choice Color\n    Red\n    Green\n\nmatch Red\n    Red -> say \"r\"\n    Green -> say \"g\"\n",
        );
        assert_clean("match true\n    true -> say \"y\"\n    false -> say \"n\"\n");
    }

    // ----- try -----

    #[test]
    fn try_errors() {
        assert_has("let v = try 5 else 0\nsay v\n", "E0235");
        assert_has(
            "fn f() -> Outcome of Int\n    return Ok(1)\n\nfn g()\n    let w = try f()\n    say w\n",
            "E0236",
        );
        assert_clean(
            "fn f() -> Outcome of Int\n    return Ok(1)\n\nfn g() -> Outcome of Int\n    let w = try f()\n    return Ok(w + 1)\n\nlet v = try f() else 0\nsay v\n",
        );
    }

    // ----- misc -----

    #[test]
    fn misc_rules() {
        assert_has("fn outer()\n    fn inner()\n        say 1\n", "E0242");
        assert_has(
            "fn f(items: List of T) -> T where T is Sortable\n    return items[0]\n",
            "E0241",
        );
        // One mistake, one diagnostic: the unknown name stays Any and
        // triggers nothing downstream.
        let d = check_source("say unknown_thing + 1\n");
        assert_eq!(d.len(), 1, "{d:#?}");
        assert_eq!(d[0].code, "E0201");
    }
}
