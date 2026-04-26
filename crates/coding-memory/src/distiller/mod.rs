//! Distiller — online writer.
//!
//! Phase A (extractive, always runs) + Phase B (LLM synthesis) + Phase C
//! (reconciliation). Phase 1 defines types; bodies land in Phase 3.

use async_trait::async_trait;
use coding_ingest::event::AgentEvent;
use coding_ingest::store::IngestEventLogRepo;
use common::Result;
use jiff::Timestamp;
use providers::ProviderManager;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use turn_buffer::{TurnBoundary, TurnBuffer};
use uuid::Uuid;

pub use retry_queue::{DistillationRetryRepo, RetryReason, RetryRow};

/// Distiller-scoped error taxonomy.
pub mod error;

pub use error::DistillerError;

/// Write chokepoint enforcing provenance-always invariant.
pub mod writer;

pub use writer::{DistillerWriter, PreparedEpisode, PreparedFact};

/// Turn boundary detection.
pub mod turn_buffer;

/// Phase A extractive pass.
pub mod phase_a;

/// `record_observation` LLM tool schema + decoder.
pub mod record_observation;

/// Observation → row-ready conversion.
pub mod fact_builder;

/// Phase B — LLM synthesis (prompt + invocation).
pub mod phase_b;

/// Distillation retry queue.
pub mod retry_queue;

/// Phase C — reconciliation (ADD/SUPERSEDE/NOOP).
pub mod phase_c;

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

/// Runtime config for the Distiller.
#[derive(Debug, Clone)]
pub struct DistillerConfig {
    /// Model id (pulled from `codingMemory.distiller.model`).
    pub model: String,
    /// Max tokens the synthesis prompt can consume.
    pub max_input_tokens: u32,
    /// LLM call timeout.
    pub timeout: std::time::Duration,
    /// Idle-turn sweep period.
    pub idle_timeout: std::time::Duration,
}

impl Default for DistillerConfig {
    fn default() -> Self {
        Self {
            model: "claude-haiku-4-5-20251001".into(),
            max_input_tokens: 8000,
            timeout: std::time::Duration::from_secs(30),
            idle_timeout: std::time::Duration::from_secs(120),
        }
    }
}

/// Distiller handle. Clone freely — internal state behind `Arc<Mutex<...>>`.
#[derive(Clone)]
pub struct Distiller {
    inner: Arc<DistillerInner>,
}

struct DistillerInner {
    config: DistillerConfig,
    ingest_repo: Arc<IngestEventLogRepo>,
    writer: writer::DistillerWriter,
    provider: Arc<ProviderManager>,
    #[allow(dead_code)]
    retriever: Arc<dyn context_engine::MemoryRetriever>,
    retry_repo: Option<DistillationRetryRepo>,
    buffer: Mutex<TurnBuffer>,
    /// Optional event bus. When `Some`, every successful fact / episode write
    /// publishes a `DomainEvent::CodingMemoryUpdated` so UI panels can refresh
    /// without polling. `None` keeps standalone use (tests, CLI replays) silent.
    event_bus: Option<Arc<bus::DomainEventBus>>,
}

impl std::fmt::Debug for DistillerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DistillerInner")
            .field("model", &self.config.model)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Distiller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Distiller").finish_non_exhaustive()
    }
}

impl Distiller {
    /// Construct. Called once during `AppCore::init`.
    #[must_use]
    pub fn new(
        config: DistillerConfig,
        ingest_repo: Arc<IngestEventLogRepo>,
        writer: writer::DistillerWriter,
        provider: Arc<ProviderManager>,
        retriever: Arc<dyn context_engine::MemoryRetriever>,
    ) -> Self {
        Self {
            inner: Arc::new(DistillerInner {
                config,
                ingest_repo,
                writer,
                provider,
                retriever,
                retry_repo: None,
                buffer: Mutex::new(TurnBuffer::new()),
                event_bus: None,
            }),
        }
    }

    /// Attach a retry-repo — enables transient-failure re-distillation.
    pub fn with_retry_repo(mut self, repo: DistillationRetryRepo) -> Self {
        let inner =
            Arc::get_mut(&mut self.inner).expect("Distiller::with_retry_repo called after clone");
        inner.retry_repo = Some(repo);
        self
    }

    /// Attach a domain-event bus. Returns `self` for chaining alongside
    /// `with_retry_repo`. Idempotent — last call wins.
    pub fn with_event_bus(mut self, bus: Arc<bus::DomainEventBus>) -> Self {
        let inner =
            Arc::get_mut(&mut self.inner).expect("Distiller::with_event_bus called after clone");
        inner.event_bus = Some(bus);
        self
    }

    /// Accept an event into the per-turn buffer. Triggers `distill_turn` on boundary.
    pub async fn accept_event(&self, event: AgentEvent) -> Result<()> {
        let boundary = {
            let mut buf = self.inner.buffer.lock().await;
            buf.accept(&event)
        };
        if let TurnBoundary::Fire {
            session_id,
            turn_id,
        } = boundary
        {
            let distiller = self.clone();
            // Fire-and-forget — Distiller failures never propagate back to ingestion.
            tokio::spawn(async move {
                if let Err(e) = distiller
                    .distill_turn(&session_id, turn_id.as_deref())
                    .await
                {
                    tracing::warn!(session_id, ?turn_id, error = %e, "distill_turn failed");
                }
            });
        }
        Ok(())
    }

    /// Flush any buffered turns that have gone idle.
    pub async fn sweep_idle(&self) -> Result<()> {
        let stale = {
            let mut buf = self.inner.buffer.lock().await;
            buf.fire_idle_turns(self.inner.config.idle_timeout)
        };
        for t in stale {
            let _ = self.distill_turn(&t.session_id, t.turn_id.as_deref()).await;
        }
        Ok(())
    }

    /// Re-run distillation for every due retry-queue row.
    pub async fn sweep_retries(&self) -> Result<u32> {
        let retry = match &self.inner.retry_repo {
            Some(r) => r,
            None => return Ok(0),
        };
        let due = retry.list_due(50).await?;
        let mut processed = 0u32;
        for row in due {
            match self
                .distill_turn(&row.session_id, row.turn_id.as_deref())
                .await
            {
                Ok(_) => {
                    let _ = retry.mark_done(&row.id).await;
                    processed += 1;
                }
                Err(_) => {
                    let _ = retry.record_attempt(&row.id).await;
                }
            }
        }
        Ok(processed)
    }

    /// Distill one turn — implemented incrementally in Tasks 9–22.
    pub async fn distill_turn(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
    ) -> Result<DistillationReport> {
        use coding_ingest::event::{AgentEvent, EventKind};
        use error::DistillerError;
        use phase_b::LlmInvocation;
        use phase_c::{reconcile, ReconcileDecision};

        let claimed = self
            .inner
            .ingest_repo
            .mark_processing(session_id, turn_id)
            .await?;
        if claimed == 0 {
            return Ok(DistillationReport::default());
        }

        let rows = self
            .inner
            .ingest_repo
            .fetch_turn(session_id, turn_id)
            .await?;
        let events: Vec<AgentEvent> = rows
            .iter()
            .filter_map(|r| serde_json::from_str::<AgentEvent>(&r.payload).ok())
            .collect();
        let source_event_ids: Vec<uuid::Uuid> = rows
            .iter()
            .filter_map(|r| uuid::Uuid::parse_str(&r.id).ok())
            .collect();
        let row_ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();

        let repo_id = events.iter().find_map(|e| {
            let AgentEvent::V1(v1) = e;
            v1.repo.as_ref().map(|r| r.repo_id.clone())
        });

        // Phase A — extractive.
        let trace = phase_a::compute_turn_trace(session_id, turn_id, &events);
        let mut report = DistillationReport::default();

        let prov_ext = crate::scope::ProvenanceMetadata {
            source_events: source_event_ids.clone(),
            session_id: session_id.to_string(),
            turn_id: turn_id.map(str::to_string),
            distilled_at: jiff::Timestamp::now(),
            distiller_model: self.inner.config.model.clone(),
            source_kind: crate::scope::ProvenanceKind::DistillerExtractive,
        };
        let trace_id =
            phase_a::persist_turn_trace(&self.inner.writer, &trace, repo_id.as_deref(), &prov_ext)
                .await
                .map_err(common::KlyntbotError::from)?;
        report.episodic_writes += 1;
        report.turn_trace_id = Some(trace_id);

        // Phase B — LLM synthesis. Failures are non-fatal — Phase A already lands.
        let user_prompt_text = events
            .iter()
            .find_map(|e| {
                let AgentEvent::V1(v1) = e;
                if let EventKind::UserPrompt { text, .. } = &v1.kind {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let assistant_text = events
            .iter()
            .rev()
            .find_map(|e| {
                let AgentEvent::V1(v1) = e;
                if let EventKind::AssistantMsg { text, .. } = &v1.kind {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let observations = match phase_b::invoke_llm(LlmInvocation {
            provider: self.inner.provider.clone(),
            model: self.inner.config.model.clone(),
            user_prompt_text: &user_prompt_text,
            assistant_text: &assistant_text,
            trace: &trace,
            repo_id: repo_id.as_deref(),
            timeout: self.inner.config.timeout,
        })
        .await
        {
            Ok(v) => {
                report.llm_calls += 1;
                v
            }
            Err(ref e)
                if matches!(
                    e,
                    DistillerError::LlmTimeout { .. }
                        | DistillerError::LlmProvider { .. }
                        | DistillerError::Transient { .. }
                ) =>
            {
                tracing::warn!(
                    session_id,
                    ?turn_id,
                    "Phase B transient failure — skipping LLM observations"
                );
                if let Some(ref retry) = self.inner.retry_repo {
                    let reason = match e {
                        DistillerError::LlmTimeout { .. } => RetryReason::LlmTimeout,
                        DistillerError::LlmProvider { .. } => RetryReason::LlmProvider,
                        _ => RetryReason::Transient,
                    };
                    let _ = retry.enqueue(session_id, turn_id, reason).await;
                }
                vec![]
            }
            Err(DistillerError::LlmMalformed { detail }) => {
                tracing::warn!(session_id, ?turn_id, %detail, "Phase B malformed — dropping observations");
                vec![]
            }
            Err(e) => return Err(e.into()),
        };

        let prov_llm = crate::scope::ProvenanceMetadata {
            source_kind: crate::scope::ProvenanceKind::DistillerLlm,
            ..prov_ext.clone()
        };

        // Phase C per observation.
        for obs in &observations {
            let prepared = match fact_builder::build_prepared(obs, repo_id.as_deref(), &prov_llm) {
                Ok(p) => p,
                Err(_) => continue,
            };
            match prepared {
                fact_builder::Prepared::Episode(ep) => {
                    self.inner
                        .writer
                        .write_episode(ep)
                        .await
                        .map_err(common::KlyntbotError::from)?;
                    report.episodic_writes += 1;
                    // Task 27 — Tier B1 counterfactual derivation.
                    // When a FixAttempt observation reports outcome ∈ {failure, abandoned},
                    // emit a derived `DeadEndAttempt` semantic fact via `derive_dead_end`.
                    if matches!(obs.kind, crate::facts::CodingKind::FixAttempt)
                        && matches!(
                            obs.outcome,
                            Some(crate::facts::FixOutcome::Failure)
                                | Some(crate::facts::FixOutcome::Abandoned)
                        )
                    {
                        let synthetic = crate::facts::FixAttempt {
                            problem_hash: crate::problem_hash::ProblemHash::of(&obs.subject)
                                .as_str()
                                .to_string(),
                            problem: obs.subject.clone(),
                            files: vec![],
                            approach: obs.object.clone(),
                            outcome: obs.outcome.unwrap(),
                            insight: Some(obs.reasoning.clone()),
                            duration_ms: 0,
                            test_before: None,
                            test_after: None,
                            anchored_symbols: vec![],
                            provenance: prov_llm.clone(),
                            sensitivity: crate::scope::Sensitivity::Normal,
                        };
                        if let Some(fact) =
                            crate::counterfactual::derive_dead_end(&synthetic, &prov_llm)
                        {
                            let prepared_fact = crate::distiller::writer::PreparedFact {
                                fact,
                                metadata_json: None,
                                scope_repo_id: match obs.scope {
                                    record_observation::ObservationScope::Repo => repo_id.clone(),
                                    record_observation::ObservationScope::Global => None,
                                },
                                provenance: prov_llm.clone(),
                            };
                            self.inner
                                .writer
                                .write_fact(prepared_fact)
                                .await
                                .map_err(common::KlyntbotError::from)?;
                            report.semantic_writes += 1;
                        }
                    }
                }
                fact_builder::Prepared::Fact(pf) => {
                    // Simple reconciliation: exact subject+predicate match via find_similar
                    let similar = self
                        .inner
                        .writer
                        .facts()
                        .find_similar(&pf.fact.subject, &pf.fact.predicate)
                        .await
                        .ok()
                        .unwrap_or_default();
                    let similar_scored: Vec<phase_c::SimilarFact> = similar
                        .into_iter()
                        .map(|f| {
                            // Simple similarity: exact object match = 1.0, else 0.5
                            let sim = if f.object == pf.fact.object { 1.0 } else { 0.5 };
                            phase_c::SimilarFact {
                                fact: f,
                                similarity: sim,
                            }
                        })
                        .collect();
                    match reconcile(&pf.fact, &similar_scored) {
                        ReconcileDecision::Noop { predecessor_id } => {
                            let _ = self.inner.writer.bump_access(&predecessor_id).await;
                        }
                        ReconcileDecision::Add => {
                            self.inner
                                .writer
                                .write_fact(pf)
                                .await
                                .map_err(common::KlyntbotError::from)?;
                            report.semantic_writes += 1;
                        }
                        ReconcileDecision::Supersede { predecessor_id } => {
                            let succ_id = pf.fact.id.clone();
                            let succ_valid_from = pf.fact.valid_from.clone();
                            self.inner
                                .writer
                                .write_fact(pf)
                                .await
                                .map_err(common::KlyntbotError::from)?;
                            let _ = self
                                .inner
                                .writer
                                .complete_supersede(&predecessor_id, &succ_id, &succ_valid_from)
                                .await;
                            report.semantic_writes += 1;
                        }
                    }
                }
            }
        }

        self.inner
            .ingest_repo
            .mark_processed_iter(row_ids.iter().copied())
            .await?;
        Ok(report)
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
