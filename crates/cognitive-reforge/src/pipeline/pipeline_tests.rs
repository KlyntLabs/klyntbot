//! Test surface for the Reforge phase pipeline. The harness owns an in-memory
//! SQLite pool + the eight required repos + a scriptable fake handler, and lends
//! a borrowed `ReforgeContext` so individual phases (or the whole `run_reforge`
//! driver) can be exercised with fakes.

use std::sync::Mutex;

use async_trait::async_trait;

use super::*; // ReforgeContext, ReforgeRun, Phase impls (private), run_reforge, etc.
use crate::types::*;

// --- Scriptable fake handler (the 3 LLM seams) -----------------------------

#[derive(Default)]
pub(super) struct FakeReforgeHandler {
    pub synthesize_out: Mutex<Option<SynthesizeOutput>>,
    pub review_out: Mutex<Option<ReviewOutput>>,
    pub narrate_out: Mutex<Option<String>>,
    /// When true, every method returns an error (exercises phase error isolation).
    pub fail: bool,
}

impl FakeReforgeHandler {
    fn empty_synth() -> SynthesizeOutput {
        SynthesizeOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            stale_facts: vec![],
            cross_session_patterns: vec![],
            extraction_quality_flag: None,
        }
    }
}

#[async_trait]
impl crate::ReforgeHandler for FakeReforgeHandler {
    async fn synthesize(&self, _: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        if self.fail {
            return Err(common::KlyntbotError::Storage("fake synth fail".into()));
        }
        Ok(self
            .synthesize_out
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(Self::empty_synth))
    }
    async fn review(&self, _: &ReviewInput) -> common::Result<ReviewOutput> {
        if self.fail {
            return Err(common::KlyntbotError::Storage("fake review fail".into()));
        }
        Ok(self.review_out.lock().unwrap().clone().unwrap_or_default())
    }
    async fn narrate(&self, _: &NarrateInput) -> common::Result<String> {
        if self.fail {
            return Err(common::KlyntbotError::Storage("fake narrate fail".into()));
        }
        Ok(self
            .narrate_out
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "fake narrative".to_string()))
    }
}

// --- Harness ---------------------------------------------------------------

pub(super) struct ReforgeTestHarness {
    pub reforge_state: storage::repos::ReforgeStateRepo,
    pub skill_version: storage::repos::SkillVersionRepo,
    pub session_memory: storage::SessionMemoryRepo,
    pub fact_repo: SemanticFactRepo,
    pub episodic_repo: EpisodicMemoryRepo,
    pub rule_repo: ProceduralRuleRepo,
    pub skill_mgr: SkillFileManager,
    pub handler: FakeReforgeHandler,
    _skills_dir: tempfile::TempDir,
}

impl ReforgeTestHarness {
    pub async fn new() -> Self {
        let pool = cognitive_schema::cognitive_test_pool().await;
        let skills_dir = tempfile::tempdir().unwrap();
        Self {
            reforge_state: storage::repos::ReforgeStateRepo::new(pool.clone()),
            skill_version: storage::repos::SkillVersionRepo::new(pool.clone()),
            session_memory: storage::SessionMemoryRepo::new(pool.clone()),
            fact_repo: SemanticFactRepo::new(pool.clone()),
            episodic_repo: EpisodicMemoryRepo::new(pool.clone()),
            rule_repo: ProceduralRuleRepo::new(pool.clone()),
            skill_mgr: SkillFileManager::new(skills_dir.path().to_path_buf()),
            handler: FakeReforgeHandler::default(),
            _skills_dir: skills_dir,
        }
    }

    /// A builder pre-loaded with the required deps; tests chain optional hooks.
    pub fn ctx_builder(&self) -> ReforgeContextBuilder<'_> {
        ReforgeContext::builder(
            &self.reforge_state,
            &self.skill_version,
            &self.session_memory,
            &self.fact_repo,
            &self.episodic_repo,
            &self.rule_repo,
            &self.handler,
            &self.skill_mgr,
        )
    }

    /// A context with all optional hooks `None`.
    pub fn ctx(&self) -> ReforgeContext<'_> {
        self.ctx_builder().build()
    }
}

// --- Smoke test ------------------------------------------------------------

#[tokio::test]
async fn harness_builds_context_with_optionals_none() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    assert!(ctx.mirror_repo.is_none());
    assert!(ctx.cross_cli_runner.is_none());
}

// --- Layer-1: LLM-handler phases ------------------------------------------------

/// Minimal non-empty collected data so phases past Collect can run.
fn seed_collected(run: &mut ReforgeRun) {
    // ReforgeCollected derives Default; empty collections are enough for the
    // handler phases, which only read what the fake returns.
    run.collected = Some(ReforgeCollected::default());
}

#[tokio::test]
async fn synthesize_populates_output_from_handler() {
    let h = ReforgeTestHarness::new().await;
    *h.handler.synthesize_out.lock().unwrap() = Some(FakeReforgeHandler::empty_synth());
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    SynthesizePhase.run(&ctx, &mut run).await;

    assert!(run.synthesize_output.is_some());
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn synthesize_records_phase_error_on_handler_failure() {
    let mut h = ReforgeTestHarness::new().await;
    h.handler.fail = true;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    SynthesizePhase.run(&ctx, &mut run).await;

    assert!(run.synthesize_output.is_none());
    assert_eq!(run.result.phase_errors.len(), 1);
    assert!(run.result.phase_errors[0].starts_with("synthesize:"));
}

#[tokio::test]
async fn review_populates_output_from_handler() {
    let h = ReforgeTestHarness::new().await;
    *h.handler.review_out.lock().unwrap() = Some(ReviewOutput::default());
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    ReviewPhase.run(&ctx, &mut run).await;

    assert!(run.review_output.is_some());
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn narrate_sets_narrative_and_falls_back_on_error() {
    // success path
    let h = ReforgeTestHarness::new().await;
    *h.handler.narrate_out.lock().unwrap() = Some("hello".to_string());
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);
    NarratePhase.run(&ctx, &mut run).await;
    assert_eq!(run.result.narrative, "hello");
    assert_eq!(run.narrative, "hello");

    // failure path → fallback string + recorded error
    let mut h2 = ReforgeTestHarness::new().await;
    h2.handler.fail = true;
    let ctx2 = h2.ctx();
    let mut run2 = ReforgeRun::default();
    seed_collected(&mut run2);
    NarratePhase.run(&ctx2, &mut run2).await;
    assert!(run2.narrative.contains("partial results"));
    assert!(run2.result.phase_errors.iter().any(|e| e.starts_with("narrate:")));
}

// --- Layer-1: None-guard phases -------------------------------------------------

struct FakeCrossCli(u32);
#[async_trait]
impl crate::CrossCliPhaseRunner for FakeCrossCli {
    async fn run_cross_cli_transfer(&self, _: &str) -> common::Result<u32> {
        Ok(self.0)
    }
}

struct FakeSkillDiscovery(u32);
#[async_trait]
impl crate::SkillDiscoveryRunner for FakeSkillDiscovery {
    async fn run_skill_discovery(&self, _: &str) -> common::Result<u32> {
        Ok(self.0)
    }
}

#[tokio::test]
async fn cross_cli_skips_when_runner_absent() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx(); // cross_cli_runner = None
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    CrossCliPhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.cross_cli_promoted, 0);
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn skill_discovery_skips_when_runner_absent() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    SkillDiscoveryPhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.skills_proposed, 0);
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn optimize_skips_when_bridge_absent() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx(); // autotuner_bridge = None
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    OptimizePhase.run(&ctx, &mut run).await;

    assert!(!run.result.champion_promoted);
    assert!(!run.result.regression_detected);
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn graph_consolidation_skips_when_handler_absent() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx(); // graph_enrichment_handler = None
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    GraphConsolidationPhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.entities_merged, 0);
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn community_intelligence_skips_when_handler_absent() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx(); // community_intelligence_handler = None
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    CommunityIntelligencePhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.communities_renamed, 0);
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn cross_cli_records_promoted_when_runner_present() {
    let h = ReforgeTestHarness::new().await;
    let runner = FakeCrossCli(3);
    let ctx = h.ctx_builder().cross_cli_runner(&runner).build();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    CrossCliPhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.cross_cli_promoted, 3);
}

#[tokio::test]
async fn skill_discovery_records_proposed_when_runner_present() {
    let h = ReforgeTestHarness::new().await;
    let runner = FakeSkillDiscovery(2);
    let ctx = h.ctx_builder().skill_discovery_runner(&runner).build();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);

    SkillDiscoveryPhase.run(&ctx, &mut run).await;

    assert_eq!(run.result.skills_proposed, 2);
}

// --- Layer-1: Apply + Collect ---------------------------------------------------

#[tokio::test]
async fn apply_stores_narrative_as_episodic_memory() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);
    run.narrative = "cycle narrative".to_string();

    ApplyPhase.run(&ctx, &mut run).await;

    // The narrative episodic is recorded under the reforge domain.
    let mems = h
        .episodic_repo
        .list_by_domain("reforge", 10)
        .await
        .unwrap();
    assert!(!mems.is_empty(), "expected a reforge-domain narrative memory");
    assert!(run.result.phase_errors.is_empty());
}

#[tokio::test]
async fn collect_yields_none_on_empty_db() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    // Set a non-bootstrap last_run_at so the empty-db skip gate triggers.
    run.last_run_at = Some(jiff::Timestamp::now().to_string());

    CollectPhase.run(&ctx, &mut run).await;

    assert!(
        run.collected.is_none(),
        "empty DB should yield no collectable data"
    );
}

// --- Layer-2: Full pipeline -----------------------------------------------------

impl ReforgeTestHarness {
    /// Seed the minimum rows that make CollectPhase yield non-empty data.
    /// Inserts a dummy session and a session memory row.
    pub async fn seed_collectable(&self) {
        // Insert a sessions row (required FK for session_memory).
        sqlx::query("INSERT OR IGNORE INTO sessions (key) VALUES ('test-session')")
            .execute(self.fact_repo.pool())
            .await
            .unwrap();
        self.session_memory
            .upsert("test-session", "test content", 1)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn run_reforge_returns_none_when_no_new_data() {
    let h = ReforgeTestHarness::new().await;
    // Pre-seed a previous run so CollectPhase does not treat this as bootstrap.
    h.reforge_state.record_run("{}").await.unwrap();
    let ctx = h.ctx();
    let run = ReforgeRun::default();

    let result = run_reforge(ctx, run).await;

    assert!(result.is_none(), "empty DB → cycle skipped, returns None");

    // And no run was recorded in reforge_state (the first record_run was our seed).
    let state = h.reforge_state.get().await.unwrap();
    assert_eq!(state.run_count, 1);
}

#[tokio::test]
async fn run_reforge_completes_and_records_a_run_when_data_present() {
    let h = ReforgeTestHarness::new().await;
    h.seed_collectable().await;
    let ctx = h.ctx();
    let run = ReforgeRun::default();

    let result = run_reforge(ctx, run).await;

    assert!(result.is_some(), "data present → cycle runs to completion");
    // The driver records the run in reforge_state after all phases.
    let state = h.reforge_state.get().await.unwrap();
    assert!(state.last_run_at.is_some(), "run should be recorded");
    assert_eq!(state.run_count, 1);
}
