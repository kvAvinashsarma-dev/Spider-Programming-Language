//! `web` — Spider's package manager (M5 MVP, local registry).

use std::path::PathBuf;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        Some("install") => cmd_install(&args[1..]),
        Some("publish") => cmd_publish(),
        Some("audit") => cmd_audit(),
        Some("remove") => cmd_remove(&args[1..]),
        Some("--version") | Some("-V") => {
            println!("web 0.1.0 (Spider package manager, local registry)");
            0
        }
        _ => {
            println!("web — Spider's package manager");
            println!();
            println!("Usage: web <command>");
            println!();
            println!("  install <name>   add a package (capability-checked, lockfile-recorded)");
            println!("  publish          publish this project to the registry");
            println!("  audit            list dependencies: versions, capabilities, integrity");
            println!("  remove <name>    remove a package");
            println!();
            println!("Registry: {}", spider_web::registry::registry_root().display());
            println!("(local in M5 — the public registry with signing arrives post-1.0)");
            if args.first().is_some() {
                2
            } else {
                0
            }
        }
    };
    exit(code);
}

fn project_root() -> Result<PathBuf, String> {
    spider_web::find_project_root(&std::env::current_dir().map_err(|e| e.to_string())?)
        .ok_or_else(|| "no web.toml found here or above — run inside a Spider project (spider new)".into())
}

fn cmd_install(args: &[String]) -> i32 {
    let Some(pkg) = args.first() else {
        eprintln!("web: install needs a package name. Example: web install greetings");
        return 2;
    };
    let root = match project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("web: {e}");
            return 2;
        }
    };
    match spider_web::registry::install(&root, pkg) {
        Ok(report) => {
            println!("installed {} {}", report.name, report.version);
            if !report.capabilities.is_empty() {
                let mut caps: Vec<&str> =
                    report.capabilities.iter().map(|s| s.as_str()).collect();
                caps.sort();
                println!("  capabilities it may use: {}", caps.join(", "));
            } else {
                println!("  capabilities it may use: none (pure code)");
            }
            println!("  recorded in web.lock");
            0
        }
        Err(e) => {
            eprintln!("web: {e}");
            1
        }
    }
}

fn cmd_publish() -> i32 {
    let root = match project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("web: {e}");
            return 2;
        }
    };
    match spider_web::registry::publish(&root) {
        Ok(meta) => {
            println!("published {} {} to the local registry", meta.name, meta.version);
            0
        }
        Err(e) => {
            eprintln!("web: {e}");
            1
        }
    }
}

fn cmd_audit() -> i32 {
    let root = match project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("web: {e}");
            return 2;
        }
    };
    match spider_web::registry::audit(&root) {
        Ok(entries) => {
            if entries.is_empty() {
                println!("no dependencies.");
                return 0;
            }
            let mut bad = 0;
            for e in &entries {
                let caps = if e.capabilities.is_empty() {
                    "no capabilities".to_string()
                } else {
                    format!("capabilities: {}", e.capabilities.join(", "))
                };
                let status = if e.intact { "ok" } else { "TAMPERED — reinstall it" };
                println!("{} {}  ({caps})  [{status}]", e.name, e.version);
                if !e.intact {
                    bad += 1;
                }
            }
            if bad > 0 {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("web: {e}");
            1
        }
    }
}

fn cmd_remove(args: &[String]) -> i32 {
    let Some(pkg) = args.first() else {
        eprintln!("web: remove needs a package name.");
        return 2;
    };
    let root = match project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("web: {e}");
            return 2;
        }
    };
    let dir = root.join("web_modules").join(pkg);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            eprintln!("web: cannot remove {pkg}: {e}");
            return 1;
        }
    }
    // Drop the lock line.
    let lock = root.join("web.lock");
    if lock.is_file() {
        if let Ok(text) = std::fs::read_to_string(&lock) {
            let kept: Vec<&str> = text
                .lines()
                .filter(|l| l.split_whitespace().next() != Some(pkg.as_str()))
                .collect();
            let _ = std::fs::write(&lock, kept.join("\n") + "\n");
        }
    }
    println!("removed {pkg}");
    0
}
