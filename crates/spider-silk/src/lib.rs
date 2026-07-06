//! spider-silk — the Silk VM and bytecode compiler (Milestone M3).
//!
//! `prepare` runs parse → check → compile; `Vm::run` executes. `Session`
//! powers the REPL: declarations and top-level bindings persist across
//! entries, values live in the session's global slots.

pub mod compile;
pub mod value;
pub mod vm;

pub use compile::{compile, compile_project, ModuleSrc, Program};
pub use value::{display, Value};
pub use vm::{render_panic, CaptureIo, ConsoleIo, Io, RuntimeError, Vm};

use spider_hir::CapPolicy;
use spider_syntax::{Diagnostic, SyntaxKind as K};
use std::collections::HashMap;

#[derive(Debug)]
pub enum PrepareError {
    /// Parse or check diagnostics (errors present).
    Diagnostics(Vec<Diagnostic>),
    /// A toolchain bug: checker approved it, compiler could not lower it.
    Internal(String),
}

pub struct Prepared {
    pub program: Program,
    pub warnings: Vec<Diagnostic>,
}

/// Safe Mode default: no capabilities. Callers with a manifest or `--allow`
/// flags use `prepare_with`.
pub fn prepare(src: &str) -> Result<Prepared, PrepareError> {
    prepare_with(src, &CapPolicy::none())
}

pub fn prepare_with(src: &str, policy: &CapPolicy) -> Result<Prepared, PrepareError> {
    let src = spider_syntax::strip_bom(src);
    let parse = spider_syntax::parse(src);
    if !parse.diagnostics.is_empty() {
        return Err(PrepareError::Diagnostics(parse.diagnostics));
    }
    let diags = spider_hir::check_parse_caps(&parse, policy);
    let (errors, warnings): (Vec<_>, Vec<_>) = diags.into_iter().partition(|d| d.is_error());
    if !errors.is_empty() {
        return Err(PrepareError::Diagnostics(errors));
    }
    let program = compile::compile(&parse, None).map_err(PrepareError::Internal)?;
    Ok(Prepared { program, warnings })
}

/// Test/tool helper: run a program with captured output and scripted input.
/// Safe Mode — no capabilities.
pub fn run_capture(src: &str, inputs: &[&str]) -> Result<String, String> {
    run_capture_caps(src, inputs, &[])
}

/// Like `run_capture`, with capabilities granted to both checker and VM.
pub fn run_capture_caps(src: &str, inputs: &[&str], caps: &[&str]) -> Result<String, String> {
    let set: std::collections::HashSet<String> = caps.iter().map(|c| c.to_string()).collect();
    let policy = CapPolicy::Only(set.clone());
    let prepared = match prepare_with(src, &policy) {
        Ok(p) => p,
        Err(PrepareError::Diagnostics(d)) => {
            return Err(format!(
                "check failed: {}",
                d.iter()
                    .map(|d| format!("{} {}", d.code, d.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        }
        Err(PrepareError::Internal(m)) => return Err(format!("internal: {m}")),
    };
    let mut io = CaptureIo::default();
    for i in inputs {
        io.inputs.push_back(i.to_string());
    }
    let mut vm = Vm::new(&mut io);
    vm.allowed = set;
    match vm.run(&prepared.program) {
        Ok(_) => Ok(io.out),
        Err(e) => Err(format!("{} {}", e.code, e.message)),
    }
}

// ----- REPL session -----

pub enum EvalOutcome {
    /// Ran; the trailing expression's value (Unit when none).
    Value(Value),
    Diagnostics(Vec<Diagnostic>),
    Runtime(RuntimeError),
    Internal(String),
}

pub struct Session {
    /// Top-level declarations, newest definition wins per name.
    decls: Vec<(String, String)>,
    /// Top-level let/var statements, kept for the type checker only.
    bindings: Vec<(String, String)>,
    globals_map: HashMap<String, u16>,
    globals: Vec<Value>,
    /// Capabilities for both checking and running (Safe Mode: empty).
    pub caps: std::collections::HashSet<String>,
}

impl Session {
    pub fn new() -> Session {
        Session {
            decls: Vec::new(),
            bindings: Vec::new(),
            globals_map: HashMap::new(),
            globals: Vec::new(),
            caps: std::collections::HashSet::new(),
        }
    }

    fn decls_src(&self) -> String {
        self.decls
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn bindings_src(&self) -> String {
        self.bindings
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("")
    }

    /// True if the entry looks unfinished (an open block or bracket at the
    /// end) — the REPL should keep reading lines.
    pub fn is_incomplete(entry: &str) -> bool {
        if entry.trim().is_empty() {
            return false;
        }
        let parse = spider_syntax::parse(entry);
        let len = entry.chars().count();
        parse
            .diagnostics
            .iter()
            .any(|d| d.offset + 1 >= len && d.is_error())
    }

    pub fn eval(&mut self, entry: &str, io: &mut dyn Io) -> EvalOutcome {
        let entry_parse = spider_syntax::parse(entry);
        if !entry_parse.diagnostics.is_empty() {
            return EvalOutcome::Diagnostics(entry_parse.diagnostics);
        }

        // Full history for the checker, so earlier names keep their types.
        let mut check_src = String::new();
        check_src.push_str(&self.decls_src());
        if !check_src.is_empty() {
            check_src.push('\n');
        }
        check_src.push_str(&self.bindings_src());
        check_src.push_str(entry);
        let check_parse = spider_syntax::parse(&check_src);
        if !check_parse.diagnostics.is_empty() {
            // History + entry should always re-parse; entry alone was clean.
            return EvalOutcome::Diagnostics(check_parse.diagnostics);
        }
        let policy = CapPolicy::Only(self.caps.clone());
        let diags = spider_hir::check_parse_caps(&check_parse, &policy);
        let errors: Vec<Diagnostic> = diags.into_iter().filter(|d| d.is_error()).collect();
        if !errors.is_empty() {
            return EvalOutcome::Diagnostics(errors);
        }

        // Compile declarations + the new entry only; old statements never
        // re-run. Globals keep their slots across entries.
        let mut compile_src = String::new();
        compile_src.push_str(&self.decls_src());
        if !compile_src.is_empty() {
            compile_src.push('\n');
        }
        compile_src.push_str(entry);
        let compile_parse = spider_syntax::parse(&compile_src);
        let new_globals = compile::globals_of(&compile_parse, &self.globals_map);
        let program = match compile::compile(&compile_parse, Some(&new_globals)) {
            Ok(p) => p,
            Err(m) => return EvalOutcome::Internal(m),
        };

        let mut vm = Vm::new(io);
        vm.globals = std::mem::take(&mut self.globals);
        vm.allowed = self.caps.clone();
        let result = vm.run_entry(&program);
        self.globals = std::mem::take(&mut vm.globals);

        match result {
            Ok(v) => {
                self.globals_map = new_globals;
                self.absorb(entry, &entry_parse);
                EvalOutcome::Value(v)
            }
            Err(e) => EvalOutcome::Runtime(e),
        }
    }

    /// Remembers the entry's declarations and bindings for future entries.
    fn absorb(&mut self, entry: &str, parse: &spider_syntax::Parse) {
        for stmt in parse.root.child_nodes() {
            let name = stmt
                .find_token(K::Ident)
                .map(|t| t.text.clone())
                .unwrap_or_default();
            match stmt.kind {
                K::FnDecl | K::RecordDecl | K::ChoiceDecl | K::ShapeDecl => {
                    let mut text = stmt.text();
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                    self.decls.retain(|(n, _)| n != &name);
                    self.decls.push((name, text));
                }
                K::LetStmt | K::VarStmt => {
                    let mut text = stmt.text();
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                    self.bindings.retain(|(n, _)| n != &name);
                    self.bindings.push((name, text));
                }
                _ => {}
            }
        }
        let _ = entry;
    }
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_runs() {
        assert_eq!(
            run_capture("say \"Hello, world!\"\n", &[]).unwrap(),
            "Hello, world!\n"
        );
    }

    #[test]
    fn interpolation_runs() {
        let out = run_capture("let name = \"Ada\"\nsay \"Hi, {name}! {1 + 2}\"\n", &[]).unwrap();
        assert_eq!(out, "Hi, Ada! 3\n");
    }

    #[test]
    fn value_semantics_copy_on_write() {
        let out = run_capture("var a = [1, 2]\nvar b = a\nb.push(3)\nsay a\nsay b\n", &[]).unwrap();
        assert_eq!(out, "[1, 2]\n[1, 2, 3]\n");
    }

    #[test]
    fn runtime_panics_carry_codes() {
        let e = run_capture("let a = 1\nsay a / 0\n", &[]).unwrap_err();
        assert!(e.starts_with("E0301"), "{e}");
        let e = run_capture("let xs = [1]\nsay xs[5]\n", &[]).unwrap_err();
        assert!(e.starts_with("E0303"), "{e}");
        let e = run_capture(
            "fn f(n: Int) -> Int\n    return f(n + 1)\n\nsay f(0)\n",
            &[],
        )
        .unwrap_err();
        assert!(e.starts_with("E0307") || e.starts_with("E0302"), "{e}");
    }

    #[test]
    fn ask_reads_input() {
        let out = run_capture("let name = ask \"Who?\"\nsay \"Hi, {name}\"\n", &["Grace"]).unwrap();
        assert_eq!(out, "Hi, Grace\n");
    }

    #[test]
    fn session_persists_values_and_decls() {
        let mut io = CaptureIo::default();
        let mut s = Session::new();
        assert!(matches!(
            s.eval("let x = 20\n", &mut io),
            EvalOutcome::Value(_)
        ));
        assert!(matches!(
            s.eval("fn double(n: Int) -> Int\n    return n * 2\n", &mut io),
            EvalOutcome::Value(_)
        ));
        match s.eval("double(x) + 2\n", &mut io) {
            EvalOutcome::Value(v) => assert_eq!(display(&v, false), "42"),
            _ => panic!("expected a value"),
        }
        // Values persist; earlier statements never re-run (no duplicate say).
        assert!(matches!(s.eval("say x\n", &mut io), EvalOutcome::Value(_)));
        assert_eq!(io.out, "20\n");
    }

    #[test]
    fn incomplete_detection() {
        assert!(Session::is_incomplete("if true\n"));
        assert!(Session::is_incomplete("fn f()\n"));
        assert!(!Session::is_incomplete("say 1\n"));
        assert!(!Session::is_incomplete("if true\n    say 1\n"));
    }

    // ----- M4: capabilities and the typed stdlib -----

    fn temp_path(name: &str) -> String {
        let dir = std::env::temp_dir().join("spider-m4-tests");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name).display().to_string().replace('\\', "/")
    }

    #[test]
    fn capability_denied_at_check_time() {
        // Safe Mode default: a script may not even import `files`.
        let e = run_capture("use files\nsay files.exists(\"x\")\n", &[]).unwrap_err();
        assert!(e.contains("E0244"), "{e}");
    }

    #[test]
    fn capability_enforced_at_runtime_even_if_check_was_permissive() {
        // Second enforcement layer: check with AllowAll (embedding scenario),
        // then run a VM that was NOT granted fs.
        let src = "use files\nsay files.exists(\"x\")\n";
        let parse = spider_syntax::parse(src);
        assert!(parse.diagnostics.is_empty());
        assert!(spider_hir::check_parse(&parse)
            .iter()
            .all(|d| !d.is_error()));
        let program = compile::compile(&parse, None).unwrap();
        let mut io = CaptureIo::default();
        let mut vm = Vm::new(&mut io); // vm.allowed stays empty
        let e = vm.run(&program).unwrap_err();
        assert_eq!(e.code, "E0310");
    }

    #[test]
    fn files_round_trip_with_capability() {
        let path = temp_path("roundtrip.txt");
        let src = format!(
            "use files\nlet ok = try files.write_text(\"{path}\", \"hello from Spider\") else false\nsay ok\nlet text = try files.read_text(\"{path}\") else \"?\"\nsay text\nsay files.exists(\"{path}\")\n"
        );
        let out = run_capture_caps(&src, &[], &["fs"]).unwrap();
        assert_eq!(out, "true\nhello from Spider\ntrue\n");
    }

    #[test]
    fn module_calls_are_typed_now() {
        // Wrong argument type to a stdlib function is a check error.
        let e = run_capture("use math\nsay math.sqrt(\"nine\")\n", &[]).unwrap_err();
        assert!(e.contains("E0211"), "{e}");
        // Unknown member is a check error with a suggestion.
        let e = run_capture("use math\nsay math.sqirt(9.0)\n", &[]).unwrap_err();
        assert!(e.contains("E0306") && e.contains("sqrt"), "{e}");
    }

    #[test]
    fn expect_and_test_blocks() {
        let src = "\
fn double(n: Int) -> Int
    return n * 2

test \"doubling works\"
    expect(double(2), 4)

test \"this one fails\"
    expect(double(2), 5)
";
        let prepared = prepare(src).unwrap();
        assert_eq!(prepared.program.tests.len(), 2);

        let mut io = CaptureIo::default();
        let mut vm = Vm::new(&mut io);
        vm.run_entry(&prepared.program).unwrap();
        assert!(vm
            .call_proto(&prepared.program, prepared.program.tests[0].1)
            .is_ok());
        let e = vm
            .call_proto(&prepared.program, prepared.program.tests[1].1)
            .unwrap_err();
        assert_eq!(e.code, "E0311");
        assert!(
            e.message.contains('5') && e.message.contains('4'),
            "{}",
            e.message
        );
    }
}
