use ai_core::AiFeatureRegistry;

#[test]
fn tasks_and_finance_register_into_registry() {
    let mut reg = AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);

    assert_eq!(reg.len(), 2);

    let tasks = reg.by_domain(&ai_core::RecallDomain::Tasks).expect("tasks");
    assert_eq!(tasks.skill, "task-management");
    assert_eq!(tasks.tool_name, Some("tasks"));
    assert_eq!(tasks.entity_kind, Some("task"));

    let finance = reg
        .by_domain(&ai_core::RecallDomain::Finance)
        .expect("finance");
    assert_eq!(finance.skill, "finance-management");
    assert_eq!(finance.tool_name, Some("finance"));
    assert_eq!(finance.entity_kind, Some("finance_transaction"));
}
