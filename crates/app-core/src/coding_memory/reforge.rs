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
    cross_cli_handler: Option<Arc<agent::adapters::reforge_handlers::LlmCrossCliSynthesisHandler>>,
    skill_discovery_handler: Option<Arc<agent::adapters::reforge_handlers::LlmSkillDiscoveryHandler>>,
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
            cross_cli_handler: None,
            skill_discovery_handler: None,
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

    /// Attach the cross-CLI synthesis handler (KCA Track 10).
    #[must_use]
    pub fn with_cross_cli_handler(
        mut self,
        h: Option<Arc<agent::adapters::reforge_handlers::LlmCrossCliSynthesisHandler>>,
    ) -> Self {
        self.cross_cli_handler = h;
        self
    }

    /// Attach the skill discovery handler (KCA Track 12).
    #[must_use]
    pub fn with_skill_discovery_handler(
        mut self,
        h: Option<Arc<agent::adapters::reforge_handlers::LlmSkillDiscoveryHandler>>,
    ) -> Self {
        self.skill_discovery_handler = h;
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

#[async_trait::async_trait]
impl cognitive::services::reforge::CrossCliPhaseRunner for CodingPhaseRunnerImpl {
    async fn run_cross_cli_transfer(&self, run_id: &str) -> common::Result<u32> {
        let candidates = coding_memory::reforge::cross_cli_synthesis::find_transferable_rules(
            &self.rule_repo,
            &self.episodic_repo,
            0.7,
        )
        .await?;
        if candidates.is_empty() {
            return Ok(0);
        }

        let handler = match self.cross_cli_handler.as_ref() {
            Some(h) => h,
            None => return Ok(0),
        };

        let decision = handler.confirm_promotions(candidates.clone()).await?;
        let mut promoted = 0u32;
        for cand in &candidates {
            let approved = decision.approved_rule_ids.contains(&cand.rule_id);
            let from_sources_json =
                serde_json::to_string(&cand.supporting_sources).unwrap_or_default();
            let promoted_to_json = if approved {
                from_sources_json.clone()
            } else {
                "[]".into()
            };
            let decision_str = if approved { "approved" } else { "rejected" };
            let log_id = uuid::Uuid::new_v4().to_string();
            sqlx::query!(
                r#"INSERT INTO cross_cli_transfer_log (id, rule_id, rule_text_snapshot, from_sources, promoted_to_sources, support_strength, decision, reforge_run_id)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                log_id,
                cand.rule_id,
                cand.rule_text,
                from_sources_json,
                promoted_to_json,
                cand.support_strength as i64,
                decision_str,
                run_id
            )
            .execute(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

            if approved {
                let mut all = self
                    .rule_repo
                    .list_observed_sources(&cand.rule_id)
                    .await
                    .unwrap_or_default();
                for s in &cand.supporting_sources {
                    if !all.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                        all.push(s.clone());
                    }
                }
                let refs: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
                self.rule_repo
                    .set_observed_sources(&cand.rule_id, &refs)
                    .await?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }
}

#[async_trait::async_trait]
impl cognitive::services::reforge::SkillDiscoveryRunner for CodingPhaseRunnerImpl {
    async fn run_skill_discovery(&self, run_id: &str) -> common::Result<u32> {
        use cognitive::services::reforge::skill_discovery::*;

        let clusters = cluster_rules_for_skill_discovery(&self.rule_repo, 0.4).await?;
        if clusters.is_empty() {
            return Ok(0);
        }

        // Resolve rule_text per cluster.
        let mut clusters_with_text = Vec::new();
        for c in clusters.into_iter().filter(|c| c.avg_confidence >= 0.7) {
            let mut texts = Vec::new();
            for id in &c.rule_ids {
                if let Ok(Some(r)) = self.rule_repo.find_by_id(id).await {
                    texts.push(r.rule_text);
                }
            }
            clusters_with_text.push((c, texts));
        }

        let handler = match self.skill_discovery_handler.as_ref() {
            Some(h) => h.clone(),
            None => return Ok(0),
        };
        let out = handler.propose_skills(clusters_with_text).await?;

        let mut written = 0u32;
        for s in &out.skills {
            // Dedup against pending or approved with same name.
            let exists = sqlx::query!(
                "SELECT id FROM skill_proposals WHERE name = ?1 AND status IN ('pending', 'approved') LIMIT 1",
                s.name
            )
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            if exists.is_some() {
                continue;
            }

            let id = uuid::Uuid::new_v4().to_string();
            let source_ids = serde_json::to_string(&s.source_rule_ids).unwrap_or_default();
            sqlx::query!(
                r#"INSERT INTO skill_proposals (id, name, description, yaml_frontmatter, body_markdown, source_rule_ids, avg_confidence, status, reforge_run_id)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)"#,
                id,
                s.name,
                s.description,
                s.yaml_frontmatter,
                s.body_markdown,
                source_ids,
                s.avg_confidence,
                run_id
            )
            .execute(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            written += 1;
        }
        Ok(written)
    }
}
