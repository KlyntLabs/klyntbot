# Lifecycle & Polish (SP3) -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the memory unification by adding independent compaction, unbounded table cleanup, co-activation decay, flashcard-to-fact cross-reinforcement, and wiring the retrieval feedback detection into the live response pipeline.

**Architecture:** Extends the existing `run_compaction()` function with 4 new cleanup targets (accumulated_observations, failed_observations, session_memory, co-activation decay). Registers a daily cron job. Bridges the flashcard FSRS system to semantic facts via the atom's `semantic_fact_id` field. Wires `detect_referenced_facts()` into the agent runtime's Phase 3 (Record) to populate the `retrieval_feedback` table.

**Tech Stack:** Rust, SQLite, tokio, cargo-nextest

**Spec:** `docs/superpowers/specs/2026-04-07-memory-unification-design.md` (sections: "Lifecycle & Compaction", "Flashcard -> Fact Cross-Reinforcement", "Retrieval -> Autotuner Feedback")

**Depends on:** SP1 (Memory Bridge Layer) and SP2 (Intelligent Scoring) -- both complete.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/src/repos/accumulated_observation.rs` | Modify | Add `delete_older_than()` method |
| `crates/cognitive/src/repos/failed_observation.rs` | Modify | Add `delete_older_than()` method |
| `crates/storage/src/repos/session_memory.rs` | Modify | Add `delete_older_than()` method |
| `crates/cognitive/src/services/compaction.rs` | Modify | Add 4 new cleanup targets, accept new repo params |
| `crates/app-core/src/init/cron.rs` | Modify | Register `JOB_COGNITIVE_COMPACTION` daily cron |
| `crates/app-core/src/handlers/notes/flashcard.rs` | Modify | Add semantic fact cross-reinforcement on review |
| `crates/cognitive/src/services/memory_retriever.rs` | Modify | Store last-retrieved fact metadata for feedback |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Add feedback recording field + call after response |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire feedback dependencies into AgentRuntime |

---

### Task 1: Cleanup Methods on Repos

Add time-based deletion methods to the three repos that currently lack them.

**Files:**
- Modify: `crates/cognitive/src/repos/accumulated_observation.rs`
- Modify: `crates/cognitive/src/repos/failed_observation.rs`
- Modify: `crates/storage/src/repos/session_memory.rs`

- [ ] **Step 1: Add `delete_older_than` to AccumulatedObservationRepo**

In `crates/cognitive/src/repos/accumulated_observation.rs`, add:

```rust
    /// Delete observations older than `days` days. Returns rows deleted.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let result = sqlx::query("DELETE FROM accumulated_observations WHERE observed_at < ?1")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
```

- [ ] **Step 2: Add `delete_older_than` to FailedObservationRepo**

In `crates/cognitive/src/repos/failed_observation.rs`, add:

```rust
    /// Delete failed observations older than `days` days. Returns rows deleted.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let result = sqlx::query("DELETE FROM failed_observations WHERE created_at < ?1")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
```

- [ ] **Step 3: Add `delete_older_than` to SessionMemoryRepo**

In `crates/storage/src/repos/session_memory.rs`, add:

```rust
    /// Delete session memory entries older than `days` days. Returns rows deleted.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, StorageError> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let result = sqlx::query("DELETE FROM session_memory WHERE updated_at < ?1")
            .bind(&cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(result.rows_affected())
    }
```

- [ ] **Step 4: Add test for each cleanup method**

Add a test in each repo's `#[cfg(test)] mod tests`:

For `AccumulatedObservationRepo`:
```rust
    #[tokio::test]
    async fn test_delete_older_than() {
        let pool = cognitive_test_pool().await;
        let repo = AccumulatedObservationRepo::new(pool);
        // Insert an observation with old timestamp
        repo.insert("old-1", "test_key", "test", "old content", 0.5, "test", "2020-01-01T00:00:00Z", "2020-01-01")
            .await
            .unwrap();
        // Insert a recent one
        repo.insert("new-1", "test_key", "test", "new content", 0.5, "test", &chrono::Utc::now().to_rfc3339(), "2026-04-08")
            .await
            .unwrap();
        let deleted = repo.delete_older_than(7).await.unwrap();
        assert_eq!(deleted, 1);
    }
```

For `FailedObservationRepo`:
```rust
    #[tokio::test]
    async fn test_delete_older_than() {
        let pool = cognitive_test_pool().await;
        let repo = FailedObservationRepo::new(pool);
        // Insert with old created_at by directly executing SQL
        sqlx::query(
            "INSERT INTO failed_observations (id, observation_json, failure_reason, failed_stage, created_at)
             VALUES ('old-1', '{}', 'test', 'extraction', '2020-01-01T00:00:00Z')"
        )
        .execute(repo.pool())
        .await
        .unwrap();
        let deleted = repo.delete_older_than(30).await.unwrap();
        assert_eq!(deleted, 1);
    }
```

For `SessionMemoryRepo` -- test needs the storage pool:
```rust
    #[tokio::test]
    async fn test_delete_older_than() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = SessionMemoryRepo::new(pool.inner().clone());
        // Insert with old updated_at
        sqlx::query(
            "INSERT INTO session_memory (session_key, content, updated_at)
             VALUES ('old-session', 'old data', '2020-01-01T00:00:00Z')"
        )
        .execute(pool.inner())
        .await
        .unwrap();
        let deleted = repo.delete_older_than(90).await.unwrap();
        assert_eq!(deleted, 1);
    }
```

Note: `FailedObservationRepo` may not expose `pool()` -- check and add `pub fn pool(&self) -> &SqlitePool { &self.pool }` if needed. Same for `SessionMemoryRepo` (in storage crate, check the pattern).

- [ ] **Step 5: Build and test**

```bash
cargo build -p cognitive -p storage 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(delete_older_than)' --no-fail-fast 2>&1
cargo nextest run -p storage -E 'test(delete_older_than)' --no-fail-fast 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/accumulated_observation.rs crates/cognitive/src/repos/failed_observation.rs crates/storage/src/repos/session_memory.rs
git commit -m "feat(cognitive): add time-based cleanup methods to 3 repos

AccumulatedObservationRepo, FailedObservationRepo, SessionMemoryRepo
each gain delete_older_than(days) for compaction consumption."
```

---

### Task 2: Extend Compaction with New Cleanup Targets

Add the 4 missing cleanup targets to `run_compaction()`.

**Files:**
- Modify: `crates/cognitive/src/services/compaction.rs`

- [ ] **Step 1: Add new repo imports and constants**

In `crates/cognitive/src/services/compaction.rs`, update imports and add constants:

```rust
use crate::repos::{
    AccumulatedObservationRepo, CoActivationRepo, EpisodicMemoryRepo, FailedObservationRepo,
    ProceduralRuleRepo, SemanticFactRepo,
};

/// Delete accumulated observations older than this many days.
const ACCUMULATED_OBS_MAX_DAYS: i64 = 7;
/// Delete failed observations older than this many days.
const FAILED_OBS_MAX_DAYS: i64 = 30;
/// Delete session memory older than this many days.
const SESSION_MEMORY_MAX_DAYS: i64 = 90;
/// Co-activation decay factor (applied weekly on Sundays).
const CO_ACTIVATION_DECAY_FACTOR: f64 = 0.95;
/// Minimum co-activation strength to survive pruning.
const CO_ACTIVATION_MIN_STRENGTH: f64 = 0.1;
```

- [ ] **Step 2: Extend `CompactionResult` struct**

Add new fields:

```rust
pub struct CompactionResult {
    pub facts_archived: u64,
    pub episodic_deleted: u64,
    pub low_stability_archived: u64,
    pub rules_deactivated: u64,
    pub accumulated_obs_deleted: u64,
    pub failed_obs_deleted: u64,
    pub session_memory_deleted: u64,
    pub co_activation_pruned: u64,
}
```

- [ ] **Step 3: Add optional repo parameters to `run_compaction`**

Change `run_compaction` to accept new optional repos:

```rust
pub async fn run_compaction(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: Option<&ProceduralRuleRepo>,
    accum_repo: Option<&AccumulatedObservationRepo>,
    failed_repo: Option<&FailedObservationRepo>,
    session_mem_repo: Option<&storage::SessionMemoryRepo>,
    co_activation_repo: Option<&CoActivationRepo>,
) -> Result<CompactionResult, sqlx::Error> {
```

Have `run_compaction` delegate to `run_compaction_with_params` for the existing 4 targets, then run the new 4 targets afterward. Or refactor both functions -- follow whichever pattern is cleanest.

- [ ] **Step 4: Add cleanup logic after existing compaction steps**

After the existing step 4 (deactivate stale rules), add:

```rust
    // 5. Clean accumulated observations (older than 7 days)
    if let Some(repo) = accum_repo {
        let deleted = repo.delete_older_than(ACCUMULATED_OBS_MAX_DAYS).await?;
        result.accumulated_obs_deleted = deleted;
        if deleted > 0 {
            info!("Compaction: deleted {deleted} old accumulated observations");
        }
    }

    // 6. Clean failed observations (older than 30 days)
    if let Some(repo) = failed_repo {
        let deleted = repo.delete_older_than(FAILED_OBS_MAX_DAYS).await?;
        result.failed_obs_deleted = deleted;
        if deleted > 0 {
            info!("Compaction: deleted {deleted} old failed observations");
        }
    }

    // 7. Clean session memory (older than 90 days)
    if let Some(repo) = session_mem_repo {
        match repo.delete_older_than(SESSION_MEMORY_MAX_DAYS).await {
            Ok(deleted) => {
                result.session_memory_deleted = deleted;
                if deleted > 0 {
                    info!("Compaction: deleted {deleted} old session memory entries");
                }
            }
            Err(e) => warn!("Session memory cleanup failed: {e}"),
        }
    }

    // 8. Co-activation decay (weekly, Sundays only)
    if let Some(repo) = co_activation_repo {
        let today = chrono::Utc::now().weekday();
        if today == chrono::Weekday::Sun {
            let pruned = repo.decay_all(CO_ACTIVATION_DECAY_FACTOR, CO_ACTIVATION_MIN_STRENGTH).await?;
            result.co_activation_pruned = pruned;
            if pruned > 0 {
                info!("Compaction: decayed co-activation, pruned {pruned} weak pairs");
            }
        }
    }
```

- [ ] **Step 5: Update all call sites of `run_compaction`**

```bash
grep -rn "run_compaction\b" crates/ tests/
```

Add `None, None, None, None` for the new optional repos at each existing call site (internal test calls, any Tauri command, etc.). The cron handler (Task 3) will pass real repos.

- [ ] **Step 6: Update existing compaction tests**

Update test calls to `run_compaction` and `run_compaction_with_params` to include the new params. Add a test for co-activation decay:

```rust
    #[tokio::test]
    async fn test_compaction_cleans_accumulated_observations() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let accum_repo = AccumulatedObservationRepo::new(pool.clone());

        // Insert old observation
        accum_repo.insert("old-1", "key", "test", "content", 0.5, "ev", "2020-01-01T00:00:00Z", "2020-01-01")
            .await.unwrap();

        let result = run_compaction(
            &fact_repo, &episodic_repo, None,
            Some(&accum_repo), None, None, None,
        ).await.unwrap();
        assert_eq!(result.accumulated_obs_deleted, 1);
    }
```

- [ ] **Step 7: Build and test**

```bash
cargo build -p cognitive 2>&1 | tail -10
cargo nextest run -p cognitive -E 'test(compaction)' --no-fail-fast 2>&1
```

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/compaction.rs
git commit -m "feat(cognitive): extend compaction with 4 new cleanup targets

Cleans accumulated_observations (>7d), failed_observations (>30d),
session_memory (>90d), and co-activation pairs (weekly 0.95x decay
on Sundays, prune strength < 0.1)."
```

---

### Task 3: Register Daily Compaction Cron Job

Wire `run_compaction` into a daily cron job at 3am UTC.

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Add the job constant**

After `JOB_LAUNCHER_USAGE_PRUNE` (line 208), add:

```rust
const JOB_COGNITIVE_COMPACTION: &str = "__klyntbot_cognitive_compaction_daily";
```

- [ ] **Step 2: Register the handler in `register_cron_callbacks`**

Find where `JOB_ATOM_DECAY` is registered (a similar daily cognitive job). Add the compaction handler nearby:

```rust
    // Daily cognitive compaction
    {
        let pool = repos.pool().clone();
        cron_service.register_handler(
            JOB_COGNITIVE_COMPACTION,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                        let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
                        let accum_repo = cognitive::AccumulatedObservationRepo::new(pool.clone());
                        let failed_repo = cognitive::FailedObservationRepo::new(pool.clone());
                        let session_mem_repo = storage::SessionMemoryRepo::new(pool.clone());
                        let co_activation_repo = cognitive::CoActivationRepo::new(pool.clone());

                        match cognitive::compaction::run_compaction(
                            &fact_repo,
                            &episodic_repo,
                            Some(&rule_repo),
                            Some(&accum_repo),
                            Some(&failed_repo),
                            Some(&session_mem_repo),
                            Some(&co_activation_repo),
                        )
                        .await
                        {
                            Ok(r) => Ok(Some(format!(
                                "Compaction: {} facts, {} episodic, {} rules, {} obs, {} failed, {} sessions, {} co-act",
                                r.facts_archived, r.episodic_deleted, r.rules_deactivated,
                                r.accumulated_obs_deleted, r.failed_obs_deleted,
                                r.session_memory_deleted, r.co_activation_pruned
                            ))),
                            Err(e) => {
                                tracing::warn!("Compaction failed: {e}");
                                Ok(Some(format!("Compaction failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }
```

Note: Check how `cognitive::compaction::run_compaction` is imported. The cognitive crate re-exports via `pub use services::compaction;` or similar. Check `crates/cognitive/src/lib.rs` for the exact path. If not re-exported, use the full qualified path or add the re-export.

- [ ] **Step 3: Register the job in `ensure_cron_jobs`**

After the weekly reflection `ensure_job!` block, add:

```rust
    // Daily cognitive compaction
    ensure_job!(
        JOB_COGNITIVE_COMPACTION,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * *".to_string(),
            tz: Some("UTC".to_string()),
        },
        "Daily memory compaction",
        system.clone()
    );
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p app-core 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "feat(app-core): register daily cognitive compaction cron job

JOB_COGNITIVE_COMPACTION runs at 3am UTC daily. Cleans superseded
facts, stale episodic memories, old observations, session memory,
and applies weekly co-activation decay."
```

---

### Task 4: Flashcard to Semantic Fact Cross-Reinforcement

When a flashcard is reviewed and its linked atom has a `semantic_fact_id`, update the corresponding semantic fact's stability.

**Files:**
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`
- Possibly modify: `crates/cognitive/src/lib.rs` (re-export `update_stability` if needed)

- [ ] **Step 1: Read current `flashcard_record_review` implementation**

Read `crates/app-core/src/handlers/notes/flashcard.rs` and find the block where `card.atom_id` is checked (around line 119). Currently it publishes `AtomFlashcardReviewed` and updates atom retention.

- [ ] **Step 2: Verify `update_stability` is accessible from app-core**

```bash
grep -rn "pub use.*update_stability\|pub use.*decay" crates/cognitive/src/lib.rs
```

If not re-exported, add to `crates/cognitive/src/lib.rs`:
```rust
pub use services::decay::update_stability;
```

Or use the module path directly: `cognitive::services::decay::update_stability`. Check which pattern the crate uses for accessing service functions from outside.

- [ ] **Step 3: Add semantic fact reinforcement after atom update**

Inside the `if let Some(ref atom_id) = card.atom_id` block in `flashcard_record_review`, after the existing atom retention update (around line 142), add:

```rust
            // Cross-reinforce linked semantic fact (SP3: flashcard -> fact bridge)
            if let Some(ref atom_repo) = self.knowledge_atom_repo {
                if let Ok(Some(atom)) = atom_repo.get(atom_id).await {
                    if let Some(ref fact_id) = atom.semantic_fact_id {
                        let fact_repo = cognitive::SemanticFactRepo::new(
                            self.flashcard_repo().pool().clone(),
                        );
                        if let Ok(Some(fact)) = fact_repo.get(fact_id).await {
                            let new_stability = cognitive::update_stability(
                                fact.stability,
                                true,
                                30.0,
                            );
                            let _ = fact_repo.record_access(fact_id, new_stability).await;
                            tracing::debug!(
                                "Flashcard review reinforced fact {fact_id} (stability: {:.2} -> {:.2})",
                                fact.stability,
                                new_stability
                            );
                        }
                    }
                }
            }
```

Note: `self.flashcard_repo()` returns the repo; check its `pool()` method is accessible. `self.knowledge_atom_repo` is `Option<cognitive::KnowledgeAtomRepo>` -- verify field name by reading the struct definition.

- [ ] **Step 4: Add test**

In the flashcard handler's test module or a new integration test:

```rust
    #[tokio::test]
    async fn test_flashcard_review_reinforces_linked_fact() {
        // Setup: create fact, atom with semantic_fact_id pointing to fact, flashcard with atom_id
        // Action: call flashcard_record_review with rating=3 (pass)
        // Assert: fact.access_count incremented, fact.stability increased from baseline
    }
```

The exact test setup depends on the handler's constructor pattern -- follow existing test patterns in the file. If the handler requires too many dependencies, this may need to be a manual E2E test instead.

- [ ] **Step 5: Build and test**

```bash
cargo build -p app-core 2>&1 | tail -15
cargo nextest run -p app-core -E 'test(flashcard)' --no-fail-fast 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/notes/flashcard.rs crates/cognitive/src/lib.rs
git commit -m "feat(app-core): flashcard review reinforces linked semantic fact

When a flashcard is reviewed and its atom has a semantic_fact_id,
increment the fact's access_count and update stability via
update_stability(). Bridges FSRS flashcard learning to semantic
memory strength."
```

---

### Task 5: Wire Retrieval Feedback Detection into Agent Response Pipeline

Connect `detect_referenced_facts()` to the live response pipeline so the `retrieval_feedback` table gets populated.

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Store last-retrieved fact metadata in UnifiedMemoryService**

In `crates/cognitive/src/services/memory_retriever.rs`, add a field to store fact info from the last retrieval:

```rust
// Add to UnifiedMemoryService struct:
    /// Last-retrieved fact metadata: (id, subject, predicate) for feedback detection.
    last_retrieved_facts: Arc<tokio::sync::Mutex<Vec<(String, String, String)>>>,
```

Initialize in `new()`:
```rust
    last_retrieved_facts: Arc::new(tokio::sync::Mutex::new(Vec::new())),
```

- [ ] **Step 2: Populate last_retrieved_facts in fetch_facts**

In `fetch_facts()`, after the successful `retrieve_relevant_facts` call, before the `.map()` that formats content, store the raw fact tuples:

```rust
            Ok(facts) => {
                // Store for feedback detection
                let tuples: Vec<(String, String, String)> = facts.iter()
                    .filter(|f| f.score > MIN_FACT_SCORE)
                    .map(|f| (f.fact.id.clone(), f.fact.subject.clone(), f.fact.predicate.clone()))
                    .collect();
                *self.last_retrieved_facts.lock().await = tuples;

                facts.into_iter()
                    .filter(|f| f.score > MIN_FACT_SCORE)
                    // ... existing .map() chain continues unchanged
```

- [ ] **Step 3: Add `record_response_feedback` method**

Add a public method to `UnifiedMemoryService`:

```rust
    /// Record retrieval feedback by detecting which facts the LLM referenced.
    pub async fn record_response_feedback(
        &self,
        response_text: &str,
        session_key: &str,
        feedback_repo: &storage::RetrievalFeedbackRepo,
    ) {
        let facts = self.last_retrieved_facts.lock().await;
        if facts.is_empty() {
            return;
        }
        let referenced = detect_referenced_facts(response_text, &facts);
        let retrieved_ids: Vec<String> = facts.iter().map(|(id, _, _)| id.clone()).collect();
        if let Err(e) = feedback_repo.insert(&retrieved_ids, &referenced, session_key).await {
            tracing::debug!("Failed to record retrieval feedback: {e}");
        }
    }
```

- [ ] **Step 4: Add feedback fields to AgentRuntime**

In `crates/agent/src/agent_runtime/runtime.rs`, add fields to the `AgentRuntime` struct:

```rust
    memory_service: Option<Arc<cognitive::UnifiedMemoryService>>,
    feedback_repo: Option<storage::RetrievalFeedbackRepo>,
```

Add builder methods:

```rust
    pub fn with_memory_service(mut self, svc: Arc<cognitive::UnifiedMemoryService>) -> Self {
        self.memory_service = Some(svc);
        self
    }

    pub fn with_feedback_repo(mut self, repo: storage::RetrievalFeedbackRepo) -> Self {
        self.feedback_repo = Some(repo);
        self
    }
```

Initialize both to `None` in `new()`.

- [ ] **Step 5: Record feedback after LLM response**

In `runtime.rs`, in the `process_message` method, after Phase 2 (execute) completes and `loop_result` is available (around line 249, at the "Phase 3: Record" section), add:

```rust
        // Record retrieval feedback (fire-and-forget)
        if let (Some(ref mem_svc), Some(ref fb_repo)) = (&self.memory_service, &self.feedback_repo) {
            let mem_svc = Arc::clone(mem_svc);
            let fb_repo = fb_repo.clone();
            let response = loop_result.content.clone();
            let session_key = ctx.session_key.clone();
            tokio::spawn(async move {
                mem_svc.record_response_feedback(&response, &session_key, &fb_repo).await;
            });
        }
```

- [ ] **Step 6: Wire in agent builder**

In `crates/agent/src/agent_loop/builder.rs`, where `AgentRuntime` is constructed, pass the memory service and feedback repo. The builder already stores `memory_service_for_shadow: Option<Arc<cognitive::UnifiedMemoryService>>`. Use that same reference:

```rust
    // After creating the AgentRuntime, wire feedback:
    if let Some(ref mem_svc) = memory_service_for_shadow {
        runtime = runtime.with_memory_service(Arc::clone(mem_svc));
    }
    if let Some(ref pool) = self.pool {
        runtime = runtime.with_feedback_repo(storage::RetrievalFeedbackRepo::new(pool.clone()));
    }
```

Find where `AgentRuntime::new(...)` is called and add these `.with_*()` calls in the builder chain.

- [ ] **Step 7: Build and test**

```bash
cargo build -p cognitive -p agent -p app-core 2>&1 | tail -15
cargo nextest run -p cognitive -E 'test(detect_referenced)' --no-fail-fast 2>&1
```

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/memory_retriever.rs crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire retrieval feedback detection into response pipeline

UnifiedMemoryService stores last-retrieved fact metadata. After each
LLM response, record_response_feedback() detects which facts were
referenced and inserts precision into retrieval_feedback table.
Autotuner reads this for trial evaluation."
```

---

### Task 6: Full Validation

- [ ] **Step 1: Verify dev DB has required tables**

```bash
sqlite3 ~/.klyntbot-dev/data.db "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('co_activation', 'retrieval_feedback');"
```

Expected: both tables listed.

- [ ] **Step 2: Build workspace**

```bash
cargo build --workspace 2>&1 | tail -10
```

- [ ] **Step 3: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -10
```

- [ ] **Step 4: Format**

```bash
cargo fmt --all --check
```

If changes needed:
```bash
cargo fmt --all
git add -A && git commit -m "style: format after lifecycle polish implementation"
```

- [ ] **Step 5: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 6: Verify spec success criteria**

After building and running the app, verify:

```bash
# Criterion 7: Compaction runs independently -- cron job registered
sqlite3 ~/.klyntbot-dev/data.db "SELECT name, schedule FROM cron_jobs WHERE name LIKE '%compaction%';"

# Criterion 9: Retrieval feedback logged after chat turns
# (send a few messages in the app, then check)
sqlite3 ~/.klyntbot-dev/data.db "SELECT COUNT(*), AVG(precision) FROM retrieval_feedback;"

# Criterion 10: Flashcard -> fact reinforcement
# (review a flashcard linked to an atom with semantic_fact_id, then check)
sqlite3 ~/.klyntbot-dev/data.db "SELECT id, access_count, stability FROM semantic_facts WHERE access_count > 0 LIMIT 5;"
```

---

## Summary

| Task | What It Builds | Key Output |
|------|---------------|------------|
| 1 | Cleanup methods on 3 repos | `delete_older_than(days)` on AccumulatedObservationRepo, FailedObservationRepo, SessionMemoryRepo |
| 2 | Extended compaction | 4 new targets: accumulated_obs, failed_obs, session_memory, co-activation decay |
| 3 | Daily compaction cron | `JOB_COGNITIVE_COMPACTION` at 3am UTC |
| 4 | Flashcard -> fact reinforcement | Review flashcard -> update semantic fact stability via atom's `semantic_fact_id` |
| 5 | Retrieval feedback wiring | `detect_referenced_facts` called after each LLM response, populating `retrieval_feedback` table |
| 6 | Full validation | Build, clippy, format, tests, dev DB verification |

## Spec Success Criteria Addressed by SP3

| # | Criterion | How SP3 Addresses It |
|---|-----------|---------------------|
| 7 | Compaction runs independently | Tasks 2-3: standalone function + daily cron |
| 9 | Retrieval feedback logged | Task 5: wired into agent response pipeline |
| 10 | Flashcard reviews strengthen facts | Task 4: cross-reinforcement via semantic_fact_id |

## How to Verify SP3

After implementation, run the app and:

```bash
# 1. Co-activation decay works (after Sunday compaction or manual trigger)
sqlite3 ~/.klyntbot-dev/data.db "SELECT * FROM co_activation ORDER BY strength DESC LIMIT 5"

# 2. Accumulated observations cleaned up
sqlite3 ~/.klyntbot-dev/data.db "SELECT COUNT(*) FROM accumulated_observations WHERE observed_at < datetime('now', '-7 days')"
# Expected: 0

# 3. Retrieval feedback populated
sqlite3 ~/.klyntbot-dev/data.db "SELECT precision, created_at FROM retrieval_feedback ORDER BY created_at DESC LIMIT 5"

# 4. Check compaction cron job registered
sqlite3 ~/.klyntbot-dev/data.db "SELECT name, schedule, last_run FROM cron_jobs WHERE name LIKE '%compaction%'"
```
