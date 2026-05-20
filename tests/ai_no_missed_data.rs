use ai_core::AiEventMeta;

#[test]
fn task_event_every_variant_produces_nonempty_signal() {
    use feature_tasks::events::TaskEvent;
    let samples: Vec<TaskEvent> = vec![
        TaskEvent::Created {
            task_id: "_".into(),
            title: "_".into(),
            area_id: "_".into(),
            project_id: None,
            priority: None,
            estimated_minutes: None,
        },
        TaskEvent::Completed {
            task_id: "_".into(),
            title: "_".into(),
            deviation_pct: None,
        },
        TaskEvent::FocusChanged {
            task_id: "_".into(),
            title: "_".into(),
            focus_deadline: None,
        },
        TaskEvent::EstimationRecorded {
            task_id: "_".into(),
            estimated_minutes: None,
            actual_minutes: None,
            deviation_pct: 0.0,
        },
    ];
    for e in samples {
        let sig = e.to_signal();
        assert!(!sig.event_kind.is_empty());
        assert!(
            (0.0..=1.0).contains(&sig.importance),
            "importance for {} out of range: {}",
            sig.event_kind,
            sig.importance
        );
    }
}
