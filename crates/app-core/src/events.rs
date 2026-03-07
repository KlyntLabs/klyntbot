/// Transport-agnostic event emitter.
pub trait AppEventEmitter: Send + Sync + 'static {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value);
}

/// No-op emitter for tests.
pub struct NoopEmitter;

impl AppEventEmitter for NoopEmitter {
    fn emit_event(&self, _event_name: &str, _payload: serde_json::Value) {}
}
