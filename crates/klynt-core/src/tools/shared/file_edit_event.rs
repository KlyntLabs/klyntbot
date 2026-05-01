use bus::DomainEventBus;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::events::ToolEvent;

#[derive(Debug, Clone)]
pub struct FileEditEvent<'a> {
    pub op: &'a str, // "edit" | "write" | "apply_patch"
    pub path: &'a str,
    pub bytes: u64,
    pub diff_full: String,
}

pub async fn emit_file_edit(
    event_tx: &Option<mpsc::Sender<ToolEvent>>,
    bus: &Arc<DomainEventBus>,
    e: FileEditEvent<'_>,
) {
    let evt = ToolEvent::FileEditWithSymbols {
        path: e.path.to_string(),
        op: e.op.to_string(),
        bytes: e.bytes,
        diff_full: e.diff_full,
        anchored_symbols: vec![],      // Phase 2: tree-sitter
        lsp_diagnostics_delta: vec![], // Phase 2: LSP
    };
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    if let Some(bus) = Some(bus) {
        let payload =
            serde_json::to_value(&evt).unwrap_or_else(|_| serde_json::json!({"type": "unknown"}));
        bus.publish(bus::DomainEvent::Generic {
            kind: "agent_event".into(),
            payload,
        });
    }
}

/// Compute a unified diff of `before` → `after`. Empty `before` = pure write.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let patch = diffy::create_patch(before, after);
    format!("--- {path}\n+++ {path}\n{patch}")
}
