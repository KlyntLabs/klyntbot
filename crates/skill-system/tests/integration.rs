use skill_system::discovery::{SkillSource, BUILTIN_SKILLS};
use skill_system::router::SkillRouter;
use skill_system::types::*;

#[test]
fn test_full_pipeline_builtin_skills() {
    let source = SkillSource::BuiltIn(
        BUILTIN_SKILLS
            .iter()
            .map(|(n, c)| (n.to_string(), c.to_string()))
            .collect(),
    );
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

    // All 5 orchestrators loaded
    assert_eq!(catalog.orchestrators().len(), 5);

    // Router can select orchestrators
    let router = SkillRouter::new(&catalog);
    let selected = router.select_orchestrator("create a task for my project planning", &catalog);
    assert_eq!(selected.name, "task-management");

    let selected = router.select_orchestrator("check my budget spending", &catalog);
    assert_eq!(selected.name, "finance-management");

    let selected = router.select_orchestrator("hello there!", &catalog);
    assert_eq!(selected.name, "general");

    // Catalog prompt is valid XML-ish
    let prompt = catalog.catalog_prompt();
    assert!(prompt.contains("<available_skills>"));
    assert!(prompt.contains("</available_skills>"));
    assert!(prompt.contains("task-management"));
    assert!(prompt.contains("finance-management"));
}

#[tokio::test]
async fn test_filesystem_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("my-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: my-skill\ndescription: A test skill.\n---\n\nTest body.",
    )
    .unwrap();

    let source = SkillSource::Directory(temp.path().to_path_buf(), SkillScope::User);
    let catalog = SkillCatalog::discover(&[source]).await.unwrap();
    assert!(catalog.get("my-skill").is_some());
    assert_eq!(catalog.get("my-skill").unwrap().scope, SkillScope::User);
}

#[test]
fn test_builtin_reference_files_loaded() {
    let refs = skill_system::discovery::builtin_reference_map();
    // Should have 16 references (5 general + 6 task + 2 finance + 1 automation + 2 communication)
    assert!(
        refs.len() >= 14,
        "Expected at least 14 references, got {}",
        refs.len()
    );
    assert!(refs.contains_key("builtin::task-management/references/todo.md"));
    assert!(refs.contains_key("builtin::general/references/search.md"));
    assert!(refs.contains_key("builtin::finance-management/references/budgeting.md"));
    assert!(refs.contains_key("builtin::automation/references/cron.md"));
    assert!(refs.contains_key("builtin::communication/references/messaging.md"));
}

#[test]
fn test_builtin_skills_info_for_ui() {
    let info = skill_system::discovery::builtin_skills_info();
    assert_eq!(info.len(), 5);

    let general = info.iter().find(|s| s.name == "general").unwrap();
    assert_eq!(general.references.len(), 5); // search, skill-creator, browser, memory, summarize

    let task = info.iter().find(|s| s.name == "task-management").unwrap();
    assert_eq!(task.references.len(), 7); // todo, daily-planner, task-decompose, project-management, weekly-review, retrospective, reports
}
