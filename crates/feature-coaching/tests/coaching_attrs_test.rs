use ai_core::AiEventMeta;
use feature_finance::events::FinanceEvent;
use feature_tasks::events::TaskEvent;

#[test]
fn budget_alert_is_coaching_signal() {
    let sig = FinanceEvent::BudgetAlert {
        category: "food".into(),
        spent: 450,
        limit: 500,
    }
    .to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.metrics.category.as_deref(), Some("food"));
    assert_eq!(sig.metrics.amount, Some(450.0));
}

#[test]
fn transaction_recorded_has_amount_metric() {
    let sig = FinanceEvent::TransactionRecorded {
        _tx_id: String::new(),
        category: "groceries".into(),
        amount: 42,
        currency: "USD".into(),
        _is_over_budget: false,
    }
    .to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.metrics.amount, Some(42.0));
}

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
