use ai_core::registry::{AiFeatureRegistry, FeatureRecord};
use ai_core::RecallDomain;

#[test]
fn registry_register_and_lookup_by_domain() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let rec = reg.by_domain(&RecallDomain::Tasks).expect("tasks registered");
    assert_eq!(rec.skill, "task-management");
    assert_eq!(rec.tool_name, Some("tasks"));
    assert_eq!(rec.entity_kind, Some("task"));
}

#[test]
fn registry_iteration_is_stable_in_insertion_order() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });
    reg.register(FeatureRecord {
        domain: RecallDomain::Finance,
        skill: "finance-management",
        tool_name: Some("finance"),
        entity_kind: Some("finance_transaction"),
    });

    let names: Vec<&'static str> =
        reg.iter().filter_map(|r| r.tool_name).collect();
    assert_eq!(names, vec!["tasks", "finance"]);
}

#[test]
fn registry_tool_names_returns_only_some() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Coaching,
        skill: "coaching",
        tool_name: None,                    // coaching has no tool exposed
        entity_kind: None,
    });
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let tools = reg.tool_names();
    assert_eq!(tools, vec!["tasks"]);
}

#[test]
fn registry_lookup_by_entity_kind() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });

    let rec = reg.by_entity_kind("task").expect("found");
    assert_eq!(rec.domain, RecallDomain::Tasks);

    assert!(reg.by_entity_kind("nonexistent").is_none());
}

#[test]
fn registry_register_panics_on_duplicate_domain() {
    let mut reg = AiFeatureRegistry::new();
    reg.register(FeatureRecord {
        domain: RecallDomain::Tasks,
        skill: "task-management",
        tool_name: Some("tasks"),
        entity_kind: Some("task"),
    });
    let result = std::panic::catch_unwind(move || {
        reg.register(FeatureRecord {
            domain: RecallDomain::Tasks,
            skill: "another",
            tool_name: Some("tasks2"),
            entity_kind: None,
        });
    });
    assert!(result.is_err(), "registering same domain twice should panic");
}
