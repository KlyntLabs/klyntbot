//! Translator — maps runtime events into `coding_ingest::AgentEvent` ingest events.
#![allow(missing_docs)]

use crate::sink::aggregator::Aggregator;
use coding_ingest::event::{
    AgentEvent as IngestEvt, AgentEventV1, AgentSource, EventKind, FileOp, SymbolRef,
};
// RepoScope re-exported from coding_ingest::event for backward compat — remove once callers migrate
use common::Result;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Simplified runtime event shaped for translation.
///
/// Decouples the translator from `agent::events::AgentEvent` so that
/// `coding-memory` (L5) does not depend on `agent` (L4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// File edit with symbol anchors.
    FileEditWithSymbols {
        /// Absolute path of the file.
        path: PathBuf,
        /// Operation performed (e.g. "edit", "create", "delete").
        op: String,
        /// New file size in bytes.
        bytes: u64,
        /// Anchored symbols extracted via tree-sitter.
        anchored_symbols: Vec<SymbolRef>,
        /// LSP diagnostic delta, if available.
        lsp_diagnostics_delta: Option<coding_ingest::event::DiagnosticsDelta>,
    },
    /// Provider response with usage / cost.
    ProviderResponse {
        /// Model identifier.
        model: String,
        /// Prompt tokens consumed.
        prompt_tokens: u32,
        /// Completion tokens produced.
        completion_tokens: u32,
        /// USD cost of this call.
        cost_usd: f64,
        /// End-to-end latency in milliseconds.
        latency_ms: u64,
        /// Retries this call required.
        retries: u32,
    },
    /// Recall context was injected.
    RecallInjected {
        /// IDs of injected memories.
        memory_ids: Vec<String>,
        /// C3 coverage score at injection time.
        coverage_score: f32,
        /// Whether a dead-end warning was attached.
        dead_end_warning: bool,
    },
    /// A skill was activated.
    SkillActivated {
        /// Skill identifier.
        skill_id: String,
        /// Path to the skill source file.
        source_path: String,
        /// Trigger reason (e.g. "path_touch").
        trigger: String,
    },
    /// Sandbox policy applied.
    SandboxPolicyApplied {
        /// Tool name.
        tool: String,
        /// One-line summary of the policy.
        policy_summary: String,
        /// Whether the call fell back to unsandboxed execution.
        fallback_unsandboxed: bool,
    },
    /// Streaming content chunk.
    ContentChunk {
        /// Text content of the chunk.
        text: String,
    },
    /// Iteration boundary start.
    IterationStart,
    /// Iteration boundary end.
    IterationEnd,
    /// Tool execution started.
    ToolStart {
        /// Correlation ID pairing start with end.
        call_id: String,
        /// Tool name.
        name: String,
        /// Tool arguments.
        args: serde_json::Value,
    },
    /// Tool execution completed.
    ToolEnd {
        /// Correlation ID pairing end with start.
        call_id: String,
        /// Whether the tool succeeded.
        success: bool,
        /// Truncated tool output.
        output: String,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
    /// Approval requested.
    ApprovalRequested {
        /// Request correlation ID.
        request_id: String,
        /// Tool name.
        tool: String,
        /// Approval layer (e.g. "privacy", "rules", "user").
        layer: String,
    },
    /// Approval resolved.
    ApprovalResolved {
        /// Request correlation ID.
        request_id: String,
        /// Decision string (e.g. "allow", "deny").
        decision: String,
    },
}

/// Stateless translator + stateful aggregator.
pub struct Translator {
    pub(super) aggregator: Aggregator,
}

impl Translator {
    /// Create a new translator with an empty aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            aggregator: Aggregator::new(),
        }
    }

    /// Translate a single runtime event into 0..N ingest `EventKind`s, then wrap
    /// each in a versioned `AgentEvent`.
    pub fn translate(&mut self, evt: &RuntimeEvent) -> Result<Vec<IngestEvt>> {
        let kinds = match evt {
            RuntimeEvent::FileEditWithSymbols {
                path,
                op,
                anchored_symbols,
                lsp_diagnostics_delta,
                ..
            } => {
                let op = parse_file_op(op);
                vec![EventKind::FileEditEnriched {
                    path: path.clone(),
                    op,
                    anchored_symbols: anchored_symbols.clone(),
                    lsp_diagnostics_delta: lsp_diagnostics_delta.clone(),
                }]
            }
            RuntimeEvent::ProviderResponse {
                model,
                prompt_tokens,
                completion_tokens,
                cost_usd,
                latency_ms,
                retries,
            } => vec![EventKind::ProviderCall {
                model: model.clone(),
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
                cost_usd: *cost_usd,
                latency_ms: *latency_ms,
                retries: *retries,
            }],
            RuntimeEvent::RecallInjected {
                memory_ids,
                coverage_score,
                dead_end_warning,
            } => vec![EventKind::RecallInjected {
                memory_ids: memory_ids.clone(),
                coverage_score: *coverage_score,
                dead_end_warning: *dead_end_warning,
            }],
            RuntimeEvent::SkillActivated {
                skill_id,
                source_path,
                trigger,
            } => vec![EventKind::SkillActivated {
                skill_id: skill_id.clone(),
                source_path: PathBuf::from(source_path),
                trigger: trigger.clone(),
            }],
            RuntimeEvent::SandboxPolicyApplied {
                tool,
                policy_summary,
                fallback_unsandboxed,
            } => vec![EventKind::SandboxApplied {
                tool: tool.clone(),
                policy_summary: policy_summary.clone(),
                fallback_unsandboxed: *fallback_unsandboxed,
            }],
            RuntimeEvent::ContentChunk { text } => self.aggregator.on_content_chunk(text)?,
            RuntimeEvent::IterationStart => self.aggregator.on_iteration_start()?,
            RuntimeEvent::IterationEnd => self.aggregator.on_iteration_end()?,
            RuntimeEvent::ToolStart {
                call_id,
                name,
                args,
            } => self.aggregator.on_tool_start(call_id, name, args.clone())?,
            RuntimeEvent::ToolEnd {
                call_id,
                success,
                output,
                duration_ms,
            } => self
                .aggregator
                .on_tool_end(call_id, *success, output.clone(), *duration_ms)?,
            RuntimeEvent::ApprovalRequested {
                request_id,
                tool,
                layer,
            } => self
                .aggregator
                .on_approval_requested(request_id, tool, layer)?,
            RuntimeEvent::ApprovalResolved {
                request_id,
                decision,
            } => self.aggregator.on_approval_resolved(request_id, decision)?,
        };

        let now = Timestamp::now();
        Ok(kinds
            .into_iter()
            .map(|kind| {
                IngestEvt::V1(AgentEventV1 {
                    id: Uuid::new_v4(),
                    source: AgentSource::KlyntCli,
                    session_id: String::new(),
                    turn_id: None,
                    cwd: PathBuf::from("."),
                    repo: None,
                    occurred_at: now,
                    kind,
                })
            })
            .collect())
    }
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_file_op(op: &str) -> FileOp {
    match op {
        "read" => FileOp::Read,
        "create" => FileOp::Create,
        "delete" => FileOp::Delete,
        _ => FileOp::Modify,
    }
}
