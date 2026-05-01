use klynt_skill_loader::{activator::ActivationConfig, DiscoveryRoots, SkillActivator, SkillIndex};
use std::fs;
use tempfile::TempDir;

const SKILL: &str =
    "---\nname: deep\ndescription: Deep skill.\npaths:\n  - \"**/*.rs\"\n---\nBody\n";

#[test]
fn dynamic_walk_up_finds_nested_skill_dir() {
    let repo = TempDir::new().unwrap();
    let nested_skills = repo.path().join("subdir/.klyntbot/skills");
    fs::create_dir_all(nested_skills.join("deep")).unwrap();
    fs::write(nested_skills.join("deep/SKILL.md"), SKILL).unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let initial = SkillIndex::discover(&roots).unwrap();
    assert!(initial.get("deep").is_none(), "skill not at static root");

    let mut act = SkillActivator::new(initial, ActivationConfig::default()).unwrap();
    let activated = act
        .touch_path_with_discovery(&repo.path().join("subdir/foo.rs"), &roots)
        .unwrap();
    assert_eq!(activated, vec!["deep".to_string()]);
}

#[test]
fn dynamic_walk_does_not_cross_cwd_boundary() {
    let outside = TempDir::new().unwrap();
    let inside = outside.path().join("repo");
    fs::create_dir_all(&inside).unwrap();
    let outside_skills = outside.path().join(".klyntbot/skills");
    fs::create_dir_all(outside_skills.join("outside")).unwrap();
    fs::write(
        outside_skills.join("outside/SKILL.md"),
        "---\nname: outside\ndescription: Outside\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(inside.clone()),
        cwd: inside.clone(),
    };
    let mut act = SkillActivator::new(SkillIndex::new(), ActivationConfig::default()).unwrap();
    let activated = act
        .touch_path_with_discovery(&inside.join("foo.rs"), &roots)
        .unwrap();
    assert!(activated.is_empty(), "should not walk above cwd");
}

#[test]
fn dynamic_walk_caches_seen_dirs() {
    let repo = TempDir::new().unwrap();
    std::fs::write(repo.path().join("a.rs"), "").unwrap();
    std::fs::write(repo.path().join("b.rs"), "").unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let mut act = SkillActivator::new(SkillIndex::new(), ActivationConfig::default()).unwrap();
    act.touch_path_with_discovery(&repo.path().join("a.rs"), &roots)
        .unwrap();
    let first_seen_count = act.dynamic_seen_dirs_len();
    act.touch_path_with_discovery(&repo.path().join("b.rs"), &roots)
        .unwrap();
    assert_eq!(
        act.dynamic_seen_dirs_len(),
        first_seen_count,
        "second touch in same dir hits cache"
    );
}
