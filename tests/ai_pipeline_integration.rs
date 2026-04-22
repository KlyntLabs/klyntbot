use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct Capture { signals: Arc<Mutex<Vec<AiSignal>>> }

#[async_trait]
impl SignalConsumer for Capture {
    fn name(&self) -> &'static str { "capture" }
    async fn consume(&self, s: &AiSignal) -> common::Result<()> {
        self.signals.lock().unwrap().push(s.clone());
        Ok(())
    }
}

#[tokio::test]
async fn every_feature_event_produces_a_typed_signal() {
    let bus = Arc::new(DomainEventBus::new(64));
    let buf = Arc::new(Mutex::new(Vec::new()));
    let consumer = Arc::new(Capture { signals: buf.clone() }) as Arc<dyn SignalConsumer>;
    let _router = SignalRouter::start(bus.clone(), vec![consumer],
        app_core::init::ai_pipeline::translate);

    let events: Vec<DomainEvent> = vec![
        feature_tasks::events::TaskEvent::Created {
            task_id: "t".into(), title: "x".into(),
            area_id: "a".into(), project_id: None,
            priority: Some(1), estimated_minutes: None,
        }.into(),
        feature_tasks::events::TaskEvent::Completed {
            task_id: "t".into(), title: "x".into(),
            deviation_pct: Some(80.0),
        }.into(),
        feature_finance::events::FinanceEvent::TransactionRecorded {
            _tx_id: "tx".into(), category: "groceries".into(),
            amount: 100, currency: "USD".into(), _is_over_budget: false,
        }.into(),
        feature_finance::events::FinanceEvent::BudgetAlert {
            category: "dining".into(), spent: 100, limit: 75,
        }.into(),
    ];

    for e in &events { bus.publish(e.clone()); }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let seen = buf.lock().unwrap().clone();
    assert_eq!(seen.len(), events.len());
    assert!(seen.iter().any(|s| s.domain == RecallDomain::Tasks
                              && matches!(s.salience, SalienceVerdict::Extract)));
    assert!(seen.iter().any(|s| s.domain == RecallDomain::Finance && s.importance >= 0.8));
    for s in &seen {
        assert!(!s.content.is_empty(), "every signal must have content");
        assert!((0.0..=1.0).contains(&s.importance), "importance in range");
    }
}
