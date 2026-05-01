//! Aggregator — per-iteration state machine for N:1 event translations.
//!
//! * `ContentChunk` × N → `AssistantMsg` × 1
//! * `ToolStart` + `ToolEnd` → `ToolCall` × 1
//! * `ApprovalRequested` + `ApprovalResolved` → `ApprovalDecision` × 1

use coding_ingest::event::EventKind;
use common::Result;
use std::collections::HashMap;

pub(crate) struct Aggregator {
    buffer: String,
    pending_tools: HashMap<String, PendingTool>,
    pending_approvals: HashMap<String, PendingApproval>,
}

struct PendingTool {
    name: String,
    args: serde_json::Value,
}

struct PendingApproval {
    tool: String,
    layer: String,
}

impl Aggregator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            pending_tools: HashMap::new(),
            pending_approvals: HashMap::new(),
        }
    }

    pub fn on_iteration_start(&mut self) -> Result<Vec<EventKind>> {
        self.buffer.clear();
        Ok(vec![])
    }

    pub fn on_content_chunk(&mut self, text: &str) -> Result<Vec<EventKind>> {
        self.buffer.push_str(text);
        Ok(vec![])
    }

    pub fn on_iteration_end(&mut self) -> Result<Vec<EventKind>> {
        if self.buffer.is_empty() {
            return Ok(vec![]);
        }
        let text = std::mem::take(&mut self.buffer);
        Ok(vec![EventKind::AssistantMsg {
            text,
            truncated: false,
            token_usage: None,
        }])
    }

    pub fn on_tool_start(
        &mut self,
        call_id: &str,
        name: &str,
        args: serde_json::Value,
    ) -> Result<Vec<EventKind>> {
        self.pending_tools.insert(
            call_id.to_string(),
            PendingTool {
                name: name.to_string(),
                args,
            },
        );
        Ok(vec![])
    }

    pub fn on_tool_end(
        &mut self,
        call_id: &str,
        success: bool,
        output: String,
        duration_ms: u64,
    ) -> Result<Vec<EventKind>> {
        let Some(p) = self.pending_tools.remove(call_id) else {
            return Ok(vec![]);
        };
        let args_preview = serde_json::to_string(&p.args).unwrap_or_default();
        let result_preview = output;
        Ok(vec![EventKind::ToolCall {
            tool: p.name,
            args_preview,
            ok: success,
            duration_ms: duration_ms as u32,
            result_preview,
        }])
    }

    pub fn on_approval_requested(
        &mut self,
        request_id: &str,
        tool: &str,
        layer: &str,
    ) -> Result<Vec<EventKind>> {
        self.pending_approvals.insert(
            request_id.to_string(),
            PendingApproval {
                tool: tool.to_string(),
                layer: layer.to_string(),
            },
        );
        Ok(vec![])
    }

    pub fn on_approval_resolved(
        &mut self,
        request_id: &str,
        decision: &str,
    ) -> Result<Vec<EventKind>> {
        let Some(p) = self.pending_approvals.remove(request_id) else {
            return Ok(vec![]);
        };
        Ok(vec![EventKind::ApprovalDecision {
            tool: p.tool,
            decision: decision.to_string(),
            layer: p.layer,
        }])
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}
