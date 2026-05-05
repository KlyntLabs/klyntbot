use bus::DomainEventBus;
use std::path::Path;
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
    lsp: Option<&lsp_client::LspClientHandle>,
) {
    let (anchored_symbols, lsp_diagnostics_delta) = if let Some(client) = lsp {
        let path = Path::new(e.path);
        let (symbols, diags) =
            tokio::join!(client.document_symbols(path), client.diagnostics_for(path),);
        let sym_values: Vec<serde_json::Value> = symbols
            .unwrap_or_default()
            .into_iter()
            .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
            .collect();
        let diag_values: Vec<serde_json::Value> = diags
            .unwrap_or_default()
            .into_iter()
            .map(|d| serde_json::to_value(d).unwrap_or(serde_json::Value::Null))
            .collect();
        (sym_values, diag_values)
    } else {
        (vec![], vec![])
    };

    let evt = ToolEvent::FileEditWithSymbols {
        path: e.path.to_string(),
        op: e.op.to_string(),
        bytes: e.bytes,
        diff_full: e.diff_full,
        anchored_symbols,
        lsp_diagnostics_delta,
    };
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    let payload =
        serde_json::to_value(&evt).unwrap_or_else(|_| serde_json::json!({"type": "unknown"}));
    bus.publish(bus::DomainEvent::Generic {
        kind: "agent_event".into(),
        payload,
    });
}

/// Compute a unified diff of `before` → `after`. Empty `before` = pure write.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let patch = diffy::create_patch(before, after);
    format!("--- {path}\n+++ {path}\n{patch}")
}
