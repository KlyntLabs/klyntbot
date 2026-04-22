//! Invariant tests for mirror snapshot coverage.
//!
//! Ensures every declared `mirror_snapshot` attribute has a registered source
//! and no legacy `broadcast::Receiver<DomainEvent>` usage remains.

use ai_core::MirrorSignalSource;
use cognitive::mirror::sources::{
    ConfigArchiverSource, FinanceSpendingDriftSource, MetaRuleSignalSource, RoutingSignalSource,
    TaskFocusPatternSource, TrialPreviewSource,
};

fn collect_specs() -> Vec<ai_core::MirrorSnapshotSpec> {
    // We need a real pool to create MirrorRepo, but we only need spec() which doesn't use the repo.
    // Use a blocking call to create an in-memory pool for the test.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let repo = rt.block_on(async {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        cognitive::mirror::MirrorRepo::new(pool)
    });
    vec![
        RoutingSignalSource::new(repo.clone()).spec(),
        MetaRuleSignalSource::new(repo.clone()).spec(),
        ConfigArchiverSource::new(repo.clone(), None).spec(),
        TrialPreviewSource::new(
            repo.clone(),
            std::sync::Arc::new(dashmap::DashMap::new()),
            None,
        )
        .spec(),
        TaskFocusPatternSource::new(repo.clone()).spec(),
        FinanceSpendingDriftSource::new(repo).spec(),
    ]
}

/// Every declared feature mirror_snapshot attr must have a matching source.
#[test]
fn every_declared_mirror_snapshot_has_a_registered_source() {
    // Hand-maintained list — update when adding a feature.
    let declared_specs: Vec<(&'static str, &'static [ai_core::MirrorSnapshotSpec])> = vec![
        (
            "TasksFeature",
            feature_tasks::TasksFeature::MIRROR_SNAPSHOTS,
        ),
        (
            "FinanceFeature",
            feature_finance::FinanceFeature::MIRROR_SNAPSHOTS,
        ),
    ];

    let registered_specs = collect_specs();

    for (feat_name, specs) in &declared_specs {
        for spec in *specs {
            let name = spec.name;
            let covered = registered_specs.iter().any(|s| s.name == name);
            assert!(
                covered,
                "{feat_name} declares mirror_snapshot(name = \"{name}\") but no \
                 MirrorSignalSource registers SPEC.name == \"{name}\""
            );
        }
    }
}

#[test]
fn feature_owned_sources_have_a_declaration() {
    const SYSTEM_OWNED: &[&str] = &["routing", "meta_rule", "config_archiver", "trial_preview"];
    let registered_specs = collect_specs();
    let all_declared: Vec<&'static str> = [
        feature_tasks::TasksFeature::MIRROR_SNAPSHOTS,
        feature_finance::FinanceFeature::MIRROR_SNAPSHOTS,
    ]
    .iter()
    .flat_map(|s| s.iter().map(|spec| spec.name))
    .collect();

    for spec in &registered_specs {
        if SYSTEM_OWNED.contains(&spec.name) {
            continue;
        }
        assert!(
            all_declared.contains(&spec.name),
            "registered source \"{}\" is not declared by any feature and not on the system \
             allow-list",
            spec.name,
        );
    }
}

#[test]
fn no_broadcast_receiver_in_mirror_sources() {
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mirror_dir = root
        .join("crates")
        .join("cognitive")
        .join("src")
        .join("mirror");
    let mut violations: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(&mirror_dir) {
        let entry = entry.unwrap();
        if !entry.file_name().to_string_lossy().ends_with(".rs") {
            continue;
        }
        let path = entry.path();
        let text = std::fs::read_to_string(path).unwrap();
        for (lineno, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("broadcast::Receiver<") && line.contains("DomainEvent") {
                violations.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    lineno + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Mirror must not subscribe to DomainEventBus directly; use SignalConsumer.\n{}",
        violations.join("\n")
    );
}
