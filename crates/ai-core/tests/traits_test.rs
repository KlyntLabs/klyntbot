use ai_core::{AiSignal, SalienceVerdict, SignalConsumer, RecallDomain};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::{Arc, Mutex};

struct RecordingConsumer {
    seen: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl SignalConsumer for RecordingConsumer {
    fn name(&self) -> &'static str { "recording" }
    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        self.seen.lock().unwrap().push(signal.event_kind);
        Ok(())
    }
}

#[tokio::test]
async fn consumer_receives_signal() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let consumer: Arc<dyn SignalConsumer> = Arc::new(RecordingConsumer { seen: log.clone() });
    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "TaskCreated",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "x".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };
    consumer.consume(&sig).await.unwrap();
    assert_eq!(log.lock().unwrap().as_slice(), &["TaskCreated"]);
}
