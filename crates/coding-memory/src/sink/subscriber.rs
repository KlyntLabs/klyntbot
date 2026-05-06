//! `MemorySinkSubscriber` — bridges a runtime event stream to a `MemorySink`.

use crate::sink::translator::{RuntimeEvent, Translator};
use crate::sink::MemorySink;
use bus::DomainEventBus;
use common::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Subscriber that translates runtime events and forwards them to a `MemorySink`.
pub struct MemorySinkSubscriber {
    sink: Arc<dyn MemorySink>,
    translator: Arc<Mutex<Translator>>,
}

impl MemorySinkSubscriber {
    /// Wrap a sink.
    pub fn new(sink: Arc<dyn MemorySink>) -> Self {
        Self {
            sink,
            translator: Arc::new(Mutex::new(Translator::new())),
        }
    }

    /// Test helper — construct from a `RecordingSink`.
    pub fn for_test(sink: RecordingSink) -> Self {
        Self::new(Arc::new(sink))
    }

    /// Translate one runtime event and forward all emitted ingest events.
    pub async fn translate(&self, evt: RuntimeEvent) -> Result<()> {
        let mut t = self.translator.lock().await;
        let emitted = t.translate(&evt)?;
        drop(t);
        for e in emitted {
            self.sink.accept_event(e).await?;
        }
        Ok(())
    }

    /// Spawn a background task that drains `DomainEvent::Generic` values from
    /// the bus, deserialises them to [`RuntimeEvent`], and forwards to the sink.
    pub async fn spawn(self: Arc<Self>, bus: Arc<DomainEventBus>) -> Result<()> {
        let mut rx = bus.subscribe();
        tokio::spawn(async move {
            while let Ok(domain_evt) = rx.recv().await {
                if let bus::DomainEvent::Generic { kind, payload } = &domain_evt {
                    if let Some(runtime_evt) = generic_to_runtime(kind, payload) {
                        if let Err(e) = self.translate(runtime_evt).await {
                            tracing::warn!(error = %e, "subscriber translate/forward failed");
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

/// Best-effort conversion from `DomainEvent::Generic` to `RuntimeEvent`.
fn generic_to_runtime(_kind: &str, payload: &serde_json::Value) -> Option<RuntimeEvent> {
    // Future: match on well-known kind strings and deserialize payload.
    // For now, attempt a direct deserialization if payload is an object.
    serde_json::from_value(payload.clone()).ok()
}

// ---------------------------------------------------------------------------
// RecordingSink — test helper
// ---------------------------------------------------------------------------

/// Test helper that records every accepted event in memory.
#[derive(Clone, Default)]
pub struct RecordingSink {
    inner: Arc<Mutex<Vec<coding_ingest::event::AgentEvent>>>,
}

impl RecordingSink {
    /// Create an empty recording sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clone the recorded events.
    pub fn events_for_test(&self) -> Vec<coding_ingest::event::AgentEvent> {
        self.inner.try_lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl MemorySink for RecordingSink {
    async fn accept_event(&self, evt: coding_ingest::event::AgentEvent) -> Result<()> {
        self.inner.lock().await.push(evt);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
