use ai_core::AiEventMeta;
use feature_tasks::events::TaskEvent;

#[test]
fn task_completed_is_coaching_signal() {
    let sig = TaskEvent::Completed {
        task_id: "t1".into(),
        title: "x".into(),
        deviation_pct: Some(20.0),
    }
    .to_signal();
    assert!(sig.coaching_signal);
}
