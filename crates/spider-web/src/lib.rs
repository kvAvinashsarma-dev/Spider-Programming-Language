//! spider-web — project services (Milestone M5).
//!
//! Ties the loader, checker, and compiler together for multi-file projects,
//! and hosts `web`, the package manager (registry.rs).

pub mod loader;
pub mod registry;

use spider_hir::CapPolicy;
use spider_syntax::Diagnostic;
use std::collections::HashSet;
use std::path::Path;

pub use loader::{find_project_root, load_project, LoadedModule, Project};

pub struct ProjectPrepared {
    pub program: spider_silk::Program,
    /// (module index, diagnostic) — render against modules[i].
    pub warnings: Vec<(usize, Diagnostic)>,
}

pub enum ProjectError {
    Load(String),
    /// (module index, diagnostics)
    Diagnostics(Vec<(usize, Diagnostic)>),
    Internal(String),
}

/// Full front half for a project: load -> check -> compile.
pub fn prepare_project(
    project: &Project,
    policy: &CapPolicy,
) -> Result<ProjectPrepared, ProjectError> {
    // Parse errors first, per file.
    let mut parse_errors = Vec::new();
    for (i, m) in project.modules.iter().enumerate() {
        for d in &m.parse.diagnostics {
            parse_errors.push((i, d.clone()));
        }
    }
    if !parse_errors.is_empty() {
        return Err(ProjectError::Diagnostics(parse_errors));
    }

    let mods: Vec<spider_hir::ProjectModule> = project
        .modules
        .iter()
        .map(|m| spider_hir::ProjectModule {
            name: m.name.clone(),
            parse: &m.parse,
            imports: m.imports.clone(),
        })
        .collect();
    let diags = spider_hir::check_project(&mods, project.entry, policy);
    let (errors, warnings): (Vec<_>, Vec<_>) = diags.into_iter().partition(|(_, d)| d.is_error());
    if !errors.is_empty() {
        return Err(ProjectError::Diagnostics(errors));
    }

    let srcs: Vec<spider_silk::ModuleSrc> = project
        .modules
        .iter()
        .map(|m| spider_silk::ModuleSrc {
            name: m.name.clone(),
            parse: &m.parse,
            imports: m.imports.clone(),
        })
        .collect();
    let program =
        spider_silk::compile_project(&srcs, project.entry, None).map_err(ProjectError::Internal)?;
    Ok(ProjectPrepared { program, warnings })
}

/// Test helper: load + run a project entry file with captured output.
pub fn run_project_capture(
    entry_file: &Path,
    inputs: &[&str],
    caps: &[&str],
) -> Result<String, String> {
    let project = load_project(entry_file).map_err(|e| format!("load: {e}"))?;
    let set: HashSet<String> = caps.iter().map(|c| c.to_string()).collect();
    let policy = CapPolicy::Only(set.clone());
    let prepared = match prepare_project(&project, &policy) {
        Ok(p) => p,
        Err(ProjectError::Load(e)) => return Err(format!("load: {e}")),
        Err(ProjectError::Diagnostics(d)) => {
            return Err(format!(
                "check failed: {}",
                d.iter()
                    .map(|(i, d)| format!(
                        "[{}] {} {}",
                        project.modules[*i].name, d.code, d.message
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        }
        Err(ProjectError::Internal(m)) => return Err(format!("internal: {m}")),
    };
    let mut io = spider_silk::CaptureIo::default();
    for i in inputs {
        io.inputs.push_back(i.to_string());
    }
    let mut vm = spider_silk::Vm::new(&mut io);
    vm.allowed = set;
    match vm.run(&prepared.program) {
        Ok(_) => Ok(io.out),
        Err(e) => Err(format!("{} {}", e.code, e.message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static STAMP: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = STAMP.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir()
            .join("spider-m5-tests")
            .join(format!("{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn make_project(dir: &Path, name: &str, caps: &str) {
        write(
            &dir.join("web.toml"),
            &format!(
                "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[capabilities]\nallow = [{caps}]\n\n[dependencies]\n"
            ),
        );
    }

    #[test]
    fn multi_file_project_runs() {
        let dir = temp_dir("multifile");
        make_project(&dir, "shop", "");
        write(
            &dir.join("src").join("main.sp"),
            "use helpers\nuse shop.cart\n\nsay helpers.greet(\"Ada\")\nsay cart.total([2, 3, 4])\n",
        );
        write(
            &dir.join("src").join("helpers.sp"),
            "public fn greet(name: Text) -> Text\n    return \"Hello, {name}!\"\n",
        );
        write(
            &dir.join("src").join("shop").join("cart.sp"),
            "public fn total(prices: List of Int) -> Int\n    var sum = 0\n    for p in prices\n        sum += p\n    return sum\n",
        );
        let out = run_project_capture(&dir.join("src").join("main.sp"), &[], &[]).unwrap();
        assert_eq!(out, "Hello, Ada!\n9\n");
    }

    #[test]
    fn private_functions_stay_private() {
        let dir = temp_dir("private");
        make_project(&dir, "app", "");
        write(
            &dir.join("src").join("main.sp"),
            "use helpers\nsay helpers.hidden()\n",
        );
        write(
            &dir.join("src").join("helpers.sp"),
            "fn hidden() -> Int\n    return 1\n",
        );
        let e = run_project_capture(&dir.join("src").join("main.sp"), &[], &[]).unwrap_err();
        assert!(e.contains("E0306") && e.contains("not public"), "{e}");
    }

    #[test]
    fn modules_cannot_run_top_level_code() {
        let dir = temp_dir("toplevel");
        make_project(&dir, "app", "");
        write(
            &dir.join("src").join("main.sp"),
            "use helpers\nsay helpers.f()\n",
        );
        write(
            &dir.join("src").join("helpers.sp"),
            "say \"surprise!\"\n\npublic fn f() -> Int\n    return 1\n",
        );
        let e = run_project_capture(&dir.join("src").join("main.sp"), &[], &[]).unwrap_err();
        assert!(e.contains("E0246"), "{e}");
    }

    #[test]
    fn import_cycles_are_explained() {
        let dir = temp_dir("cycle");
        make_project(&dir, "app", "");
        write(
            &dir.join("src").join("main.sp"),
            "use alpha\nsay alpha.f()\n",
        );
        write(
            &dir.join("src").join("alpha.sp"),
            "use beta\npublic fn f() -> Int\n    return beta.g()\n",
        );
        write(
            &dir.join("src").join("beta.sp"),
            "use alpha\npublic fn g() -> Int\n    return 1\n",
        );
        let e = run_project_capture(&dir.join("src").join("main.sp"), &[], &[]).unwrap_err();
        assert!(e.contains("circle"), "{e}");
    }

    #[test]
    fn cross_module_types_work() {
        let dir = temp_dir("types");
        make_project(&dir, "app", "");
        write(
            &dir.join("src").join("main.sp"),
            "use shapes\n\nlet c = Circle(2.0)\nsay shapes.area(c)\n",
        );
        write(
            &dir.join("src").join("shapes.sp"),
            "choice Shape\n    Circle(radius: Float)\n    Dot\n\npublic fn area(shape: Shape) -> Float\n    match shape\n        Circle(r) -> 3.0 * r * r\n        Dot -> 0.0\n",
        );
        let out = run_project_capture(&dir.join("src").join("main.sp"), &[], &[]).unwrap();
        assert_eq!(out, "12.0\n");
    }

    // ----- the package manager -----
    // SPIDER_REGISTRY is process-global; registry tests serialize on a lock.

    static REG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_registry(tag: &str) -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
        let guard = REG_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let reg = temp_dir(&format!("registry-{tag}"));
        std::env::set_var("SPIDER_REGISTRY", &reg);
        (reg, guard)
    }

    #[test]
    fn publish_install_roundtrip_and_lockfile() {
        let (_reg, _guard) = with_registry("roundtrip");
        // A library package.
        let lib = temp_dir("libpkg");
        make_project(&lib, "greetings", "");
        write(
            &lib.join("src").join("lib.sp"),
            "public fn hello(name: Text) -> Text\n    return \"Hi, {name}!\"\n",
        );
        let meta = registry::publish(&lib).unwrap();
        assert_eq!(meta.name, "greetings");

        // An app that installs and uses it.
        let app = temp_dir("app");
        make_project(&app, "party", "");
        write(
            &app.join("src").join("main.sp"),
            "use greetings\nsay greetings.hello(\"Lin\")\n",
        );
        let report = registry::install(&app, "greetings").unwrap();
        assert_eq!(report.name, "greetings");
        assert!(app
            .join("web_modules")
            .join("greetings")
            .join("src")
            .join("lib.sp")
            .is_file());
        let lock = fs::read_to_string(app.join("web.lock")).unwrap();
        assert!(lock.contains("greetings 0.1.0"), "{lock}");
        let manifest = fs::read_to_string(app.join("web.toml")).unwrap();
        assert!(manifest.contains("greetings = \"0.1.0\""), "{manifest}");

        // And the installed package actually runs.
        let out = run_project_capture(&app.join("src").join("main.sp"), &[], &[]).unwrap();
        assert_eq!(out, "Hi, Lin!\n");

        // Audit: intact.
        let audit = registry::audit(&app).unwrap();
        assert_eq!(audit.len(), 1);
        assert!(audit[0].intact);

        // Tamper -> audit catches it.
        write(
            &app.join("web_modules")
                .join("greetings")
                .join("src")
                .join("lib.sp"),
            "public fn hello(name: Text) -> Text\n    return \"pwned\"\n",
        );
        let audit = registry::audit(&app).unwrap();
        assert!(!audit[0].intact);
    }

    /// SRS M5 exit criterion: capability escalation blocked in test.
    #[test]
    fn capability_escalation_is_blocked_at_install() {
        let (_reg, _guard) = with_registry("escalation");
        let lib = temp_dir("fs-pkg");
        make_project(&lib, "sneaky", "\"fs\"");
        write(
            &lib.join("src").join("lib.sp"),
            "use files\npublic fn peek(path: Text) -> Text\n    return try files.read_text(path) else \"\"\n",
        );
        registry::publish(&lib).unwrap();

        // The app allows NO capabilities: install must refuse, naming `fs`.
        let app = temp_dir("safe-app");
        make_project(&app, "safe", "");
        let e = registry::install(&app, "sneaky").unwrap_err();
        assert!(e.contains("fs"), "{e}");
        assert!(!app.join("web_modules").join("sneaky").exists());

        // Once granted, the same install succeeds.
        make_project(&app, "safe", "\"fs\"");
        let report = registry::install(&app, "sneaky").unwrap();
        assert!(report.capabilities.contains("fs"));
    }
}
