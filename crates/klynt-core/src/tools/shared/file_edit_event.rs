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
        anchored_symbols: vec![],
        lsp_diagnostics_delta: vec![],
    };
    fan_out_tool_event(event_tx.as_ref(), Some(bus), evt).await;
}

/// Compute a unified diff of `before` → `after`. Empty `before` = pure write.
///
/// Replaces `diffy`'s default `--- original / +++ modified` headers with
/// path-based headers so the FE diff parser (`PierreDiffBlock`) sees a
/// well-formed single-header unified diff. Also strips the standard
/// `\ No newline at end of file` markers — these are correct unified-diff
/// convention but visually noisy in the chat UI, and we never pipe these
/// diffs through `patch`, so dropping them is safe.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let raw = diffy::create_patch(before, after).to_string();
    // diffy always prefixes two header lines; skip them and prepend ours.
    let body = raw.splitn(3, '\n').nth(2).unwrap_or("");
    let cleaned: String = body
        .lines()
        .filter(|line| !line.starts_with("\\ No newline at end of file"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("--- {path}\n+++ {path}\n{cleaned}")
}

pub(crate) async fn fan_out_tool_event(
    event_tx: Option<&mpsc::Sender<ToolEvent>>,
    domain_bus: Option<&Arc<DomainEventBus>>,
    evt: ToolEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    if let Some(bus) = domain_bus {
        let payload = serde_json::to_value(&evt).unwrap_or_else(|e| {
            tracing::error!("failed to serialize ToolEvent: {e}");
            serde_json::json!({"type": "unknown"})
        });
        bus.publish(bus::DomainEvent::Generic {
            kind: "agent_event".into(),
            payload,
        });
    }
}
