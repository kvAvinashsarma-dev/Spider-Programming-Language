//! The `spider` command-line tool — Milestone M3 surface.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const VERSION: &str = "spider 0.1.0 (Milestone M4 \"Web-spinning\")";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        Some("run") => cmd_run(&args[1..]),
        Some("test") => cmd_test(&args[1..]),
        Some("new") => cmd_new(&args[1..]),
        Some("repl") => cmd_repl(&args[1..]),
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
    println!("  test <path>          run every `test \"…\"` block");
    println!("  repl                 interactive Spider session");
    println!("  new <name>           create a new Spider project");
    println!("  fmt <paths...>       format .sp files in place (--check: report only)");
    println!("  check <file.sp>      parse + resolve + type-check, explain every problem");
    println!("  tree <file.sp>       show the syntax tree (debugging)");
    println!("  tokens <file.sp>     show the token stream (debugging)");
    println!("  explain <E0123>      explain an error code");
    println!("  --version            show the toolchain version");
    println!();
    println!("Capabilities (Safe Mode): programs may only touch files/network/etc.");
    println!("when web.toml allows it, or per run: spider run --allow fs script.sp");
    println!();
    println!("Coming later: build (native, M8).");
}

/// Splits `--allow <cap>` / `--allow cap1,cap2` flags out of an argument
/// list; validates capability names against the known set.
fn split_allow(args: &[String]) -> Result<(Vec<String>, std::collections::HashSet<String>), String> {
    let mut rest = Vec::new();
    let mut caps = std::collections::HashSet::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--allow" {
            let Some(list) = args.get(i + 1) else {
                return Err("--allow needs a capability, like: --allow fs".into());
            };
            for cap in list.split(',') {
                let cap = cap.trim();
                if !spider_hir::stdlib::KNOWN_CAPABILITIES.contains(&cap) {
                    return Err(format!(
                        "`{cap}` is not a capability — the capabilities are: {}",
                        spider_hir::stdlib::KNOWN_CAPABILITIES.join(", ")
                    ));
                }
                caps.insert(cap.to_string());
            }
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }
    Ok((rest, caps))
}

/// Walks up from the file looking for web.toml; returns its capabilities.
fn manifest_caps(start: &Path) -> Result<Option<std::collections::HashSet<String>>, String> {
    let mut dir = if start.is_dir() {
        Some(start.to_path_buf())
    } else {
        start.parent().map(|p| p.to_path_buf())
    };
    for _ in 0..16 {
        let Some(d) = dir else { break };
        let candidate = d.join("web.toml");
        if candidate.is_file() {
            let text = fs::read_to_string(&candidate)
                .map_err(|e| format!("cannot read {}: {e}", candidate.display()))?;
            let m = spider_hir::parse_manifest(&text)
                .map_err(|e| format!("{}: {e}", candidate.display()))?;
            return Ok(Some(m.capabilities));
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    Ok(None)
}

/// Manifest capabilities (if a project) plus any `--allow` grants.
fn effective_caps(
    file: &Path,
    allow: &std::collections::HashSet<String>,
) -> Result<std::collections::HashSet<String>, String> {
    let mut caps = manifest_caps(file)?.unwrap_or_default();
    caps.extend(allow.iter().cloned());
    Ok(caps)
}

fn cmd_run(args: &[String]) -> i32 {
    let (rest, allow) = match split_allow(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("spider: {e}");
            return 2;
        }
    };
    let Some(path) = require_file(&rest, "run") else {
        return 2;
    };
    let path = if path.is_dir() {
        path.join("src").join("main.sp")
    } else {
        path
    };
    let caps = match effective_caps(&path, &allow) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("spider: {e}");
            return 2;
        }
    };
    let project = match spider_web::load_project(&path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("spider: {e}");
            return 1;
        }
    };
    let policy = spider_hir::CapPolicy::Only(caps.clone());
    match spider_web::prepare_project(&project, &policy) {
        Ok(prepared) => {
            for (i, w) in &prepared.warnings {
                let m = &project.modules[*i];
                eprint!("{}", spider_syntax::render(&m.src, &m.path.display().to_string(), w));
                eprintln!();
            }
            let mut io = spider_silk::ConsoleIo;
            let mut vm = spider_silk::Vm::new(&mut io);
            vm.allowed = caps;
            match vm.run(&prepared.program) {
                Ok(_) => 0,
                Err(e) => {
                    eprint!("{}", spider_silk::render_panic(&e));
                    1
                }
            }
        }
        Err(err) => render_project_error(&project, err),
    }
}

fn render_project_error(project: &spider_web::Project, err: spider_web::ProjectError) -> i32 {
    match err {
        spider_web::ProjectError::Load(e) => {
            eprintln!("spider: {e}");
            1
        }
        spider_web::ProjectError::Diagnostics(diags) => {
            for (i, d) in &diags {
                let m = &project.modules[*i];
                eprint!("{}", spider_syntax::render(&m.src, &m.path.display().to_string(), d));
                eprintln!();
            }
            eprintln!("{} problem(s) — nothing was run", diags.len());
            1
        }
        spider_web::ProjectError::Internal(m) => {
            eprintln!("internal Spider error (a bug in Spider, not your code): {m}");
            eprintln!("please report it: https://github.com/spider-lang/spider/issues");
            1
        }
    }
}

fn cmd_test(args: &[String]) -> i32 {
    let (rest, allow) = match split_allow(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("spider: {e}");
            return 2;
        }
    };
    let target = rest
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut files = Vec::new();
    collect_sp_files(&target, &mut files);
    if files.is_empty() {
        eprintln!("spider: no .sp files found under {}", target.display());
        return 2;
    }

    let (mut passed, mut failed, mut broken) = (0usize, 0usize, 0usize);
    for file in &files {
        let src = match read_source(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                broken += 1;
                continue;
            }
        };
        let caps = match effective_caps(file, &allow) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spider: {e}");
                broken += 1;
                continue;
            }
        };
        let _ = &src;
        let project = match spider_web::load_project(file) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("spider: {e}");
                broken += 1;
                continue;
            }
        };
        let policy = spider_hir::CapPolicy::Only(caps.clone());
        let prepared = match spider_web::prepare_project(&project, &policy) {
            Ok(p) => p,
            Err(spider_web::ProjectError::Diagnostics(diags)) => {
                eprintln!("{}: {} problem(s) — its tests cannot run:", file.display(), diags.len());
                if let Some((i, d)) = diags.first() {
                    let m = &project.modules[*i];
                    eprint!("{}", spider_syntax::render(&m.src, &m.path.display().to_string(), d));
                }
                broken += 1;
                continue;
            }
            Err(spider_web::ProjectError::Load(e)) => {
                eprintln!("spider: {e}");
                broken += 1;
                continue;
            }
            Err(spider_web::ProjectError::Internal(m)) => {
                eprintln!("internal Spider error: {m}");
                broken += 1;
                continue;
            }
        };
        if prepared.program.tests.is_empty() {
            continue;
        }
        println!("{}:", file.display());
        for (name, proto) in &prepared.program.tests {
            // Fresh VM per test: tests can never depend on each other.
            let mut io = spider_silk::CaptureIo::default();
            let mut vm = spider_silk::Vm::new(&mut io);
            vm.allowed = caps.clone();
            let setup = vm.run_entry(&prepared.program);
            let result = setup.and_then(|_| vm.call_proto(&prepared.program, *proto));
            match result {
                Ok(_) => {
                    println!("  test \"{name}\" ... ok");
                    passed += 1;
                }
                Err(e) => {
                    println!("  test \"{name}\" ... FAILED");
                    if !io.out.is_empty() {
                        for line in io.out.lines() {
                            println!("      | {line}");
                        }
                    }
                    print!("{}", indent_lines(&spider_silk::render_panic(&e), "      "));
                    failed += 1;
                }
            }
        }
    }
    println!();
    println!("{passed} passed, {failed} failed, {broken} file(s) with problems");
    if failed > 0 || broken > 0 {
        1
    } else {
        0
    }
}

fn indent_lines(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|l| format!("{prefix}{l}\n"))
        .collect::<String>()
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

fn cmd_repl(args: &[String]) -> i32 {
    use std::io::{BufRead, Write};
    let (_, allow) = match split_allow(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("spider: {e}");
            return 2;
        }
    };
    println!("{VERSION}");
    println!("Type Spider code. Finish a block with an empty line. `exit` leaves.");
    if !allow.is_empty() {
        let mut list: Vec<&str> = allow.iter().map(|s| s.as_str()).collect();
        list.sort();
        println!("Capabilities granted this session: {}", list.join(", "));
    }
    let stdin = std::io::stdin();
    let mut session = spider_silk::Session::new();
    session.caps = allow;
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
    let _ = &src;
    let project = match spider_web::load_project(&path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("spider: {e}");
            return 1;
        }
    };
    let mut diags: Vec<(usize, spider_syntax::Diagnostic)> = Vec::new();
    for (i, m) in project.modules.iter().enumerate() {
        for d in &m.parse.diagnostics {
            diags.push((i, d.clone()));
        }
    }
    if diags.is_empty() {
        let mods: Vec<spider_hir::ProjectModule> = project
            .modules
            .iter()
            .map(|m| spider_hir::ProjectModule {
                name: m.name.clone(),
                parse: &m.parse,
                imports: m.imports.clone(),
            })
            .collect();
        let caps = manifest_caps(&path).ok().flatten().unwrap_or_default();
        diags = spider_hir::check_project(&mods, project.entry, &spider_hir::CapPolicy::Only(caps));
    }
    if diags.is_empty() {
        let n = project.modules.len();
        if n > 1 {
            println!("OK: no problems found in {} ({} files)", path.display(), n);
        } else {
            println!("OK: no problems found in {}", path.display());
        }
        return 0;
    }
    for (i, d) in &diags {
        let m = &project.modules[*i];
        print!("{}", spider_syntax::render(&m.src, &m.path.display().to_string(), d));
        println!();
    }
    let errors = diags.iter().filter(|(_, d)| d.is_error()).count();
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
