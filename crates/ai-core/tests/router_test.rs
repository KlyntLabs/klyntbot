use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct Recorder { log: Arc<Mutex<Vec<String>>> }

#[async_trait]
impl SignalConsumer for Recorder {
    fn name(&self) -> &'static str { "rec" }
    async fn consume(&self, s: &AiSignal) -> common::Result<()> {
        self.log.lock().unwrap().push(format!("{}:{:?}", s.event_kind, s.domain));
        Ok(())
    }
}

#[tokio::test]
async fn router_broadcasts_signal_to_all_consumers() {
    let bus = Arc::new(DomainEventBus::new(64));
    let log = Arc::new(Mutex::new(Vec::new()));
    let consumer = Arc::new(Recorder { log: log.clone() }) as Arc<dyn SignalConsumer>;

    let router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        |_event| Some(AiSignal {
            domain: RecallDomain::Tasks,
            event_kind: "TaskCreated",
            importance: 0.7,
            salience: SalienceVerdict::Accumulate,
            content: "stub".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
        }),
    );

    // Publish a minimal event - the translator will produce a signal regardless
    bus.publish(DomainEvent::ChatTurnCompleted {
        session_key: "test".into(),
        user_message: None,
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let entries = log.lock().unwrap().clone();
    assert_eq!(entries, vec!["TaskCreated:Tasks".to_string()]);

    router.shutdown();
}
