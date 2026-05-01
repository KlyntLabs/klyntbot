use klynt_skill_loader::{DiscoveryRoots, SkillIndex, SkillSource};
use std::fs;
use tempfile::TempDir;

fn write_skill(dir: &std::path::Path, name: &str, body: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(dir.join(name).join("SKILL.md"), body).unwrap();
}

const MIN_FRONTMATTER: &str = r#"---
name: alpha
description: Alpha skill.
---
# Alpha

Body.
"#;

#[test]
fn discovers_user_skill() {
    let home = TempDir::new().unwrap();
    write_skill(&home.path().join("skills"), "alpha", MIN_FRONTMATTER);

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: std::env::temp_dir(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    let entry = idx.get("alpha").expect("alpha discovered");
    assert_eq!(entry.frontmatter.name, "alpha");
    assert!(matches!(entry.source, SkillSource::User));
}

#[test]
fn project_skill_overrides_user() {
    let home = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    write_skill(
        &home.path().join("skills"),
        "alpha",
        "---\nname: alpha\ndescription: User-side.\n---\nUser body.",
    );
    write_skill(
        &repo.path().join(".klyntbot/skills"),
        "alpha",
        "---\nname: alpha\ndescription: Project-side.\n---\nProject body.",
    );

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: Some(repo.path().to_path_buf()),
        cwd: repo.path().to_path_buf(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    let entry = idx.get("alpha").expect("alpha discovered");
    assert_eq!(entry.frontmatter.description, "Project-side.");
    assert!(matches!(entry.source, SkillSource::Project));
}

#[test]
fn missing_paths_skipped_silently() {
    let home = TempDir::new().unwrap();
    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: home.path().to_path_buf(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    assert_eq!(idx.len(), 0);
}

#[test]
fn malformed_skill_emits_warning_and_continues() {
    let home = TempDir::new().unwrap();
    write_skill(&home.path().join("skills"), "alpha", MIN_FRONTMATTER);
    write_skill(
        &home.path().join("skills"),
        "bad",
        "this has no frontmatter at all",
    );

    let roots = DiscoveryRoots {
        klyntbot_home: home.path().to_path_buf(),
        repo_id: None,
        repo_root: None,
        cwd: std::env::temp_dir(),
    };
    let idx = SkillIndex::discover(&roots).unwrap();
    assert!(idx.get("alpha").is_some());
    assert!(idx.get("bad").is_none());
}
