use klynt_skill_loader::{
    activator::ActivationConfig, KlyntFrontmatter, SkillActivator, SkillIndex, SkillSource,
};
use std::path::PathBuf;

fn make_index_with_paths(name: &str, paths: &[&str]) -> SkillIndex {
    let mut idx = SkillIndex::new();
    let fm = KlyntFrontmatter {
        name: name.into(),
        description: "test".into(),
        paths: paths.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    idx.insert_for_test(name.into(), fm, SkillSource::User, PathBuf::from("/tmp/x"));
    idx
}

#[test]
fn activates_when_path_matches_glob() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let activated = act.touch_path(std::path::Path::new("src/main.rs")).unwrap();
    assert_eq!(activated, vec!["rust-helper".to_string()]);
    assert!(act.active_set().contains("rust-helper"));
}

#[test]
fn does_not_activate_unrelated_path() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let activated = act.touch_path(std::path::Path::new("README.md")).unwrap();
    assert!(activated.is_empty());
    assert!(act.active_set().is_empty());
}

#[test]
fn idempotent_activation() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();
    let first = act.touch_path(std::path::Path::new("a.rs")).unwrap();
    let second = act.touch_path(std::path::Path::new("b.rs")).unwrap();
    assert_eq!(first, vec!["rust-helper".to_string()]);
    assert!(second.is_empty()); // already active, do not re-emit
}

#[test]
fn always_activate_short_circuits() {
    let idx = make_index_with_paths("forced", &["doesnt/match"]);
    let cfg = ActivationConfig {
        always_activate: vec!["forced".into()],
        ..Default::default()
    };
    let act = SkillActivator::new(idx, cfg).unwrap();
    assert!(act.active_set().contains("forced"));
}

#[test]
fn never_activate_blocks_path_match() {
    let idx = make_index_with_paths("blocked", &["**/*.rs"]);
    let cfg = ActivationConfig {
        never_activate: vec!["blocked".into()],
        ..Default::default()
    };
    let mut act = SkillActivator::new(idx, cfg).unwrap();
    let activated = act.touch_path(std::path::Path::new("a.rs")).unwrap();
    assert!(activated.is_empty());
    assert!(!act.active_set().contains("blocked"));
}

#[test]
fn max_active_skills_cap_enforced() {
    let mut idx = SkillIndex::new();
    for i in 0..5 {
        let name = format!("s{i}");
        let fm = KlyntFrontmatter {
            name: name.clone(),
            description: "test".into(),
            paths: vec!["**/*.rs".into()],
            ..Default::default()
        };
        idx.insert_for_test(name, fm, SkillSource::User, PathBuf::from("/tmp/x"));
    }
    let cfg = ActivationConfig {
        max_active_skills: 3,
        ..Default::default()
    };
    let mut act = SkillActivator::new(idx, cfg).unwrap();
    act.touch_path(std::path::Path::new("a.rs")).unwrap();
    assert_eq!(act.active_set().len(), 3);
}

#[test]
fn lru_cache_returns_same_result_on_second_touch() {
    let idx = make_index_with_paths("rust-helper", &["**/*.rs"]);
    let mut act = SkillActivator::new(idx, ActivationConfig::default()).unwrap();

    // First touch activates.
    let first = act.touch_path(std::path::Path::new("src/lib.rs")).unwrap();
    assert_eq!(first, vec!["rust-helper".to_string()]);

    // Second touch of the same path hits the cache and sees the skill is already active.
    let second = act.touch_path(std::path::Path::new("src/lib.rs")).unwrap();
    assert!(second.is_empty());

    // A different matching path still works (cache miss, new entry).
    let third = act.touch_path(std::path::Path::new("src/main.rs")).unwrap();
    assert!(third.is_empty()); // already active from first touch
}
