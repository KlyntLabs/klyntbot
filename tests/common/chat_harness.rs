//! ChatTestHarness — minimal AppCore + MockProvider + recorded emitter.
//!
//! Use this in integration tests and benchmarks where you need an end-to-end
//! `chat_send` → first `ContentChunk` path without spinning up a full Tauri
//! window or a real LLM provider.

use crate::common::test_provider;
use app_core::events::AppEventEmitter;
use std::sync::{Arc, Mutex};

pub struct RecordedEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

#[derive(Default)]
pub struct RecordingEmitter {
    pub events: Arc<Mutex<Vec<RecordedEvent>>>,
}

impl AppEventEmitter for RecordingEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        self.events.lock().unwrap().push(RecordedEvent {
            name: event_name.to_string(),
            payload,
        });
    }
}

/// Test utility for constructing an AppCore with a mock provider and
/// recording emitter. All methods are associated functions — the struct
/// carries no state.
pub struct ChatTestHarness;

impl ChatTestHarness {
    pub async fn new_real() -> (Arc<app_core::AppCore>, Arc<RecordingEmitter>) {
        let emitter = Arc::new(RecordingEmitter::default());
        let provider: providers::DynProvider = Arc::new(test_provider("hello world"));
        let emitter_clone = Arc::clone(&emitter);
        let emitter_dyn: std::sync::Arc<dyn app_core::events::AppEventEmitter> = emitter;
        let core = app_core::AppCore::for_tests(provider, emitter_dyn)
            .await
            .expect("test AppCore should build");
        (Arc::new(core), emitter_clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn harness_builds() {
        let (core, emitter) = ChatTestHarness::new_real().await;
        assert_eq!(emitter.events.lock().unwrap().len(), 0);
        drop(core);
    }
}
