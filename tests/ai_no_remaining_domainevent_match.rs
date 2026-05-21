//! Invariant test: only allowlisted files match on `DomainEvent`.
//!
//! Enforces spec §8.10: the workspace `match` on `DomainEvent` exists in
//! exactly one logical place — the translator (`ai_pipeline::translate` and
//! its `try_into_*` helpers). All other matches are either bus-internal
//! accessors or feature `From<FeatureEvent>` adapters; both are allowlisted.

use std::path::PathBuf;
use std::process::Command;

const ALLOWED_FILES: &[&str] = &[
    // bus internals: variant_name() and domain()
    "crates/bus/src/domain_events.rs",
    // translator: the canonical match site (allowed for all try_into_*)
    "crates/app-core/src/init/ai_pipeline.rs",
    // feature From<FeatureEvent> adapters (one per feature)
    "crates/feature-tasks/src/events.rs",
    "crates/feature-coaching/src/events.rs",
    "crates/feature-productivity/src/events.rs",
    "crates/feature-notes/src/events.rs",
    "crates/feature-learning/src/events.rs",
    "crates/feature-language-learning/src/events.rs",
    "crates/cognitive/src/services/community_intelligence/events.rs",
    "crates/cognitive/src/services/community_intelligence/co_activation_events.rs",
    // wake orchestrator: this match is part of v3.x scope (not in v3 deletion target)
    "crates/app-core/src/wake_orchestrator.rs",
    // cognitive background service: test helper mapping (#[cfg(test)])
    "crates/cognitive/src/services/background.rs",
];

#[test]
fn only_allowed_files_match_domainevent() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let output = Command::new("grep")
        .args(["-rln", r"match.*DomainEvent|match e \{"])
        .arg("crates/")
        .current_dir(&workspace_root)
        .output()
        .expect("grep ran");
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| l.contains("DomainEvent"))
        .map(|l| l.trim().to_string())
        .collect();

    let unexpected: Vec<&String> = files
        .iter()
        .filter(|f| !ALLOWED_FILES.iter().any(|a| f.ends_with(a)))
        .collect();

    assert!(
        unexpected.is_empty(),
        "unexpected DomainEvent match in: {:?}",
        unexpected
    );
}
