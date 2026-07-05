//! Minimal `web.toml` reading — just enough for the capability model.
//!
//! M4 reads `[capabilities] allow = ["fs", …]` (single line). The real TOML
//! surface arrives with the `web` package manager in M5; keeping this parser
//! tiny and dependency-free is deliberate.

use crate::stdlib::KNOWN_CAPABILITIES;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct Manifest {
    pub capabilities: HashSet<String>,
}

pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut in_capabilities = false;
    let mut caps = HashSet::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_capabilities = line == "[capabilities]";
            continue;
        }
        if !in_capabilities {
            continue;
        }
        if let Some(rest) = line.strip_prefix("allow") {
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                return Err("expected `allow = [ ... ]` in [capabilities]".into());
            };
            let rest = rest.trim();
            if !(rest.starts_with('[') && rest.ends_with(']')) {
                return Err(
                    "the capability list must be on one line: allow = [\"fs\", \"net\"]".into(),
                );
            }
            let inner = &rest[1..rest.len() - 1];
            for part in inner.split(',') {
                let part = part.trim().trim_matches('"').trim_matches('\'');
                if part.is_empty() {
                    continue;
                }
                if !KNOWN_CAPABILITIES.contains(&part) {
                    return Err(format!(
                        "`{part}` is not a capability — the capabilities are: {}",
                        KNOWN_CAPABILITIES.join(", ")
                    ));
                }
                caps.insert(part.to_string());
            }
        }
    }
    Ok(Manifest { capabilities: caps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_capabilities() {
        let m = parse_manifest(
            "[project]\nname = \"x\"\n\n[capabilities]\nallow = [\"fs\", \"net\"]\n\n[dependencies]\n",
        )
        .unwrap();
        assert!(m.capabilities.contains("fs"));
        assert!(m.capabilities.contains("net"));
        assert_eq!(m.capabilities.len(), 2);
    }

    #[test]
    fn empty_allow_is_safe_mode() {
        let m = parse_manifest("[capabilities]\nallow = []\n").unwrap();
        assert!(m.capabilities.is_empty());
    }

    #[test]
    fn unknown_capability_is_rejected() {
        let e = parse_manifest("[capabilities]\nallow = [\"wizardry\"]\n").unwrap_err();
        assert!(e.contains("wizardry"), "{e}");
    }

    #[test]
    fn comments_and_other_sections_ignored() {
        let m = parse_manifest(
            "# hello\n[dependencies]\nallow = [\"fs\"]\n[capabilities]\nallow = [\"env\"]  # just env\n",
        )
        .unwrap();
        assert!(m.capabilities.contains("env"));
        assert!(!m.capabilities.contains("fs"));
    }
}
