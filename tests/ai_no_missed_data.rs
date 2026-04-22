use ai_core::AiEventMeta;

#[test]
fn task_event_every_variant_produces_nonempty_signal() {
    use feature_tasks::events::TaskEvent;
    let samples: Vec<TaskEvent> = vec![
        TaskEvent::Created { task_id: "_".into(), title: "_".into(),
                             area_id: "_".into(), project_id: None,
                             priority: None, estimated_minutes: None },
        TaskEvent::Completed { task_id: "_".into(), title: "_".into(),
                               deviation_pct: None },
        TaskEvent::FocusChanged { task_id: "_".into(), title: "_".into(),
                                  focus_deadline: None },
        TaskEvent::EstimationRecorded { task_id: "_".into(),
                                        estimated_minutes: None,
                                        actual_minutes: None,
                                        deviation_pct: 0.0 },
    ];
    for e in samples {
        let sig = e.to_signal();
        assert!(!sig.event_kind.is_empty());
        assert!((0.0..=1.0).contains(&sig.importance),
                "importance for {} out of range: {}", sig.event_kind, sig.importance);
    }
}

#[test]
fn finance_event_every_variant_produces_nonempty_signal() {
    use feature_finance::events::FinanceEvent;
    let samples: Vec<FinanceEvent> = vec![
        FinanceEvent::TransactionRecorded { tx_id: "_".into(), category: "_".into(),
                                            amount: 0, currency: "_".into(),
                                            is_over_budget: false },
        FinanceEvent::BudgetAlert { category: "_".into(), spent: 0, limit: 0 },
        FinanceEvent::AccountCreated { account_id: "_".into(), name: "_".into(),
                                       currency: "_".into() },
        FinanceEvent::BudgetCreated { budget_id: "_".into(), name: "_".into(),
                                      amount: 0, currency: "_".into() },
        FinanceEvent::GoalCreated { goal_id: "_".into(), name: "_".into(),
                                    target_amount: 0 },
        FinanceEvent::GoalAchieved { goal_id: "_".into(), name: "_".into() },
    ];
    for e in samples {
        let sig = e.to_signal();
        assert!(!sig.event_kind.is_empty());
        assert!((0.0..=1.0).contains(&sig.importance));
    }
}
