//! End-to-end test: Finance transaction flows through the AI pipeline.

use ai_core::{AiSignal, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct TestConsumer {
    observations: Arc<Mutex<Vec<TestObservation>>>,
}

#[derive(Debug, Clone)]
struct TestObservation {
    event_kind: String,
    domain: String,
    content: String,
    importance: f64,
}

#[async_trait]
impl SignalConsumer for TestConsumer {
    fn name(&self) -> &'static str {
        "test_consumer"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        self.observations.lock().unwrap().push(TestObservation {
            event_kind: signal.event_kind.to_string(),
            domain: signal.domain.as_str().to_string(),
            content: signal.content.clone(),
            importance: signal.importance,
        });
        Ok(())
    }
}

#[tokio::test]
async fn finance_transaction_recorded_produces_signal() {
    let bus = Arc::new(DomainEventBus::new(64));
    let observations = Arc::new(Mutex::new(Vec::new()));

    let consumer = Arc::new(TestConsumer {
        observations: observations.clone(),
    }) as Arc<dyn SignalConsumer>;

    let _router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    // Publish a TransactionRecorded domain event
    let event = DomainEvent::TransactionRecorded {
        category: "groceries".into(),
        amount: 45.50,
        is_over_budget: false,
    };

    bus.publish(event);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let obs = observations.lock().unwrap().clone();
    assert_eq!(obs.len(), 1, "expected exactly one observation");
    assert_eq!(obs[0].event_kind, "TransactionRecorded");
    assert_eq!(obs[0].domain, "finance");
    assert!(
        obs[0].content.contains("groceries"),
        "content should contain category"
    );
    assert!(
        obs[0].importance >= 0.4 && obs[0].importance <= 0.6,
        "importance should be ~0.5"
    );
}

#[tokio::test]
async fn finance_budget_alert_high_importance() {
    let bus = Arc::new(DomainEventBus::new(64));
    let observations = Arc::new(Mutex::new(Vec::new()));

    let consumer = Arc::new(TestConsumer {
        observations: observations.clone(),
    }) as Arc<dyn SignalConsumer>;

    let _router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    // Publish a BudgetAlert domain event
    let event = DomainEvent::BudgetAlert {
        category: "dining".into(),
        spent: 850.0,
        limit: 750.0,
    };

    bus.publish(event);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let obs = observations.lock().unwrap().clone();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].event_kind, "BudgetAlert");
    assert!(
        obs[0].importance >= 0.8,
        "budget alert should have high importance"
    );
    assert!(
        obs[0].content.contains("dining"),
        "content should mention category"
    );
}
