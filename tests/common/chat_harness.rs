//! ChatTestHarness — minimal AppCore + MockProvider + recorded emitter.
//!
//! Use this in integration tests and benchmarks where you need an end-to-end
//! `chat_send` → first `ContentChunk` path without spinning up a full Tauri
//! window or a real LLM provider.

use crate::common::{test_provider, test_repos};
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
    // Populated incrementally — see Task 8 onward for real construction.
    pub emitter: Arc<RecordingEmitter>,
}

impl ChatTestHarness {
    pub async fn new() -> Self {
        let emitter = Arc::new(RecordingEmitter::default());
        let _ = test_repos().await; // ensure schema migrates
        let _ = test_provider("hello");
        Self { emitter }
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
