//! The `spider` command-line tool — Milestone M1 surface.
//!
//! M1 ships: fmt, check, tree, tokens, explain. Later milestones add
//! run/build/test/new (execution arrives with the Silk VM in M3; `spider new`
//! ships alongside `spider run` so a freshly scaffolded project can actually
//! be run the moment it is created).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const VERSION: &str = "spider 0.1.0 (Milestone M1 \"Hatchling\")";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
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
            eprintln!("  (run, build, test, and new arrive with the Silk VM in Milestone M3)");
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
    println!("  fmt <paths...>       format .sp files in place (--check: report only)");
    println!("  check <file.sp>      parse a file and explain every problem found");
    println!("  tree <file.sp>       show the syntax tree (debugging)");
    println!("  tokens <file.sp>     show the token stream (debugging)");
    println!("  explain <E0123>      explain an error code");
    println!("  --version            show the toolchain version");
    println!();
    println!("Coming in M3: run, build, test, new, repl.");
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
    let p = spider_syntax::parse(&src);
    if p.diagnostics.is_empty() {
        println!("OK: no problems found in {}", path.display());
        return 0;
    }
    for d in &p.diagnostics {
        print!("{}", spider_syntax::render(&src, &path.display().to_string(), d));
        println!();
    }
    println!(
        "{} problem(s) found in {}",
        p.diagnostics.len(),
        path.display()
    );
    1
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
