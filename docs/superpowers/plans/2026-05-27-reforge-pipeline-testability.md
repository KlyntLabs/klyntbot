# Reforge Pipeline Reliability & Testability — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the nightly reforge cycle reliable by adding an enforced `ReforgeContext` builder and real test coverage for the previously-untested 11-phase pipeline, retiring the `2==2` phantom test.

**Architecture:** Additive, behavior-preserving. Part A adds a `ReforgeContextBuilder` in `pipeline.rs` and migrates the single production call site (`app-core/src/init/cron.rs`). Part B adds an in-crate `#[cfg(test)]` module with a `ReforgeTestHarness` that owns an in-memory SQLite pool + the 8 required repos + a scriptable `FakeReforgeHandler` and lends a borrowed `ReforgeContext<'_>`; per-phase unit tests cover happy-path + `None`-guard skips, and full-pipeline tests drive `run_reforge` end-to-end.

**Tech Stack:** Rust, `async-trait`, `tokio`, `sqlx` (in-memory SQLite via `cognitive_schema::cognitive_test_pool`), `cargo-nextest`, `tempfile` (already a dev-dep).

**Spec:** `docs/superpowers/specs/2026-05-27-reforge-pipeline-testability-design.md`

---

## File Structure

- **Modify** `crates/cognitive-reforge/src/pipeline.rs` — add `ReforgeContextBuilder<'a>` + `ReforgeContext::builder(...)`; add `#[cfg(test)] mod pipeline_tests;` at the bottom.
- **Create** `crates/cognitive-reforge/src/pipeline_tests.rs` — harness, fakes, Layer-1 + Layer-2 tests (in-crate so private phase structs are reachable).
- **Modify** `crates/app-core/src/init/cron.rs:440` — migrate the struct-literal `ReforgeContext { … }` to the builder.
- **Delete** `crates/cognitive/tests/phase_d_moat.rs` — the phantom test.

### Key existing types (do not redefine — reference these)

- `ReforgeContext<'a>` fields (8 required, then optional): `reforge_state_repo: &storage::repos::ReforgeStateRepo`, `skill_version_repo: &storage::repos::SkillVersionRepo`, `session_memory_repo: &storage::SessionMemoryRepo`, `fact_repo: &SemanticFactRepo`, `episodic_repo: &EpisodicMemoryRepo`, `rule_repo: &ProceduralRuleRepo`, `handler: &dyn ReforgeHandler`, `skill_mgr: &SkillFileManager`; optionals: `mirror_repo`, `feedback_repo`, `autotuner_bridge`, `feedback_sources`, `graph_enrichment_handler`, `density_repo`, `entity_repo`, `snapshot_repo`, `community_intelligence_handler`, `community_repo`, `co_activation_repo_for_split`, `domain_event_bus`, `cross_cli_runner`, `skill_discovery_runner`.
- `ReforgeRun` (`#[derive(Default)]`) — phases read/write; `run.collected: Option<ReforgeCollected>`, `run.synthesize_output`, `run.review_output`, `run.narrative: String`, `run.result: ReforgeResult`.
- `ReforgeResult` (`Default`) counters: `facts_added`, `rules_added`, `patterns_persisted`, `suggestions_persisted`, `cross_cli_promoted`, `skills_proposed`, `champion_promoted: bool`, `phase_errors: Vec<String>`, `narrative: String`, etc.
- `ReforgeHandler` trait (3 async methods): `synthesize(&SynthesizeInput) -> Result<SynthesizeOutput>`, `review(&ReviewInput) -> Result<ReviewOutput>`, `narrate(&NarrateInput) -> Result<String>`.
- `SynthesizeOutput` (no `Default`): fields `fact_updates: Vec<FactUpdate>`, `rule_updates: Vec<RuleUpdate>`, `stale_facts: Vec<StaleFact>`, `cross_session_patterns: Vec<CrossSessionPattern>`, `extraction_quality_flag: Option<String>`.
- Repo constructors are uniform: `SomeRepo::new(pool.clone())` taking `sqlx::SqlitePool`. `SkillFileManager::new(skills_dir: PathBuf)`. `cognitive_schema::cognitive_test_pool().await -> sqlx::SqlitePool`.

---

## Task 1: `ReforgeContextBuilder` (production)

**Files:**
- Modify: `crates/cognitive-reforge/src/pipeline.rs` (after the `ReforgeContext` struct, ~line 79)

- [ ] **Step 1: Write the failing test**

Add a temporary inline test at the bottom of `pipeline.rs` (it will move into `pipeline_tests.rs` in Task 2; for now it proves the builder API). First add `#[cfg(test)] mod builder_smoke;` is **not** needed — instead put this in an inline `#[cfg(test)] mod builder_test` block:

```rust
#[cfg(test)]
mod builder_test {
    use super::*;

    // A no-op handler so we can build a context in a pure (no-DB) unit test.
    struct NoopHandler;
    #[async_trait]
    impl crate::ReforgeHandler for NoopHandler {
        async fn synthesize(&self, _: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
            Ok(SynthesizeOutput {
                fact_updates: vec![], rule_updates: vec![], stale_facts: vec![],
                cross_session_patterns: vec![], extraction_quality_flag: None,
            })
        }
        async fn review(&self, _: &ReviewInput) -> common::Result<ReviewOutput> {
            Ok(ReviewOutput::default())
        }
        async fn narrate(&self, _: &NarrateInput) -> common::Result<String> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn builder_sets_required_and_defaults_optionals_to_none() {
        let pool = cognitive_schema::cognitive_test_pool().await;
        let state = storage::repos::ReforgeStateRepo::new(pool.clone());
        let skillv = storage::repos::SkillVersionRepo::new(pool.clone());
        let sess = storage::SessionMemoryRepo::new(pool.clone());
        let facts = SemanticFactRepo::new(pool.clone());
        let epis = EpisodicMemoryRepo::new(pool.clone());
        let rules = ProceduralRuleRepo::new(pool.clone());
        let dir = tempfile::tempdir().unwrap();
        let skill_mgr = SkillFileManager::new(dir.path().to_path_buf());
        let handler = NoopHandler;

        let ctx = ReforgeContext::builder(
            &state, &skillv, &sess, &facts, &epis, &rules, &handler, &skill_mgr,
        )
        .build();

        assert!(ctx.mirror_repo.is_none());
        assert!(ctx.cross_cli_runner.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive-reforge -E 'test(builder_sets_required)'`
Expected: FAIL — compile error, `no function or associated item named 'builder' found for struct 'ReforgeContext'`.

- [ ] **Step 3: Implement the builder**

Add directly after the `ReforgeContext` struct definition (after line ~79) in `pipeline.rs`:

```rust
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
    pub fn snapshot_repo(
        mut self,
        v: &'a cognitive_memory::repos::KnowledgeSnapshotRepo,
    ) -> Self {
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
```

Note: confirm the imports at the top of `pipeline.rs` already bring `SynthesizeInput`, `ReviewInput`, `NarrateInput`, `ReviewOutput` into scope via `use crate::types::*;` (they do). If `ReviewOutput::default()` fails to compile, check the `ReviewOutput` derive in `types.rs` — line ~224 shows it derives `Default` (see the `#[derive(... Default)]` cluster). If it does not, construct it field-by-field instead.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive-reforge -E 'test(builder_sets_required)'`
Expected: PASS.

- [ ] **Step 5: Migrate the production call site**

In `crates/app-core/src/init/cron.rs`, replace the `ReforgeContext { … }` struct literal (currently ~lines 440–462) with the builder. The values are identical — only the syntax changes:

```rust
let reforge_ctx = cognitive::services::reforge::ReforgeContext::builder(
    &repos_reforge.reforge_state,
    &repos_reforge.skill_version,
    &repos_reforge.session_memory,
    &fact_repo,
    &episodic_repo,
    &rule_repo,
    handler.as_ref(),
    &skill_mgr,
)
.mirror_repo(&mirror_repo)
.feedback_repo(&feedback_repo)
.autotuner_bridge(bridge_ref)
.feedback_sources(&feedback_sources)
.graph_enrichment_handler(graph_handler.as_deref())
.density_repo(&density_repo)
.entity_repo(&entity_repo)
.snapshot_repo(&snapshot_repo)
.community_intelligence_handler(community_handler.as_deref())
.community_repo(&community_repo)
.co_activation_repo_for_split(&co_activation_repo)
.domain_event_bus(domain_event_bus)
.build();
// cross_cli_runner / skill_discovery_runner intentionally left unset (None).
```

- [ ] **Step 6: Verify the migration compiles**

Run: `cargo build -p app-core`
Expected: builds clean (no errors). If `domain_event_bus` is moved, ensure it is not used afterward in the same scope.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline.rs crates/app-core/src/init/cron.rs
git commit -m "feat(cognitive-reforge): add ReforgeContext builder, migrate cron call site

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Test harness + `FakeReforgeHandler`

**Files:**
- Create: `crates/cognitive-reforge/src/pipeline_tests.rs`
- Modify: `crates/cognitive-reforge/src/pipeline.rs` (remove the temporary `builder_test` mod from Task 1; add `#[cfg(test)] mod pipeline_tests;` at the bottom)

- [ ] **Step 1: Wire the test module**

In `pipeline.rs`: delete the inline `#[cfg(test)] mod builder_test { … }` from Task 1, and add at the very bottom of the file:

```rust
#[cfg(test)]
mod pipeline_tests;
```

- [ ] **Step 2: Create the harness + fake handler (with the migrated smoke test)**

Create `crates/cognitive-reforge/src/pipeline_tests.rs`:

```rust
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
            return Err(common::KlyntbotError::Internal("fake synth fail".into()));
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
            return Err(common::KlyntbotError::Internal("fake review fail".into()));
        }
        Ok(self.review_out.lock().unwrap().clone().unwrap_or_default())
    }
    async fn narrate(&self, _: &NarrateInput) -> common::Result<String> {
        if self.fail {
            return Err(common::KlyntbotError::Internal("fake narrate fail".into()));
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
```

- [ ] **Step 3: Run the smoke test**

Run: `cargo nextest run -p cognitive-reforge -E 'test(harness_builds_context)'`
Expected: PASS. If a repo constructor signature differs (e.g. takes `&pool` or `StoragePool`), adjust that one line — the uniform pattern across the codebase is `Repo::new(pool.clone())` taking `sqlx::SqlitePool`, matching `cognitive_test_pool()`'s return type. Also confirm `common::KlyntbotError::Internal` is the correct error variant; if not, use whatever variant `common::Result` errors construct with (grep `KlyntbotError::` in the crate).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline.rs crates/cognitive-reforge/src/pipeline_tests.rs
git commit -m "test(cognitive-reforge): add ReforgeTestHarness + FakeReforgeHandler

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Layer-1 tests — LLM-handler phases (Synthesize, Review, Narrate)

**Files:**
- Modify: `crates/cognitive-reforge/src/pipeline_tests.rs` (append)

These phases require `run.collected` to be `Some` (later phases call `run.collected()` which `expect()`s it). Add a helper that seeds a minimal collected value, then test each handler phase.

- [ ] **Step 1: Add a `collected`-seeding helper**

First inspect `ReforgeCollected` to construct a minimal value:
Run: `grep -n "pub struct ReforgeCollected" crates/cognitive-reforge/src/types.rs`
Then read its fields. If it derives `Default`, the helper is trivial; if not, construct the minimum.

Append to `pipeline_tests.rs`:

```rust
/// Minimal non-empty collected data so phases past Collect can run.
fn seed_collected(run: &mut ReforgeRun) {
    // ReforgeCollected derives Default (verify at types.rs); empty collections
    // are enough for the handler phases, which only read what the fake returns.
    run.collected = Some(ReforgeCollected::default());
}
```

If `ReforgeCollected` does **not** derive `Default`, add `#[derive(Default)]` to it in `types.rs` (it is an internal collection struct; this is a safe, behavior-neutral change) and note it in the commit.

- [ ] **Step 2: Write the Synthesize happy-path + error tests**

```rust
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
```

- [ ] **Step 3: Write the Review + Narrate tests**

```rust
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
```

- [ ] **Step 4: Run the LLM-phase tests**

Run: `cargo nextest run -p cognitive-reforge -E 'test(synthesize) + test(review_populates) + test(narrate)'`
Expected: PASS (4 tests). If `run.collected()` panics, the seed helper is not setting `collected`; fix the helper.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline_tests.rs crates/cognitive-reforge/src/types.rs
git commit -m "test(cognitive-reforge): cover Synthesize/Review/Narrate phases

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Layer-1 tests — `None`-guard phases (skip + present)

**Files:**
- Modify: `crates/cognitive-reforge/src/pipeline_tests.rs` (append)

The 5 hook-gated phases (`CrossCliPhase`, `SkillDiscoveryPhase`, `OptimizePhase`, `GraphConsolidationPhase`, `CommunityIntelligencePhase`) early-return when their optional hook is absent. We test both the skip (hook absent → no mutation) and the present path for the two with the simplest fake seams (`CrossCli`, `SkillDiscovery`).

- [ ] **Step 1: Add fakes for the two simple runner traits**

```rust
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
```

- [ ] **Step 2: Write the skip tests (hook absent → no-op)**

```rust
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
```

Note: confirm `GraphConsolidationPhase` and `CommunityIntelligencePhase` early-return on their respective `Option` hook by reading `pipeline.rs` lines ~628 and ~773. If a phase has an additional guard (e.g. also needs `entity_repo`), the all-`None` `ctx()` still satisfies "skip", so these assertions hold. Adjust the asserted counter field only if the field name differs from `ReforgeResult`.

- [ ] **Step 3: Write the present-path tests (CrossCli, SkillDiscovery)**

```rust
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
```

- [ ] **Step 4: Run the None-guard tests**

Run: `cargo nextest run -p cognitive-reforge -E 'test(cross_cli) + test(skill_discovery) + test(optimize_skips) + test(graph_consolidation_skips) + test(community_intelligence_skips)'`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline_tests.rs
git commit -m "test(cognitive-reforge): cover None-guard skips + runner present paths

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Layer-1 tests — Apply (DB writes) + Collect short-circuit

**Files:**
- Modify: `crates/cognitive-reforge/src/pipeline_tests.rs` (append)

- [ ] **Step 1: Write the Apply DB-write test**

`ApplyPhase` always stores the run narrative as an episodic memory (domain `reforge`). Assert the row lands in the DB.

```rust
#[tokio::test]
async fn apply_stores_narrative_as_episodic_memory() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();
    seed_collected(&mut run);
    run.narrative = "cycle narrative".to_string();

    ApplyPhase.run(&ctx, &mut run).await;

    // The narrative episodic is recorded under the reforge domain. Use the
    // episodic repo's query API to confirm at least one reforge-domain memory
    // exists. (Read EpisodicMemoryRepo for the exact query method — e.g.
    // `recent_by_domain` / `list_by_domain`; assert the count is >= 1.)
    let mems = h
        .episodic_repo
        .recent_by_domain("reforge", 10)
        .await
        .unwrap();
    assert!(!mems.is_empty(), "expected a reforge-domain narrative memory");
    assert!(run.result.phase_errors.is_empty());
}
```

Note: the exact episodic query method name must be confirmed by reading `crates/cognitive-memory/src/repos/episodic_memory.rs`. Replace `recent_by_domain("reforge", 10)` with the actual read method (e.g. `by_domain`, `list`, or a `count` helper). The assertion is "a reforge-domain memory now exists".

- [ ] **Step 2: Write the Collect short-circuit test**

`CollectPhase` with an empty in-memory DB (no sessions/episodics/rules) produces no new data, so it leaves `run.collected = None`.

```rust
#[tokio::test]
async fn collect_yields_none_on_empty_db() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let mut run = ReforgeRun::default();

    CollectPhase.run(&ctx, &mut run).await;

    assert!(
        run.collected.is_none(),
        "empty DB should yield no collectable data"
    );
}
```

Note: if `CollectPhase` returns `Some` even on an empty DB (e.g. it always collects skill files from the empty temp dir), invert this into asserting the specific empty shape instead, or seed nothing and assert `collected` reflects empty inputs. Read `collector::collect` (`crates/cognitive-reforge/src/collector.rs`) to confirm the "no new data" condition.

- [ ] **Step 3: Run the Apply + Collect tests**

Run: `cargo nextest run -p cognitive-reforge -E 'test(apply_stores) + test(collect_yields)'`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline_tests.rs
git commit -m "test(cognitive-reforge): cover Apply DB write + Collect short-circuit

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Layer-2 tests — full `run_reforge` pipeline

**Files:**
- Modify: `crates/cognitive-reforge/src/pipeline_tests.rs` (append)

- [ ] **Step 1: Write the no-new-data short-circuit test**

```rust
#[tokio::test]
async fn run_reforge_returns_none_when_no_new_data() {
    let h = ReforgeTestHarness::new().await;
    let ctx = h.ctx();
    let run = ReforgeRun::default();

    let result = run_reforge(ctx, run).await;

    assert!(result.is_none(), "empty DB → cycle skipped, returns None");

    // And no run was recorded in reforge_state.
    let state = h.reforge_state.get().await.unwrap();
    assert!(state.last_run_at.is_none());
}
```

Note: confirm `ReforgeStateRepo::get()` returns a struct with `last_run_at: Option<String>` (used by `run_reforge` at line ~164). If `get()` errors on a fresh DB instead of returning an empty state, assert on the `None` return only and drop the state assertion.

- [ ] **Step 2: Write the full-run test (data present → records a run)**

To make Collect yield data, seed the DB so `collector::collect` returns `Some`. The minimal seed depends on the collector; read `collector::collect` to find the cheapest trigger (e.g. one `session_memory` row or one recent episodic). Add a harness helper:

```rust
impl ReforgeTestHarness {
    /// Seed the minimum rows that make CollectPhase yield non-empty data.
    /// Read `collector::collect` to confirm which input it keys on; insert one
    /// row of that kind via the owned repo here.
    async fn seed_collectable(&self) {
        // Example shape — replace with the actual minimal trigger:
        // self.session_memory.insert(&SessionMemoryRow { .. }).await.unwrap();
    }
}
```

```rust
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
}
```

Note: if seeding a collectable input proves involved, this test may borrow the seed approach the (now-deleted) `phase_d_moat` neighbours used, or the integration fixtures in `crates/cognitive/tests/`. The assertion that matters: `run_reforge` returns `Some` and `reforge_state` shows a recorded run — the nightly DB-mutating path, exercised end-to-end.

- [ ] **Step 3: Run the full-pipeline tests**

Run: `cargo nextest run -p cognitive-reforge -E 'test(run_reforge_returns_none) + test(run_reforge_completes)'`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive-reforge/src/pipeline_tests.rs
git commit -m "test(cognitive-reforge): full run_reforge pipeline (skip + complete paths)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Retire the phantom test + final verification

**Files:**
- Delete: `crates/cognitive/tests/phase_d_moat.rs`
- Possibly modify: `crates/cognitive/Cargo.toml` (only if deletion orphans a dev-dep)

- [ ] **Step 1: Delete the phantom**

```bash
git rm crates/cognitive/tests/phase_d_moat.rs
```

- [ ] **Step 2: Confirm the facade still builds + no orphaned dev-deps**

Run: `cargo nextest run -p cognitive 2>&1 | tail -3`
Expected: remaining facade tests pass (the deleted file's `CrossCli`/`SkillDiscovery` coverage is now in `cognitive-reforge`).

Run: `cargo machete 2>&1 | grep -A3 "cognitive "`
Expected: no NEW unused dev-deps in the `cognitive` facade caused by the deletion. If `async-trait` (used only by the deleted file) is now unused in the facade, remove it from `crates/cognitive/Cargo.toml` `[dev-dependencies]`.

- [ ] **Step 3: Full verification sweep**

```bash
cargo nextest run -p cognitive-reforge 2>&1 | tail -3
cargo build --workspace 2>&1 | grep -E "^error|Finished"
cargo clippy -p cognitive-reforge -p app-core 2>&1 | grep -E "^warning|^error" | sort | uniq -c
```
Expected: all cognitive-reforge tests pass; workspace builds; no new clippy warnings attributable to `cognitive-reforge` or the `cron.rs` change.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test(cognitive-reforge): retire the phase_d_moat phantom test

Subsumed by real CrossCli/SkillDiscovery phase coverage in pipeline_tests.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review (completed during planning)

**Spec coverage:**
- Part A builder → Task 1 (incl. cron.rs migration). ✓
- Harness owns deps + lends context → Task 2. ✓
- FakeReforgeHandler + optional-hook fakes → Tasks 2, 4. ✓
- Per-phase happy path → Tasks 3 (LLM), 4 (runners present), 5 (Apply). ✓
- None-guard skips → Task 4 (5 gated phases). ✓
- Representative error isolation → Task 3 (synthesize/narrate failure). ✓
- Full-pipeline DB mutation + no-new-data None → Task 6. ✓
- Retire phantom → Task 7. ✓
- Verification (nextest, build, clippy, machete) → Task 7. ✓

**Known read-first points (not placeholders — assertions against existing code):** the exact episodic read method (Task 5), the `ReforgeCollected`/`ReviewOutput` `Default` derives (Tasks 1, 3), the minimal collectable seed (Task 6), and the `GraphConsolidation`/`CommunityIntelligence` skip guards (Task 4). Each step states the file/line to confirm and the concrete fallback if reality differs. These are inherent to writing tests against existing code and are resolved by reading the named file during execution.

**Type consistency:** `ReforgeContextBuilder` / `ctx_builder()` / `ctx()` / `seed_collected()` / `seed_collectable()` / `FakeReforgeHandler` field names are used consistently across tasks.
