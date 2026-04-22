//! Invariant: `RecallDomain` is the source of truth in AI subsystems.
//!
//! No file under the scanned AI crates may carry a raw "general" / "tasks" /
//! "finance" / "productivity" / "learning" string literal — those are the
//! variants of `RecallDomain` and must be referenced via the enum, not as
//! magic strings. The allowlist covers files where the conversion happens at
//! a boundary (e.g. RecallDomain::as_str / from_str, repo column parsers).

use std::path::PathBuf;

const FORBIDDEN: &[&str] = &[
    "\"general\"",
    "\"tasks\"",
    "\"finance\"",
    "\"productivity\"",
    "\"learning\"",
];

const SCAN_DIRS: &[&str] = &[
    "crates/cognitive/src/pipeline",
    "crates/feature-coaching/src/pattern_detector",
    "crates/feature-coaching/src/signal_accumulator",
];

/// File suffixes that are allowed to carry the raw literals (boundary code).
const ALLOWLIST_SUFFIXES: &[&str] = &[
    "ai-core/src/recall_domain.rs",
    "storage/src/repos/retrieval_feedback.rs",
];

#[test]
fn no_raw_domain_literals_in_ai_crates() {
    let root = workspace_root();
    let mut violations: Vec<String> = Vec::new();

    for dir in SCAN_DIRS {
        let abs = root.join(dir);
        if !abs.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&abs) {
            let entry = entry.expect("walkdir");
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let path = entry.path();
            let path_str = path.to_string_lossy();
            if ALLOWLIST_SUFFIXES.iter().any(|s| path_str.ends_with(s)) {
                continue;
            }
            let text = std::fs::read_to_string(path).unwrap();
            for (lineno, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for lit in FORBIDDEN {
                    if line.contains(lit) {
                        violations.push(format!(
                            "{}:{}: {}",
                            path.display(),
                            lineno + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "raw domain literals found in AI crates (use RecallDomain instead):\n{}",
        violations.join("\n")
    );
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for an integration test inside the facade crate
    // is the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
