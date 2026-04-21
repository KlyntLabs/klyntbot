use ai_core::{AiEventMeta, AiSignal, SalienceVerdict};
use ai_core_macros::AiEvent;

#[derive(AiEvent)]
pub enum TaskEvent {
    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Created task: {title}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    )]
    Created { task_id: String, title: String },

    #[ai(
        importance = 0.5,
        salience = "extract_if(deviation_pct.is_some_and(|v| v > 50.0))",
        observation_template = "Completed {title} (dev {deviation_pct:?}%)",
    )]
    Completed { task_id: String, title: String, deviation_pct: Option<f64> },
}

fn main() {
    let e = TaskEvent::Created { task_id: "abc".into(), title: "Ship".into() };
    let sig: AiSignal = e.to_signal();
    assert_eq!(sig.event_kind, "Created");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert_eq!(sig.content, "Created task: Ship");
    let entity = sig.entity.as_ref().unwrap();
    assert_eq!(entity.entity_type, "task");
    assert_eq!(entity.id, "abc");
    assert_eq!(entity.name, "Ship");

    let e = TaskEvent::Completed { task_id: "x".into(), title: "y".into(), deviation_pct: Some(80.0) };
    let sig = e.to_signal();
    assert!(matches!(sig.salience, SalienceVerdict::Extract));

    let e = TaskEvent::Completed { task_id: "x".into(), title: "y".into(), deviation_pct: Some(10.0) };
    let sig = e.to_signal();
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
}
