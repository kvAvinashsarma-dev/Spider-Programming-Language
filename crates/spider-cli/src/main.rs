//! The `spider` command-line tool — Milestone M3 surface.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const VERSION: &str = "spider 0.1.0 (Milestone M3 \"Silk\")";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        Some("run") => cmd_run(&args[1..]),
        Some("new") => cmd_new(&args[1..]),
        Some("repl") => cmd_repl(),
        Some("fmt") => cmd_fmt(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("tree") => cmd_tree(&args[1..]),
        Some("tokens") => cmd_tokens(&args[1..]),
        Some("explain") => cmd_explain(&args[1..]),
        Some("--version") | Some("-V") => {
            println!("{VERSION}");
            0
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            0
        }
        Some(other) => {
            eprintln!("spider: `{other}` is not a command yet.");
            eprintln!("  (build and test arrive in M4/M8)");
            print_help();
            2
        }
    };
    exit(code);
}

fn print_help() {
    println!("{VERSION}");
    println!();
    println!("Usage: spider <command> [arguments]");
    println!();
    println!("Commands:");
    println!("  run <file.sp>        check and run a Spider program");
    println!("  repl                 interactive Spider session");
    println!("  new <name>           create a new Spider project");
    println!("  fmt <paths...>       format .sp files in place (--check: report only)");
    println!("  check <file.sp>      parse + resolve + type-check, explain every problem");
    println!("  tree <file.sp>       show the syntax tree (debugging)");
    println!("  tokens <file.sp>     show the token stream (debugging)");
    println!("  explain <E0123>      explain an error code");
    println!("  --version            show the toolchain version");
    println!();
    println!("Coming later: build (native, M8), test (M4).");
}

fn cmd_run(args: &[String]) -> i32 {
    let Some(path) = require_file(args, "run") else {
        return 2;
    };
    let path = if path.is_dir() {
        path.join("src").join("main.sp")
    } else {
        path
    };
    let src = match read_source(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let file = path.display().to_string();
    match spider_silk::prepare(&src) {
        Ok(prepared) => {
            for w in &prepared.warnings {
                eprint!("{}", spider_syntax::render(&src, &file, w));
                eprintln!();
            }
            let mut io = spider_silk::ConsoleIo;
            let mut vm = spider_silk::Vm::new(&mut io);
            match vm.run(&prepared.program) {
                Ok(_) => 0,
                Err(e) => {
                    eprint!("{}", spider_silk::render_panic(&e));
                    1
                }
            }
        }
        Err(spider_silk::PrepareError::Diagnostics(diags)) => {
            for d in &diags {
                eprint!("{}", spider_syntax::render(&src, &file, d));
                eprintln!();
            }
            eprintln!("{} problem(s) — nothing was run", diags.len());
            1
        }
        Err(spider_silk::PrepareError::Internal(m)) => {
            eprintln!("internal Spider error (a bug in Spider, not your code): {m}");
            eprintln!("please report it: https://github.com/spider-lang/spider/issues");
            1
        }
    }
}

fn cmd_new(args: &[String]) -> i32 {
    let Some(name) = args.first() else {
        eprintln!("spider: new needs a project name. Example: spider new lemonade-stand");
        return 2;
    };
    let root = PathBuf::from(name);
    if root.exists() {
        eprintln!("spider: `{name}` already exists — pick a fresh name.");
        return 1;
    }
    let src_dir = root.join("src");
    if let Err(e) = fs::create_dir_all(&src_dir) {
        eprintln!("spider: cannot create {}: {e}", src_dir.display());
        return 1;
    }
    let manifest = format!(
        "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nspider = \"0.1\"\n\n[capabilities]\nallow = []\n\n[dependencies]\n"
    );
    let main_sp = "say \"Hello from Spider!\"\n\nlet name = ask \"What is your name?\"\nsay \"Welcome, {name}!\"\n";
    let readme = format!(
        "# {name}\n\nA Spider project.\n\n```\nspider run .        # run it\nspider check src    # explain any problems\nspider fmt src      # canonical formatting\n```\n"
    );
    let writes = [
        (root.join("web.toml"), manifest),
        (src_dir.join("main.sp"), main_sp.to_string()),
        (root.join("README.md"), readme),
        (root.join(".gitignore"), "/target\n".to_string()),
    ];
    for (p, content) in writes {
        if let Err(e) = fs::write(&p, content) {
            eprintln!("spider: cannot write {}: {e}", p.display());
            return 1;
        }
    }
    println!("Spun up `{name}`:");
    println!("  {name}/web.toml       project manifest (capabilities start empty)");
    println!("  {name}/src/main.sp    your program");
    println!();
    println!("Next:  cd {name}  then  spider run .");
    0
}

fn cmd_repl() -> i32 {
    use std::io::{BufRead, Write};
    println!("{VERSION}");
    println!("Type Spider code. Finish a block with an empty line. `exit` leaves.");
    let stdin = std::io::stdin();
    let mut session = spider_silk::Session::new();
    let mut io = spider_silk::ConsoleIo;
    loop {
        print!("spider> ");
        let _ = std::io::stdout().flush();
        let mut entry = String::new();
        if stdin.lock().read_line(&mut entry).unwrap_or(0) == 0 {
            return 0; // EOF
        }
        let first = entry.trim();
        if first.is_empty() {
            continue;
        }
        if matches!(first, "exit" | "quit" | "exit()" | "quit()") {
            return 0;
        }
        while spider_silk::Session::is_incomplete(&entry) {
            print!("   ...> ");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.trim().is_empty() {
                break;
            }
            entry.push_str(&line);
        }
        match session.eval(&entry, &mut io) {
            spider_silk::EvalOutcome::Value(v) => {
                if !matches!(v, spider_silk::Value::Unit) {
                    println!("= {}", spider_silk::display(&v, true));
                }
            }
            spider_silk::EvalOutcome::Diagnostics(diags) => {
                for d in diags.iter().filter(|d| d.is_error() || d.code != "W0001") {
                    print!("{}", spider_syntax::render(&entry, "<repl>", d));
                }
            }
            spider_silk::EvalOutcome::Runtime(e) => {
                print!("{}", spider_silk::render_panic(&e));
            }
            spider_silk::EvalOutcome::Internal(m) => {
                println!("internal Spider error: {m}");
            }
        }
    }
}

fn read_source(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(spider_syntax::strip_bom(&s).to_string()),
        Err(e) => Err(format!("spider: cannot read {}: {e}", path.display())),
    }
}

fn require_file(args: &[String], what: &str) -> Option<PathBuf> {
    match args.first() {
        Some(a) => Some(PathBuf::from(a)),
        None => {
            eprintln!("spider: {what} needs a file. Example: spider {what} src/main.sp");
            None
        }
    }
}

fn collect_sp_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                collect_sp_files(&p, out);
            }
        }
    } else if path.extension().is_some_and(|x| x == "sp") {
        out.push(path.to_path_buf());
    }
}

fn cmd_fmt(args: &[String]) -> i32 {
    let check_only = args.iter().any(|a| a == "--check");
    let paths: Vec<&String> = args.iter().filter(|a| *a != "--check").collect();
    if paths.is_empty() {
        eprintln!("spider: fmt needs at least one file or folder. Example: spider fmt src");
        return 2;
    }

    let mut files = Vec::new();
    for p in paths {
        collect_sp_files(Path::new(p), &mut files);
    }
    if files.is_empty() {
        eprintln!("spider: no .sp files found.");
        return 2;
    }

    let (mut changed, mut clean, mut failed) = (0, 0, 0);
    for file in &files {
        let src = match read_source(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                failed += 1;
                continue;
            }
        };
        match spider_fmt::format_source(&src) {
            Ok(formatted) => {
                if formatted == src {
                    clean += 1;
                } else if check_only {
                    println!("would reformat: {}", file.display());
                    changed += 1;
                } else {
                    if let Err(e) = fs::write(file, &formatted) {
                        eprintln!("spider: cannot write {}: {e}", file.display());
                        failed += 1;
                        continue;
                    }
                    println!("formatted: {}", file.display());
                    changed += 1;
                }
            }
            Err(diags) => {
                eprintln!(
                    "spider: {} has {} problem(s) — fix them first, then format:",
                    file.display(),
                    diags.len()
                );
                let shown = diags.len().min(3);
                for d in &diags[..shown] {
                    eprint!("{}", spider_syntax::render(&src, &file.display().to_string(), d));
                }
                failed += 1;
            }
        }
    }

    println!("{changed} formatted, {clean} already perfect, {failed} with problems");
    if failed > 0 || (check_only && changed > 0) {
        1
    } else {
        0
    }
}

fn cmd_check(args: &[String]) -> i32 {
    let Some(path) = require_file(args, "check") else {
        return 2;
    };
    let src = match read_source(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    // Syntax first; names and types only run over a clean parse, so every
    // message describes the user's actual mistake, not a recovery artifact.
    let diags = spider_hir::check_source(&src);
    if diags.is_empty() {
        println!("OK: no problems found in {}", path.display());
        return 0;
    }
    for d in &diags {
        print!("{}", spider_syntax::render(&src, &path.display().to_string(), d));
        println!();
    }
    let errors = diags.iter().filter(|d| d.is_error()).count();
    let warnings = diags.len() - errors;
    println!(
        "{errors} error(s), {warnings} warning(s) in {}",
        path.display()
    );
    if errors > 0 {
        1
    } else {
        0
    }
}

fn cmd_tree(args: &[String]) -> i32 {
    let Some(path) = require_file(args, "tree") else {
        return 2;
    };
    match read_source(&path) {
        Ok(src) => {
            print!("{}", spider_syntax::parse(&src).root.dump());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            2
        }
    }
}

fn cmd_tokens(args: &[String]) -> i32 {
    let Some(path) = require_file(args, "tokens") else {
        return 2;
    };
    match read_source(&path) {
        Ok(src) => {
            let (tokens, _) = spider_syntax::lex(&src);
            for t in tokens {
                println!("{:?} {:?}", t.kind, t.text);
            }
            0
        }
        Err(e) => {
            eprintln!("{e}");
            2
        }
    }
}

fn cmd_explain(args: &[String]) -> i32 {
    let Some(code) = args.first() else {
        eprintln!("spider: explain needs an error code. Example: spider explain E0110");
        return 2;
    };
    let code = code.to_uppercase();
    match spider_syntax::explain(&code) {
        Some(e) => {
            println!("{code}");
            println!("what happened: {}", e.what);
            println!("why it's an error: {}", e.why);
            println!("how to fix: {}", e.fix);
            0
        }
        None => {
            eprintln!("spider: no explanation for `{code}` yet. If a spider command showed you this code, that's a bug in our explanation database — please report it.");
            1
        }
    }
}
