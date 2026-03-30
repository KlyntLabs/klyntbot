use desktop_shared::events::{EntityUpdatedPayload, ENTITY_UPDATED};
use desktop_shared::types::EntityKind;

/// Emitted when an LLM provider degrades or falls back to a secondary.
/// Payload: `{ level: "fallback" | "offline" }`.
pub const PROVIDER_DEGRADED: &str = "provider:degraded";

/// Transport-agnostic event emitter.
pub trait AppEventEmitter: Send + Sync + 'static {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value);

    /// Emit an `entity:updated` event for the given entity kind and ID.
    fn emit_entity_updated(&self, kind: EntityKind, id: &str) {
        let payload = EntityUpdatedPayload {
            entity_kind: kind,
            id: id.to_string(),
        };
        if let Ok(value) = serde_json::to_value(&payload) {
            self.emit_event(ENTITY_UPDATED, value);
        }
    }
}

/// No-op emitter for tests.
pub struct NoopEmitter;

impl AppEventEmitter for NoopEmitter {
    fn emit_event(&self, _event_name: &str, _payload: serde_json::Value) {}
}

/// Emitter that forwards events to a primary emitter and an optional broadcast channel.
///
/// Used in the desktop app to fan out events to both the Tauri webview (primary)
/// and the dev HTTP server's SSE endpoint (broadcast) so browser dev mode
/// receives events like `brain:ambient` and `provider:degraded`.
pub struct CompoundEmitter {
    primary: Box<dyn AppEventEmitter>,
    broadcast: tokio::sync::broadcast::Sender<(String, serde_json::Value)>,
}

impl CompoundEmitter {
    pub fn new(
        primary: impl AppEventEmitter + 'static,
        broadcast: tokio::sync::broadcast::Sender<(String, serde_json::Value)>,
    ) -> Self {
        Self {
            primary: Box::new(primary),
            broadcast,
        }
    }
}

impl AppEventEmitter for CompoundEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        self.primary.emit_event(event_name, payload.clone());
        let _ = self.broadcast.send((event_name.to_string(), payload));
    }
}
