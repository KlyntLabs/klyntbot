# Batch Pipeline & Dead-Letter Queue Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Restructure the cognitive memory pipeline to batch LLM calls (1+1 per window instead of N+N) and add a dead-letter queue with self-healing retry for failed observations.

**Architecture:** Replace the event-at-a-time loop in `BackgroundConsolidationService` with a collect→classify→batch-extract→batch-consolidate→execute pattern. Both handler traits become batch-native. A new `failed_observations` table captures heuristic-fallback events for LLM reprocessing, drained piggyback-style on the next successful batch.

**Tech Stack:** Rust, async_trait, sqlx (SQLite), tokio (broadcast, select, sleep), serde_json, futures-util (join_all)

**Spec:** `docs/superpowers/specs/2026-03-11-batch-pipeline-dead-letter-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Add | `crates/cognitive/migrations/007_failed_observations.sql` | Dead-letter table schema |
| Add | `crates/cognitive/src/repos/failed_observation.rs` | `FailedObservationRepo` CRUD |
| Rewrite | `crates/cognitive/src/extraction.rs` | Batch `ExtractionHandler` trait + types |
| Rewrite | `crates/cognitive/src/consolidation.rs` | Batch `ConsolidationHandler` trait + types |
| Rewrite | `crates/cognitive/src/background.rs` | Micro-batch event loop + dead-letter drain |
| Rewrite | `crates/agent/src/cognitive_handlers.rs` | Batch LLM prompts + fallback tracking |
| Modify | `crates/cognitive/src/repos/mod.rs` | Add migration v7 + `FailedObservationRepo` export |
| Modify | `crates/cognitive/src/lib.rs` | Re-export new types |
| Modify | `crates/agent/src/agent_loop/builder.rs` | Wire new handler signatures + pass `FailedObservationRepo` |

---

## Chunk 1: Foundation (Tasks 1-4)

These tasks build the new types, traits, dead-letter repo, and migration — all independently compilable and testable before the big rewrite.

### Task 1: Add batch types to extraction.rs

**Files:**
- Modify: `crates/cognitive/src/extraction.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [x] **Step 1: Add `BatchExtraction` and `BatchExtractionResult` types**

In `crates/cognitive/src/extraction.rs`, add after the `ExtractedFact` struct (after line 21):

```rust
/// Maps extracted facts back to their source observation in a batch.
#[derive(Debug, Clone)]
pub struct BatchExtraction {
    pub observation_index: usize,
    pub facts: Vec<ExtractedFact>,
}

/// Result of batch extraction, including fallback tracking.
#[derive(Debug, Clone)]
pub struct BatchExtractionResult {
    /// Facts grouped by source observation index.
    pub extractions: Vec<BatchExtraction>,
    /// Indices of observations that used heuristic fallback (LLM failed).
    pub fallback_indices: Vec<usize>,
}
```

- [x] **Step 2: Change `ExtractionHandler` trait to batch-native**

Replace the existing `ExtractionHandler` trait (lines 28-33) with:

```rust
/// Trait for fact extraction from observations.
///
/// Defined here (L3), implemented in the agent crate (L5) with actual LLM
/// providers. This follows the same dependency inversion pattern as
/// `EnrichmentHandler` and `SpawnHandler`.
#[async_trait]
pub trait ExtractionHandler: Send + Sync {
    /// Extract structured semantic facts from a batch of observations.
    /// Returns facts grouped by observation index, plus indices of any
    /// observations that fell back to heuristic extraction.
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<BatchExtractionResult>;
}
```

- [x] **Step 3: Remove `extract_from_observation` free function**

Delete the `extract_from_observation` function (lines 100-123). This logic moves into the batch handler implementations. Keep `to_semantic_fact` and `classify_memory_type` — they're still used.

- [x] **Step 4: Update re-exports in `lib.rs`**

In `crates/cognitive/src/lib.rs`, update the extraction re-export (line 29) to include new types:

```rust
pub use extraction::{
    BatchExtraction, BatchExtractionResult, ExtractedFact, ExtractionHandler,
};
```

- [x] **Step 5: Verify compilation fails expectedly**

Run: `cargo build -p cognitive 2>&1 | head -30`

Expected: Compilation errors in `background.rs` (calls to removed `extract_from_observation`) and in `agent` crate (handlers implement old trait signature). This is expected — we'll fix these in later tasks.

- [x] **Step 6: Add unit test for `BatchExtractionResult`**

Add to the existing `#[cfg(test)] mod tests` in `extraction.rs`:

```rust
#[test]
fn test_batch_extraction_result_structure() {
    let result = BatchExtractionResult {
        extractions: vec![
            BatchExtraction {
                observation_index: 0,
                facts: vec![ExtractedFact {
                    domain: "productivity".into(),
                    subject: "user".into(),
                    predicate: "peak_hours".into(),
                    object: "10am-12pm".into(),
                    confidence: 0.8,
                    source: "observed".into(),
                }],
            },
            BatchExtraction {
                observation_index: 1,
                facts: vec![],
            },
        ],
        fallback_indices: vec![1],
    };
    assert_eq!(result.extractions.len(), 2);
    assert_eq!(result.extractions[0].facts.len(), 1);
    assert_eq!(result.fallback_indices, vec![1]);
}
```

---

### Task 2: Add batch types to consolidation.rs

**Files:**
- Modify: `crates/cognitive/src/consolidation.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [x] **Step 1: Add `ConsolidationCandidate` type**

In `crates/cognitive/src/consolidation.rs`, add after the imports (after line 13):

```rust
/// A candidate fact paired with its existing matches for batch consolidation.
#[derive(Debug, Clone)]
pub struct ConsolidationCandidate {
    pub candidate: SemanticFact,
    pub existing: Vec<SemanticFact>,
}
```

- [x] **Step 2: Change `ConsolidationHandler` trait to batch-native**

Replace the existing `ConsolidationHandler` trait (lines 37-45) with:

```rust
/// Trait for batch consolidation decisions.
///
/// Given candidate facts paired with their existing similar facts,
/// decide what to do with each. Defined here (L3), implemented in agent (L5).
#[async_trait]
pub trait ConsolidationHandler: Send + Sync {
    /// Decide ADD/UPDATE/DELETE/NOOP for each candidate in the batch.
    /// Returns one `MemoryOp` per candidate, in the same order.
    async fn decide_batch(
        &self,
        candidates: &[ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>>;
}
```

- [x] **Step 3: Replace `consolidate_fact` and `consolidate_batch` with `execute_memory_ops`**

Remove `consolidate_fact` (lines 53-108) and `consolidate_batch` (lines 110-128). Replace with:

```rust
/// Execute consolidation decisions against the repo and embedder.
///
/// Each `MemoryOp` is applied to the corresponding `ConsolidationCandidate`.
/// This replaces the old `consolidate_fact`/`consolidate_batch` functions —
/// the repo lookup and LLM decision now happen separately in the batch pipeline.
pub async fn execute_memory_ops(
    ops: &[MemoryOp],
    candidates: &[ConsolidationCandidate],
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
) {
    for (op, entry) in ops.iter().zip(candidates.iter()) {
        match op {
            MemoryOp::Add { .. } => {
                if let Err(e) = repo.upsert(&entry.candidate).await {
                    warn!("Failed to upsert fact '{}': {e}", entry.candidate.id);
                    continue;
                }
                try_embed(embedder, &entry.candidate).await;
                debug!(
                    "Consolidated: ADD fact '{}' ({}.{} = {})",
                    entry.candidate.id,
                    entry.candidate.subject,
                    entry.candidate.predicate,
                    entry.candidate.object
                );
            }
            MemoryOp::Update { id, old_id } => {
                if let Err(e) = repo.supersede(old_id, id).await {
                    warn!("Failed to supersede '{old_id}': {e}");
                    continue;
                }
                if let Err(e) = repo.upsert(&entry.candidate).await {
                    warn!("Failed to upsert updated fact '{id}': {e}");
                    continue;
                }
                try_remove_embedding(embedder, old_id).await;
                try_embed(embedder, &entry.candidate).await;
                debug!("Consolidated: UPDATE '{old_id}' → '{id}'");
            }
            MemoryOp::Delete { id, superseded_by } => {
                if let Err(e) = repo.supersede(id, superseded_by).await {
                    warn!("Failed to supersede '{id}': {e}");
                    continue;
                }
                try_remove_embedding(embedder, id).await;
                debug!("Consolidated: DELETE '{id}' (superseded by '{superseded_by}')");
            }
            MemoryOp::Noop => {
                debug!(
                    "Consolidated: NOOP for candidate '{}'",
                    entry.candidate.id
                );
            }
        }
    }
}
```

- [x] **Step 4: Update re-exports in `lib.rs`**

In `crates/cognitive/src/lib.rs`, update the consolidation re-export (line 23) to:

```rust
pub use consolidation::{ConsolidationCandidate, ConsolidationHandler, execute_memory_ops};
```

- [x] **Step 5: Update consolidation tests**

Replace the existing `consolidate_batch` test (lines 253-271) with a test for `execute_memory_ops`:

```rust
#[tokio::test]
async fn test_execute_memory_ops_add() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool);

    let candidate = test_fact("f1", "peak_hours", "10am-12pm");
    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![],
    }];
    let ops = vec![MemoryOp::Add {
        id: "f1".into(),
    }];

    execute_memory_ops(&ops, &candidates, &repo, None).await;

    let stored = repo.get("f1").await.unwrap().unwrap();
    assert_eq!(stored.object, "10am-12pm");
}

#[tokio::test]
async fn test_execute_memory_ops_update() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool);

    let old = test_fact("f1", "peak_hours", "10am-12pm");
    repo.upsert(&old).await.unwrap();

    let new_fact = test_fact("f2", "peak_hours", "9am-11am");
    let candidates = vec![ConsolidationCandidate {
        candidate: new_fact,
        existing: vec![old],
    }];
    let ops = vec![MemoryOp::Update {
        id: "f2".into(),
        old_id: "f1".into(),
    }];

    execute_memory_ops(&ops, &candidates, &repo, None).await;

    let old_fact = repo.get("f1").await.unwrap().unwrap();
    assert!(old_fact.superseded_at.is_some());

    let new_stored = repo.get("f2").await.unwrap().unwrap();
    assert_eq!(new_stored.object, "9am-11am");
}

#[tokio::test]
async fn test_execute_memory_ops_noop() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool);

    let candidate = test_fact("f1", "peak_hours", "10am-12pm");
    let candidates = vec![ConsolidationCandidate {
        candidate,
        existing: vec![],
    }];
    let ops = vec![MemoryOp::Noop];

    execute_memory_ops(&ops, &candidates, &repo, None).await;

    // Nothing stored
    let stored = repo.get("f1").await.unwrap();
    assert!(stored.is_none());
}
```

Also update the remaining `consolidate_fact` tests to work with `execute_memory_ops`, or remove them if fully covered by the new tests. The `consolidate_adds_when_no_existing`, `consolidate_updates_existing`, and `consolidate_noop_on_duplicate` tests test the old `consolidate_fact` function which no longer exists — remove them.

- [x] **Step 6: Verify cognitive crate tests pass (extraction + consolidation only)**

Run: `cargo nextest run -p cognitive -E 'test(test_execute_memory_ops) | test(test_batch_extraction) | test(test_classify) | test(test_to_semantic)'`

Expected: All new and retained tests PASS. Build errors still expected in `background.rs` and `agent` crate.

---

### Task 3: Add `FailedObservationRepo` and migration

**Files:**
- Create: `crates/cognitive/migrations/007_failed_observations.sql`
- Create: `crates/cognitive/src/repos/failed_observation.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [x] **Step 1: Write the failing test for `FailedObservationRepo`**

Create `crates/cognitive/src/repos/failed_observation.rs`:

```rust
//! Dead-letter queue for observations that failed LLM extraction/consolidation.
//!
//! When the LLM call fails and the pipeline falls back to heuristic handlers,
//! the original observation is persisted here for later reprocessing.

use sqlx::SqlitePool;
use tracing::warn;

use crate::types::Observation;

/// Repository for failed observations (dead-letter queue).
#[derive(Debug, Clone)]
pub struct FailedObservationRepo {
    pool: SqlitePool,
}

/// A failed observation row from the dead-letter table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FailedObservationRow {
    pub id: String,
    pub observation_json: String,
    pub failure_reason: String,
    pub failed_stage: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub next_retry_at: Option<String>,
}

impl FailedObservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    async fn setup() -> (SqlitePool, FailedObservationRepo) {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = FailedObservationRepo::new(pool.clone());
        (pool, repo)
    }

    fn test_observation() -> Observation {
        Observation {
            domain: "productivity".into(),
            content: "User prefers morning work".into(),
            importance: 0.8,
            source_event: "ChatTurnCompleted".into(),
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_list_eligible() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;

        let eligible = repo.list_eligible(10).await;
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].failure_reason, "llm_error");
        assert_eq!(eligible[0].failed_stage, "extraction");
        assert_eq!(eligible[0].retry_count, 0);
    }

    #[tokio::test]
    async fn test_mark_succeeded_removes_row() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let eligible = repo.list_eligible(10).await;
        assert_eq!(eligible.len(), 1);

        repo.mark_succeeded(&eligible[0].id).await;

        let remaining = repo.list_eligible(10).await;
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_mark_failed_increments_retry() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "parse_error").await;
        let eligible = repo.list_eligible(10).await;
        let id = &eligible[0].id;

        repo.mark_failed(id).await;

        // After mark_failed, next_retry_at is set in the future, so not eligible yet
        let eligible_now = repo.list_eligible(10).await;
        assert!(eligible_now.is_empty());
    }

    #[tokio::test]
    async fn test_max_retries_excludes_from_eligible() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let eligible = repo.list_eligible(10).await;
        let id = eligible[0].id.clone();

        // Exhaust retries by directly updating the retry_count
        sqlx::query("UPDATE failed_observations SET retry_count = max_retries WHERE id = ?1")
            .bind(&id)
            .execute(&repo.pool)
            .await
            .unwrap();

        let eligible = repo.list_eligible(10).await;
        assert!(eligible.is_empty());
    }

    #[tokio::test]
    async fn test_count_pending() {
        let (_pool, repo) = setup().await;

        assert_eq!(repo.count_pending().await, 0);

        let obs = test_observation();
        repo.insert(&obs, "extraction", "llm_error").await;
        repo.insert(&obs, "consolidation", "parse_error").await;

        assert_eq!(repo.count_pending().await, 2);
    }

    #[tokio::test]
    async fn test_deserialize_observation_from_row() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let rows = repo.list_eligible(10).await;

        let deserialized: Observation =
            serde_json::from_str(&rows[0].observation_json).unwrap();
        assert_eq!(deserialized.domain, "productivity");
        assert_eq!(deserialized.content, "User prefers morning work");
    }
}
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(test_insert_and_list_eligible)'`

Expected: FAIL — `insert` method doesn't exist yet, and migration table doesn't exist.

- [x] **Step 3: Create migration file**

Create `crates/cognitive/migrations/007_failed_observations.sql`:

```sql
-- Dead-letter queue for observations that failed LLM processing.
-- Observations are stored for later reprocessing when the LLM recovers.
CREATE TABLE IF NOT EXISTS failed_observations (
    id TEXT PRIMARY KEY,
    observation_json TEXT NOT NULL,
    failure_reason TEXT NOT NULL,
    failed_stage TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    next_retry_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_failed_observations_eligible
    ON failed_observations(retry_count, next_retry_at);
```

- [x] **Step 4: Register migration in `repos/mod.rs`**

In `crates/cognitive/src/repos/mod.rs`, add after the v6 migration entry (after line 76, before the closing `]`):

```rust
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 7,
            description: "Add dead-letter queue for failed observations".to_string(),
            sql: include_str!("../../migrations/007_failed_observations.sql").to_string(),
        },
```

Also add the module declaration and re-export. Add near the other module declarations at the top:

```rust
pub mod failed_observation;
```

And add to the re-exports:

```rust
pub use failed_observation::FailedObservationRepo;
```

- [x] **Step 5: Implement `FailedObservationRepo` methods**

In `crates/cognitive/src/repos/failed_observation.rs`, add the implementation methods inside the `impl FailedObservationRepo` block:

```rust
impl FailedObservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a failed observation into the dead-letter queue.
    pub async fn insert(&self, observation: &Observation, stage: &str, reason: &str) {
        let id = uuid::Uuid::new_v4().to_string();
        let json = match serde_json::to_string(observation) {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to serialize observation for dead-letter: {e}");
                return;
            }
        };
        if let Err(e) = sqlx::query(
            "INSERT INTO failed_observations (id, observation_json, failure_reason, failed_stage) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(&json)
        .bind(reason)
        .bind(stage)
        .execute(&self.pool)
        .await
        {
            warn!("Failed to insert dead-letter observation: {e}");
        }
    }

    /// List observations eligible for retry.
    pub async fn list_eligible(&self, limit: i64) -> Vec<FailedObservationRow> {
        sqlx::query_as::<_, FailedObservationRow>(
            "SELECT * FROM failed_observations \
             WHERE retry_count < max_retries \
             AND (next_retry_at IS NULL OR next_retry_at <= datetime('now')) \
             ORDER BY created_at ASC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_else(|e| {
            warn!("Failed to list eligible dead-letter observations: {e}");
            Vec::new()
        })
    }

    /// Remove a successfully reprocessed observation.
    pub async fn mark_succeeded(&self, id: &str) {
        if let Err(e) =
            sqlx::query("DELETE FROM failed_observations WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await
        {
            warn!("Failed to mark dead-letter observation as succeeded: {e}");
        }
    }

    /// Increment retry count and set backoff delay.
    pub async fn mark_failed(&self, id: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE failed_observations \
             SET retry_count = retry_count + 1, \
                 next_retry_at = datetime('now', '+' || ((retry_count + 1) * 5) || ' minutes') \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        {
            warn!("Failed to mark dead-letter observation as failed: {e}");
        }
    }

    /// Count all pending observations (including those not yet eligible for retry).
    pub async fn count_pending(&self) -> i64 {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM failed_observations WHERE retry_count < max_retries",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));
        row.0
    }
}
```

- [x] **Step 6: Update `lib.rs` re-exports**

In `crates/cognitive/src/lib.rs`, add to the repos re-export block:

```rust
pub use repos::FailedObservationRepo;
```

- [x] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(failed_observation)'`

Expected: All 6 `FailedObservationRepo` tests PASS.

---

### Task 4: Update `PipelineEvent` enum

**Files:**
- Modify: `crates/cognitive/src/background.rs` (only the enum and `op_to_string`, NOT the event loop yet)

- [x] **Step 1: Update `PipelineEvent` enum**

In `crates/cognitive/src/background.rs`, replace the `PipelineEvent` enum (lines 28-39) with:

```rust
/// Debug events emitted by the pipeline for the debug dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    /// A new batch processing cycle started.
    BatchStarted {
        #[serde(rename = "observationCount")]
        observation_count: usize,
    },
    /// An extraction step completed.
    Extraction {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
        #[serde(rename = "usedFallback")]
        used_fallback: bool,
    },
    /// A consolidation operation was performed.
    Consolidation {
        operation: String,
        fact: String,
    },
    /// An observation was queued in the dead-letter table.
    DeadLetterQueued {
        observation: String,
        #[serde(rename = "failureReason")]
        failure_reason: String,
    },
    /// A dead-letter observation was successfully reprocessed.
    DeadLetterReprocessed {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
    },
}
```

- [x] **Step 2: Fix exhaustive `PipelineEvent` match sites**

Three files have exhaustive matches on `PipelineEvent` that will break with new variants. Add a wildcard arm to each:

**`crates/desktop/src/app_core.rs`** — find the `PipelineEvent` match block and add:
```rust
_ => {} // BatchStarted, DeadLetterQueued, DeadLetterReprocessed — no desktop handling needed
```

**`crates/desktop/src/dev_server.rs`** — same fix, add wildcard arm to the `PipelineEvent` match.

**`crates/app-core/src/init.rs`** — same fix, add wildcard arm to the `PipelineEvent` match.

Also update the existing `PipelineEvent::Extraction` emission in `background.rs` (the current event loop emits this variant — add `used_fallback: false` to the existing emission sites so the code compiles in the interim before the full rewrite in Task 7).

- [x] **Step 3: Verify with grep (full build deferred to Task 8)**

Note: `background.rs` will not compile at this point due to removed functions from Tasks 1-2. Full workspace build verification is deferred to Task 8. Verify the downstream fixes with:

Run: `grep -rn "PipelineEvent" crates/desktop/src/ crates/app-core/src/ | grep -v "^Binary"`

Confirm all three match sites have wildcard arms.

---

## Chunk 2: Handler Implementations (Tasks 5-6)

Rewrite the LLM and heuristic handlers to implement the new batch traits. After this chunk, the handler implementations compile but `background.rs` still needs rewriting.

### Task 5: Rewrite heuristic handlers for batch interface

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

- [x] **Step 1: Extract `extract_single` as `pub(crate)` on `HeuristicExtractionHandler`**

Move the body of the current `extract_facts` method (lines 27-73 of `cognitive_handlers.rs`) into a new `pub(crate)` method. This must be `pub(crate)` because `LlmExtractionHandler` calls it on `self.fallback`:

```rust
impl HeuristicExtractionHandler {
    /// Single-observation extraction logic (used by both heuristic and LLM fallback).
    pub(crate) fn extract_single(&self, observation: &Observation) -> Vec<ExtractedFact> {
        // Paste the body of the current `extract_facts` method here,
        // removing the `async` and `Ok(...)` wrapper.
        // The body starts with `let content = &observation.content;`
        // and ends with the Vec of ExtractedFact.
    }
}
```

- [x] **Step 2: Implement batch `ExtractionHandler` trait for `HeuristicExtractionHandler`**

Replace the old `ExtractionHandler` impl with:

```rust
#[async_trait]
impl ExtractionHandler for HeuristicExtractionHandler {
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<cognitive::BatchExtractionResult> {
        let mut extractions = Vec::new();
        for (i, observation) in observations.iter().enumerate() {
            let facts = self.extract_single(observation);
            extractions.push(cognitive::BatchExtraction {
                observation_index: i,
                facts,
            });
        }
        Ok(cognitive::BatchExtractionResult {
            extractions,
            fallback_indices: Vec::new(), // Heuristic IS the fallback
        })
    }
}
```

- [x] **Step 3: Extract `decide_single` as `pub(crate)` on `HeuristicConsolidationHandler`**

Same pattern — move the body of `decide` (lines 83-111) into a `pub(crate)` method:

```rust
impl HeuristicConsolidationHandler {
    /// Single-candidate consolidation logic (used by both heuristic and LLM fallback).
    pub(crate) fn decide_single(
        &self,
        candidate: &cognitive::types::SemanticFact,
        existing: &[cognitive::types::SemanticFact],
    ) -> MemoryOp {
        // Paste the body of the current `decide` method here,
        // removing the `async` and `Ok(...)` wrapper.
    }
}
```

- [x] **Step 4: Implement batch `ConsolidationHandler` trait for `HeuristicConsolidationHandler`**

```rust
#[async_trait]
impl ConsolidationHandler for HeuristicConsolidationHandler {
    async fn decide_batch(
        &self,
        candidates: &[cognitive::ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>> {
        let mut ops = Vec::with_capacity(candidates.len());
        for entry in candidates {
            let op = self.decide_single(&entry.candidate, &entry.existing);
            ops.push(op);
        }
        Ok(ops)
    }
}
```

- [x] **Step 5: Update existing tests to use batch API**

The existing tests in `cognitive_handlers.rs` call old trait methods (`extract_facts`, `decide`). Update them to use the batch API:

```rust
// Old: handler.extract_facts(&obs).await
// New:
let result = handler.extract_facts_batch(&[obs]).await.unwrap();
let facts: Vec<_> = result.extractions.into_iter().flat_map(|e| e.facts).collect();

// Old: handler.decide(&candidate, &existing).await
// New:
let ops = handler.decide_batch(&[cognitive::ConsolidationCandidate { candidate, existing }]).await.unwrap();
let op = ops.into_iter().next().unwrap();
```

- [x] **Step 6: Verify heuristic handler tests pass**

Run: `cargo nextest run -p agent -E 'test(heuristic) | test(test_extraction)'`

Expected: All updated tests PASS.

---

### Task 6: Rewrite LLM handlers for batch interface

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

- [x] **Step 1: Rewrite `LlmExtractionHandler` for batch trait**

Replace the `ExtractionHandler` impl for `LlmExtractionHandler` (lines 203-248) with:

```rust
#[derive(serde::Deserialize)]
struct BatchExtractionLlmResult {
    results: Vec<ObservationExtraction>,
}

#[derive(serde::Deserialize)]
struct ObservationExtraction {
    observation_index: usize,
    facts: Vec<ExtractedFactJson>,
}

#[async_trait]
impl ExtractionHandler for LlmExtractionHandler {
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<cognitive::BatchExtractionResult> {
        if observations.is_empty() {
            return Ok(cognitive::BatchExtractionResult {
                extractions: Vec::new(),
                fallback_indices: Vec::new(),
            });
        }

        // Build numbered observation list for the prompt
        let mut user_msg = String::new();
        for (i, obs) in observations.iter().enumerate() {
            use std::fmt::Write;
            writeln!(
                &mut user_msg,
                "Observation {}:\nDomain: {}\nSource: {}\nImportance: {:.1}\nContent: {}\n",
                i + 1,
                obs.domain,
                obs.source_event,
                obs.importance,
                obs.content
            )
            .unwrap();
        }
        user_msg.push_str(
            "Extract facts from ALL observations. Return JSON:\n\
             {\"results\": [{\"observation_index\": 1, \"facts\": [{\"domain\": \"...\", \"subject\": \"...\", \"predicate\": \"...\", \"object\": \"...\", \"confidence\": 0.0, \"source\": \"...\"}]}]}"
        );

        let messages = vec![
            Message::system(EXTRACTION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<BatchExtractionLlmResult>(&content) {
                    Ok(result) => {
                        let extractions = result
                            .results
                            .into_iter()
                            .map(|r| cognitive::BatchExtraction {
                                observation_index: r.observation_index.saturating_sub(1), // 1-based → 0-based
                                facts: r
                                    .facts
                                    .into_iter()
                                    .map(|f| ExtractedFact {
                                        domain: f.domain,
                                        subject: f.subject,
                                        predicate: f.predicate,
                                        object: f.object,
                                        confidence: f.confidence,
                                        source: f.source,
                                    })
                                    .collect(),
                            })
                            .collect();
                        Ok(cognitive::BatchExtractionResult {
                            extractions,
                            fallback_indices: Vec::new(),
                        })
                    }
                    Err(e) => {
                        tracing::warn!(
                            "LLM batch extraction JSON parse failed: {e}, falling back to heuristic for all"
                        );
                        // Fall back to heuristic for all observations
                        let mut extractions = Vec::new();
                        let mut fallback_indices = Vec::new();
                        for (i, obs) in observations.iter().enumerate() {
                            let facts = self.fallback.extract_single(obs);
                            extractions.push(cognitive::BatchExtraction {
                                observation_index: i,
                                facts,
                            });
                            fallback_indices.push(i);
                        }
                        Ok(cognitive::BatchExtractionResult {
                            extractions,
                            fallback_indices,
                        })
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "LLM batch extraction call failed: {e}, falling back to heuristic for all"
                );
                let mut extractions = Vec::new();
                let mut fallback_indices = Vec::new();
                for (i, obs) in observations.iter().enumerate() {
                    let facts = self.fallback.extract_single(obs);
                    extractions.push(cognitive::BatchExtraction {
                        observation_index: i,
                        facts,
                    });
                    fallback_indices.push(i);
                }
                Ok(cognitive::BatchExtractionResult {
                    extractions,
                    fallback_indices,
                })
            }
        }
    }
}
```

Note: The fallback paths are duplicated. Extract into a helper:

```rust
impl LlmExtractionHandler {
    fn fallback_all(&self, observations: &[Observation]) -> cognitive::BatchExtractionResult {
        let mut extractions = Vec::new();
        let mut fallback_indices = Vec::new();
        for (i, obs) in observations.iter().enumerate() {
            let facts = self.fallback.extract_single(obs);
            extractions.push(cognitive::BatchExtraction {
                observation_index: i,
                facts,
            });
            fallback_indices.push(i);
        }
        cognitive::BatchExtractionResult {
            extractions,
            fallback_indices,
        }
    }
}
```

- [x] **Step 2: Rewrite `LlmConsolidationHandler` for batch trait**

**Note:** This code uses the `json!()` macro. Ensure `use serde_json::json;` is in the imports at the top of the file.

Replace the `ConsolidationHandler` impl for `LlmConsolidationHandler` (lines 292-363) with:

```rust
#[derive(serde::Deserialize)]
struct BatchConsolidationLlmResult {
    decisions: Vec<ConsolidationDecisionIndexed>,
}

#[derive(serde::Deserialize)]
struct ConsolidationDecisionIndexed {
    index: usize,
    action: String,
    target_id: Option<String>,
}

#[async_trait]
impl ConsolidationHandler for LlmConsolidationHandler {
    async fn decide_batch(
        &self,
        candidates: &[cognitive::ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Candidates with no existing matches → direct ADD (skip LLM)
        let mut ops = vec![None; candidates.len()];
        let mut llm_indices = Vec::new();

        for (i, entry) in candidates.iter().enumerate() {
            if entry.existing.is_empty() {
                ops[i] = Some(MemoryOp::Add {
                    id: entry.candidate.id.clone(),
                });
            } else {
                llm_indices.push(i);
            }
        }

        if llm_indices.is_empty() {
            return Ok(ops.into_iter().map(|o| o.unwrap()).collect());
        }

        // Build prompt for candidates that need LLM decisions
        let mut user_msg = String::new();
        for (prompt_idx, &cand_idx) in llm_indices.iter().enumerate() {
            let entry = &candidates[cand_idx];
            let existing_json: Vec<serde_json::Value> = entry
                .existing
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "subject": f.subject,
                        "predicate": f.predicate,
                        "object": f.object,
                        "confidence": f.confidence,
                    })
                })
                .collect();

            use std::fmt::Write;
            writeln!(
                &mut user_msg,
                "Decision {}:\nCandidate: {}.{} = {} (confidence: {})\nExisting: {}\n",
                prompt_idx + 1,
                entry.candidate.subject,
                entry.candidate.predicate,
                entry.candidate.object,
                entry.candidate.confidence,
                serde_json::to_string(&existing_json).unwrap_or_default()
            )
            .unwrap();
        }
        user_msg.push_str(
            "Decide for ALL candidates. Return JSON:\n\
             {\"decisions\": [{\"index\": 1, \"action\": \"add|update|delete|noop\", \"target_id\": null}]}"
        );

        let messages = vec![
            Message::system(CONSOLIDATION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<BatchConsolidationLlmResult>(&content) {
                    Ok(result) => {
                        for decision in result.decisions {
                            let prompt_idx = decision.index.saturating_sub(1); // 1-based → 0-based
                            if let Some(&cand_idx) = llm_indices.get(prompt_idx) {
                                let candidate = &candidates[cand_idx].candidate;
                                let existing = &candidates[cand_idx].existing;
                                ops[cand_idx] = Some(self.decision_to_op(
                                    &decision.action,
                                    decision.target_id.as_deref(),
                                    candidate,
                                    existing,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("LLM batch consolidation JSON parse failed: {e}, falling back");
                        for &cand_idx in &llm_indices {
                            let entry = &candidates[cand_idx];
                            ops[cand_idx] = Some(
                                self.fallback
                                    .decide_single(&entry.candidate, &entry.existing),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM batch consolidation call failed: {e}, falling back");
                for &cand_idx in &llm_indices {
                    let entry = &candidates[cand_idx];
                    ops[cand_idx] = Some(
                        self.fallback
                            .decide_single(&entry.candidate, &entry.existing),
                    );
                }
            }
        }

        // Fill any remaining None entries with Noop (missing from LLM response)
        Ok(ops
            .into_iter()
            .map(|o| o.unwrap_or(MemoryOp::Noop))
            .collect())
    }
}
```

Add helper method to `LlmConsolidationHandler`:

```rust
impl LlmConsolidationHandler {
    fn decision_to_op(
        &self,
        action: &str,
        target_id: Option<&str>,
        candidate: &cognitive::types::SemanticFact,
        existing: &[cognitive::types::SemanticFact],
    ) -> MemoryOp {
        match action {
            "add" => MemoryOp::Add {
                id: candidate.id.clone(),
            },
            "update" => {
                let old_id = target_id
                    .map(String::from)
                    .or_else(|| existing.first().map(|f| f.id.clone()))
                    .unwrap_or_default();
                MemoryOp::Update {
                    id: candidate.id.clone(),
                    old_id,
                }
            }
            "delete" => {
                let target = target_id
                    .map(String::from)
                    .or_else(|| existing.first().map(|f| f.id.clone()))
                    .unwrap_or_default();
                MemoryOp::Delete {
                    id: target,
                    superseded_by: candidate.id.clone(),
                }
            }
            _ => MemoryOp::Noop,
        }
    }
}
```

- [x] **Step 3: Remove old single-item deserialization structs if unused**

Remove `ExtractionResult` and `ConsolidationDecisionJson` if they're no longer referenced.

- [x] **Step 4: Verify agent crate builds**

Run: `cargo build -p agent 2>&1 | head -30`

Expected: Build errors in `background.rs` (still uses old API). Handler implementations should compile.

---

## Chunk 3: Background Service Rewrite (Tasks 7-8)

The big rewrite — replace the event-at-a-time loop with the micro-batch pipeline and wire everything together.

### Task 7: Rewrite `background.rs` with micro-batch event loop

**Files:**
- Rewrite: `crates/cognitive/src/background.rs`

This is the largest task. The entire event loop changes. Keep all unchanged functions (`event_to_observation`, `event_type_key`, `summarize_accumulated`, `op_to_string`, `AccumulatedEntry`) and rewrite `BackgroundConsolidationService::start`.

- [x] **Step 1: Add `collect_batch` function**

Add before the `BackgroundConsolidationService` impl:

```rust
/// Collect domain events into a batch, waiting up to `timeout` or `max_size` events.
///
/// Handles broadcast lag (logs + continues), channel close (returns partial),
/// and cancellation (returns partial).
async fn collect_batch(
    event_rx: &mut broadcast::Receiver<DomainEvent>,
    cancel: &CancellationToken,
    timeout: std::time::Duration,
    max_size: usize,
) -> Vec<DomainEvent> {
    let mut batch = Vec::new();
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        if batch.len() >= max_size {
            break;
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = &mut deadline => break,
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        // Start the timeout after the first event
                        if batch.is_empty() {
                            deadline.as_mut().reset(tokio::time::Instant::now() + timeout);
                        }
                        batch.push(event);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("BackgroundConsolidation lagged, skipped {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    batch
}
```

- [x] **Step 2: Add `classify_batch` function**

```rust
/// Classify a batch of events by salience, returning observations split by verdict.
fn classify_batch(
    events: Vec<DomainEvent>,
) -> (Vec<Observation>, Vec<(String, Observation)>) {
    let mut to_extract = Vec::new();
    let mut to_accumulate = Vec::new();

    for event in events {
        let verdict = evaluate_salience(&event);
        if let Some(obs) = event_to_observation(&event) {
            match verdict {
                SalienceVerdict::Extract => to_extract.push(obs),
                SalienceVerdict::Accumulate => {
                    let key = event_type_key(&event);
                    to_accumulate.push((key, obs));
                }
                SalienceVerdict::Discard => {}
            }
        }
    }
    (to_extract, to_accumulate)
}
```

- [x] **Step 3: Add `prefetch_existing` function**

```rust
use futures_util::future::join_all;

/// For each extracted fact, concurrently look up existing similar facts from the repo.
async fn prefetch_existing(
    extractions: &[cognitive_extraction::BatchExtraction],
    observations: &[Observation],
    repo: &SemanticFactRepo,
) -> Vec<consolidation::ConsolidationCandidate> {
    let mut all_facts: Vec<(SemanticFact, futures_util::future::BoxFuture<'_, Vec<SemanticFact>>)> =
        Vec::new();

    for batch_ext in extractions {
        let obs = &observations[batch_ext.observation_index.min(observations.len() - 1)];
        for extracted in &batch_ext.facts {
            let fact = cognitive_extraction::to_semantic_fact(extracted, obs);
            let subject = fact.subject.clone();
            let predicate = fact.predicate.clone();
            let repo = repo.clone();
            let fut = Box::pin(async move {
                repo.find_similar(&subject, &predicate)
                    .await
                    .unwrap_or_default()
            });
            all_facts.push((fact, fut));
        }
    }

    let (facts, futs): (Vec<_>, Vec<_>) = all_facts.into_iter().unzip();
    let existing_results = join_all(futs).await;

    facts
        .into_iter()
        .zip(existing_results)
        .map(|(candidate, existing)| consolidation::ConsolidationCandidate {
            candidate,
            existing,
        })
        .collect()
}
```

- [x] **Step 4: Rewrite `BackgroundConsolidationService::start`**

Update the `start` method signature to accept `FailedObservationRepo`:

```rust
pub fn start(
    mut event_rx: broadcast::Receiver<DomainEvent>,
    extraction: Arc<dyn ExtractionHandler>,
    consolidation: Arc<dyn ConsolidationHandler>,
    repo: SemanticFactRepo,
    episodic_repo: Option<EpisodicMemoryRepo>,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    cancel: CancellationToken,
    pipeline_tx: Option<tokio::sync::broadcast::Sender<PipelineEvent>>,
    accum_repo: Option<AccumulatedObservationRepo>,
    failed_obs_repo: Option<FailedObservationRepo>,
    promote_threshold: usize,
    min_days: usize,
) -> Self {
```

Then rewrite the spawned task to use the batch pipeline. The full loop body:

```rust
let handle = tokio::spawn(async move {
    // Restore accumulated entries from previous session
    let mut accumulator: HashMap<String, AccumulatedEntry> = /* same as before */;
    let mut reprocess_queue: Vec<Observation> = Vec::new();
    // Track DLQ row IDs for observations in reprocess_queue (paired by index)
    let mut reprocess_dlq_ids: Vec<String> = Vec::new();

    loop {
        // Collect batch (3s window, max 10 events)
        let batch = collect_batch(
            &mut event_rx,
            &cancel_clone,
            std::time::Duration::from_secs(3),
            10,
        ).await;

        if cancel_clone.is_cancelled() && batch.is_empty() {
            break;
        }
        if batch.is_empty() {
            continue;
        }

        let (mut to_extract, to_accumulate) = classify_batch(batch);

        // Prepend any reprocess items (from dead-letter drain or accumulator promotion)
        // Track how many came from DLQ so we can mark them after extraction
        let dlq_ids_this_batch = std::mem::take(&mut reprocess_dlq_ids);
        let dlq_count = dlq_ids_this_batch.len();
        if !reprocess_queue.is_empty() {
            let mut combined = std::mem::take(&mut reprocess_queue);
            combined.append(&mut to_extract);
            to_extract = combined;
        }

        if !to_extract.is_empty() {
            if let Some(tx) = &pipeline_tx {
                let _ = tx.send(PipelineEvent::BatchStarted {
                    observation_count: to_extract.len(),
                });
            }

            // Batch extraction
            let result = match extraction.extract_facts_batch(&to_extract).await {
                Ok(r) => r,
                Err(e) => {
                    warn!("Batch extraction failed: {e}");
                    continue;
                }
            };

            // Emit extraction pipeline events
            if let Some(tx) = &pipeline_tx {
                for ext in &result.extractions {
                    if let Some(obs) = to_extract.get(ext.observation_index) {
                        let _ = tx.send(PipelineEvent::Extraction {
                            observation: obs.content.clone(),
                            facts_extracted: ext.facts.len(),
                            used_fallback: result.fallback_indices.contains(&ext.observation_index),
                        });
                    }
                }
            }

            // Resolve DLQ items from previous cycle that were in this batch
            // DLQ items were prepended at indices 0..dlq_count
            if let Some(ref dlq) = failed_obs_repo {
                for (i, dlq_id) in dlq_ids_this_batch.iter().enumerate() {
                    if result.fallback_indices.contains(&i) {
                        // Still failing — increment retry count
                        dlq.mark_failed(dlq_id).await;
                    } else {
                        // Successfully reprocessed by LLM
                        dlq.mark_succeeded(dlq_id).await;
                        if let Some(tx) = &pipeline_tx {
                            if let Some(obs) = to_extract.get(i) {
                                let facts_count = result.extractions.iter()
                                    .find(|e| e.observation_index == i)
                                    .map(|e| e.facts.len())
                                    .unwrap_or(0);
                                let _ = tx.send(PipelineEvent::DeadLetterReprocessed {
                                    observation: obs.content.clone(),
                                    facts_extracted: facts_count,
                                });
                            }
                        }
                    }
                }
            }

            // Queue NEW fallback observations to dead-letter (skip DLQ items — already tracked above)
            if let Some(ref dlq) = failed_obs_repo {
                for &idx in &result.fallback_indices {
                    if idx >= dlq_count {
                        // This is a new observation (not a DLQ reprocess)
                        if let Some(obs) = to_extract.get(idx) {
                            dlq.insert(obs, "extraction", "llm_fallback").await;
                            if let Some(tx) = &pipeline_tx {
                                let _ = tx.send(PipelineEvent::DeadLetterQueued {
                                    observation: obs.content.clone(),
                                    failure_reason: "llm_fallback".into(),
                                });
                            }
                        }
                    }
                }
            }

            // Episodic memory for high-importance observations
            for obs in &to_extract {
                if obs.importance >= 0.7 {
                    if let Some(ref ep_repo) = episodic_repo {
                        let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                        let mem = EpisodicMemory {
                            id: uuid::Uuid::new_v4().to_string(),
                            domain: obs.domain.clone(),
                            content: obs.content.clone(),
                            summary: None,
                            importance: obs.importance,
                            occurred_at: ts.clone(),
                            recorded_at: ts,
                            stability: 1.0,
                            last_accessed: None,
                            access_count: 0,
                            project_id: None,
                        };
                        if let Err(e) = ep_repo.insert(&mem).await {
                            warn!("Failed to store episodic memory: {e}");
                        }
                    }
                }
            }

            // Prefetch existing facts + batch consolidation
            let candidates = prefetch_existing(
                &result.extractions,
                &to_extract,
                &repo,
            ).await;

            if !candidates.is_empty() {
                let ops = match consolidation.decide_batch(&candidates).await {
                    Ok(o) => o,
                    Err(e) => {
                        warn!("Batch consolidation failed: {e}");
                        vec![MemoryOp::Noop; candidates.len()]
                    }
                };

                // Emit consolidation pipeline events
                if let Some(tx) = &pipeline_tx {
                    for (c, op) in candidates.iter().zip(ops.iter()) {
                        let _ = tx.send(PipelineEvent::Consolidation {
                            operation: op_to_string(op),
                            fact: format!(
                                "{}.{} = {}",
                                c.candidate.subject, c.candidate.predicate, c.candidate.object
                            ),
                        });
                    }
                }

                consolidation_mod::execute_memory_ops(
                    &ops,
                    &candidates,
                    &repo,
                    embedder.as_deref(),
                ).await;
            }

            // Self-healing: drain dead-letter if LLM is healthy (no fallbacks this batch)
            if result.fallback_indices.is_empty() {
                if let Some(ref dlq) = failed_obs_repo {
                    let eligible = dlq.list_eligible(5).await;
                    for row in eligible {
                        match serde_json::from_str::<Observation>(&row.observation_json) {
                            Ok(obs) => {
                                reprocess_queue.push(obs);
                                reprocess_dlq_ids.push(row.id.clone());
                                // NOTE: Do NOT mark_succeeded here — the observation hasn't
                                // been reprocessed yet. It joins the next batch cycle, and
                                // mark_succeeded/mark_failed is called after that extraction.
                            }
                            Err(e) => {
                                warn!("Failed to deserialize dead-letter observation: {e}");
                                dlq.mark_failed(&row.id).await;
                            }
                        }
                    }
                }
            }
        }

        // Handle accumulation
        for (key, obs) in to_accumulate {
            if let Some(ref ar) = accum_repo {
                ar.insert(&key, &obs).await;
            }

            let entry = accumulator
                .entry(key.clone())
                .or_insert_with(AccumulatedEntry::new);
            entry.add(obs);

            if entry.should_promote(promote_threshold, min_days) {
                debug!(
                    "Promoting accumulated events for '{key}' ({} events, {} days)",
                    entry.observations.len(),
                    entry.days_seen.len()
                );
                let summary = summarize_accumulated(&key, &entry.observations);
                reprocess_queue.push(summary);
                accumulator.remove(&key);
                if let Some(ref ar) = accum_repo {
                    ar.delete_by_key(&key).await;
                }
            }
        }
    }
});
```

**Important:** You'll need to add `use futures_util::future::join_all;` to the imports and add `futures-util` as a dependency of the cognitive crate if it isn't already. Check `crates/cognitive/Cargo.toml`. Note: use `futures-util` (not `futures`) — it's lighter and provides `join_all` and `BoxFuture` directly.

- [x] **Step 5: Verify build**

Run: `cargo build -p cognitive 2>&1 | head -30`

Fix any import issues. The `background.rs` module references types from `extraction` and `consolidation` — use module aliases if needed to avoid name conflicts:

```rust
use crate::extraction as cognitive_extraction;
use crate::consolidation as consolidation_mod;
```

---

### Task 8: Update builder wiring

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [x] **Step 1: Add `FailedObservationRepo` to `BackgroundConsolidationService::start` call**

In the builder's cognitive wiring section (around line 329-341), update the `BackgroundConsolidationService::start` call to pass the new `failed_obs_repo` parameter:

```rust
let failed_obs_repo = cognitive::FailedObservationRepo::new(pool.clone());

let bg_service = cognitive::background::BackgroundConsolidationService::start(
    event_rx,
    extraction,
    consolidation,
    fact_repo,
    Some(episodic_repo),
    cognitive_embedder_local,
    cancel.clone(),
    self.pipeline_tx.take(),
    Some(accum_repo),
    Some(failed_obs_repo),  // NEW: dead-letter repo
    config.cognitive.accumulate_promote_threshold,
    config.cognitive.accumulate_min_days,
);
```

- [x] **Step 2: Verify full workspace builds**

Run: `cargo build --workspace 2>&1 | tail -10`

Expected: Clean build (maybe pre-existing desktop warnings).

- [x] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`

Expected: All tests pass. Pay special attention to:
- `cargo nextest run -p cognitive` — new repo tests + updated extraction/consolidation tests
- `cargo nextest run -p agent` — handler tests

- [x] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep "warning" | head -20`

Expected: No new clippy warnings from our changes.

- [x] **Step 5: Run fmt**

Run: `cargo fmt --all --check`

If failures: `cargo fmt --all`

---

## Chunk 4: Verification (Task 9)

### Task 9: Final verification and cleanup

- [x] **Step 1: Verify no references to old API remain**

Search for removed function names:

```bash
# Should return 0 results (excluding docs/plans/specs)
```

Run these greps and confirm zero hits outside of docs:

- `extract_from_observation` — removed, should not appear in `.rs` files
- `consolidate_fact` — removed, should not appear in `.rs` files
- `consolidate_batch` — removed, should not appear in `.rs` files
- `fn extract_facts(` — old single-item trait method, should not appear
- `fn decide(` on ConsolidationHandler — old single-item trait method

- [x] **Step 2: Run full workspace test suite**

Run: `cargo nextest run --workspace`

Expected: All tests pass (2243+ tests, 0 failures).

- [x] **Step 3: Verify dead-letter integration manually**

Check that the `FailedObservationRepo` is wired correctly by examining the builder code path:
1. `FailedObservationRepo::new(pool.clone())` is called
2. Passed as `Some(failed_obs_repo)` to `BackgroundConsolidationService::start`
3. Inside the event loop, fallback observations are inserted
4. On successful batches, eligible items are drained

- [x] **Step 4: Check `futures-util` dependency**

Verify `futures-util` is in `crates/cognitive/Cargo.toml`. If not, add it:

```toml
futures-util = "0.3"
```

Note: use `futures-util` (not the full `futures` crate) — it's lighter and provides `join_all` and `BoxFuture`.

---

## Summary

| Chunk | Tasks | What it produces |
|-------|-------|-----------------|
| 1: Foundation | 1-4 | New batch types, `FailedObservationRepo`, migration, updated `PipelineEvent` |
| 2: Handlers | 5-6 | Heuristic + LLM handlers implementing batch traits |
| 3: Background | 7-8 | Micro-batch event loop, dead-letter drain, builder wiring |
| 4: Verification | 9 | Full workspace green, no old API references |

**Task dependency:** Tasks 1-4 are independent of each other. Tasks 5-6 depend on Tasks 1-2 (traits). Tasks 7-8 depend on everything. Task 9 is final.

**Non-compiling windows:** Between Tasks 1-2 (trait changes) and Task 7 (background rewrite), the workspace will not compile. Tasks 5-6 (handler rewrites) partially restore compilability. Full compilation restored after Task 8.

**Atomic task groups:** Tasks 1-2 + 5-6 + 7-8 should be done as a single atomic group if avoiding non-compiling states is important. Tasks 3-4 can be done independently at any point.
