//! Distiller — online writer.
//!
//! Phase A (extractive, always runs) + Phase B (LLM synthesis) + Phase C
//! (reconciliation). Phase 1 defines types; bodies land in Phase 3.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use coding_ingest::AgentEvent;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

/// Which distiller phase produced a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillerPhase {
    /// Phase A — deterministic extractive pass.
    Extractive,
    /// Phase B — LLM synthesis.
    Llm,
    /// Phase C — reconciliation (ADD / SUPERSEDE / NOOP).
    Reconciliation,
}

/// Deterministic pass output — always produced, never lost.
#[derive(Debug, Clone)]
pub struct TurnTrace {
    /// Session id.
    pub session_id: String,
    /// Turn id.
    pub turn_id: Option<String>,
    /// Files read during the turn.
    pub files_read: Vec<PathBuf>,
    /// Files modified with byte deltas.
    pub files_modified: Vec<(PathBuf, i64)>,
    /// Shell commands run.
    pub commands_run: Vec<String>,
    /// Test runner outcomes.
    pub test_outcomes: Vec<TestOutcome>,
    /// Errors encountered.
    pub errors_encountered: Vec<(Option<String>, String)>,
    /// Final assistant token usage (if any).
    pub token_usage: Option<TurnTokenUsage>,
    /// Start of turn.
    pub started_at: Timestamp,
    /// End of turn.
    pub ended_at: Option<Timestamp>,
}

/// Test-run outcome observed during a turn.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// Command.
    pub command: String,
    /// Framework.
    pub framework: Option<String>,
    /// Passed count.
    pub passed: u32,
    /// Failed count.
    pub failed: u32,
}

/// Token usage aggregated across a turn.
#[derive(Debug, Clone, Copy)]
pub struct TurnTokenUsage {
    /// Prompt tokens.
    pub prompt: u32,
    /// Completion tokens.
    pub completion: u32,
    /// Cache hits.
    pub cached: u32,
}

/// Distiller handle — constructed once per desktop; accepts events per turn.
#[derive(Debug)]
pub struct Distiller {
    /// Phase-3+ wiring will carry repo handles, provider manager, etc.
    _phase_stub: (),
}

impl Distiller {
    /// Construct a Distiller. Phase 1 stub (no deps wired).
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }

    /// Accept a single event into the per-turn buffer. Phase 3.
    pub async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(3))
    }

    /// Trigger distillation for one turn (typically on `SessionEnd` or
    /// `AssistantMsg` with `token_usage`). Phase 3.
    pub async fn distill_turn(
        &self,
        _session_id: &str,
        _turn_id: Option<&str>,
    ) -> Result<DistillationReport> {
        Err(phase(3))
    }
}

impl Default for Distiller {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of what was written during one distillation cycle.
#[derive(Debug, Clone, Default)]
pub struct DistillationReport {
    /// Number of `SemanticFact` rows added or superseded.
    pub semantic_writes: u32,
    /// Number of `EpisodicMemory` rows added.
    pub episodic_writes: u32,
    /// Phase B LLM invocation count (0 when extractive-only).
    pub llm_calls: u32,
    /// Phase B cost in USD (0.0 when extractive-only).
    pub llm_cost_usd: f64,
    /// Turn trace id (`episodic_memories.id`).
    pub turn_trace_id: Option<Uuid>,
}

/// The LLM tool schema the Distiller exposes to Phase B providers.
#[async_trait]
pub trait RecordObservationTool: Send + Sync {
    /// Handle an observation the LLM emitted.
    #[allow(clippy::too_many_arguments)]
    async fn record_observation(
        &self,
        kind: crate::facts::CodingKind,
        subject: String,
        predicate: String,
        object: String,
        confidence: f32,
        scope: crate::facts::StyleScope,
        reasoning: String,
    ) -> Result<()>;
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
