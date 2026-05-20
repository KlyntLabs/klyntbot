use ai_core::AiFeatureRegistry;

#[test]
fn tasks_register_into_registry() {
    let mut reg = AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);

    assert_eq!(reg.len(), 1);

    let tasks = reg.by_domain(&ai_core::RecallDomain::Tasks).expect("tasks");
    assert_eq!(tasks.skill, "task-management");
    assert_eq!(tasks.tool_name, Some("tasks"));
    assert_eq!(tasks.entity_kind, Some("task"));
}
