use ai_core::{AiEntity, AiEventMeta, AiFeature, RecallDomain, SalienceVerdict};
use feature_tasks::events::TaskEvent;
use feature_tasks::types::Task;
use feature_tasks::TasksFeature;

#[test]
fn task_event_created_signal() {
    let e = TaskEvent::Created {
        task_id: "t1".into(),
        title: "Ship v1".into(),
        area_id: "a1".into(),
        priority: Some(2),
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "Created");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert_eq!(sig.content, "Created task: Ship v1 (priority Some(2))");
    assert_eq!(sig.entity.as_ref().unwrap().entity_type, "task");
    assert_eq!(sig.entity.as_ref().unwrap().id, "t1");
}

#[test]
fn task_event_completed_high_deviation_extracts() {
    let e = TaskEvent::Completed {
        task_id: "t1".into(),
        title: "Ship v1".into(),
        deviation_pct: Some(80.0),
    };
    assert!(matches!(e.to_signal().salience, SalienceVerdict::Extract));
}

#[test]
fn tasks_feature_declaration() {
    assert_eq!(<TasksFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(<TasksFeature as AiFeature>::SKILL, "task-management");
}

#[test]
fn task_embed_text_uses_title_and_description() {
    let t = Task {
        id: "x".into(),
        title: "Ship v1".into(),
        description: Some("Finish the thing".into()),
        ..Task::default_for_test()
    };
    assert_eq!(t.embed_text(), "Ship v1\nFinish the thing");
    assert_eq!(Task::entity_type(), "task");
}
