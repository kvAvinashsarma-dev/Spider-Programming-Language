//! Multi-file module resolution (ADR-015).
//!
//! `use helpers` in a project loads `src/helpers.sp`; `use shop.cart` loads
//! `src/shop/cart.sp`; `use <pkg>` loads `web_modules/<pkg>/src/lib.sp`.
//! Standard-library names always win (nobody may shadow `math` with a file).
//! Cycles are an error with the cycle path spelled out.

use spider_syntax::{Parse, SyntaxKind as K};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Module names the loader must never resolve to files.
const STD_NAMES: &[&str] = &[
    "math", "time", "random", "json", "regex", "test", "files", "net", "env", "exec", "ai",
];

pub struct LoadedModule {
    /// Binding name (last path segment) — how code refers to it.
    pub name: String,
    pub path: PathBuf,
    pub src: String,
    pub parse: Parse,
    /// use-alias -> loaded module name, for imports resolved to files.
    pub imports: HashMap<String, String>,
}

pub struct Project {
    pub modules: Vec<LoadedModule>,
    pub entry: usize,
    /// The project root (directory holding web.toml), if any.
    pub root: Option<PathBuf>,
}

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() {
        Some(start.to_path_buf())
    } else {
        start.parent().map(|p| p.to_path_buf())
    };
    for _ in 0..16 {
        let d = dir?;
        if d.join("web.toml").is_file() {
            return Some(d);
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}

pub fn load_project(entry_file: &Path) -> Result<Project, String> {
    let root = find_project_root(entry_file);
    let entry_src = std::fs::read_to_string(entry_file)
        .map_err(|e| format!("cannot read {}: {e}", entry_file.display()))?;
    let mut loader = Loader {
        root: root.clone(),
        entry_dir: entry_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
        modules: Vec::new(),
        by_name: HashMap::new(),
        loading: Vec::new(),
    };
    let entry_idx = loader.load(
        "main",
        entry_file.to_path_buf(),
        spider_syntax::strip_bom(&entry_src).to_string(),
    )?;
    Ok(Project {
        modules: loader.modules,
        entry: entry_idx,
        root,
    })
}

struct Loader {
    root: Option<PathBuf>,
    entry_dir: PathBuf,
    modules: Vec<LoadedModule>,
    by_name: HashMap<String, usize>,
    loading: Vec<String>,
}

impl Loader {
    fn load(&mut self, name: &str, path: PathBuf, src: String) -> Result<usize, String> {
        if let Some(pos) = self.loading.iter().position(|n| n == name) {
            let mut cycle: Vec<&str> = self.loading[pos..].iter().map(|s| s.as_str()).collect();
            cycle.push(name);
            return Err(format!(
                "modules import each other in a circle: {} — break the circle by moving the shared definitions into a third module",
                cycle.join(" -> ")
            ));
        }
        if let Some(&idx) = self.by_name.get(name) {
            return Ok(idx);
        }
        self.loading.push(name.to_string());

        let src = src.replace("\r\n", "\n");
        let parse = spider_syntax::parse(&src);
        let mut imports = HashMap::new();
        // Even if the file has parse errors, record it — the caller reports
        // diagnostics per file. Only follow imports on a clean parse.
        if parse.diagnostics.is_empty() {
            for n in parse.root.child_nodes() {
                if n.kind != K::UseDecl {
                    continue;
                }
                let segments: Vec<String> = n
                    .child_tokens()
                    .into_iter()
                    .filter(|t| t.kind == K::Ident)
                    .map(|t| t.text.clone())
                    .collect();
                let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
                    continue;
                };
                if STD_NAMES.contains(&first.as_str()) {
                    continue; // stdlib always wins
                }
                if let Some((mod_path, mod_src)) = self.resolve_file(&segments)? {
                    let idx = self.load(last, mod_path, mod_src)?;
                    imports.insert(last.clone(), self.modules[idx].name.clone());
                }
                // Unresolved non-std imports keep M2 behavior (W0002).
            }
        }

        self.loading.pop();
        let idx = self.modules.len();
        self.modules.push(LoadedModule {
            name: name.to_string(),
            path,
            src,
            parse,
            imports,
        });
        self.by_name.insert(name.to_string(), idx);
        Ok(idx)
    }

    /// Search order: project src/ (dotted path -> folders), entry dir
    /// (bare scripts with sibling files), then installed packages.
    fn resolve_file(&self, segments: &[String]) -> Result<Option<(PathBuf, String)>, String> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        let rel: PathBuf = segments.iter().collect::<PathBuf>().with_extension("sp");
        if let Some(root) = &self.root {
            candidates.push(root.join("src").join(&rel));
        }
        candidates.push(self.entry_dir.join(&rel));
        if segments.len() == 1 {
            if let Some(root) = &self.root {
                candidates.push(
                    root.join("web_modules")
                        .join(&segments[0])
                        .join("src")
                        .join("lib.sp"),
                );
            }
        }
        for c in candidates {
            if c.is_file() {
                let src = std::fs::read_to_string(&c)
                    .map_err(|e| format!("cannot read {}: {e}", c.display()))?;
                return Ok(Some((c, spider_syntax::strip_bom(&src).to_string())));
            }
        }
        Ok(None)
    }
}
