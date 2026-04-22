use ai_core::{AiEntity, AiEventMeta, AiFeature, RecallDomain};
use feature_finance::events::FinanceEvent;
use feature_finance::types::FinanceTransaction;
use feature_finance::FinanceFeature;

#[test]
fn transaction_embed_text_concatenates_key_fields() {
    let tx = FinanceTransaction {
        counterparty: Some("Whole Foods".into()),
        category: Some("groceries".into()),
        subcategory: Some("produce".into()),
        ..FinanceTransaction::default_for_test()
    };
    assert_eq!(tx.embed_text(), "Whole Foods\ngroceries\nproduce");
    assert_eq!(FinanceTransaction::entity_type(), "finance_transaction");
}

#[test]
fn transaction_recorded_signal() {
    let e = FinanceEvent::TransactionRecorded {
        _tx_id: "t1".into(),
        category: "groceries".into(),
        amount: 4500,
        currency: "USD".into(),
        _is_over_budget: false,
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
