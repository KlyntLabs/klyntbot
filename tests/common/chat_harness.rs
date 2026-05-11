//! ChatTestHarness — minimal AppCore + MockProvider + recorded emitter.
//!
//! Use this in integration tests and benchmarks where you need an end-to-end
//! `chat_send` → first `ContentChunk` path without spinning up a full Tauri
//! window or a real LLM provider.

use crate::common::{test_pool, test_provider};
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

pub struct ChatTestHarness {
    pub core: Arc<app_core::AppCore>,
    pub emitter: Arc<RecordingEmitter>,
}

impl ChatTestHarness {
    pub async fn new_real() -> (Arc<app_core::AppCore>, Arc<RecordingEmitter>) {
        let emitter = Arc::new(RecordingEmitter::default());
        let provider: providers::DynProvider = Arc::new(test_provider("hello world"));
        let core = app_core::AppCore::for_tests(provider, Arc::clone(&emitter) as _)
            .await
            .expect("test AppCore should build");
        (Arc::new(core), emitter)
    }

    pub fn event_names(&self) -> Vec<String> {
        self.emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.name.clone())
            .collect()
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
