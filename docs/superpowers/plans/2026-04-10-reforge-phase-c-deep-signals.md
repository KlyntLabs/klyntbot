# Phase C — Deep Signal Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist runtime signals currently discarded after each message — budget exhaustion, loop detection, validation warnings, per-message tokens, retrieval score breakdowns, and feature behavioral outcomes — so Reforge can use them for deeper self-improvement.

**Architecture:** Extend existing tables (`strategy_records`, `autotuner_shadow_log`, `retrieval_feedback`, `accumulated_observations`, `coaching_strategies`) with new columns for signal data. Create one new table (`response_warnings`). Thread `RuntimeResult` fields through `run_pipeline()` to the strategy recorder. Remove underscore prefixes from ignored autotuner hook parameters. Add signal-loading functions to the Reforge collector. Promote high-confidence distraction rules to semantic facts via a nightly Reforge collector step.

**Tech Stack:** Rust, SQLite (storage/cognitive/feature-* crates), existing Reforge collector pipeline

**Depends on:** Phase A (feedback wiring, complete) + Phase B (memory upgrade, complete)

---

## Scope Note

Phase C has three sub-sections (C1: agent runtime, C2: cognitive pipeline, C3: feature signals). All three feed into the Reforge collector and share no code dependencies between themselves, but the collector changes are bundled here since they're small. This is a single plan.

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/storage/src/repos/response_warning.rs` | Repo for `response_warnings` table |

### Modified Files
| File | Change |
|------|--------|
| `crates/storage/migrations/001_initial.sql` | Add 5 columns to `strategy_records`, 2 columns to `autotuner_shadow_log`, create `response_warnings` table, add `score_breakdown` to `retrieval_feedback`, add `near_miss_count` to `accumulated_observations` (cognitive migration) |
| `crates/storage/src/rows/learning.rs` | Add fields to `StrategyRecordRow` |
| `crates/storage/src/repos/strategy.rs` | Update `create()` INSERT to include new columns |
| `crates/storage/src/repos/mod.rs` | Export `ResponseWarningRepo` |
| `crates/storage/src/repos/retrieval_feedback.rs` | Accept and store `score_breakdown` JSON |
| `crates/agent/src/agent_runtime/runtime.rs` | Persist `RuntimeResult` to `strategy_records` after execution |
| `crates/agent/src/output/validator.rs` | Persist `ValidationWarning`s to `response_warnings` table |
| `crates/agent/src/execution/loop_detector.rs` | Emit loop detection results via return value |
| `crates/agent/src/autotuner/hooks.rs` | Remove `_` prefix, persist `tokens_used` and `response_time_ms` to shadow log |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `near_miss_count` to `accumulated_observations`, add `behavioral_positive`/`behavioral_negative` to `coaching_strategies` |
| `crates/cognitive/src/repos/mod.rs` | Bump migration version |
| `crates/cognitive/src/services/reforge/feedback.rs` | Add `load_runtime_signals()`, `load_validation_warnings()`, `load_near_miss_stats()`, `load_coaching_behavioral()` |
| `crates/cognitive/src/services/reforge/types.rs` | Add C signal fields to `ReforgeCollected` and types |
| `crates/cognitive/src/services/reforge/collector.rs` | Wire new signal loaders, add fields to `FeedbackSources` |
| `crates/cognitive/src/services/background.rs` | Track near-miss count on accumulated observations |
| `crates/feature-productivity/src/repos/learned_rule.rs` | Add `list_high_confidence()` for distraction rule promotion |
| `crates/app-core/src/init/cron.rs` | Pass new repos to collector, add distraction rule promotion step |

---

### Task 1: Extend `strategy_records` with runtime signal columns

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Modify: `crates/storage/src/rows/learning.rs`
- Modify: `crates/storage/src/repos/strategy.rs`

- [ ] **Step 1: Add columns to migration**

In `crates/storage/migrations/001_initial.sql`, find the `strategy_records` table definition. Add these columns after `rewrite_source`:

```sql
    budget_exhausted INTEGER DEFAULT 0,    -- 1 if iteration budget was hit
    turns_used INTEGER DEFAULT 0,          -- actual turn count
    loop_detected INTEGER DEFAULT 0,       -- 1 if LoopDetector fired
    loop_tools TEXT,                        -- CSV of tools in the detected loop
    context_fill_pct REAL                   -- token usage / budget as percentage
```

- [ ] **Step 2: Add fields to `StrategyRecordRow`**

In `crates/storage/src/rows/learning.rs`, add after `rewrite_source`:

```rust
    /// Whether the iteration budget was exhausted.
    pub budget_exhausted: bool,
    /// Actual number of turns executed.
    pub turns_used: i32,
    /// Whether the LoopDetector fired during execution.
    pub loop_detected: bool,
    /// CSV of tool names in the detected loop pattern (if any).
    pub loop_tools: Option<String>,
    /// Context token fill rate as percentage (tokens_used / budget * 100).
    pub context_fill_pct: Option<f64>,
```

- [ ] **Step 3: Update `create()` INSERT**

In `crates/storage/src/repos/strategy.rs`, update the `create()` method's INSERT statement to include the 5 new columns. Add them to both the column list and the VALUES placeholders:

```sql
INSERT INTO strategy_records (id, timestamp, request_id, predicted_strategy,
                               actual_strategy, escalation_count, iterations_used,
                               max_iterations, success, user_satisfaction,
                               response_time_ms, chat_id,
                               tool_name, tool_success, tool_duration_ms,
                               complexity_signals, execution_mode,
                               retrieved_memory_count,
                               rewrite_triggered, rewrite_source,
                               budget_exhausted, turns_used, loop_detected,
                               loop_tools, context_fill_pct)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
 RETURNING *
```

Add the `.bind()` calls for the 5 new fields after the existing ones.

- [ ] **Step 4: Bump storage migration version**

In `crates/storage/src/migrations.rs` (or wherever the storage feature migration version is defined), bump the version. The implementer should find the storage migration registration and increment.

- [ ] **Step 5: Verify**

Run: `cargo build -p storage`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): extend strategy_records with budget, loop, and context fill columns"
```

---

### Task 2: Create `response_warnings` table and repo

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/repos/response_warning.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Add table to migration**

Append to `crates/storage/migrations/001_initial.sql`:

```sql
-- Response validation warnings from the agent output validator.
CREATE TABLE IF NOT EXISTS response_warnings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL,
    warning_type TEXT NOT NULL,     -- 'length_truncated', 'system_leak', 'low_quality'
    detail TEXT,                    -- warning-specific detail (pattern, reason, etc.)
    chat_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_response_warnings_created
    ON response_warnings(created_at);
```

- [ ] **Step 2: Create the repo**

Create `crates/storage/src/repos/response_warning.rs`:

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResponseWarningRow {
    pub id: i64,
    pub request_id: String,
    pub warning_type: String,
    pub detail: Option<String>,
    pub chat_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ResponseWarningRepo {
    pool: SqlitePool,
}

impl ResponseWarningRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a validation warning.
    pub async fn insert(
        &self,
        request_id: &str,
        warning_type: &str,
        detail: Option<&str>,
        chat_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO response_warnings (request_id, warning_type, detail, chat_id)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(request_id)
        .bind(warning_type)
        .bind(detail)
        .bind(chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count warnings by type since a timestamp.
    pub async fn count_by_type_since(
        &self,
        since: &str,
    ) -> Result<Vec<(String, i64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT warning_type, COUNT(*) FROM response_warnings
             WHERE created_at > ?1
             GROUP BY warning_type
             ORDER BY COUNT(*) DESC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete warnings older than N days.
    pub async fn prune(&self, max_age_days: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM response_warnings WHERE created_at < datetime('now', ?1)",
        )
        .bind(format!("-{max_age_days} days"))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
```

- [ ] **Step 3: Export the repo**

In `crates/storage/src/repos/mod.rs`, add:

```rust
pub mod response_warning;
pub use response_warning::{ResponseWarningRepo, ResponseWarningRow};
```

- [ ] **Step 4: Verify**

Run: `cargo build -p storage`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add response_warnings table for validation signal persistence"
```

---

### Task 3: Persist `RuntimeResult` signals to `strategy_records`

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Thread strategy_repo into the runtime**

The implementer needs to find where `RuntimeResult` is produced in `runtime.rs` (after `process_message()` completes). At that point, if a `StrategyRepo` is available, build a `StrategyRecordRow` from the result and call `strategy_repo.create()`.

The key fields to map:
```rust
budget_exhausted: result.budget_exhausted,
turns_used: result.turns as i32,
loop_detected: false, // LoopDetector integration is separate
loop_tools: None,
context_fill_pct: Some(context_assembled_tokens as f64 / context_budget as f64 * 100.0),
```

The `context_fill_pct` is computed from `ContextAssembled` event data (`total_tokens` / `budget`). The implementer should find where this event is emitted and capture the values.

Note: This is a complex integration step. The implementer should:
1. Find where `RuntimeResult` is constructed (~line 265 in runtime.rs)
2. Find where `ContextAssembled` event is emitted (~line 210-218)
3. Capture `total_tokens` and `budget` into local variables
4. After execution completes, build the `StrategyRecordRow` and insert
5. The `StrategyRepo` needs to be available — check if it's accessible via `AgentRuntime` fields or needs threading through

- [ ] **Step 2: Verify**

Run: `cargo build -p agent`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): persist RuntimeResult budget/turns/context signals to strategy_records"
```

---

### Task 4: Persist validation warnings

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Persist warnings after validation**

After the `ResponseValidator` runs and produces `ValidationResult` (runtime.rs ~line 265), persist any warnings:

```rust
if let Some(ref warning_repo) = self.warning_repo {
    for warning in &result.validation.warnings {
        let (warning_type, detail) = match warning {
            ValidationWarning::LengthTruncated { original_chars } => {
                ("length_truncated", Some(format!("original_chars={original_chars}")))
            }
            ValidationWarning::PotentialSystemLeak { pattern } => {
                ("system_leak", Some(pattern.clone()))
            }
            ValidationWarning::LowQuality { reason } => {
                ("low_quality", Some(reason.clone()))
            }
        };
        let _ = warning_repo
            .insert(request_id, warning_type, detail.as_deref(), Some(chat_id))
            .await;
    }
}
```

The `warning_repo` needs to be added as a field to the runtime struct (`Option<storage::ResponseWarningRepo>`) and wired through `builder.rs`.

- [ ] **Step 2: Wire warning_repo in builder**

In `crates/agent/src/agent_loop/builder.rs`, find where the runtime is constructed. Add `ResponseWarningRepo::new(pool.clone())` and pass it.

- [ ] **Step 3: Verify**

Run: `cargo build -p agent`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): persist ResponseValidator warnings to response_warnings table"
```

---

### Task 5: Remove underscore prefixes in autotuner hook — persist tokens and response time

**Files:**
- Modify: `crates/agent/src/autotuner/hooks.rs`
- Modify: `crates/storage/migrations/001_initial.sql`

- [ ] **Step 1: Add columns to `autotuner_shadow_log`**

In the storage migration, find `autotuner_shadow_log` table. Add:

```sql
    tokens_used INTEGER,
    response_time_ms INTEGER
```

- [ ] **Step 2: Remove underscore prefixes and persist**

In `crates/agent/src/autotuner/hooks.rs`, find `on_message_completed()` (~line 189). Change:

```rust
    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        tokens_used: u32,        // was _tokens_used
        response_time_ms: u64,   // was _response_time_ms
    )
```

Then in the method body, after the existing `update_shadow_log_ground_truth()` call, add:

```rust
        // Persist execution metrics to shadow log
        if let Err(e) = self
            .trial_repo
            .update_shadow_log_metrics(chat_id, tokens_used, response_time_ms)
            .await
        {
            tracing::debug!("Failed to update shadow log metrics: {e}");
        }
```

- [ ] **Step 3: Add `update_shadow_log_metrics` to TrialRepo**

In `crates/storage/src/repos/trial_repo.rs`, add:

```rust
    /// Update the most recent shadow log entry for a chat with execution metrics.
    pub async fn update_shadow_log_metrics(
        &self,
        chat_id: &str,
        tokens_used: u32,
        response_time_ms: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET tokens_used = ?1, response_time_ms = ?2
             WHERE id = (
                 SELECT id FROM autotuner_shadow_log
                 WHERE chat_id = ?3
                 ORDER BY created_at DESC LIMIT 1
             )",
        )
        .bind(tokens_used as i64)
        .bind(response_time_ms as i64)
        .bind(chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
```

- [ ] **Step 4: Verify**

Run: `cargo build -p agent -p storage`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/ crates/storage/
git commit -m "feat(agent): persist per-message tokens and response time to autotuner shadow log"
```

---

### Task 6: Add `score_breakdown` to retrieval feedback

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Modify: `crates/storage/src/repos/retrieval_feedback.rs`

- [ ] **Step 1: Add column to migration**

In the `retrieval_feedback` table definition, add after `created_at`:

```sql
    score_breakdown TEXT           -- JSON: per-component scores for the top-K facts
```

- [ ] **Step 2: Accept score_breakdown in insert()**

In `crates/storage/src/repos/retrieval_feedback.rs`, update the `insert()` method to accept an optional `score_breakdown: Option<&str>` parameter and include it in the INSERT.

- [ ] **Step 3: Verify**

Run: `cargo build -p storage`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add score_breakdown JSON column to retrieval_feedback"
```

---

### Task 7: Track near-miss accumulations

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Add column to `accumulated_observations`**

In the `accumulated_observations` table in the cognitive migration, add:

```sql
    near_miss_count INTEGER DEFAULT 0   -- times this pattern almost met promotion threshold
```

- [ ] **Step 2: Bump cognitive migration version**

In `crates/cognitive/src/repos/mod.rs`, bump the cognitive migration version from `3` to `4`.

- [ ] **Step 3: Track near-misses in background service**

In `crates/cognitive/src/services/background.rs`, find the accumulator promotion check (~line 95 where it checks `days_seen >= 3`). When an accumulated entry has `days_seen` at exactly `min_days - 1` (i.e., 2 days) and gets cleaned up without promotion, increment `near_miss_count` via the repo before deletion.

The implementer should find the cleanup logic and add:

```rust
// Before deleting a non-promoted entry, check if it was close to promotion
if entry.days_seen.len() == (min_days - 1) {
    if let Some(ref ar) = accum_repo {
        let _ = ar.increment_near_miss(&key).await;
    }
}
```

This requires adding `increment_near_miss()` to `AccumulatedObservationRepo`:

```rust
pub async fn increment_near_miss(&self, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accumulated_observations SET near_miss_count = near_miss_count + 1
         WHERE event_type_key = ?1",
    )
    .bind(key)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 4: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): track near-miss accumulations for promotion threshold analysis"
```

---

### Task 8: Add behavioral columns to coaching strategies

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`

- [ ] **Step 1: Add columns**

In the `coaching_strategies` table in the cognitive migration, add:

```sql
    behavioral_positive INTEGER DEFAULT 0,
    behavioral_negative INTEGER DEFAULT 0
```

Note: The cognitive migration version was already bumped in Task 7. If Task 7 hasn't been committed yet, combine the version bump.

- [ ] **Step 2: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add behavioral outcome columns to coaching_strategies"
```

---

### Task 9: Add distraction rule promotion to Reforge collector

**Files:**
- Modify: `crates/feature-productivity/src/repos/learned_rule.rs`
- Modify: `crates/cognitive/src/services/reforge/feedback.rs`
- Modify: `crates/cognitive/src/services/reforge/types.rs`
- Modify: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: Add `list_high_confidence()` to LearnedRuleRepo**

In `crates/feature-productivity/src/repos/learned_rule.rs`, add:

```rust
    /// List distraction rules with confidence above threshold and sufficient hits.
    /// Used by Reforge to promote high-confidence rules to semantic facts.
    pub async fn list_high_confidence(
        &self,
        min_confidence: f64,
        min_hits: i32,
    ) -> Result<Vec<LearnedRuleRow>, sqlx::Error> {
        sqlx::query_as::<_, LearnedRuleRow>(
            "SELECT * FROM distraction_learned_rules
             WHERE confidence >= ?1 AND hit_count >= ?2
             ORDER BY confidence DESC",
        )
        .bind(min_confidence)
        .bind(min_hits)
        .fetch_all(&self.pool)
        .await
    }
```

- [ ] **Step 2: Add C signal types to reforge types**

In `crates/cognitive/src/services/reforge/types.rs`, add after the B2 fields on `ReforgeCollected`:

```rust
    // Phase C: Deep signals
    pub runtime_signal_summary: Option<RuntimeSignalSummary>,
    pub validation_warning_counts: Vec<(String, i64)>,
    pub near_miss_patterns: u32,
    pub coaching_behavioral: Option<CoachingBehavioralSummary>,
    pub distraction_rules_to_promote: u32,
```

Add the new types:

```rust
/// Summary of agent runtime signals since last Reforge run.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RuntimeSignalSummary {
    pub budget_exhaustions: u32,
    pub avg_turns: f64,
    pub loop_detections: u32,
    pub avg_context_fill_pct: f64,
}

/// Summary of coaching behavioral outcomes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CoachingBehavioralSummary {
    pub total_positive: u32,
    pub total_negative: u32,
    pub acceptance_rate: f64,
}
```

- [ ] **Step 3: Add signal loaders to feedback.rs**

In `crates/cognitive/src/services/reforge/feedback.rs`, add:

```rust
/// Load runtime signal summaries from strategy_records since last run.
pub async fn load_runtime_signals(
    pool: &sqlx::SqlitePool,
    since: &str,
) -> RuntimeSignalSummary {
    let row: Option<(i64, f64, i64, f64)> = sqlx::query_as(
        "SELECT
            SUM(CASE WHEN budget_exhausted = 1 THEN 1 ELSE 0 END),
            AVG(turns_used),
            SUM(CASE WHEN loop_detected = 1 THEN 1 ELSE 0 END),
            AVG(context_fill_pct)
         FROM strategy_records WHERE timestamp > ?1",
    )
    .bind(since)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some((exhaustions, avg_turns, loops, fill)) => RuntimeSignalSummary {
            budget_exhaustions: exhaustions as u32,
            avg_turns,
            loop_detections: loops as u32,
            avg_context_fill_pct: fill,
        },
        None => RuntimeSignalSummary::default(),
    }
}

/// Load validation warning counts since last run.
pub async fn load_validation_warnings(
    pool: &sqlx::SqlitePool,
    since: &str,
) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT warning_type, COUNT(*) FROM response_warnings
         WHERE created_at > ?1 GROUP BY warning_type",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Load near-miss accumulation count.
pub async fn load_near_miss_count(
    pool: &sqlx::SqlitePool,
) -> u32 {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT SUM(near_miss_count) FROM accumulated_observations WHERE near_miss_count > 0",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    row.map(|r| r.0 as u32).unwrap_or(0)
}

/// Load coaching behavioral outcome summary.
pub async fn load_coaching_behavioral(
    pool: &sqlx::SqlitePool,
) -> Option<CoachingBehavioralSummary> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT SUM(behavioral_positive), SUM(behavioral_negative),
                SUM(times_accepted), SUM(times_used)
         FROM coaching_strategies",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(pos, neg, accepted, used)| {
        let acceptance_rate = if used > 0 {
            accepted as f64 / used as f64
        } else {
            0.0
        };
        CoachingBehavioralSummary {
            total_positive: pos as u32,
            total_negative: neg as u32,
            acceptance_rate,
        }
    })
}
```

- [ ] **Step 4: Wire into collector**

In `crates/cognitive/src/services/reforge/collector.rs`, in the `collect()` function where `ReforgeCollected` is built, add the new fields. Load them from `FeedbackSources.pool`:

```rust
        runtime_signal_summary: if let Some(pool) = feedback_sources.and_then(|f| f.pool) {
            Some(super::feedback::load_runtime_signals(pool, since).await)
        } else {
            None
        },
        validation_warning_counts: if let Some(pool) = feedback_sources.and_then(|f| f.pool) {
            super::feedback::load_validation_warnings(pool, since).await
        } else {
            Vec::new()
        },
        near_miss_patterns: if let Some(pool) = feedback_sources.and_then(|f| f.pool) {
            super::feedback::load_near_miss_count(pool).await
        } else {
            0
        },
        coaching_behavioral: if let Some(pool) = feedback_sources.and_then(|f| f.pool) {
            super::feedback::load_coaching_behavioral(pool).await
        } else {
            None
        },
        distraction_rules_to_promote: 0, // Counted during promotion step in cron
```

- [ ] **Step 5: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/ crates/feature-productivity/
git commit -m "feat(cognitive): wire Phase C deep signals into Reforge collector"
```

---

### Task 10: Integration tests and full verification

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add response warnings repo test**

```rust
#[tokio::test]
async fn test_response_warnings_count_by_type() {
    let pool = klyntbot::storage::StoragePool::connect_in_memory().await.unwrap();
    let repo = klyntbot::storage::ResponseWarningRepo::new(pool.inner().clone());

    repo.insert("req1", "low_quality", Some("too short"), Some("chat1"))
        .await
        .unwrap();
    repo.insert("req2", "system_leak", Some("pattern"), Some("chat1"))
        .await
        .unwrap();
    repo.insert("req3", "low_quality", Some("generic"), Some("chat2"))
        .await
        .unwrap();

    let counts = repo.count_by_type_since("2020-01-01").await.unwrap();
    assert!(counts.len() >= 2, "Should have at least 2 warning types");
}
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero new warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: add Phase C integration tests for response warnings and deep signal collection"
```

---

## Summary

| Task | Component | Sub-phase | Files | Key Change |
|------|-----------|-----------|-------|------------|
| 1 | strategy_records columns | C1 | 3 | Add budget/loop/context_fill columns |
| 2 | response_warnings table | C1 | 3 | New table + repo |
| 3 | Persist RuntimeResult | C1 | 1 | Thread result → strategy_records |
| 4 | Persist validation warnings | C1 | 2 | ResponseValidator → response_warnings |
| 5 | Autotuner token/time persistence | C1 | 3 | Remove `_` prefixes, persist to shadow_log |
| 6 | Retrieval score breakdown | C2 | 2 | Add JSON column for component scores |
| 7 | Near-miss tracking | C2 | 3 | Track almost-promoted patterns |
| 8 | Coaching behavioral columns | C3 | 1 | Add positive/negative counters |
| 9 | Distraction rule promotion + collector | C3 | 4 | New signal loaders + collector wiring |
| 10 | Integration tests | All | 1 | Workspace verification |

**Total: ~22 files modified/created, 10 commits**

---

## What's NOT in this plan (spec says it, but deferred or already done)

- **Extraction yield per domain** — Already implemented in Phase A (`load_extraction_yield` in feedback.rs)
- **Salience verdict distribution** — Already available via `domain_event_log.salience` column; collector can read it if needed, but it's low priority
- **Phoneme mastery → Reforge** — The `phoneme_mastery.recent_errors` column exists; reading it in the collector is a one-line addition but requires `feature-language-learning` as a dependency on `cognitive` which creates a circular dep. Better handled as a cron-side step similar to distraction rules.
- **Note link density** — `note_links` table exists; aggregating in the collector requires `feature-notes` dep. Same circular dep concern. Deferred to a future cron-side metric step.
- **Fabricated tool response logging** — The spec mentions a `fabrication_log` table, but the current `max_fabrication_retries` param doesn't actually detect fabrication (it's a retry limit). Detection logic doesn't exist yet. Deferred — requires design work.
