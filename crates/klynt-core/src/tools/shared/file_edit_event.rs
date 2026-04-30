use agent::events::AgentEvent;
use bus::DomainEventBus;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct FileEditEvent<'a> {
    pub op: &'a str,             // "edit" | "write" | "apply_patch"
    pub path: &'a str,
    pub bytes: u64,
    pub diff_full: String,
}

pub async fn emit_file_edit(
    event_tx: &Option<mpsc::Sender<AgentEvent>>,
    bus: &Arc<DomainEventBus>,
    e: FileEditEvent<'_>,
) {
    let evt = AgentEvent::FileEditWithSymbols {
        path: e.path.to_string(),
        op: e.op.to_string(),
        bytes: e.bytes,
        diff_full: e.diff_full,
        anchored_symbols: vec![],          // Phase 2: tree-sitter
        lsp_diagnostics_delta: vec![],     // Phase 2: LSP
    };
    agent::execution::core::fan_out_event(event_tx.as_ref(), Some(bus), evt).await;
}

/// Compute a unified diff of `before` → `after`. Empty `before` = pure write.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let patch = diffy::create_patch(before, after);
    format!("--- {path}\n+++ {path}\n{patch}")
}
