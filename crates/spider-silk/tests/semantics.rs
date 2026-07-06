//! The M3 semantics suite (SRS exit criterion: >= 1000 cases green).
//!
//! Cases are (program, expected output) pairs. Most are generated
//! combinatorially with expected values computed independently in Rust —
//! differential testing against a second implementation of the semantics.
//! The final test asserts the case count so the exit criterion is enforced
//! by CI, not by a claim in a document.

use std::cell::Cell;

// Thread-local so the exit-criterion count reflects exactly one serial run
// of every suite — parallel test threads can never inflate it.
thread_local! {
    static CASES: Cell<usize> = const { Cell::new(0) };
}

fn check(src: &str, expected: &str) {
    CASES.with(|c| c.set(c.get() + 1));
    match spider_silk::run_capture(src, &[]) {
        Ok(out) => assert_eq!(out, expected, "program:\n{src}"),
        Err(e) => panic!("program failed: {e}\n{src}"),
    }
}

fn check_panic(src: &str, code: &str) {
    CASES.with(|c| c.set(c.get() + 1));
    let e = spider_silk::run_capture(src, &[]).expect_err(src);
    assert!(e.starts_with(code), "expected {code}, got {e}\n{src}");
}

fn fmt_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

#[test]
fn int_arithmetic_matrix() {
    let a_vals = [-41_i64, -9, -3, -1, 0, 2, 7, 15, 100];
    let b_vals = [-13_i64, -6, -2, 1, 3, 7, 10, 25];
    for a in a_vals {
        for b in b_vals {
            for (op, f) in [
                ("+", i64::checked_add as fn(i64, i64) -> Option<i64>),
                ("-", i64::checked_sub),
                ("*", i64::checked_mul),
                ("/", i64::checked_div),
                ("%", i64::checked_rem),
            ] {
                // Unary minus binds tighter than every binary operator, so
                // negative literals in the grid mean what they look like.
                let src = format!("say {a} {op} {b}\n");
                let expected = f(a, b).unwrap();
                check(&src, &format!("{expected}\n"));
            }
        }
    }
}

#[test]
fn int_comparison_matrix() {
    let a_vals = [-41_i64, -9, -3, -1, 0, 2, 7, 15, 100];
    let b_vals = [-13_i64, -6, -2, 1, 3, 7, 10, 25];
    for a in a_vals {
        for b in b_vals {
            for (op, r) in [
                ("==", a == b),
                ("!=", a != b),
                ("<", a < b),
                ("<=", a <= b),
                (">", a > b),
                (">=", a >= b),
            ] {
                let src = format!("let a = 0 + {a}\nlet b = 0 + {b}\nsay a {op} b\n");
                check(&src, &format!("{r}\n"));
            }
        }
    }
}

#[test]
fn float_arithmetic_and_comparisons() {
    let a_vals = [0.0_f64, 1.5, 2.0, 3.25, 10.0];
    let b_vals = [0.5_f64, 1.25, 4.0];
    for a in a_vals {
        for b in b_vals {
            for (op, r) in [("+", a + b), ("-", a - b), ("*", a * b), ("/", a / b)] {
                check(
                    &format!("say {a:?} {op} {b:?}\n"),
                    &format!("{}\n", fmt_float(r)),
                );
            }
            for (op, r) in [("<", a < b), (">=", a >= b), ("==", a == b)] {
                check(&format!("say {a:?} {op} {b:?}\n"), &format!("{r}\n"));
            }
        }
    }
}

#[test]
fn bool_logic_and_short_circuit() {
    for a in [true, false] {
        for b in [true, false] {
            check(&format!("say {a} and {b}\n"), &format!("{}\n", a && b));
            check(&format!("say {a} or {b}\n"), &format!("{}\n", a || b));
        }
        check(&format!("say not {a}\n"), &format!("{}\n", !a));
    }
    // Short-circuit: the right side must not run.
    check(
        "fn boom() -> Bool\n    say \"ran\"\n    return true\n\nsay false and boom()\n",
        "false\n",
    );
    check(
        "fn boom() -> Bool\n    say \"ran\"\n    return true\n\nsay true or boom()\n",
        "true\n",
    );
}

#[test]
fn unary_negation() {
    for v in [-7_i64, -1, 0, 5, 123] {
        check(&format!("let x = 0 + {v}\nsay -x\n"), &format!("{}\n", -v));
    }
    for v in [1.5_f64, 0.0, 2.25] {
        check(&format!("say -{v:?}\n"), &format!("{}\n", fmt_float(-v)));
    }
}

#[test]
fn text_interpolation_grid() {
    let words = ["Ada", "Grace", "Alan", "Lin"];
    for w in words {
        for n in [0_i64, 7, 42] {
            check(
                &format!("let w = \"{w}\"\nsay \"{{w}} has {{0 + {n}}}\"\n"),
                &format!("{w} has {n}\n"),
            );
        }
        check(
            &format!("let w = \"{w}\"\nsay \"upper: {{w.upper()}}\"\n"),
            &format!("upper: {}\n", w.to_uppercase()),
        );
        check(
            &format!("say \"{w}\".length()\n"),
            &format!("{}\n", w.chars().count()),
        );
    }
    check("say \"tab\\there\"\n", "tab\there\n");
    check("say \"brace \\{x\\}\"\n", "brace {x}\n");
    check("say \"1 + 1 = {1 + 1}\"\n", "1 + 1 = 2\n");
    // Holes may nest braces (no string literals inside holes until the
    // lexer tokenizes interpolation — M3 notes §3).
    check("say \"nested {{1: 2}.length()}\"\n", "nested 1\n");
}

#[test]
fn text_methods() {
    check("say \"  pad  \".trim()\n", "pad\n");
    check("say \"Spider\".lower()\n", "spider\n");
    check("say \"Spider\".contains(\"pid\")\n", "true\n");
    check("say \"Spider\".contains(\"web\")\n", "false\n");
    check("say \"a,b,c\".split(\",\")\n", "[\"a\", \"b\", \"c\"]\n");
    check("say \"abc\".split(\"\")\n", "[\"a\", \"b\", \"c\"]\n");
}

#[test]
fn list_operations_grid() {
    let lists: [&[i64]; 5] = [&[], &[5], &[3, 1, 2], &[9, -4, 7, 7], &[10, 20, 30, 40, 50]];
    for items in lists {
        let lit = format!(
            "[{}]",
            items
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let typed = if items.is_empty() {
            format!("let xs: List of Int = {lit}\n")
        } else {
            format!("let xs = {lit}\n")
        };
        check(
            &format!("{typed}say xs.length()\n"),
            &format!("{}\n", items.len()),
        );
        let mut sorted = items.to_vec();
        sorted.sort();
        check(&format!("{typed}say xs.sort()\n"), &format!("{sorted:?}\n"));
        let mut rev = items.to_vec();
        rev.reverse();
        check(&format!("{typed}say xs.reverse()\n"), &format!("{rev:?}\n"));
        match items.first() {
            Some(f) => check(
                &format!(
                    "{typed}match xs.first()\n    Some(v) -> say v\n    None -> say \"none\"\n"
                ),
                &format!("{f}\n"),
            ),
            None => check(
                &format!(
                    "{typed}match xs.first()\n    Some(v) -> say v\n    None -> say \"none\"\n"
                ),
                "none\n",
            ),
        }
        for (i, item) in items.iter().enumerate() {
            check(&format!("{typed}say xs[{i}]\n"), &format!("{item}\n"));
        }
        check(
            &format!("{typed}say xs.contains(7)\n"),
            &format!("{}\n", items.contains(&7)),
        );
        // Sum via for-loop.
        let sum: i64 = items.iter().sum();
        check(
            &format!("{typed}var total = 0\nfor x in xs\n    total += x\nsay total\n"),
            &format!("{sum}\n"),
        );
    }
    check(
        "var xs = [1]\nxs.push(2)\nxs.push(3)\nsay xs\n",
        "[1, 2, 3]\n",
    );
    check("let n = [[1, 2], [3, 4]]\nsay n[1][0]\n", "3\n");
    check(
        "var n = [[1, 2], [3, 4]]\nn[0][1] = 9\nsay n\n",
        "[[1, 9], [3, 4]]\n",
    );
}

#[test]
fn map_operations() {
    check(
        "let m = {\"a\": 1, \"b\": 2}\nsay m.length()\nsay m.keys()\nsay m.values()\nsay m.has(\"a\")\nsay m.has(\"z\")\nsay m[\"b\"]\n",
        "2\n[\"a\", \"b\"]\n[1, 2]\ntrue\nfalse\n2\n",
    );
    check(
        "var m = {\"a\": 1}\nm[\"b\"] = 2\nm[\"a\"] = 10\nsay m\n",
        "{\"a\": 10, \"b\": 2}\n",
    );
    for k in ["x", "y", "z"] {
        for v in [1_i64, 2, 3] {
            check(
                &format!("var m = {{\"seed\": 0}}\nm[\"{k}\"] = {v}\nsay m[\"{k}\"]\n"),
                &format!("{v}\n"),
            );
        }
    }
}

#[test]
fn range_loops_grid() {
    for a in [-3_i64, 0, 1, 5] {
        for b in [-1_i64, 0, 4, 7] {
            let sum: i64 = (a..=b).sum();
            check(
                &format!("var total = 0\nfor i in {a} to {b}\n    total += i\nsay total\n"),
                &format!("{sum}\n"),
            );
        }
    }
}

#[test]
fn while_repeat_and_if_grids() {
    for start in [0_i64, 1, 3, 7] {
        check(
            &format!("var n = {start}\nvar steps = 0\nwhile n > 0\n    n -= 1\n    steps += 1\nsay steps\n"),
            &format!("{}\n", start.max(0)),
        );
    }
    for n in 0..6 {
        let expected = "spin\n".repeat(n);
        check(&format!("repeat {n} times\n    say \"spin\"\n"), &expected);
    }
    for age in [2_i64, 3, 12, 13, 30] {
        let label = if age >= 13 {
            "teen+"
        } else if age >= 3 {
            "kid"
        } else {
            "toddler"
        };
        check(
            &format!(
                "let age = {age}\nif age >= 13\n    say \"teen+\"\nelse if age >= 3\n    say \"kid\"\nelse\n    say \"toddler\"\n"
            ),
            &format!("{label}\n"),
        );
    }
}

#[test]
fn functions_grid() {
    for a in [-5_i64, 0, 3, 11] {
        for b in [-2_i64, 1, 6] {
            check(
                &format!("fn add(x: Int, y: Int) -> Int\n    return x + y\n\nsay add({a}, {b})\n"),
                &format!("{}\n", a + b),
            );
        }
    }
    // Implicit trailing return via match.
    check(
        "choice Size\n    Small\n    Big\n\nfn score(s: Size) -> Int\n    match s\n        Small -> 1\n        Big -> 10\n\nsay score(Big) + score(Small)\n",
        "11\n",
    );
    // Recursion.
    check(
        "fn fib(n: Int) -> Int\n    if n < 2\n        return n\n    return fib(n - 1) + fib(n - 2)\n\nsay fib(10)\n",
        "55\n",
    );
    // Function values.
    check(
        "fn double(n: Int) -> Int\n    return n * 2\n\nlet f = double\nsay f(21)\n",
        "42\n",
    );
    // Generics.
    check(
        "fn largest(items: List of T) -> Maybe of T where T is Comparable\n    return items.sort().last()\n\nmatch largest([3, 9, 1])\n    Some(v) -> say v\n    None -> say \"none\"\n",
        "9\n",
    );
}

#[test]
fn records_and_choices() {
    check(
        "record Point\n    x: Float\n    y: Float\n\nlet p = Point(1.5, 2.5)\nsay p.x\nsay p\n",
        "1.5\nPoint(x: 1.5, y: 2.5)\n",
    );
    check(
        "record Point\n    x: Float\n    y: Float\n\nvar p = Point(1.0, 2.0)\np.x = 9.0\nsay p.x\n",
        "9.0\n",
    );
    // Value semantics for records too.
    check(
        "record Box\n    v: Int\n\nvar a = Box(1)\nvar b = a\nb.v = 2\nsay a.v\nsay b.v\n",
        "1\n2\n",
    );
    for r in [1.0_f64, 2.0, 3.5] {
        let area = 3.14159 * r * r;
        check(
            &format!(
                "choice Shape\n    Circle(radius: Float)\n    Dot\n\nfn area(shape: Shape) -> Float\n    match shape\n        Circle(x) -> 3.14159 * x * x\n        Dot -> 0.0\n\nsay area(Circle({r:?}))\n"
            ),
            &format!("{}\n", fmt_float(area)),
        );
    }
    check(
        "choice Color\n    Red\n    Green\n\nsay Red\nsay Red == Red\nsay Red == Green\n",
        "Red\ntrue\nfalse\n",
    );
}

#[test]
fn outcomes_and_try() {
    check(
        "fn half(n: Int) -> Outcome of Int\n    if n % 2 == 0\n        return Ok(n / 2)\n    return Fail(\"odd\")\n\nsay try half(10) else 0\nsay try half(7) else 0\nmatch half(7)\n    Ok(v) -> say v\n    Fail(p) -> say \"failed: {p}\"\n",
        "5\n0\nfailed: odd\n",
    );
    // Bare try propagates.
    check(
        "fn half(n: Int) -> Outcome of Int\n    if n % 2 == 0\n        return Ok(n / 2)\n    return Fail(\"odd\")\n\nfn quarter(n: Int) -> Outcome of Int\n    let h = try half(n)\n    return half(h)\n\nmatch quarter(12)\n    Ok(v) -> say v\n    Fail(p) -> say p\nmatch quarter(6)\n    Ok(v) -> say v\n    Fail(p) -> say p\n",
        "3\nodd\n",
    );
    for n in [0_i64, 1, 2, 9, 10] {
        let expected = if n % 2 == 0 { n / 2 } else { -1 };
        check(
            &format!("fn half(n: Int) -> Outcome of Int\n    if n % 2 == 0\n        return Ok(n / 2)\n    return Fail(\"odd\")\n\nsay try half({n}) else 0 - 1\n"),
            &format!("{expected}\n"),
        );
    }
}

#[test]
fn match_literals_grid() {
    for n in 0..8 {
        let expected = match n {
            0 => "zero",
            1 => "one",
            _ => "many",
        };
        check(
            &format!(
                "match {n}\n    0 -> say \"zero\"\n    1 -> say \"one\"\n    other -> say \"many\"\n"
            ),
            &format!("{expected}\n"),
        );
    }
    check(
        "match true\n    true -> say \"yes\"\n    false -> say \"no\"\n",
        "yes\n",
    );
    check(
        "match \"b\"\n    \"a\" -> say 1\n    \"b\" -> say 2\n    other -> say 3\n",
        "2\n",
    );
}

#[test]
fn compound_assign_grid() {
    for start in [0_i64, 4, 10] {
        for delta in [1_i64, 3, 5] {
            for (op, f) in [
                ("+=", (start + delta)),
                ("-=", (start - delta)),
                ("*=", (start * delta)),
            ] {
                check(
                    &format!("var x = {start}\nx {op} {delta}\nsay x\n"),
                    &format!("{f}\n"),
                );
            }
        }
    }
}

#[test]
fn modules_math_random() {
    check("use math\nsay math.sqrt(9.0)\n", "3.0\n");
    check(
        "use math\nsay math.min(3, 7)\nsay math.max(3, 7)\n",
        "3\n7\n",
    );
    check(
        "use math\nsay math.floor(2.9)\nsay math.round(2.5)\n",
        "2\n3\n",
    );
    // Seeded random is deterministic: same seed, same sequence.
    let out1 = spider_silk::run_capture(
        "use random\nrandom.seed(42)\nsay random.int(1, 100)\nsay random.int(1, 100)\n",
        &[],
    )
    .unwrap();
    let out2 = spider_silk::run_capture(
        "use random\nrandom.seed(42)\nsay random.int(1, 100)\nsay random.int(1, 100)\n",
        &[],
    )
    .unwrap();
    assert_eq!(out1, out2);
    CASES.with(|c| c.set(c.get() + 1));
}

#[test]
fn runtime_panic_grid() {
    check_panic("let a = 10\nsay a / 0\n", "E0301");
    check_panic("let a = 10\nsay a % 0\n", "E0301");
    check_panic(
        "var x = 9_000_000_000_000_000_000\nx *= 9\nsay x\n",
        "E0302",
    );
    for i in [3_i64, 5, 100] {
        check_panic(&format!("let xs = [1, 2, 3]\nsay xs[{i}]\n"), "E0303");
    }
    check_panic("let m = {\"a\": 1}\nsay m[\"zzz\"]\n", "E0304");
    check_panic("record P\n    x: Int\n\nsay [P(1), P(2)].sort()\n", "E0305");
    // Unknown module members moved to check time in M4 (typed stdlib).
    CASES.with(|c| c.set(c.get() + 1));
    let e = spider_silk::run_capture("use math\nsay math.wibble(1.0)\n", &[]).unwrap_err();
    assert!(e.contains("E0306") && e.contains("wibble"), "{e}");
    check_panic(
        "fn f(n: Int) -> Int\n    return f(n)\n\nsay f(0)\n",
        "E0307",
    );
    // Safe Mode: capability denial is a check error without a grant…
    CASES.with(|c| c.set(c.get() + 1));
    let e = spider_silk::run_capture("use files\nsay files.exists(\"x\")\n", &[]).unwrap_err();
    assert!(e.contains("E0244"), "{e}");
}

#[test]
fn value_semantics_suite() {
    check(
        "var a = [1, 2]\nvar b = a\nb.push(3)\nsay a\nsay b\n",
        "[1, 2]\n[1, 2, 3]\n",
    );
    check(
        "var a = {\"k\": 1}\nvar b = a\nb[\"k\"] = 2\nsay a[\"k\"]\nsay b[\"k\"]\n",
        "1\n2\n",
    );
    check(
        "fn grow(xs: List of Int) -> Int\n    var copy = xs\n    copy.push(99)\n    return copy.length()\n\nvar a = [1]\nsay grow(a)\nsay a.length()\n",
        "2\n1\n",
    );
}

/// Must run last alphabetically? No — Rust runs tests in parallel. This
/// test just re-runs a tiny case and then asserts the global count that the
/// other tests accumulated. Test order doesn't matter because cargo runs all
/// tests in one process and this assertion happens after the others via
/// explicit re-invocation below.
#[test]
fn zz_exit_criterion_case_count() {
    // Re-run every suite serially on this thread; the thread-local counter
    // sees exactly these runs and nothing else.
    CASES.with(|c| c.set(0));
    int_arithmetic_matrix();
    int_comparison_matrix();
    float_arithmetic_and_comparisons();
    bool_logic_and_short_circuit();
    unary_negation();
    text_interpolation_grid();
    text_methods();
    list_operations_grid();
    map_operations();
    range_loops_grid();
    while_repeat_and_if_grids();
    functions_grid();
    records_and_choices();
    outcomes_and_try();
    match_literals_grid();
    compound_assign_grid();
    modules_math_random();
    runtime_panic_grid();
    value_semantics_suite();
    let n = CASES.with(|c| c.get());
    assert!(
        n >= 1000,
        "M3 exit criterion: semantics suite must hold >= 1000 cases, found {n}"
    );
    println!("semantics suite cases: {n}");
}
