//! Runtime golden tests: each corpus/run/*.sp runs on the Silk VM and its
//! output must match the frozen .out.txt snapshot exactly.
//!
//! Regenerate with:  SPIDER_UPDATE_GOLDEN=1 cargo test

use std::fs;
use std::path::PathBuf;

fn corpus_run() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("run")
}

#[test]
fn run_corpus_matches_goldens() {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_run())
        .expect("missing corpus/run")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sp"))
        .collect();
    files.sort();
    assert!(!files.is_empty());

    for file in files {
        let src = fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
        let out = spider_silk::run_capture(&src, &[])
            .unwrap_or_else(|e| panic!("{} failed: {e}", file.display()));
        let golden = file.with_extension("out.txt");
        if std::env::var("SPIDER_UPDATE_GOLDEN").is_ok() {
            fs::write(&golden, &out).unwrap();
        } else {
            let expected = fs::read_to_string(&golden)
                .unwrap_or_else(|_| {
                    panic!(
                        "missing golden {} — run with SPIDER_UPDATE_GOLDEN=1",
                        golden.display()
                    )
                })
                .replace("\r\n", "\n");
            assert_eq!(expected, out, "output changed: {}", file.display());
        }
    }
}
