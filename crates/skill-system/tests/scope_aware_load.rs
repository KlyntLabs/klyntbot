//! Scope-aware SkillStore filtering.

use skill_system::store::SkillStore;

#[test]
fn list_for_scope_filters_correctly() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    std::fs::create_dir_all(&dir).unwrap();

    // Write a coding-scoped skill.
    std::fs::write(
        dir.join("coding.md"),
        "---\nname: coding-style\ndescription: Rust style guide\nscope: coding\n---\nUse Rust.",
    )
    .unwrap();

    // Write a global skill.
    std::fs::write(
        dir.join("general.md"),
        "---\nname: general-comm\ndescription: Communication tips\n---\nBe clear.",
    )
    .unwrap();

    let store = SkillStore::load(&dir).unwrap();
    let coding = store.list_for_scope(Some("coding"));
    assert_eq!(coding.len(), 1);
    assert_eq!(coding[0].frontmatter.name, "coding-style");

    let global = store.list_for_scope(None);
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].frontmatter.name, "general-comm");
}

#[test]
fn list_for_scope_matches_repo_id() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("repo.md"),
        "---\nname: repo-specific\ndescription: Repo tips\nscope_repo_id: my-org/my-repo\n---\nRepo rules.",
    )
    .unwrap();

    let store = SkillStore::load(&dir).unwrap();

    let repo = store.list_for_scope(Some("my-org/my-repo"));
    assert_eq!(
        repo.len(),
        1,
        "expected 1 repo-scoped skill, got {}",
        repo.len()
    );
    assert_eq!(repo[0].frontmatter.name, "repo-specific");
}
