//! Golden-file tests over the corpus.
//!
//! - `corpus/ok/*.sp`  must parse clean; tree dumps are snapshotted.
//! - `corpus/err/*.sp` must produce diagnostics; codes+positions snapshotted.
//! - Every corpus file must round-trip losslessly.
//!
//! Regenerate snapshots with:  SPIDER_UPDATE_GOLDEN=1 cargo test

use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
}

fn update_mode() -> bool {
    std::env::var("SPIDER_UPDATE_GOLDEN").is_ok()
}

fn sp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("missing corpus dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "sp")
                // Formatter goldens (*.fmt.sp) are outputs, not corpus inputs.
                && !p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.ends_with(".fmt"))
        })
        .collect();
    files.sort();
    files
}

fn check_golden(golden_path: &Path, actual: &str) {
    if update_mode() {
        fs::write(golden_path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(golden_path).unwrap_or_else(|_| {
        panic!(
            "missing golden file {} — run with SPIDER_UPDATE_GOLDEN=1 to create it",
            golden_path.display()
        )
    });
    assert_eq!(
        expected.replace("\r\n", "\n"),
        actual,
        "golden mismatch: {}",
        golden_path.display()
    );
}

#[test]
fn ok_corpus() {
    for file in sp_files(&corpus_dir().join("ok")) {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
        let p = spider_syntax::parse(&src);

        assert_eq!(p.root.text(), src, "lossless failed: {}", file.display());
        assert!(
            p.diagnostics.is_empty(),
            "unexpected diagnostics in {}:\n{:#?}",
            file.display(),
            p.diagnostics
        );

        check_golden(&file.with_extension("tree.txt"), &p.root.dump());
    }
}

#[test]
fn err_corpus() {
    for file in sp_files(&corpus_dir().join("err")) {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
        let p = spider_syntax::parse(&src);

        assert_eq!(p.root.text(), src, "lossless failed: {}", file.display());
        assert!(
            !p.diagnostics.is_empty(),
            "expected diagnostics in {}",
            file.display()
        );

        let mut report = String::new();
        for d in &p.diagnostics {
            let (line, col) = spider_syntax::line_col(&src, d.offset);
            report.push_str(&format!("{} {}:{} {}\n", d.code, line, col, d.message));
        }
        check_golden(&file.with_extension("diag.txt"), &report);
        check_golden(&file.with_extension("tree.txt"), &p.root.dump());
    }
}

#[test]
fn every_reported_code_has_an_explain_entry() {
    for sub in ["ok", "err"] {
        for file in sp_files(&corpus_dir().join(sub)) {
            let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
            for d in spider_syntax::parse(&src).diagnostics {
                assert!(
                    spider_syntax::explain(d.code).is_some(),
                    "diagnostic {} has no Explain entry (from {})",
                    d.code,
                    file.display()
                );
            }
        }
    }
}
