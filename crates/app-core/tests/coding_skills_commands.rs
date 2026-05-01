use app_core::AppCore;
use common::Result;
use std::fs;
use tempfile::TempDir;

async fn make_test_core_with_skills(home: &TempDir) -> Result<AppCore> {
    let core = AppCore::for_test(Some(home.path().to_path_buf()))
        .await
        .map_err(|e| common::KlyntbotError::Config(common::ConfigError::Invalid(e)))?;
    Ok(core)
}

#[tokio::test]
async fn list_returns_discovered_skills() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/alpha");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: alpha\ndescription: Alpha\n---\nBody\n",
    )
    .unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let listed = core.coding_skills_list().await.unwrap();
    let names: Vec<&str> = listed.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"alpha"));
}

#[tokio::test]
async fn info_returns_frontmatter_summary() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/alpha");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill body.\ntags: [\"test\"]\n---\n# A\nBody\n",
    )
    .unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let info = core.coding_skills_info("alpha").await.unwrap();
    assert_eq!(info.name, "alpha");
    assert_eq!(info.description, "Alpha skill body.");
    assert_eq!(info.tags, vec!["test".to_string()]);
}

#[tokio::test]
async fn info_unknown_skill_errors() {
    let home = TempDir::new().unwrap();
    let core = make_test_core_with_skills(&home).await.unwrap();
    let res = core.coding_skills_info("nope").await;
    assert!(res.is_err());
}

#[tokio::test]
async fn install_local_path_copies_skill() {
    let home = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    let src_skill = src.path().join("freshly");
    fs::create_dir_all(&src_skill).unwrap();
    fs::write(
        src_skill.join("SKILL.md"),
        "---\nname: freshly\ndescription: Freshly installed.\n---\nBody\n",
    )
    .unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    core.coding_skills_install(src_skill.to_string_lossy().into())
        .await
        .unwrap();

    let installed = home.path().join("skills/freshly/SKILL.md");
    assert!(
        installed.exists(),
        "skill copied to ~/.klyntbot/skills/freshly/"
    );

    let listed = core.coding_skills_list().await.unwrap();
    assert!(
        listed.iter().any(|s| s.name == "freshly"),
        "appears in list after install"
    );
}

#[tokio::test]
async fn toggle_disables_then_reenables() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/togg");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: togg\ndescription: Toggle me\n---\nBody\n",
    )
    .unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    core.coding_skills_toggle("togg", false).await.unwrap();
    let cfg = core.config.read().await;
    assert!(cfg.coding.skills.never_activate.contains(&"togg".into()));
    drop(cfg);

    core.coding_skills_toggle("togg", true).await.unwrap();
    let cfg = core.config.read().await;
    assert!(!cfg.coding.skills.never_activate.contains(&"togg".into()));
}

#[tokio::test]
async fn validate_returns_ok_for_well_formed() {
    let home = TempDir::new().unwrap();
    let skills = home.path().join("skills/valid");
    fs::create_dir_all(&skills).unwrap();
    fs::write(
        skills.join("SKILL.md"),
        "---\nname: valid\ndescription: Valid\n---\nBody\n",
    )
    .unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let result = core.coding_skills_validate("valid").await.unwrap();
    assert!(result.ok);
}

#[tokio::test]
async fn reload_picks_up_new_skill() {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join("skills")).unwrap();

    let core = make_test_core_with_skills(&home).await.unwrap();
    let initial_count = core.coding_skills_list().await.unwrap().len();

    let new = home.path().join("skills/freshly_added");
    fs::create_dir_all(&new).unwrap();
    fs::write(
        new.join("SKILL.md"),
        "---\nname: freshly_added\ndescription: New\n---\nBody\n",
    )
    .unwrap();

    core.coding_skills_reload().await.unwrap();
    let after_count = core.coding_skills_list().await.unwrap().len();
    assert_eq!(after_count, initial_count + 1);
}
