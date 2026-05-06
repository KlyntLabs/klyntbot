//! Provider-agnostic DTOs for the tracing UI.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Phase 3.1: Scope ────────────────────────────────────────────────────

/// Selector for "main agent" vs a specific subagent within a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Scope {
    Main,
    #[serde(rename_all = "camelCase")]
    Subagent {
        agent_id: String,
    },
}

impl Scope {
    pub fn is_main(&self) -> bool {
        matches!(self, Scope::Main)
    }
    pub fn subagent_id(&self) -> Option<&str> {
        match self {
            Scope::Main => None,
            Scope::Subagent { agent_id } => Some(agent_id),
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn main_serializes_with_tag() {
        let s = serde_json::to_string(&Scope::Main).unwrap();
        assert_eq!(s, r#"{"kind":"main"}"#);
    }
    #[test]
    fn subagent_serializes_with_camel_case_field() {
        let s = serde_json::to_string(&Scope::Subagent {
            agent_id: "a1".into(),
        })
        .unwrap();
        assert_eq!(s, r#"{"kind":"subagent","agentId":"a1"}"#);
    }
}

// ── Phase 3.2: SemanticCategory ─────────────────────────────────────────

/// Closed semantic category the UI dispatches per event card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SemanticCategory {
    TurnBegin,
    TurnEnd,
    StepBegin,
    StepInterrupted,
    Thinking,
    AssistantText,
    UserInput,
    ToolCall,
    ToolCallStream,
    ToolResult,
    StatusUpdate,
    Subagent,
    CompactionBegin,
    CompactionEnd,
    Error,
    Other,
}

#[cfg(test)]
mod category_tests {
    use super::*;
    #[test]
    fn category_serializes_camel_case() {
        let s = serde_json::to_string(&SemanticCategory::ToolCall).unwrap();
        assert_eq!(s, r#""toolCall""#);
        let s = serde_json::to_string(&SemanticCategory::CompactionBegin).unwrap();
        assert_eq!(s, r#""compactionBegin""#);
    }
}

// ── Phase 3.3: TraceEvent ───────────────────────────────────────────────

/// One event in a session's wire stream, in provider-agnostic form.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TraceEvent {
    /// Per-session-monotonic sequence (line index in wire.jsonl, skipping metadata).
    pub seq: u64,
    /// Provider id ("kimi", "claudeCode", …).
    pub provider_id: String,
    /// Verbatim raw kind string from the source ("TurnBegin", "ContentPart", …).
    pub raw_kind: String,
    /// Verbatim payload from the source.
    #[specta(type = desktop_shared::specta_helpers::JsonValue)]
    pub payload: serde_json::Value,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub occurred_at: Timestamp,
    pub category: SemanticCategory,
    pub turn_index: Option<u32>,
    pub step_index: Option<u32>,
    pub parent_subagent_id: Option<String>,
    /// True when the source line carried `isMeta: true` (Claude Code metadata).
    /// Kimi sets this to false. UI default-filters meta events with a toggle.
    #[serde(default)]
    pub meta: bool,
}

// ── Phase 4.1: HeaderStats ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeaderStats {
    pub turn_count: u32,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub compaction_count: u32,
    pub agent_count: u32,
    pub total_duration_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_hit_pct: f32,
    pub model: Option<String>,
}

// ── Phase 4.2: SessionSummary ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub provider_id: String,
    pub source_dir: PathBuf,
    pub cwd: Option<PathBuf>,
    pub project_basename: Option<String>,
    pub custom_title: Option<String>,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub started_at: Timestamp,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub last_event_at: Timestamp,
    pub size_bytes: u64,
    pub turn_count: u32,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub subagent_count: u32,
    pub has_wire: bool,
    pub has_context: bool,
    pub imported: bool,

    // ── Tracing UI port additions ──
    pub work_dir_hash: String,
    pub has_state: bool,
    pub wire_size: u64,
    pub context_size: u64,
    pub state_size: u64,
    pub total_size: u64,
    pub metadata: Option<SessionMetadataInfo>,
}

// ── Phase 4.3: SessionDetail ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session_id: String,
    pub provider_id: String,
    pub scope: Scope,
    pub stats: HeaderStats,
    pub events: Vec<TraceEvent>,
    /// `true` when events were truncated due to file-size guardrail.
    pub truncated: bool,
    pub total_event_count: u64,
}

// ── Phase 5.1: ContextMessage ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextMessage {
    pub index: u32,
    pub role: String,
    #[specta(type = desktop_shared::specta_helpers::JsonValue)]
    pub content: serde_json::Value,
}

// ── Phase 5.2: KimiTodo + SessionState ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KimiTodo {
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub custom_title: Option<String>,
    pub plan_mode: bool,
    pub archived: bool,
    pub todos: Vec<KimiTodo>,
    /// Verbatim parsed state.json for the State tab's "raw" view.
    #[specta(type = desktop_shared::specta_helpers::JsonValue)]
    pub raw: serde_json::Value,
}

// ── Phase 5.3: SubagentSummary ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub agent_id: String,
    pub subagent_type: String,
    pub status: String,
    pub description: Option<String>,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub created_at: Timestamp,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub updated_at: Timestamp,
    pub event_count: u32,
}

// ── Phase 5.4: StatsBundle and friends ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTotals {
    pub project_basename: String,
    pub cwd: PathBuf,
    pub session_count: u32,
    pub turn_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsage {
    pub tool: String,
    pub call_count: u32,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ErrorByTool {
    pub tool: String,
    pub error_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TokenSeriesPoint {
    /// Day key in `YYYY-MM-DD`.
    pub day: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentTypeCount {
    pub subagent_type: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatsBundle {
    pub per_project: Vec<ProjectTotals>,
    pub tool_usage: Vec<ToolUsage>,
    pub errors_by_tool: Vec<ErrorByTool>,
    pub token_series: Vec<TokenSeriesPoint>,
    pub subagent_types: Vec<SubagentTypeCount>,
    pub cache_hit_pct: f32,
}

// ── Phase 5.5: ProviderInfo ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub session_count: u32,
}

// ── Tracing UI port: SessionMetadataInfo ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataInfo {
    pub session_id: String,
    pub title: String,
    pub title_generated: bool,
    pub archived: bool,
    pub archived_at: Option<i64>,
    pub auto_archive_exempt: bool,
    pub wire_mtime: Option<i64>,
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn metadata_serializes_camel_case() {
        let m = SessionMetadataInfo {
            session_id: "s1".into(),
            title: "t".into(),
            title_generated: false,
            archived: false,
            archived_at: None,
            auto_archive_exempt: false,
            wire_mtime: Some(1_700_000_000),
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains(r#""sessionId":"s1""#));
        assert!(s.contains(r#""titleGenerated":false"#));
        assert!(s.contains(r#""wireMtime":1700000000"#));
    }
}
