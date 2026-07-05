//! web — the package manager (M5 MVP).
//!
//! The registry is a local directory (`~/.spider/registry`, overridable via
//! SPIDER_REGISTRY for tests and offline classrooms); the public networked
//! registry with signing arrives post-1.0. What is real today, per SRS
//! FR-15/16/19: publish/install round-trips, an exact lockfile with content
//! fingerprints, capability diffing that blocks escalation at install time,
//! and no install scripts — a package is data, never code that runs at
//! install.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn registry_root() -> PathBuf {
    if let Ok(p) = std::env::var("SPIDER_REGISTRY") {
        return PathBuf::from(p);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".spider").join("registry")
}

pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub capabilities: HashSet<String>,
}

fn read_project_meta(root: &Path) -> Result<PackageMeta, String> {
    let manifest_path = root.join("web.toml");
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
    let caps = spider_hir::parse_manifest(&text)?.capabilities;
    let mut name = None;
    let mut version = None;
    let mut in_project = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            match k.trim() {
                "name" => name = Some(v),
                "version" => version = Some(v),
                _ => {}
            }
        }
    }
    Ok(PackageMeta {
        name: name.ok_or("web.toml has no [project] name")?,
        version: version.unwrap_or_else(|| "0.1.0".into()),
        capabilities: caps,
    })
}

/// FNV-1a over sorted relative paths + contents: a deterministic content
/// fingerprint. (Cryptographic signing ships with the public registry.)
pub fn fingerprint(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    };
    for rel in &files {
        feed(rel.as_bytes());
        let content = fs::read(dir.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))?;
        // Normalize line endings so the fingerprint survives git/OS churn.
        let content: Vec<u8> = content.into_iter().filter(|b| *b != b'\r').collect();
        feed(&content);
    }
    Ok(format!("{hash:016x}"))
}

fn collect_files(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "web_modules" || name == "target" {
            continue;
        }
        if p.is_dir() {
            collect_files(base, &p, out)?;
        } else {
            let rel = p
                .strip_prefix(base)
                .map_err(|_| "path outside package")?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let entries = fs::read_dir(from).map_err(|e| format!("cannot list {}: {e}", from.display()))?;
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "web_modules" || name == "target" {
            continue;
        }
        let dest = to.join(&name);
        if p.is_dir() {
            copy_tree(&p, &dest)?;
        } else {
            fs::copy(&p, &dest).map_err(|e| format!("cannot copy {name}: {e}"))?;
        }
    }
    Ok(())
}

/// `web publish` — copies the project into the local registry.
pub fn publish(project_root: &Path) -> Result<PackageMeta, String> {
    let meta = read_project_meta(project_root)?;
    if !project_root.join("src").join("lib.sp").is_file() {
        return Err(
            "a package shares code through src/lib.sp — create it (apps have main.sp, libraries lib.sp)"
                .into(),
        );
    }
    let dest = registry_root().join(&meta.name).join(&meta.version);
    if dest.exists() {
        return Err(format!(
            "{} {} is already published — bump the version in web.toml (published versions are immutable)",
            meta.name, meta.version
        ));
    }
    copy_tree(project_root, &dest)?;
    Ok(meta)
}

/// Latest published version of a package (highest by dotted-number order).
fn latest_version(name: &str) -> Result<(String, PathBuf), String> {
    let dir = registry_root().join(name);
    let entries = fs::read_dir(&dir)
        .map_err(|_| format!("no package named `{name}` in the registry — `web publish` it first"))?;
    let mut versions: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort_by_key(|v| {
        v.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    });
    let v = versions
        .pop()
        .ok_or_else(|| format!("`{name}` has no published versions"))?;
    let path = dir.join(&v);
    Ok((v, path))
}

#[derive(Debug)]
pub struct InstallReport {
    pub name: String,
    pub version: String,
    pub hash: String,
    pub capabilities: HashSet<String>,
}

/// `web install` — capability-diffed, lockfile-recorded, no install scripts.
pub fn install(project_root: &Path, pkg: &str) -> Result<InstallReport, String> {
    let project = read_project_meta(project_root)?;
    let (version, pkg_dir) = latest_version(pkg)?;
    let pkg_meta = read_project_meta(&pkg_dir)?;

    // THE security moment (SRS FR-16): a dependency may never hold more
    // capabilities than the project grants.
    let escalation: Vec<String> = pkg_meta
        .capabilities
        .difference(&project.capabilities)
        .cloned()
        .collect();
    if !escalation.is_empty() {
        let mut list = escalation;
        list.sort();
        return Err(format!(
            "`{pkg}` needs capabilities this project does not allow: {}\n  the project allows: [{}]\n  to accept, add the missing capabilities to `allow` in web.toml — only if you trust `{pkg}` with them",
            list.join(", "),
            {
                let mut have: Vec<&str> =
                    project.capabilities.iter().map(|s| s.as_str()).collect();
                have.sort();
                have.join(", ")
            }
        ));
    }

    let dest = project_root.join("web_modules").join(pkg);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| format!("cannot refresh {pkg}: {e}"))?;
    }
    copy_tree(&pkg_dir, &dest)?;
    let hash = fingerprint(&dest)?;
    update_lock(project_root, pkg, &version, &hash)?;
    add_dependency_line(project_root, pkg, &version)?;
    Ok(InstallReport {
        name: pkg.to_string(),
        version,
        hash,
        capabilities: pkg_meta.capabilities,
    })
}

fn update_lock(project_root: &Path, name: &str, version: &str, hash: &str) -> Result<(), String> {
    let lock_path = project_root.join("web.lock");
    let mut lines: Vec<String> = if lock_path.is_file() {
        fs::read_to_string(&lock_path)
            .map_err(|e| e.to_string())?
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter(|l| l.split_whitespace().next() != Some(name))
            .map(|l| l.to_string())
            .collect()
    } else {
        Vec::new()
    };
    lines.push(format!("{name} {version} {hash}"));
    lines.sort();
    let mut out = String::from("# web.lock — exact installed versions; commit this file.\n");
    out.push_str(&lines.join("\n"));
    out.push('\n');
    fs::write(&lock_path, out).map_err(|e| e.to_string())
}

fn add_dependency_line(project_root: &Path, name: &str, version: &str) -> Result<(), String> {
    let manifest_path = project_root.join("web.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    if text
        .lines()
        .any(|l| l.trim_start().starts_with(&format!("{name} ")) || l.trim_start().starts_with(&format!("{name}=")))
    {
        return Ok(());
    }
    let dep_line = format!("{name} = \"{version}\"");
    let new_text = if text.contains("[dependencies]") {
        text.replace("[dependencies]", &format!("[dependencies]\n{dep_line}"))
    } else {
        format!("{text}\n[dependencies]\n{dep_line}\n")
    };
    fs::write(&manifest_path, new_text).map_err(|e| e.to_string())
}

pub struct AuditEntry {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub intact: bool,
}

/// `web audit` — every dependency: version, capabilities, fingerprint check.
pub fn audit(project_root: &Path) -> Result<Vec<AuditEntry>, String> {
    let lock_path = project_root.join("web.lock");
    if !lock_path.is_file() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for line in fs::read_to_string(&lock_path).map_err(|e| e.to_string())?.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [name, version, hash] = parts[..] else {
            continue;
        };
        let dir = project_root.join("web_modules").join(name);
        let intact = dir.is_dir() && fingerprint(&dir)? == hash;
        let caps = if dir.is_dir() {
            let mut c: Vec<String> = read_project_meta(&dir)
                .map(|m| m.capabilities.into_iter().collect())
                .unwrap_or_default();
            c.sort();
            c
        } else {
            Vec::new()
        };
        out.push(AuditEntry {
            name: name.to_string(),
            version: version.to_string(),
            capabilities: caps,
            intact,
        });
    }
    Ok(out)
}
