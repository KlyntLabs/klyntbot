use klynt_skill_loader::{
    activator::ActivationConfig, replay::replay_session_history, DiscoveryRoots, SkillActivator,
    SkillIndex,
};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn replay_activates_path_conditional_skills() {
    let repo = TempDir::new().unwrap();
    let skills_dir = repo.path().join(".klyntbot/skills");
    std::fs::create_dir_all(skills_dir.join("rust")).unwrap();
    std::fs::write(
        skills_dir.join("rust/SKILL.md"),
        "---\nname: rust\ndescription: Rust\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();
    std::fs::create_dir_all(skills_dir.join("toml")).unwrap();
    std::fs::write(
        skills_dir.join("toml/SKILL.md"),
        "---\nname: toml\ndescription: TOML\npaths: [\"**/*.toml\"]\n---\nBody\n",
    )
    .unwrap();

    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let history_paths = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
    ];

    let mut act = SkillActivator::new(
        SkillIndex::discover(&roots).unwrap(),
        ActivationConfig::default(),
    )
    .unwrap();
    let activated = replay_session_history(&mut act, &history_paths, &roots).unwrap();
    assert!(activated.contains(&"rust".to_string()));
    assert!(activated.contains(&"toml".to_string()));
}

#[test]
fn replay_is_deterministic_k6() {
    // K6: same persisted history → same active set, regardless of order beyond "first hit wins".
    let repo = TempDir::new().unwrap();
    let skills_dir = repo.path().join(".klyntbot/skills");
    std::fs::create_dir_all(skills_dir.join("rust")).unwrap();
    std::fs::write(
        skills_dir.join("rust/SKILL.md"),
        "---\nname: rust\ndescription: Rust\npaths: [\"**/*.rs\"]\n---\nBody\n",
    )
    .unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: TempDir::new().unwrap().path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let history = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];

    let run = |hist: &[PathBuf]| -> Vec<String> {
        let mut act = SkillActivator::new(
            SkillIndex::discover(&roots).unwrap(),
            ActivationConfig::default(),
        )
        .unwrap();
        replay_session_history(&mut act, hist, &roots).unwrap();
        let mut s: Vec<String> = act.active_set().iter().cloned().collect();
        s.sort();
        s
    };
    assert_eq!(run(&history), run(&history));
}
