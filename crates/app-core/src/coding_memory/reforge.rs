//! App-core wiring for Phase 5 Reforge integration.
//!
//! - Dispatches `DomainEvent::CodingSessionEnded` into `SessionEndPass`.
//! - Builds `CodingPhaseHandlers` for the cron `run_reforge` call.

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use coding_memory::reforge::types::CodingPhaseHandlers;
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisPhase, CrossSessionDedup, RuleArtifactGenerationPhase,
    RuleArtifactsHandler, SelectiveDeleteSignal, SessionEndPass,
};
use cognitive::services::reforge::CodingPhaseRunnerOutcome;
use std::sync::Arc;
use tracing::warn;

/// Subscribe `SessionEndPass` to `DomainEvent::CodingSessionEnded`.
pub async fn register_session_end_dispatch(bus: Arc<DomainEventBus>, pass: Arc<SessionEndPass>) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let DomainEvent::CodingSessionEnded {
                session_id,
                repo_id,
            } = event
            {
                let pass = pass.clone();
                tokio::spawn(async move {
                    if let Err(e) = pass.run(&session_id, repo_id.as_deref()).await {
                        warn!("SessionEndPass failed for {session_id}: {e}");
                    }
                });
            }
        }
    });
}

// ---------------------------------------------------------------------------
// CodingPhaseRunnerImpl — concrete bridge from cognitive trait to coding-memory phases
// ---------------------------------------------------------------------------

/// Concrete runner that satisfies the cognitive trait via the coding-memory phases.
pub struct CodingPhaseRunnerImpl {
    pool: storage::StoragePool,
    fact_repo: cognitive::SemanticFactRepo,
    episodic_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
    co_activation_repo: cognitive::CoActivationRepo,
    utilization_repo: coding_memory::recall::telemetry::RecallInvocationRepo,
    session_summary_repo: coding_memory::reforge::SessionSummaryRepo,
    selective_delete_log: coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo,
    pattern_effectiveness_log:
        coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo,
    synthesis_handler: Option<Arc<dyn CodingSynthesisHandler>>,
    rule_artifacts_handler: Option<Arc<dyn RuleArtifactsHandler>>,
    enabled_artifacts: Vec<String>,
    bus: Option<Arc<bus::DomainEventBus>>,
    cross_session_dedup_threshold: f32,
    selective_delete_threshold: u32,
    #[allow(dead_code)]
    symbol_extractor: Option<Arc<dyn coding_memory::symbols::SymbolExtractor>>,
    #[allow(dead_code)]
    repo_roots: std::collections::HashMap<String, std::path::PathBuf>,
    #[allow(dead_code)]
    causal_repo: Option<Arc<coding_memory::causal::CausalEdgeRepo>>,
}

impl CodingPhaseRunnerImpl {
    /// Production constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: storage::StoragePool,
        synthesis_handler: Option<Arc<dyn CodingSynthesisHandler>>,
        rule_artifacts_handler: Option<Arc<dyn RuleArtifactsHandler>>,
        enabled_artifacts: Vec<String>,
        bus: Option<Arc<bus::DomainEventBus>>,
        cross_session_dedup_threshold: f32,
        selective_delete_threshold: u32,
    ) -> Self {
        let db = pool.inner().clone();
        let fact_repo = cognitive::SemanticFactRepo::new(db.clone());
        let episodic_repo = cognitive::EpisodicMemoryRepo::new(db.clone());
        let rule_repo = cognitive::ProceduralRuleRepo::new(db.clone());
        let co_activation_repo = cognitive::CoActivationRepo::new(db.clone());
        let utilization_repo =
            coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
        let session_summary_repo = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
        let selective_delete_log =
            coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
        let pattern_effectiveness_log =
            coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(
                pool.clone(),
            );
        Self {
            pool,
            fact_repo,
            episodic_repo,
            rule_repo,
            co_activation_repo,
            utilization_repo,
            session_summary_repo,
            selective_delete_log,
            pattern_effectiveness_log,
            synthesis_handler,
            rule_artifacts_handler,
            enabled_artifacts,
            bus,
            cross_session_dedup_threshold,
            selective_delete_threshold,
            symbol_extractor: None,
            repo_roots: Default::default(),
            causal_repo: None,
        }
    }

    /// Test constructor — no LLM handlers, no enabled artifacts.
    pub fn new_for_test(pool: storage::StoragePool) -> Self {
        Self::new(pool, None, None, vec![], None, 0.92, 5)
    }

    /// Attach a symbol extractor (Phase-6 wiring).
    #[must_use]
    pub fn with_symbol_extractor(
        mut self,
        e: Option<Arc<dyn coding_memory::symbols::SymbolExtractor>>,
    ) -> Self {
        self.symbol_extractor = e;
        self
    }

    /// Attach repo roots (Phase-6 symbol validation wiring).
    #[must_use]
    pub fn with_repo_roots(
        mut self,
        r: std::collections::HashMap<String, std::path::PathBuf>,
    ) -> Self {
        self.repo_roots = r;
        self
    }

    /// Attach the causal edge repo (Phase-6 wiring).
    #[must_use]
    pub fn with_causal_repo(
        mut self,
        r: Option<Arc<coding_memory::causal::CausalEdgeRepo>>,
    ) -> Self {
        self.causal_repo = r;
        self
    }

    fn handlers(&self) -> CodingPhaseHandlers<'_> {
        CodingPhaseHandlers {
            synthesis: self.synthesis_handler.as_deref(),
            rule_artifacts: self.rule_artifacts_handler.as_deref(),
            fact_repo: &self.fact_repo,
            episodic_repo: &self.episodic_repo,
            rule_repo: &self.rule_repo,
            co_activation_repo: &self.co_activation_repo,
            utilization_repo: &self.utilization_repo,
            session_summary_repo: &self.session_summary_repo,
            selective_delete_log: &self.selective_delete_log,
            pattern_effectiveness_log: &self.pattern_effectiveness_log,
            bus: self.bus.clone(),
            causal_repo: self.causal_repo.as_deref(),
            symbol_extractor: self.symbol_extractor.as_deref(),
            repo_roots: &self.repo_roots,
        }
    }

    /// Return the optional causal repo (for wiring into other components).
    pub fn causal_repo(&self) -> Option<&Arc<coding_memory::causal::CausalEdgeRepo>> {
        self.causal_repo.as_ref()
    }
}

#[async_trait]
impl cognitive::services::reforge::CodingPhaseRunner for CodingPhaseRunnerImpl {
    async fn run_synthesis(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = CodingSynthesisPhase::run(&self.handlers()).await?;
        Ok(CodingPhaseRunnerOutcome {
            applied,
            narrative: None,
        })
    }

    async fn run_rule_artifacts(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied =
            RuleArtifactGenerationPhase::run(&self.handlers(), &self.enabled_artifacts).await?;
        Ok(CodingPhaseRunnerOutcome {
            applied,
            narrative: None,
        })
    }

    async fn run_cross_session_dedup(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied =
            CrossSessionDedup::run(&self.fact_repo, self.cross_session_dedup_threshold, None)
                .await?;
        Ok(CodingPhaseRunnerOutcome {
            applied,
            narrative: None,
        })
    }

    async fn run_selective_delete(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = SelectiveDeleteSignal::apply_with_threshold(
            &self.pool,
            &self.selective_delete_log,
            self.selective_delete_threshold,
        )
        .await?;
        Ok(CodingPhaseRunnerOutcome {
            applied,
            narrative: None,
        })
    }

    async fn run_symbol_validation(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let Some(extractor) = self.symbol_extractor.clone() else {
            return Err(common::KlyntbotError::Storage(
                "symbol_extractor not wired".into(),
            ));
        };
        let causal_repo = self.causal_repo.clone().unwrap_or_else(|| {
            Arc::new(coding_memory::causal::CausalEdgeRepo::new(
                self.pool.clone(),
            ))
        });
        let phase = coding_memory::reforge::SymbolValidationPhase::new(
            Arc::new(self.fact_repo.clone()),
            Arc::new(self.episodic_repo.clone()),
            extractor,
            self.repo_roots.clone(),
            causal_repo,
        );
        let outcome = phase.run().await?;
        Ok(CodingPhaseRunnerOutcome {
            applied: outcome.invalidated + outcome.marked_stale,
            narrative: Some(format!(
                "invalidated={}, stale={}, untouched={}",
                outcome.invalidated, outcome.marked_stale, outcome.untouched
            )),
        })
    }
}
