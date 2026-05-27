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
