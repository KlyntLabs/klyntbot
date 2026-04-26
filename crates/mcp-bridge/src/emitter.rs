use crate::client::BridgeClient;
use crate::protocol::BridgeFrame;
use app_core::events::AppEventEmitter;

/// `AppEventEmitter` that ships every event over the bridge socket to a
/// running desktop process. When no desktop is running, frames are dropped
/// silently by `BridgeClient`.
pub struct SocketBridgeEmitter {
    client: BridgeClient,
}

impl SocketBridgeEmitter {
    pub fn new(client: BridgeClient) -> Self {
        Self { client }
    }
}

impl AppEventEmitter for SocketBridgeEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        self.client.send(BridgeFrame {
            event: event_name.to_string(),
            payload,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::events::AppEventEmitter;
    use desktop_shared::types::EntityKind;
    use std::path::PathBuf;

    #[tokio::test]
    async fn emit_to_missing_socket_is_silent() {
        let path = PathBuf::from("/tmp/klynt-bridge-emit-test-39483.sock");
        let _ = std::fs::remove_file(&path);
        let client = BridgeClient::new(path);
        let emitter = SocketBridgeEmitter::new(client);

        // Drives the default `emit_entity_updated` → `emit_event` impl.
        emitter.emit_entity_updated(EntityKind::Task, "t1");
        emitter.emit_event("provider:degraded", serde_json::json!({"x": 1}));

        // No panic, no block — give the writer task a moment to fail.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
