//! Formatter golden tests over the ok-corpus, plus the two formatter laws:
//! idempotence (fmt(fmt(x)) == fmt(x)) and structure preservation (the
//! formatted output parses cleanly).

use std::fs;
use std::path::{Path, PathBuf};

fn corpus_ok() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("ok")
}

fn update_mode() -> bool {
    std::env::var("SPIDER_UPDATE_GOLDEN").is_ok()
}

fn sp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "sp")
                // Golden outputs (*.fmt.sp) are not corpus inputs.
                && !p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.ends_with(".fmt"))
        })
        .collect();
    files.sort();
    files
}

#[test]
fn formatter_goldens_idempotence_and_reparse() {
    for file in sp_files(&corpus_ok()) {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");

        let once = spider_fmt::format_source(&src)
            .unwrap_or_else(|d| panic!("fmt refused {}:\n{d:#?}", file.display()));

        // Law 1: idempotence.
        let twice = spider_fmt::format_source(&once)
            .unwrap_or_else(|d| panic!("fmt broke its own output {}:\n{d:#?}", file.display()));
        assert_eq!(once, twice, "not idempotent: {}", file.display());

        // Law 2: output parses cleanly.
        let reparse = spider_syntax::parse(&once);
        assert!(
            reparse.diagnostics.is_empty(),
            "formatted output of {} no longer parses:\n{:#?}",
            file.display(),
            reparse.diagnostics
        );

        // Golden snapshot.
        let golden = file.with_extension("fmt.sp");
        if update_mode() {
            fs::write(&golden, &once).unwrap();
        } else {
            let expected = fs::read_to_string(&golden)
                .unwrap_or_else(|_| {
                    panic!(
                        "missing golden {} — run with SPIDER_UPDATE_GOLDEN=1",
                        golden.display()
                    )
                })
                .replace("\r\n", "\n");
            assert_eq!(expected, once, "golden mismatch: {}", golden.display());
        }
    }
}
