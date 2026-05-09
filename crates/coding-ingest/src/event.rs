//! `AgentEvent` — the versioned cross-CLI contract.
//!
//! External CLIs (Claude Code, Codex, kimi-cli, opencode) emit the 9 base
//! `EventKind` variants. klynt-cli additionally emits 10 rich variants.

use crate::scope::RepoScope;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Versioned wrapper. Future `V2` never breaks `V1`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "v", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Current version.
    V1(AgentEventV1),
}

/// The V1 payload. All CLI adapters normalize to this shape.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventV1 {
    /// Per-event UUID.
    pub id: Uuid,
    /// Which CLI emitted this event.
    pub source: AgentSource,
    /// CLI-assigned session id (shape varies per CLI).
    pub session_id: String,
    /// CLI-assigned turn id when present.
    pub turn_id: Option<String>,
    /// Working directory at event time.
    pub cwd: PathBuf,
    /// Resolved repo scope if detectable.
    pub repo: Option<RepoScope>,
    /// Wall-clock timestamp.
    pub occurred_at: Timestamp,
    /// The semantic payload.
    pub kind: EventKind,
}

/// Which coding CLI emitted the event.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AgentSource {
    /// Anthropic Claude Code.
    ClaudeCode,
    /// OpenAI Codex.
    Codex,
    /// Moonshot kimi-cli.
    KimiCli,
    /// sst/opencode.
    OpenCode,
    /// Native klynt-cli (future — see linked spec).
    KlyntCli,
}

/// File-op classifier for `FileEdit` events.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileOp {
    /// File was read (no mutation).
    Read,
    /// File was created.
    Create,
    /// File was modified in place.
    Modify,
    /// File was deleted.
    Delete,
}

/// Token accounting reported by the provider for one assistant turn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Prompt tokens consumed.
    pub prompt_tokens: u32,
    /// Completion tokens produced.
    pub completion_tokens: u32,
    /// Cached-input tokens (prompt-cache hit).
    pub cached_tokens: Option<u32>,
}

/// Symbol reference for `FileEditEnriched` anchoring.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolRef {
    /// Absolute file path.
    pub file_path: PathBuf,
    /// Symbol name (function, method, type, const).
    pub symbol: String,
    /// Commit hash at which the symbol was anchored.
    pub git_hash: String,
}

/// LSP diagnostics delta attached to `FileEditEnriched`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsDelta {
    /// Diagnostics present before the edit.
    pub before: u32,
    /// Diagnostics present after the edit.
    pub after: u32,
    /// Newly introduced diagnostic messages.
    pub introduced: Vec<String>,
    /// Diagnostic messages resolved by the edit.
    pub resolved: Vec<String>,
}

/// A single test failure captured by `TestRunEnriched`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestFailure {
    /// Test identifier (per-framework convention).
    pub test_name: String,
    /// Truncated error or assertion message.
    pub message: String,
    /// Stack trace when available.
    pub stack: Option<String>,
}

/// A skill considered by klynt-cli's router, with its composite score.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillScore {
    /// Skill identifier.
    pub skill_id: String,
    /// Composite score (0.0 – 1.0).
    pub score: f32,
    /// Breakdown components for observability.
    pub components: Vec<(String, f32)>,
}

/// Semantic payload of an `AgentEvent`. 9 base variants for external CLIs;
/// 10 rich variants for klynt-cli.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    /// A coding-CLI session just started.
    SessionStart {
        /// Model chosen for this session, if known.
        model: Option<String>,
        /// Origin of the session (e.g. `"cli-arg"`, `"resumed"`).
        source_reason: String,
    },
    /// The session terminated.
    SessionEnd {
        /// Why it ended (`"user-quit"`, `"error"`, `"compact"`, …).
        reason: String,
    },
    /// User typed a prompt.
    UserPrompt {
        /// Raw prompt text.
        text: String,
        /// Attached files the CLI reported.
        attachments: Vec<PathBuf>,
    },
    /// Assistant finished a response.
    AssistantMsg {
        /// Response text (may be truncated).
        text: String,
        /// Whether the text was truncated at this boundary.
        truncated: bool,
        /// Provider-reported usage if available.
        token_usage: Option<TokenUsage>,
    },
    /// Assistant invoked a tool.
    ToolCall {
        /// Tool name.
        tool: String,
        /// Truncated args preview.
        args_preview: String,
        /// Whether the call succeeded.
        ok: bool,
        /// Wall-clock duration.
        duration_ms: u32,
        /// Truncated result preview.
        result_preview: String,
    },
    /// A file was touched.
    FileEdit {
        /// Absolute path.
        path: PathBuf,
        /// Operation performed.
        op: FileOp,
        /// New file size in bytes.
        bytes: u64,
        /// Optional unified-diff preview.
        diff_preview: Option<String>,
    },
    /// A test runner was invoked.
    TestRun {
        /// Command as executed.
        command: String,
        /// Detected framework (`"cargo"`, `"pytest"`, …).
        framework: Option<String>,
        /// Number of passing tests.
        passed: u32,
        /// Number of failing tests.
        failed: u32,
        /// Wall-clock duration.
        duration_ms: u32,
    },
    /// Compaction/summarization boundary.
    CompactEvent {
        /// What triggered compaction.
        trigger: String,
        /// Pre-compaction token count.
        token_count: u32,
    },
    /// A tool or provider error surfaced.
    Error {
        /// Tool that failed (if tool-scoped).
        tool: Option<String>,
        /// Human-readable message.
        message: String,
    },
    /// klynt-cli: a skill was activated for this turn.
    SkillActivated {
        /// Skill identifier.
        skill_id: String,
        /// SKILL.md source path.
        source_path: PathBuf,
        /// Why it was activated.
        trigger: String,
    },
    /// klynt-cli: memories were injected into the turn context.
    RecallInjected {
        /// IDs of injected memories.
        memory_ids: Vec<String>,
        /// C3 coverage score at injection time.
        coverage_score: f32,
        /// Whether a dead-end warning was attached.
        dead_end_warning: bool,
    },
    /// klynt-cli: approval/sandbox layer decided to allow or deny a tool call.
    ApprovalDecision {
        /// Tool name.
        tool: String,
        /// `"allow"`, `"deny"`, `"ask"`.
        decision: String,
        /// Which approval layer decided (`"privacy"`, `"rules"`, `"user"`).
        layer: String,
    },
    /// klynt-cli: sandbox policy applied for a tool call.
    SandboxApplied {
        /// Tool name.
        tool: String,
        /// One-line summary of the policy.
        policy_summary: String,
        /// Whether the call fell back to unsandboxed execution.
        fallback_unsandboxed: bool,
    },
    /// klynt-cli: `FileEdit` enriched with tree-sitter anchors + LSP diagnostics.
    FileEditEnriched {
        /// Absolute path.
        path: PathBuf,
        /// Operation performed.
        op: FileOp,
        /// Anchored symbols extracted via tree-sitter.
        anchored_symbols: Vec<SymbolRef>,
        /// LSP diagnostic delta.
        lsp_diagnostics_delta: Option<DiagnosticsDelta>,
    },
    /// klynt-cli: `TestRun` enriched with per-test outcomes.
    TestRunEnriched {
        /// Command as executed.
        command: String,
        /// Names of passing tests.
        passed_tests: Vec<String>,
        /// Failure details per failed test.
        failed_tests: Vec<TestFailure>,
        /// Tests newly failing vs. last run.
        newly_failing: Vec<String>,
    },
    /// klynt-cli: a provider call was completed (cost + latency visible).
    ProviderCall {
        /// Model identifier.
        model: String,
        /// Prompt tokens.
        prompt_tokens: u32,
        /// Completion tokens.
        completion_tokens: u32,
        /// USD cost of this call.
        cost_usd: f64,
        /// End-to-end latency.
        latency_ms: u64,
        /// Retries this call required.
        retries: u32,
    },
    /// klynt-cli: mid-loop compression was applied.
    CompressionApplied {
        /// Token count before compression.
        before_tokens: u32,
        /// Token count after compression.
        after_tokens: u32,
        /// Number of messages condensed.
        messages_condensed: u32,
    },
    /// klynt-cli: a Mirror alert was surfaced to the user.
    MirrorAlert {
        /// Alert identifier (maps to `mirror_snippets.id`).
        alert_id: String,
        /// Severity level (`"low"`, `"medium"`, `"high"`, `"critical"`).
        severity: String,
        /// Alert kind (closed enum string — see coding-memory design §10).
        #[serde(rename = "mirrorKind")]
        kind: String,
    },
    /// klynt-cli: skill router trace for a turn.
    SkillRoutingTrace {
        /// Skills considered with scores.
        considered: Vec<SkillScore>,
        /// Skill ids chosen for injection.
        chosen: Vec<String>,
    },
    /// A git post-commit hook fired. Drives Phase-6 symbol invalidation.
    GitCommit {
        /// Commit hash (HEAD after the commit).
        commit_hash: String,
        /// Parent commit hash (None for initial commit).
        parent_hash: Option<String>,
        /// Repo root absolute path.
        repo_root: PathBuf,
        /// Files changed in this commit (relative to `repo_root`).
        changed_files: Vec<PathBuf>,
    },

    /// Background bash job lifecycle event (Phase 2.3a).
    /// Klynt-only: this kind never appears in claude-code/codex/kimi/opencode streams.
    BackgroundJobLifecycle {
        /// Stable job id.
        job_id: String,
        /// Lifecycle phase.
        phase: BackgroundJobPhase,
        /// Exit code when terminal.
        exit_code: Option<i32>,
        /// Failure kind classification when terminal.
        failure_kind: Option<String>,
        /// Human-readable gate summary.
        gate_summary: Option<String>,
    },

    /// Emitted when a job's ring-buffer output file is bisected due to overflow.
    BackgroundJobOutputBisect {
        /// Stable job id.
        job_id: String,
        /// Monotonic bisect generation counter.
        bisect_gen: u64,
        /// Bytes dropped from the middle.
        dropped_bytes: u64,
    },
}

/// Phase of a background bash job lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundJobPhase {
    /// Job started.
    Started,
    /// Job was explicitly stopped.
    Stopped,
    /// Job finished successfully.
    Completed,
    /// Job finished with a non-zero exit code.
    Failed,
    /// Job was lost due to klynt restart.
    Lost,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_job_lifecycle_round_trips() {
        let kind = EventKind::BackgroundJobLifecycle {
            job_id: "bash-aB3kF7c2qR".into(),
            phase: BackgroundJobPhase::Completed,
            exit_code: Some(0),
            failure_kind: None,
            gate_summary: None,
        };
        let s = serde_json::to_string(&kind).unwrap();
        let back: EventKind = serde_json::from_str(&s).unwrap();
        assert_eq!(kind, back);
    }

    #[test]
    fn background_job_output_bisect_round_trips() {
        let kind = EventKind::BackgroundJobOutputBisect {
            job_id: "bash-aB3kF7c2qR".into(),
            bisect_gen: 1,
            dropped_bytes: 1_500_000,
        };
        let s = serde_json::to_string(&kind).unwrap();
        let back: EventKind = serde_json::from_str(&s).unwrap();
        assert_eq!(kind, back);
    }
}
