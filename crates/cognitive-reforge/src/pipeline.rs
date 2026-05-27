//! The Reforge cycle as an ordered phase pipeline.
//!
//! `run_reforge` drives **14 phase markers** with 3 LLM calls at the handler
//! level (Synthesize, Review, Narrate). The dependencies that used to be 24
//! positional parameters now live in one [`ReforgeContext`]; the per-phase
//! intermediate state (collected data, the two LLM-output pipes, the running
//! [`ReforgeResult`]) lives in [`ReforgeRun`]. Each phase is a small
//! [`Phase`] implementation, so the orchestration is a `Vec<Box<dyn Phase>>`
//! the test surface can step through with fakes.
//!
//! Execution order (the marker numbers are historical, not sequential):
//!
//!   1   Collect
//!   2   Synthesize  [LLM #1]
//!   3   Review      [LLM #2]
//!   2.6 Cross-CLI transfer
//!   4   Narrate     [LLM #3]
//!   5   Apply
//!   3.6 Skill discovery
//!   6   Optimize
//!   6.5 Graph Consolidation
//!   6.5b Community Intelligence
//!   7   Compact
//!
//! Each phase is isolated so a single failure does not abort the remaining
//! phases. The extension hooks are `Option` fields on [`ReforgeContext`] — the
//! cycle degrades gracefully when a handler isn't installed.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
use tracing::{debug, info, warn};

use crate::service::{
    apply_knowledge, apply_skill_edits, build_narrate_input, build_review_input,
    build_synthesize_input, create_trials_from_suggestions, record_knowledge_snapshot,
    run_phase6_autotuner,
};
use crate::skill_files::{SkillFile, SkillFileManager};
use crate::types::*;
use cognitive_memory::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
use cognitive_memory::types::EpisodicMemory;

// ---------------------------------------------------------------------------
// Context: the resolved dependencies (was 24 positional params)
// ---------------------------------------------------------------------------

/// Every dependency the Reforge cycle borrows for the duration of a run.
///
/// The required handlers/repos are bare references; the optional extension
/// hooks are `Option`, and the cycle skips the corresponding phase when one is
/// absent. The two *consumable* inputs (`pre_read_skill_files`, `autotuner_ctx`)
/// are not here — they are moved during Phase 1 and live on [`ReforgeRun`].
pub struct ReforgeContext<'a> {
    pub reforge_state_repo: &'a storage::repos::ReforgeStateRepo,
    pub skill_version_repo: &'a storage::repos::SkillVersionRepo,
    pub session_memory_repo: &'a storage::SessionMemoryRepo,
    pub fact_repo: &'a SemanticFactRepo,
    pub episodic_repo: &'a EpisodicMemoryRepo,
    pub rule_repo: &'a ProceduralRuleRepo,
    pub handler: &'a dyn super::ReforgeHandler,
    pub skill_mgr: &'a SkillFileManager,
    pub mirror_repo: Option<&'a cognitive_mirror::MirrorRepo>,
    pub feedback_repo: Option<&'a storage::RetrievalFeedbackRepo>,
    pub autotuner_bridge: Option<&'a dyn super::AutotunerBridge>,
    pub feedback_sources: Option<&'a super::collector::FeedbackSources<'a>>,
    pub graph_enrichment_handler: Option<&'a dyn super::GraphEnrichmentHandler>,
    pub density_repo: Option<&'a cognitive_memory::repos::ConversationDensityRepo>,
    pub entity_repo: Option<&'a cognitive_memory::repos::EntityRepo>,
    pub snapshot_repo: Option<&'a cognitive_memory::repos::KnowledgeSnapshotRepo>,
    pub community_intelligence_handler: Option<&'a dyn super::CommunityIntelligenceHandler>,
    pub community_repo: Option<&'a cognitive_memory::repos::CommunityRepo>,
    pub co_activation_repo_for_split: Option<&'a cognitive_memory::repos::CoActivationRepo>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub cross_cli_runner: Option<&'a dyn super::CrossCliPhaseRunner>,
    pub skill_discovery_runner: Option<&'a dyn super::SkillDiscoveryRunner>,
}

impl<'a> ReforgeContext<'a> {
    /// Start building a context from the eight always-required dependencies.
    /// Optional phase hooks default to `None`; set them with the chained
    /// setters below. This is the canonical construction path — see
    /// `app-core/src/init/cron.rs` for the production call site.
    #[allow(clippy::too_many_arguments)]
    pub fn builder(
        reforge_state_repo: &'a storage::repos::ReforgeStateRepo,
        skill_version_repo: &'a storage::repos::SkillVersionRepo,
        session_memory_repo: &'a storage::SessionMemoryRepo,
        fact_repo: &'a SemanticFactRepo,
        episodic_repo: &'a EpisodicMemoryRepo,
        rule_repo: &'a ProceduralRuleRepo,
        handler: &'a dyn super::ReforgeHandler,
        skill_mgr: &'a SkillFileManager,
    ) -> ReforgeContextBuilder<'a> {
        ReforgeContextBuilder {
            ctx: ReforgeContext {
                reforge_state_repo,
                skill_version_repo,
                session_memory_repo,
                fact_repo,
                episodic_repo,
                rule_repo,
                handler,
                skill_mgr,
                mirror_repo: None,
                feedback_repo: None,
                autotuner_bridge: None,
                feedback_sources: None,
                graph_enrichment_handler: None,
                density_repo: None,
                entity_repo: None,
                snapshot_repo: None,
                community_intelligence_handler: None,
                community_repo: None,
                co_activation_repo_for_split: None,
                domain_event_bus: None,
                cross_cli_runner: None,
                skill_discovery_runner: None,
            },
        }
    }
}

/// Builder for [`ReforgeContext`]. Required deps are fixed at `builder(...)`;
/// each optional phase hook has a chained setter. Hooks left unset stay `None`
/// and the corresponding phase degrades to a no-op.
pub struct ReforgeContextBuilder<'a> {
    ctx: ReforgeContext<'a>,
}

impl<'a> ReforgeContextBuilder<'a> {
    pub fn mirror_repo(mut self, v: &'a cognitive_mirror::MirrorRepo) -> Self {
        self.ctx.mirror_repo = Some(v);
        self
    }
    pub fn feedback_repo(mut self, v: &'a storage::RetrievalFeedbackRepo) -> Self {
        self.ctx.feedback_repo = Some(v);
        self
    }
    /// Already-`Option` at the call site — accepts the option directly.
    pub fn autotuner_bridge(mut self, v: Option<&'a dyn super::AutotunerBridge>) -> Self {
        self.ctx.autotuner_bridge = v;
        self
    }
    pub fn feedback_sources(mut self, v: &'a super::collector::FeedbackSources<'a>) -> Self {
        self.ctx.feedback_sources = Some(v);
        self
    }
    /// Already-`Option` at the call site — accepts the option directly.
    pub fn graph_enrichment_handler(
        mut self,
        v: Option<&'a dyn super::GraphEnrichmentHandler>,
    ) -> Self {
        self.ctx.graph_enrichment_handler = v;
        self
    }
    pub fn density_repo(mut self, v: &'a cognitive_memory::repos::ConversationDensityRepo) -> Self {
        self.ctx.density_repo = Some(v);
        self
    }
    pub fn entity_repo(mut self, v: &'a cognitive_memory::repos::EntityRepo) -> Self {
        self.ctx.entity_repo = Some(v);
        self
    }
    pub fn snapshot_repo(mut self, v: &'a cognitive_memory::repos::KnowledgeSnapshotRepo) -> Self {
        self.ctx.snapshot_repo = Some(v);
        self
    }
    /// Already-`Option` at the call site — accepts the option directly.
    pub fn community_intelligence_handler(
        mut self,
        v: Option<&'a dyn super::CommunityIntelligenceHandler>,
    ) -> Self {
        self.ctx.community_intelligence_handler = v;
        self
    }
    pub fn community_repo(mut self, v: &'a cognitive_memory::repos::CommunityRepo) -> Self {
        self.ctx.community_repo = Some(v);
        self
    }
    pub fn co_activation_repo_for_split(
        mut self,
        v: &'a cognitive_memory::repos::CoActivationRepo,
    ) -> Self {
        self.ctx.co_activation_repo_for_split = Some(v);
        self
    }
    pub fn domain_event_bus(mut self, v: std::sync::Arc<bus::DomainEventBus>) -> Self {
        self.ctx.domain_event_bus = Some(v);
        self
    }
    pub fn cross_cli_runner(mut self, v: &'a dyn super::CrossCliPhaseRunner) -> Self {
        self.ctx.cross_cli_runner = Some(v);
        self
    }
    pub fn skill_discovery_runner(mut self, v: &'a dyn super::SkillDiscoveryRunner) -> Self {
        self.ctx.skill_discovery_runner = Some(v);
        self
    }
    pub fn build(self) -> ReforgeContext<'a> {
        self.ctx
    }
}

// ---------------------------------------------------------------------------
// Run state: the mutable per-cycle bindings phases hand off between each other
// ---------------------------------------------------------------------------

/// The mutable state threaded through the phase pipeline.
///
/// The two consumable inputs are seeded by the caller before the run; every
/// other field starts empty and is filled by the phase that produces it. The
/// `result` accumulates across all phases and is what `run_reforge` returns.
#[derive(Default)]
pub struct ReforgeRun {
    pub run_id: String,
    pub last_run_at: Option<String>,
    /// Seed input: skill files pre-read by the caller to avoid re-reading.
    pub pre_read_skill_files: Option<HashMap<String, Vec<SkillFile>>>,
    /// Seed input: autotuner context assembled where `metric_source` is available.
    pub autotuner_ctx: Option<AutotunerContext>,
    /// Phase 1 output; the read-only context for Phases 2–5.
    pub collected: Option<ReforgeCollected>,
    /// Phase 1 output: content hashes for Phase 5b conflict detection.
    pub collected_hashes: HashMap<(String, String), String>,
    /// Phase 2 output (LLM #1); consumed by Review, Narrate, Apply.
    pub synthesize_output: Option<SynthesizeOutput>,
    /// Phase 3 output (LLM #2); consumed by Narrate, Apply, Optimize.
    pub review_output: Option<ReviewOutput>,
    /// Phase 4 output (LLM #3); stored as episodic memory in Apply.
    pub narrative: String,
    /// Phase 7 output, folded into the recorded run stats.
    pub compaction_stats: Option<serde_json::Value>,
    pub result: ReforgeResult,
}

impl ReforgeRun {
    /// The `collected` data, which is guaranteed present for every phase after
    /// Collect — the driver short-circuits the run when Collect yields nothing.
    fn collected(&self) -> &ReforgeCollected {
        self.collected
            .as_ref()
            .expect("collected is populated by CollectPhase before later phases run")
    }
}

// ---------------------------------------------------------------------------
// Phase trait
// ---------------------------------------------------------------------------

/// One step of the Reforge cycle. Phases read [`ReforgeContext`] and read/write
/// [`ReforgeRun`]; a failure inside a phase is recorded in `run.result.phase_errors`
/// and never aborts the remaining phases.
#[async_trait]
pub trait Phase: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun);
}

/// The ordered pipeline. Collect is first; the driver bails after it when
/// there is no new data. Marker numbers are historical (see module docs).
pub fn reforge_phases() -> Vec<Box<dyn Phase>> {
    vec![
        Box::new(CollectPhase),
        Box::new(SynthesizePhase),
        Box::new(ReviewPhase),
        Box::new(CrossCliPhase),
        Box::new(NarratePhase),
        Box::new(ApplyPhase),
        Box::new(SkillDiscoveryPhase),
        Box::new(OptimizePhase),
        Box::new(GraphConsolidationPhase),
        Box::new(CommunityIntelligencePhase),
        Box::new(CompactPhase),
    ]
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Run the full Reforge cycle, returning `None` when the collector decides
/// there is nothing new to process.
pub async fn run_reforge(ctx: ReforgeContext<'_>, mut run: ReforgeRun) -> Option<ReforgeResult> {
    run.run_id = uuid::Uuid::new_v4().to_string();

    // Fetch last run timestamp (consumed by Phase 1).
    run.last_run_at = match ctx.reforge_state_repo.get().await {
        Ok(state) => state.last_run_at,
        Err(e) => {
            warn!("Reforge: failed to read reforge_state: {e}");
            None
        }
    };

    for phase in reforge_phases() {
        phase.run(&ctx, &mut run).await;
        // Only Collect (the first phase) can legitimately leave `collected`
        // empty; that means "no new data" and the whole cycle is skipped.
        if run.collected.is_none() {
            info!("Reforge: skipped — no new data");
            return None;
        }
    }

    // Record run in reforge_state (after all phases, including compaction).
    let stats_json = serde_json::json!({
        "facts_added": run.result.facts_added,
        "facts_updated": run.result.facts_updated,
        "facts_stale_flagged": run.result.facts_stale_flagged,
        "rules_added": run.result.rules_added,
        "rules_reinforced": run.result.rules_reinforced,
        "skills_edited": run.result.skills_edited,
        "skipped_skill_edits": run.result.skipped_skill_edits.len(),
        "phase_errors": run.result.phase_errors.len(),
        "trials_created": run.result.trials_created,
        "champion_promoted": run.result.champion_promoted,
        "regression_detected": run.result.regression_detected,
        "suggestions_persisted": run.result.suggestions_persisted,
        "patterns_persisted": run.result.patterns_persisted,
        "communities_renamed": run.result.communities_renamed,
        "communities_merged": run.result.communities_merged,
        "communities_split": run.result.communities_split,
        "compaction": run.compaction_stats,
    });
    if let Err(e) = ctx
        .reforge_state_repo
        .record_run(&stats_json.to_string())
        .await
    {
        warn!("Reforge: failed to record run: {e}");
    }

    info!(
        facts_added = run.result.facts_added,
        facts_updated = run.result.facts_updated,
        rules_added = run.result.rules_added,
        skills_edited = run.result.skills_edited,
        errors = run.result.phase_errors.len(),
        "Reforge cycle complete"
    );

    Some(run.result)
}

// ---------------------------------------------------------------------------
// Phase 1: Collect
// ---------------------------------------------------------------------------

struct CollectPhase;

#[async_trait]
impl Phase for CollectPhase {
    fn name(&self) -> &str {
        "collect"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 1: Collect");
        let collected = super::collector::collect(
            run.last_run_at.as_deref(),
            ctx.session_memory_repo,
            ctx.fact_repo,
            ctx.episodic_repo,
            ctx.rule_repo,
            ctx.skill_mgr,
            run.pre_read_skill_files.take(),
            ctx.mirror_repo,
            ctx.feedback_repo,
            ctx.feedback_sources,
        )
        .await;

        let collected = match collected {
            Some(mut c) => {
                // Inject autotuner context from the cron handler (where metric_source is available).
                c.autotuner_ctx = run.autotuner_ctx.take();
                c
            }
            None => {
                run.collected = None;
                return;
            }
        };

        // Snapshot content hashes at collection time for conflict detection.
        run.collected_hashes = collected
            .skill_files
            .iter()
            .flat_map(|(_, files)| {
                files.iter().map(|f| {
                    (
                        (f.skill_name.clone(), f.file_path.clone()),
                        f.content_hash.clone(),
                    )
                })
            })
            .collect();

        run.collected = Some(collected);
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Synthesize (LLM call #1)
// ---------------------------------------------------------------------------

struct SynthesizePhase;

#[async_trait]
impl Phase for SynthesizePhase {
    fn name(&self) -> &str {
        "synthesize"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 2: Synthesize");
        let synthesize_input = build_synthesize_input(run.collected());
        run.synthesize_output = match ctx.handler.synthesize(&synthesize_input).await {
            Ok(output) => {
                debug!(
                    facts = output.fact_updates.len(),
                    rules = output.rule_updates.len(),
                    stale = output.stale_facts.len(),
                    "Reforge Phase 2 complete"
                );
                Some(output)
            }
            Err(e) => {
                warn!("Reforge Phase 2 failed: {e}");
                run.result.phase_errors.push(format!("synthesize: {e}"));
                None
            }
        };

        // Persist high-confidence cross-session patterns as episodic memories.
        if let Some(ref syn) = run.synthesize_output {
            for pattern in &syn.cross_session_patterns {
                if pattern.confidence >= 0.7 {
                    let mem = EpisodicMemory {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: SOURCE_REFORGE.to_string(),
                        content: pattern.pattern.clone(),
                        summary: Some("Cross-session pattern".to_string()),
                        importance: pattern.confidence,
                        occurred_at: Timestamp::now().to_string(),
                        recorded_at: Timestamp::now().to_string(),
                        stability: 3.0,
                        last_accessed: None,
                        access_count: 0,
                        project_id: None,
                        scope_type: "system".to_string(),
                        scope_id: None,
                        kind: None,
                        scope_repo_id: None,
                        metadata: None,
                        actor_id: None,
                        tier: "raw".to_string(),
                        parent_id: None,
                        child_count: 0,
                        rolled_up_at: None,
                    };
                    if let Err(e) = ctx.episodic_repo.insert(&mem).await {
                        warn!("Reforge: failed to persist cross-session pattern: {e}");
                    } else {
                        run.result.patterns_persisted += 1;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3: Review (LLM call #2)
// ---------------------------------------------------------------------------

struct ReviewPhase;

#[async_trait]
impl Phase for ReviewPhase {
    fn name(&self) -> &str {
        "review"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 3: Review");
        let review_input = build_review_input(run.collected(), &run.synthesize_output);
        run.review_output = match ctx.handler.review(&review_input).await {
            Ok(output) => {
                debug!(
                    skill_edits = output.skill_edits.len(),
                    routing_insights = output.routing_insights.len(),
                    "Reforge Phase 3 complete"
                );
                Some(output)
            }
            Err(e) => {
                warn!("Reforge Phase 3 failed: {e}");
                run.result.phase_errors.push(format!("review: {e}"));
                None
            }
        };

        // Persist context priority suggestions for the next cycle's feedback loop.
        if let Some(ref review) = run.review_output {
            if let Some(repo) = ctx.feedback_sources.and_then(|fb| fb.suggestion_repo) {
                let now = Timestamp::now().to_string();
                for suggestion in &review.context_priority_suggestions {
                    let row = storage::repos::reforge_suggestion::ReforgeSuggestionRow {
                        id: uuid::Uuid::new_v4().to_string(),
                        suggestion_type: "context_priority".to_string(),
                        content: suggestion.suggestion.clone(),
                        reason: suggestion.reason.clone(),
                        confidence: 0.8,
                        cycle_run_at: now.clone(),
                        acted_upon: false,
                        created_at: now.clone(),
                    };
                    if let Err(e) = repo.insert(&row).await {
                        warn!("Reforge: failed to persist context priority suggestion: {e}");
                    } else {
                        run.result.suggestions_persisted += 1;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2.6: Cross-CLI transfer (KCA Track 10)
// ---------------------------------------------------------------------------

struct CrossCliPhase;

#[async_trait]
impl Phase for CrossCliPhase {
    fn name(&self) -> &str {
        "cross_cli"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        if let Some(runner) = ctx.cross_cli_runner {
            info!("Reforge Phase 2.6: Cross-CLI transfer");
            match runner.run_cross_cli_transfer(&run.run_id).await {
                Ok(promoted) => {
                    debug!(promoted, "Phase 2.6 complete");
                    run.result.cross_cli_promoted = promoted;
                }
                Err(e) => {
                    warn!("Phase 2.6 failed: {e}");
                    run.result.phase_errors.push(format!("phase 2.6: {e}"));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 4: Narrate (LLM call #3)
// ---------------------------------------------------------------------------

struct NarratePhase;

#[async_trait]
impl Phase for NarratePhase {
    fn name(&self) -> &str {
        "narrate"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 4: Narrate");
        let narrate_input = build_narrate_input(&run.synthesize_output, &run.review_output);
        let narrative = match ctx.handler.narrate(&narrate_input).await {
            Ok(text) => {
                debug!(len = text.len(), "Reforge Phase 4 complete");
                text
            }
            Err(e) => {
                warn!("Reforge Phase 4 failed: {e}");
                run.result.phase_errors.push(format!("narrate: {e}"));
                "Reforge cycle completed with partial results.".to_string()
            }
        };
        run.result.narrative = narrative.clone();
        run.narrative = narrative;
    }
}

// ---------------------------------------------------------------------------
// Phase 5: Apply
// ---------------------------------------------------------------------------

struct ApplyPhase;

#[async_trait]
impl Phase for ApplyPhase {
    fn name(&self) -> &str {
        "apply"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 5: Apply");

        // 5a. Apply knowledge (facts + rules) from Phase 2.
        if let Some(ref syn) = run.synthesize_output {
            apply_knowledge(syn, ctx.fact_repo, ctx.rule_repo, &mut run.result).await;
        }

        // 5b. Apply skill edits from Phase 3.
        if let Some(ref rev) = run.review_output {
            apply_skill_edits(
                &rev.skill_edits,
                &run.collected_hashes,
                ctx.skill_mgr,
                ctx.skill_version_repo,
                &mut run.result,
            )
            .await;
        }

        // 5c. Store narrative as episodic memory.
        let narrative_mem = EpisodicMemory {
            id: uuid::Uuid::new_v4().to_string(),
            domain: SOURCE_REFORGE.to_string(),
            content: run.narrative.clone(),
            summary: Some("Reforge cycle narrative".to_string()),
            importance: 0.9,
            occurred_at: Timestamp::now().to_string(),
            recorded_at: Timestamp::now().to_string(),
            stability: 5.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            kind: None,
            scope_repo_id: None,
            metadata: None,
            actor_id: None,
            tier: "raw".to_string(),
            parent_id: None,
            child_count: 0,
            rolled_up_at: None,
        };
        if let Err(e) = ctx.episodic_repo.insert(&narrative_mem).await {
            warn!("Reforge: failed to store narrative memory: {e}");
            run.result
                .phase_errors
                .push(format!("narrative_store: {e}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3.6: Skill discovery (KCA Track 12)
// ---------------------------------------------------------------------------

struct SkillDiscoveryPhase;

#[async_trait]
impl Phase for SkillDiscoveryPhase {
    fn name(&self) -> &str {
        "skill_discovery"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        if let Some(runner) = ctx.skill_discovery_runner {
            info!("Reforge Phase 3.6: Skill discovery");
            match runner.run_skill_discovery(&run.run_id).await {
                Ok(proposed) => {
                    debug!(proposed, "Phase 3.6 complete");
                    run.result.skills_proposed = proposed;
                }
                Err(e) => {
                    warn!("Phase 3.6 failed: {e}");
                    run.result.phase_errors.push(format!("phase 3.6: {e}"));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 6: Optimize
// ---------------------------------------------------------------------------

struct OptimizePhase;

#[async_trait]
impl Phase for OptimizePhase {
    fn name(&self) -> &str {
        "optimize"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 6: Optimize");
        if let Some(bridge) = ctx.autotuner_bridge {
            // Step 1: Evaluate existing trials
            match run_phase6_autotuner(bridge).await {
                Ok(eval) => {
                    run.result.champion_promoted = eval.promoted;
                    run.result.regression_detected = eval.regression;
                    if let Some(ref summary) = eval.promotion_summary {
                        info!("Reforge Phase 6: {summary}");
                    }
                    if eval.regression {
                        warn!("Reforge Phase 6: champion regression detected");
                    }
                    if !eval.failed_constraints.is_empty() {
                        debug!(
                            "Reforge Phase 6: {} trial(s) failed constraints: {}",
                            eval.failed_constraints.len(),
                            eval.failed_constraints.join("; "),
                        );
                    }
                    debug!(
                        evaluated = eval.evaluated_count,
                        promoted = eval.promoted,
                        regression = eval.regression,
                        "Reforge Phase 6 evaluation complete"
                    );
                }
                Err(e) => {
                    warn!("Reforge Phase 6 evaluation failed: {e}");
                    run.result
                        .phase_errors
                        .push(format!("optimize/evaluate: {e}"));
                }
            }

            // Step 2: Create new trials from Phase 3 suggestions
            if let Some(ref review) = run.review_output {
                let created =
                    create_trials_from_suggestions(&review.trial_suggestions, bridge).await;
                run.result.trials_created = created;
            }
        } else {
            debug!("Reforge Phase 6: skipped (no autotuner bridge)");
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 6.5: Graph Consolidation
// ---------------------------------------------------------------------------

struct GraphConsolidationPhase;

#[async_trait]
impl Phase for GraphConsolidationPhase {
    fn name(&self) -> &str {
        "graph_consolidation"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        if let (Some(enricher), Some(density_repo), Some(entity_repo)) = (
            ctx.graph_enrichment_handler,
            ctx.density_repo,
            ctx.entity_repo,
        ) {
            info!("Reforge Phase 6.5: Graph Consolidation");

            // Step 1: Load medium-density turns queued since last cycle
            let pending_turns = match density_repo.load_pending_medium(50).await {
                Ok(turns) => turns,
                Err(e) => {
                    warn!("Reforge Phase 6.5: failed to load pending turns: {e}");
                    run.result
                        .phase_errors
                        .push(format!("graph_consolidation/load: {e}"));
                    Vec::new()
                }
            };

            // Step 2: Find duplicate entity candidates
            let dup_candidates = match entity_repo.find_duplicate_candidates(30).await {
                Ok(dups) => dups,
                Err(e) => {
                    warn!("Reforge Phase 6.5: failed to find duplicates: {e}");
                    Vec::new()
                }
            };

            // Step 3: Run LLM enrichment (single call) if there's work to do
            if !pending_turns.is_empty() || !dup_candidates.is_empty() {
                let input = cognitive_memory::services::graph_enrichment::GraphEnrichmentInput {
                    turn_previews: pending_turns
                        .iter()
                        .map(|t| t.content_preview.clone())
                        .collect(),
                    duplicate_candidates: dup_candidates
                        .iter()
                        .map(|(a_id, b_id, a_name, b_name)| {
                            cognitive_memory::services::graph_enrichment::DuplicateCandidate {
                                entity_a_id: a_id.clone(),
                                entity_b_id: b_id.clone(),
                                entity_a_name: a_name.clone(),
                                entity_b_name: b_name.clone(),
                            }
                        })
                        .collect(),
                };

                match enricher.enrich_graph(&input).await {
                    Ok(output) => {
                        // Apply merge decisions
                        for decision in &output.merge_decisions {
                            if decision.should_merge {
                                if let Err(e) = entity_repo
                                    .merge_entities(&decision.entity_a_id, &decision.entity_b_id)
                                    .await
                                {
                                    debug!("Phase 6.5: merge failed: {e}");
                                } else {
                                    run.result.entities_merged += 1;
                                }
                            }
                        }

                        // Apply discovered relationships
                        for rel in &output.discovered_relationships {
                            let source_entities = entity_repo
                                .find_by_name(&rel.source_entity_name)
                                .await
                                .unwrap_or_default();
                            let target_entities = entity_repo
                                .find_by_name(&rel.target_entity_name)
                                .await
                                .unwrap_or_default();

                            if let (Some(src), Some(tgt)) =
                                (source_entities.first(), target_entities.first())
                            {
                                let new_rel = cognitive_memory::repos::entity::NewRelationship {
                                    source_entity_id: src.id.clone(),
                                    target_entity_id: tgt.id.clone(),
                                    relationship_type: rel.relationship_type.clone(),
                                    evidence: None,
                                    source: "reforge_phase_6.5".to_string(),
                                };
                                if entity_repo.upsert_relationship(&new_rel).await.is_ok() {
                                    run.result.relationships_discovered += 1;
                                }
                            }
                        }

                        // Mark processed turns as enriched
                        let turn_ids: Vec<String> =
                            pending_turns.iter().map(|t| t.id.clone()).collect();
                        if let Err(e) = density_repo.mark_enriched(&turn_ids).await {
                            debug!("Phase 6.5: mark_enriched failed: {e}");
                        }

                        info!(
                            merged = run.result.entities_merged,
                            relationships = run.result.relationships_discovered,
                            turns_processed = pending_turns.len(),
                            "Reforge Phase 6.5 complete"
                        );
                    }
                    Err(e) => {
                        warn!("Reforge Phase 6.5 enrichment failed: {e}");
                        run.result
                            .phase_errors
                            .push(format!("graph_consolidation/enrich: {e}"));
                    }
                }
            } else {
                debug!("Reforge Phase 6.5: nothing to consolidate");
            }

            // Step 4: Record knowledge snapshot
            if let Some(snapshot_repo) = ctx.snapshot_repo {
                if let Err(e) =
                    record_knowledge_snapshot(entity_repo, ctx.fact_repo, snapshot_repo).await
                {
                    debug!("Phase 6.5: snapshot failed: {e}");
                } else {
                    run.result.snapshot_recorded = true;
                }
            }
        } else {
            debug!("Reforge Phase 6.5: skipped (missing repos or handler)");
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 6.5b: Community Intelligence — LLM naming, merge, split
// ---------------------------------------------------------------------------

struct CommunityIntelligencePhase;

#[async_trait]
impl Phase for CommunityIntelligencePhase {
    fn name(&self) -> &str {
        "community_intelligence"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        if let (Some(ci_handler), Some(community_repo)) =
            (ctx.community_intelligence_handler, ctx.community_repo)
        {
            match cognitive_memory::services::community_intelligence::build_intelligence_input(
                community_repo,
            )
            .await
            {
                Ok(input) if !input.communities.is_empty() => {
                    match ci_handler.analyze_communities(&input).await {
                        Ok(output) => {
                            let (renamed, merged, split_count) = if let Some(co_act) =
                                ctx.co_activation_repo_for_split
                            {
                                cognitive_memory::services::community_intelligence::apply_intelligence(
                                    &output,
                                    community_repo,
                                    co_act,
                                    ctx.domain_event_bus.clone(),
                                )
                                .await
                            } else {
                                // No co-activation repo — can rename/merge but not split
                                let no_splits =
                                    cognitive_memory::services::community_intelligence::CommunityIntelligenceOutput {
                                        names: output.names.clone(),
                                        merges: output.merges.clone(),
                                        splits: Vec::new(),
                                    };
                                let fallback_co_act =
                                    cognitive_memory::repos::CoActivationRepo::new(
                                        community_repo.pool().clone(),
                                    );
                                cognitive_memory::services::community_intelligence::apply_intelligence(
                                    &no_splits,
                                    community_repo,
                                    &fallback_co_act,
                                    ctx.domain_event_bus.clone(),
                                )
                                .await
                            };
                            run.result.communities_renamed = renamed;
                            run.result.communities_merged = merged;
                            run.result.communities_split = split_count;
                            info!(
                                renamed,
                                merged,
                                split = split_count,
                                "Phase 6.5b: community intelligence complete"
                            );
                        }
                        Err(e) => {
                            warn!("Phase 6.5b community intelligence failed: {e}");
                            run.result
                                .phase_errors
                                .push(format!("community_intelligence: {e}"));
                        }
                    }
                }
                Ok(_) => {
                    debug!("Phase 6.5b: no active communities for intelligence");
                }
                Err(e) => {
                    debug!("Phase 6.5b: failed to build community input: {e}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 7: Compact
// ---------------------------------------------------------------------------

struct CompactPhase;

#[async_trait]
impl Phase for CompactPhase {
    fn name(&self) -> &str {
        "compact"
    }

    async fn run(&self, ctx: &ReforgeContext<'_>, run: &mut ReforgeRun) {
        info!("Reforge Phase 7: Compact");
        match cognitive_memory::services::compaction::run_compaction(
            ctx.fact_repo,
            ctx.episodic_repo,
            Some(ctx.rule_repo),
            None,
            None,
            Some(ctx.session_memory_repo),
            None,
            None,
        )
        .await
        {
            Ok(cr) => {
                debug!(
                    facts_archived = cr.facts_archived,
                    episodic_deleted = cr.episodic_deleted,
                    rules_deactivated = cr.rules_deactivated,
                    "Reforge Phase 7 complete"
                );
                run.compaction_stats = Some(serde_json::json!({
                    "facts_archived": cr.facts_archived,
                    "episodic_deleted": cr.episodic_deleted,
                    "low_stability_archived": cr.low_stability_archived,
                    "rules_deactivated": cr.rules_deactivated,
                }));
            }
            Err(e) => {
                warn!("Reforge Phase 7 failed: {e}");
                run.result.phase_errors.push(format!("compact: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod pipeline_tests;
