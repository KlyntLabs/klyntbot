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
