use ai_core::{AiEventMeta, AiFeature, RecallDomain};
use feature_finance::events::FinanceEvent;
use feature_finance::FinanceFeature;

#[test]
fn transaction_recorded_signal() {
    let e = FinanceEvent::TransactionRecorded {
        tx_id: "t1".into(),
        category: "groceries".into(),
        amount: 4500,
        currency: "USD".into(),
        is_over_budget: false,
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "TransactionRecorded");
    assert!(sig.content.contains("groceries"));
}

#[test]
fn budget_alert_high_importance() {
    let e = FinanceEvent::BudgetAlert {
        category: "dining".into(),
        spent: 80000,
        limit: 75000,
    };
    let sig = e.to_signal();
    assert!(sig.importance >= 0.8);
}

#[test]
fn finance_feature_declaration() {
    assert_eq!(<FinanceFeature as AiFeature>::DOMAIN, RecallDomain::Finance);
    assert_eq!(<FinanceFeature as AiFeature>::SKILL, "finance-management");
}
