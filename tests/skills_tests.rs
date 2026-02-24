//! Integration tests for skills system

use klyntbot::agent::SkillManager;
use tempfile::TempDir;

/// Test built-in skills loading
#[tokio::test]
async fn test_builtin_skills_loading() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut manager = SkillManager::new();

    // Load skills (includes built-in skills)
    manager.load(workspace).await.unwrap();

    // Should load at least the built-in skills
    let skills = manager.all();
    assert!(!skills.is_empty());

    // Verify some built-in skills exist
    assert!(manager.get("cron").is_some());
    assert!(manager.get("todo").is_some());
    assert!(manager.get("summarize").is_some());
}

/// Test workspace skills override
#[tokio::test]
async fn test_workspace_skills_override() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let custom_skill_dir = skills_dir.join("custom_test");
    std::fs::create_dir_all(&custom_skill_dir).unwrap();

    // Create a custom skill in workspace
    let skill_file = custom_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: A custom test skill
version: "1.0"
metadata: '{"klyntbot":{"always":false,"triggers":["test"],"requires":{"bins":["bash"],"env":[]}}}'
---

# Custom Test Skill

This is a custom skill for testing.

## Usage

Run with: `custom_test`
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();

    // Load workspace skills
    manager.load(workspace).await.unwrap();

    // Should find the custom skill
    let custom = manager.get("custom_test");
    assert!(custom.is_some());
    assert_eq!(custom.unwrap().description, "A custom test skill");
}

/// Test skill metadata parsing
#[tokio::test]
async fn test_skill_metadata_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let test_skill_dir = skills_dir.join("metadata_test");
    std::fs::create_dir_all(&test_skill_dir).unwrap();

    let skill_file = test_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Test metadata parsing
version: "2.0"
metadata: '{"klyntbot":{"always":true,"triggers":["meta","test"],"requires":{"bins":["bash","curl"],"env":["TEST_VAR"]}}}'
---

# Metadata Test Skill

Testing metadata extraction.
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let skill = manager.get("metadata_test").unwrap();

    assert_eq!(skill.description, "Test metadata parsing");
    assert_eq!(skill.version, "2.0");
    assert!(skill.always);
    assert_eq!(skill.triggers, vec!["meta", "test"]);
    assert!(skill.requires_bins.contains(&"bash".to_string()));
    assert!(skill.requires_bins.contains(&"curl".to_string()));
    assert!(skill.requires_env.contains(&"TEST_VAR".to_string()));
}

/// Test skill with minimal metadata
#[tokio::test]
async fn test_skill_minimal_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let minimal_skill_dir = skills_dir.join("minimal");
    std::fs::create_dir_all(&minimal_skill_dir).unwrap();

    let skill_file = minimal_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Minimal skill
---

# Minimal Skill

Just a minimal skill.
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let skill = manager.get("minimal").unwrap();

    assert_eq!(skill.description, "Minimal skill");
    assert_eq!(skill.version, "1.0"); // Default version
    assert!(!skill.always);
    assert!(skill.triggers.is_empty());
    assert!(skill.requires_bins.is_empty());
    assert!(skill.requires_env.is_empty());
}

/// Test requirement checking
#[tokio::test]
async fn test_skill_requirement_checking() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let req_skill_dir = skills_dir.join("requirements_test");
    std::fs::create_dir_all(&req_skill_dir).unwrap();

    let skill_file = req_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Testing requirement checking
metadata: '{"klyntbot":{"requires":{"bins":["bash","this_binary_definitely_does_not_exist_xyz123"]}}}'
---

# Requirements Test

This skill has requirements that can't be met.
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let skill = manager.get("requirements_test").unwrap();

    // Should have requirements listed
    assert!(skill.requires_bins.contains(&"bash".to_string()));
    assert!(skill
        .requires_bins
        .contains(&"this_binary_definitely_does_not_exist_xyz123".to_string()));

    // Available should be false because of missing binary
    assert!(!skill.available);
}

/// Test always-loaded skills
#[tokio::test]
async fn test_skill_always_loaded() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");

    // Create always-loaded skill
    let always_dir = skills_dir.join("always_loaded");
    std::fs::create_dir_all(&always_dir).unwrap();
    let always_file = always_dir.join("SKILL.md");
    std::fs::write(
        &always_file,
        r#"---
description: Always loaded skill
metadata: '{"klyntbot":{"always":true}}'
---

# Always Loaded
"#,
    )
    .unwrap();

    // Create regular skill
    let regular_dir = skills_dir.join("regular");
    std::fs::create_dir_all(&regular_dir).unwrap();
    let regular_file = regular_dir.join("SKILL.md");
    std::fs::write(
        &regular_file,
        r#"---
description: Regular skill
---

# Regular
"#,
    )
    .unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    // Get always-loaded skills
    let always_skills = manager.get_always_loaded();

    // Should include the always-loaded skill
    assert!(always_skills.iter().any(|s| s.name == "always_loaded"));

    // Should not include the regular skill
    assert!(!always_skills.iter().any(|s| s.name == "regular"));
}

/// Test skill XML summary generation
#[tokio::test]
async fn test_skill_summary_generation() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let test_skill_dir = skills_dir.join("summary_test");
    std::fs::create_dir_all(&test_skill_dir).unwrap();

    let skill_file = test_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Test summary generation
metadata: '{"klyntbot":{"triggers":["summary"]}}'
---

# Summary Test
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    // Generate XML summary
    let summary = manager.generate_summary();

    // Should contain XML structure
    assert!(summary.contains("<skills>"));
    assert!(summary.contains("</skills>"));
    assert!(summary.contains("<skill"));
    assert!(summary.contains("summary_test"));
    assert!(summary.contains("Test summary generation"));
}

/// Test skill content loading
#[tokio::test]
async fn test_skill_content_loading() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let content_skill_dir = skills_dir.join("content_test");
    std::fs::create_dir_all(&content_skill_dir).unwrap();

    let skill_file = content_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Test content loading
---

# Content Test Skill

This is the skill content that should be loaded.

## Details

More detailed information here.
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let skill = manager.get("content_test").unwrap();

    // Content should be loaded
    assert!(skill.content.is_some());
    let content = skill.content.as_ref().unwrap();
    assert!(content.contains("Content Test Skill"));
    assert!(content.contains("More detailed information here"));
}

/// Test skill discovery in nested directories
#[tokio::test]
async fn test_skill_discovery_nested() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");

    // Create skill in nested directory (should not be discovered)
    let nested_dir = skills_dir.join("category").join("nested");
    std::fs::create_dir_all(&nested_dir).unwrap();
    let nested_file = nested_dir.join("SKILL.md");
    std::fs::write(&nested_file, "---\ndescription: Nested\n---\n# Nested").unwrap();

    // Create skill in top-level directory (should be discovered)
    let top_dir = skills_dir.join("toplevel");
    std::fs::create_dir_all(&top_dir).unwrap();
    let top_file = top_dir.join("SKILL.md");
    std::fs::write(&top_file, "---\ndescription: Top\n---\n# Top").unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    // Should find top-level skill
    assert!(manager.get("toplevel").is_some());

    // Nested skill won't be discovered (only scans immediate subdirectories)
    assert!(manager.get("nested").is_none());
}

/// Test skill loading with missing SKILL.md
#[tokio::test]
async fn test_skill_loading_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");

    // Create directory without SKILL.md
    let empty_dir = skills_dir.join("empty_skill");
    std::fs::create_dir_all(&empty_dir).unwrap();

    // Create directory with SKILL.md
    let valid_dir = skills_dir.join("valid_skill");
    std::fs::create_dir_all(&valid_dir).unwrap();
    let valid_file = valid_dir.join("SKILL.md");
    std::fs::write(&valid_file, "---\ndescription: Valid\n---\n# Valid").unwrap();

    let mut manager = SkillManager::new();

    // Should load successfully despite empty directory
    let result = manager.load(workspace).await;
    assert!(result.is_ok());

    // Should find the valid skill
    assert!(manager.get("valid_skill").is_some());

    // Should not find the empty skill
    assert!(manager.get("empty_skill").is_none());
}

/// Test skill triggers
#[tokio::test]
async fn test_skill_triggers() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    let trigger_skill_dir = skills_dir.join("trigger_test");
    std::fs::create_dir_all(&trigger_skill_dir).unwrap();

    let skill_file = trigger_skill_dir.join("SKILL.md");
    let skill_content = r#"---
description: Trigger test skill
metadata: '{"klyntbot":{"triggers":["weather","forecast","temperature"]}}'
---

# Trigger Test
"#;
    std::fs::write(&skill_file, skill_content).unwrap();

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let skill = manager.get("trigger_test").unwrap();

    assert_eq!(skill.triggers.len(), 3);
    assert!(skill.triggers.contains(&"weather".to_string()));
    assert!(skill.triggers.contains(&"forecast".to_string()));
    assert!(skill.triggers.contains(&"temperature".to_string()));
}

/// Test get all skills
#[tokio::test]
async fn test_get_all_skills() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let skills_dir = workspace.join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create multiple skills
    for i in 1..=3 {
        let skill_dir = skills_dir.join(format!("skill_{}", i));
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            format!("---\ndescription: Skill {}\n---\n# Skill {}", i, i),
        )
        .unwrap();
    }

    let mut manager = SkillManager::new();
    manager.load(workspace).await.unwrap();

    let all_skills = manager.all();

    // Should include the 3 workspace skills plus built-in skills
    assert!(all_skills.len() >= 3);

    // Should find our custom skills
    assert!(all_skills.iter().any(|s| s.name == "skill_1"));
    assert!(all_skills.iter().any(|s| s.name == "skill_2"));
    assert!(all_skills.iter().any(|s| s.name == "skill_3"));
}
