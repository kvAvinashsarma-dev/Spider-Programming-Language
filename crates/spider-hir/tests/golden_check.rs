//! Semantic golden tests.
//!
//! - `corpus/check_ok/*.sp`  parse clean AND check clean (inference sound).
//! - `corpus/check_err/*.sp` parse clean, check dirty; codes+positions
//!   snapshotted in `.diag.txt`.
//!
//! Regenerate with:  SPIDER_UPDATE_GOLDEN=1 cargo test

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
        .filter(|p| p.extension().is_some_and(|x| x == "sp"))
        .collect();
    files.sort();
    files
}

#[test]
fn check_ok_corpus_is_semantically_clean() {
    for file in sp_files(&corpus_dir().join("check_ok")) {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
        let parse = spider_syntax::parse(&src);
        assert!(
            parse.diagnostics.is_empty(),
            "syntax errors in {}: {:#?}",
            file.display(),
            parse.diagnostics
        );
        let diags = spider_hir::check_parse(&parse);
        assert!(
            diags.is_empty(),
            "unexpected semantic diagnostics in {}:\n{:#?}",
            file.display(),
            diags
        );
    }
}

#[test]
fn check_err_corpus_diagnostics_match_goldens() {
    for file in sp_files(&corpus_dir().join("check_err")) {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
        let parse = spider_syntax::parse(&src);
        assert!(
            parse.diagnostics.is_empty(),
            "check_err files must be syntactically valid; {} is not: {:#?}",
            file.display(),
            parse.diagnostics
        );
        let diags = spider_hir::check_parse(&parse);
        assert!(
            !diags.is_empty(),
            "expected semantic diagnostics in {}",
            file.display()
        );

        let mut report = String::new();
        for d in &diags {
            let (line, col) = spider_syntax::line_col(&src, d.offset);
            report.push_str(&format!("{} {}:{} {}\n", d.code, line, col, d.message));
        }
        let golden = file.with_extension("diag.txt");
        if update_mode() {
            fs::write(&golden, &report).unwrap();
        } else {
            let expected = fs::read_to_string(&golden)
                .unwrap_or_else(|_| {
                    panic!(
                        "missing golden {} — run with SPIDER_UPDATE_GOLDEN=1",
                        golden.display()
                    )
                })
                .replace("\r\n", "\n");
            assert_eq!(expected, report, "golden mismatch: {}", golden.display());
        }

        // Every emitted code must have an authored Explain entry.
        for d in &diags {
            assert!(
                spider_syntax::explain(d.code).is_some(),
                "diagnostic {} has no Explain entry",
                d.code
            );
        }
    }
}
