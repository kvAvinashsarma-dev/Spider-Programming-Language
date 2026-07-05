//! spider-silk — the Silk VM and bytecode compiler (Milestone M3).
//!
//! `prepare` runs parse → check → compile; `Vm::run` executes. `Session`
//! powers the REPL: declarations and top-level bindings persist across
//! entries, values live in the session's global slots.

pub mod compile;
pub mod value;
pub mod vm;

pub use compile::{compile, Program};
pub use value::{display, Value};
pub use vm::{render_panic, CaptureIo, ConsoleIo, Io, RuntimeError, Vm};

use spider_syntax::{Diagnostic, SyntaxKind as K};
use std::collections::HashMap;

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

pub fn prepare(src: &str) -> Result<Prepared, PrepareError> {
    let src = spider_syntax::strip_bom(src);
    let parse = spider_syntax::parse(src);
    if !parse.diagnostics.is_empty() {
        return Err(PrepareError::Diagnostics(parse.diagnostics));
    }
    let diags = spider_hir::check_parse(&parse);
    let (errors, warnings): (Vec<_>, Vec<_>) = diags.into_iter().partition(|d| d.is_error());
    if !errors.is_empty() {
        return Err(PrepareError::Diagnostics(errors));
    }
    let program = compile::compile(&parse, None).map_err(PrepareError::Internal)?;
    Ok(Prepared { program, warnings })
}

/// Test/tool helper: run a program with captured output and scripted input.
pub fn run_capture(src: &str, inputs: &[&str]) -> Result<String, String> {
    let prepared = match prepare(src) {
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
}

impl Session {
    pub fn new() -> Session {
        Session {
            decls: Vec::new(),
            bindings: Vec::new(),
            globals_map: HashMap::new(),
            globals: Vec::new(),
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
        let diags = spider_hir::check_parse(&check_parse);
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
        assert_eq!(run_capture("say \"Hello, world!\"\n", &[]).unwrap(), "Hello, world!\n");
    }

    #[test]
    fn interpolation_runs() {
        let out = run_capture("let name = \"Ada\"\nsay \"Hi, {name}! {1 + 2}\"\n", &[]).unwrap();
        assert_eq!(out, "Hi, Ada! 3\n");
    }

    #[test]
    fn value_semantics_copy_on_write() {
        let out = run_capture(
            "var a = [1, 2]\nvar b = a\nb.push(3)\nsay a\nsay b\n",
            &[],
        )
        .unwrap();
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
        let out = run_capture(
            "let name = ask \"Who?\"\nsay \"Hi, {name}\"\n",
            &["Grace"],
        )
        .unwrap();
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
}
