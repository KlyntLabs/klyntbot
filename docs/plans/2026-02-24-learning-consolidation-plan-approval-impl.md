# Learning Tool Consolidation + Interactive Plan Approval — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Consolidate learning data into `strategy_records` so the learning tool shows real chat data, and add interactive plan approval via `ask_user` before persisting plans.

**Architecture:** Two changes: (1) Add 3 columns to `strategy_records`, rewrite `LearningHandlerImpl` to read `StrategyRepo` instead of `OutcomeStore`, remove dead outcome code. (2) Add `preview_steps` to `PlanHandler`, rewrite `PlanTool::create` to preview → ask_user → persist loop.

**Tech Stack:** Rust, sqlx (SQLite migrations), tokio (async), serde_json, async-trait

---

### Task 1: Migration 003 — Add Tool Columns to `strategy_records`

**Files:**
- Create: `crates/storage/migrations/003_strategy_tool_columns.sql`

**Step 1: Write the migration**

```sql
-- Add tool outcome columns to strategy_records for learning consolidation.
-- These are nullable: multi-tool turns leave them NULL.
ALTER TABLE strategy_records ADD COLUMN tool_name TEXT;
ALTER TABLE strategy_records ADD COLUMN tool_success INTEGER;
ALTER TABLE strategy_records ADD COLUMN tool_duration_ms INTEGER;
```

**Step 2: Run tests to verify migration applies**

Run: `cargo nextest run -p storage --no-capture 2>&1 | head -30`
Expected: All storage tests pass (migrations auto-apply on `connect_in_memory()`).

**Step 3: Commit**

```bash
git add crates/storage/migrations/003_strategy_tool_columns.sql
git commit -m "feat(storage): migration 003 — add tool columns to strategy_records"
```

---

### Task 2: Update `StrategyRecordRow` with Tool Fields

**Files:**
- Modify: `crates/storage/src/rows/learning.rs:24-37` — add 3 fields to `StrategyRecordRow`
- Modify: `crates/storage/src/repos/strategy.rs:22-46` — update `create()` INSERT to bind 15 columns
- Test: `crates/storage/src/repos/strategy.rs` (existing tests + 1 new test)

**Step 1: Write a failing test in `crates/storage/src/repos/strategy.rs`**

Add to the bottom of the `mod tests` block:

```rust
#[tokio::test]
async fn test_create_strategy_record_with_tool_fields() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.inner().clone());

    let row = StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "req-tool".to_string(),
        predicted_strategy: "ToolAssisted".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 0,
        iterations_used: 3,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 800,
        chat_id: Some("tg:99".to_string()),
        tool_name: Some("todo".to_string()),
        tool_success: Some(true),
        tool_duration_ms: Some(45),
    };

    let created = repo.create(&row).await.unwrap();
    assert_eq!(created.tool_name, Some("todo".to_string()));
    assert_eq!(created.tool_success, Some(true));
    assert_eq!(created.tool_duration_ms, Some(45));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(test_create_strategy_record_with_tool_fields)' --no-capture`
Expected: FAIL — `StrategyRecordRow` doesn't have `tool_name`, `tool_success`, `tool_duration_ms` fields.

**Step 3: Add fields to `StrategyRecordRow`**

In `crates/storage/src/rows/learning.rs`, add these 3 fields after `chat_id`:

```rust
pub tool_name: Option<String>,
pub tool_success: Option<bool>,
pub tool_duration_ms: Option<i64>,
```

**Step 4: Update `StrategyRepo::create()` INSERT**

In `crates/storage/src/repos/strategy.rs`, update the `create()` method's INSERT statement to include the 3 new columns (positions 13, 14, 15) and add `.bind(&row.tool_name)`, `.bind(row.tool_success)`, `.bind(row.tool_duration_ms)`.

The new INSERT should be:
```sql
INSERT INTO strategy_records (id, timestamp, request_id, predicted_strategy,
                               actual_strategy, escalation_count, iterations_used,
                               max_iterations, success, user_satisfaction,
                               response_time_ms, chat_id,
                               tool_name, tool_success, tool_duration_ms)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
RETURNING *
```

**Step 5: Fix all existing `StrategyRecordRow` construction sites**

Every place that constructs a `StrategyRecordRow` now needs the 3 new fields. Search for `StrategyRecordRow {` across the workspace:
- `crates/storage/src/repos/strategy.rs` — all test records: add `tool_name: None, tool_success: None, tool_duration_ms: None`
- `crates/agent/src/pipeline.rs:251` — Step 6 construction: add `tool_name: None, tool_success: None, tool_duration_ms: None` (we'll populate these in Task 4)
- `tests/learning_loop_test.rs` — integration tests: add the 3 fields

Run: `cargo nextest run -p storage -E 'test(test_create_strategy_record_with_tool_fields)' --no-capture`
Expected: PASS

**Step 6: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 7: Commit**

```bash
git add crates/storage/src/rows/learning.rs crates/storage/src/repos/strategy.rs crates/agent/src/pipeline.rs tests/learning_loop_test.rs
git commit -m "feat(storage): add tool_name, tool_success, tool_duration_ms to StrategyRecordRow"
```

---

### Task 3: Add Strategy Query Methods to `StrategyRepo`

**Files:**
- Modify: `crates/storage/src/repos/strategy.rs` — add 3 new query methods
- Test: `crates/storage/src/repos/strategy.rs`

**Step 1: Write failing tests**

Add to `mod tests`:

```rust
#[tokio::test]
async fn test_count_all() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.inner().clone());

    // Empty
    let count = repo.count_all().await.unwrap();
    assert_eq!(count, 0);

    // Insert one
    let row = StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "req-cnt".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "DirectResponse".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 1,
        success: true,
        user_satisfaction: None,
        response_time_ms: 100,
        chat_id: None,
        tool_name: None,
        tool_success: None,
        tool_duration_ms: None,
    };
    repo.create(&row).await.unwrap();
    let count = repo.count_all().await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_get_overall_stats() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.inner().clone());

    let now = chrono::Utc::now();

    // Insert 3 records: 2 accurate, 1 inaccurate, 1 with satisfaction
    for (i, (pred, actual, sat)) in [
        ("DirectResponse", "DirectResponse", Some(1.0f32)),
        ("ToolAssisted", "ToolAssisted", None),
        ("DirectResponse", "ToolAssisted", Some(0.0)),
    ]
    .iter()
    .enumerate()
    {
        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: now + chrono::Duration::seconds(i as i64),
            request_id: format!("req-{}", i),
            predicted_strategy: pred.to_string(),
            actual_strategy: actual.to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: *sat,
            response_time_ms: 100 * (i as i64 + 1),
            chat_id: None,
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
        };
        repo.create(&row).await.unwrap();
    }

    let stats = repo.get_overall_stats().await.unwrap();
    assert_eq!(stats.total_records, 3);
    assert!((stats.accuracy - 2.0 / 3.0).abs() < 0.01);
    assert_eq!(stats.avg_response_time_ms, 200); // (100+200+300)/3
    assert!((stats.avg_satisfaction.unwrap() - 0.5).abs() < 0.01); // (1.0+0.0)/2
}

#[tokio::test]
async fn test_get_tool_stats() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = StrategyRepo::new(pool.inner().clone());

    for (tool, success) in [("todo", true), ("todo", true), ("todo", false), ("shell", true)] {
        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
            predicted_strategy: "ToolAssisted".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 100,
            chat_id: None,
            tool_name: Some(tool.to_string()),
            tool_success: Some(success),
            tool_duration_ms: Some(50),
        };
        repo.create(&row).await.unwrap();
    }

    let stats = repo.get_tool_stats().await.unwrap();
    assert_eq!(stats.len(), 2);
    let todo = stats.iter().find(|s| s.tool_name == "todo").unwrap();
    assert_eq!(todo.total_calls, 3);
    assert_eq!(todo.success_count, 2);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(test_count_all)' --no-capture`
Expected: FAIL — `count_all()` method doesn't exist.

**Step 3: Implement the methods**

Add to `StrategyRepo`:

```rust
/// Overall stats row returned by get_overall_stats().
#[derive(Debug, Clone)]
pub struct OverallStats {
    pub total_records: i64,
    pub accuracy: f64,
    pub avg_response_time_ms: i64,
    pub avg_satisfaction: Option<f64>,
}

/// Per-tool stats row returned by get_tool_stats().
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ToolStats {
    pub tool_name: String,
    pub total_calls: i64,
    pub success_count: i64,
    pub avg_duration_ms: i64,
}

/// Count total strategy records.
pub async fn count_all(&self) -> Result<i64, StorageError> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM strategy_records")
            .fetch_one(&self.pool)
            .await?;
    Ok(count)
}

/// Get overall stats: total records, accuracy, avg response time, avg satisfaction.
pub async fn get_overall_stats(&self) -> Result<OverallStats, StorageError> {
    let row: (i64, i64, i64, Option<f64>) = sqlx::query_as(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN predicted_strategy = actual_strategy THEN 1 ELSE 0 END), 0),
                COALESCE(AVG(response_time_ms), 0),
                AVG(user_satisfaction)
         FROM strategy_records",
    )
    .fetch_one(&self.pool)
    .await?;

    let accuracy = if row.0 > 0 {
        row.1 as f64 / row.0 as f64
    } else {
        0.0
    };

    Ok(OverallStats {
        total_records: row.0,
        accuracy,
        avg_response_time_ms: row.2,
        avg_satisfaction: row.3,
    })
}

/// Get per-tool stats (only for records where tool_name is non-null).
pub async fn get_tool_stats(&self) -> Result<Vec<ToolStats>, StorageError> {
    let rows = sqlx::query_as::<_, ToolStats>(
        "SELECT tool_name,
                COUNT(*) AS total_calls,
                COALESCE(SUM(CASE WHEN tool_success = 1 THEN 1 ELSE 0 END), 0) AS success_count,
                COALESCE(AVG(tool_duration_ms), 0) AS avg_duration_ms
         FROM strategy_records
         WHERE tool_name IS NOT NULL
         GROUP BY tool_name
         ORDER BY total_calls DESC",
    )
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(test_count_all) | test(test_get_overall_stats) | test(test_get_tool_stats)' --no-capture`
Expected: All PASS.

**Step 5: Commit**

```bash
git add crates/storage/src/repos/strategy.rs
git commit -m "feat(storage): add count_all, get_overall_stats, get_tool_stats to StrategyRepo"
```

---

### Task 4: Populate Tool Fields in Pipeline Step 6

**Files:**
- Modify: `crates/agent/src/pipeline.rs:240-268` — extract tool info from DispatchResult
- Modify: `crates/agent/src/execution/dispatch.rs` — add `last_tool_name` to `DispatchResult`
- Test: `crates/agent/src/pipeline.rs`

**Step 1: Add `last_tool_name` field to `DispatchResult`**

In `crates/agent/src/execution/dispatch.rs`, add to the `DispatchResult` struct:

```rust
/// Name of the last tool called (for learning analytics). None if no tools called.
pub last_tool_name: Option<String>,
```

Update all construction sites of `DispatchResult` to set `last_tool_name`:
- `DirectEngine` responses: `last_tool_name: None`
- `ReactPlusEngine` responses: `last_tool_name: result.last_tool_name` (extract from `ReactOutcome`)
- Escalation paths: `last_tool_name: None`

For the `ReactOutcome` in `react_plus.rs`, the tool name of the last iteration can be extracted from the iteration's tool calls. Add a `last_tool_name` field to `ReactOutcome` that captures the tool name from the final iteration's tool calls (or `None` if the final iteration had no tool calls).

**Step 2: Update Pipeline Step 6 to use tool fields**

In `crates/agent/src/pipeline.rs`, update the `StrategyRecordRow` construction at line ~251:

```rust
tool_name: dispatch_result.last_tool_name.clone(),
tool_success: dispatch_result.last_tool_name.as_ref().map(|_| validation.is_valid),
tool_duration_ms: dispatch_result.last_tool_name.as_ref().map(|_| {
    classify_start.elapsed().as_millis() as i64
}),
```

**Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 4: Commit**

```bash
git add crates/agent/src/execution/dispatch.rs crates/agent/src/pipeline.rs
git commit -m "feat(pipeline): populate tool fields in strategy records from DispatchResult"
```

---

### Task 5: Rewrite `LearningHandlerImpl` to Read `StrategyRepo`

**Files:**
- Modify: `crates/tools/src/learning_tool.rs:17-31` — update `LearningStatus` struct to include strategy fields
- Modify: `crates/agent/src/learning_handler.rs` — replace `OutcomeStore` with `StrategyRepo`
- Test: `crates/agent/src/learning_handler.rs`

**Step 1: Update `LearningStatus` in `crates/tools/src/learning_tool.rs`**

Replace the existing `LearningStatus` struct:

```rust
/// High-level learning status returned to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStatus {
    pub current_threshold: f32,
    pub total_strategy_records: i64,
    pub strategy_accuracy: f64,
    pub avg_response_time_ms: i64,
    pub avg_satisfaction: Option<f64>,
    pub suggested_threshold: f32,
    pub per_tool: HashMap<String, ToolSummary>,
}
```

Update `ToolSummary` to match the new data source:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub total_calls: i64,
    pub success_count: i64,
    pub avg_duration_ms: i64,
}
```

**Step 2: Rewrite `LearningHandlerImpl` in `crates/agent/src/learning_handler.rs`**

Replace the entire implementation. The new version takes `StrategyRepo` + `AdaptiveThresholds`:

```rust
use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::learning_tool::{LearningHandler, LearningStatus, ThresholdEntry, ToolSummary};

use crate::learning::adaptive::AdaptiveThresholds;

/// Implements LearningHandler by reading from StrategyRepo.
pub struct LearningHandlerImpl {
    strategy_repo: storage::StrategyRepo,
    adaptive: Arc<RwLock<AdaptiveThresholds>>,
}

impl LearningHandlerImpl {
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        adaptive: Arc<RwLock<AdaptiveThresholds>>,
    ) -> Self {
        Self {
            strategy_repo,
            adaptive,
        }
    }
}

#[async_trait]
impl LearningHandler for LearningHandlerImpl {
    async fn get_status(&self) -> Result<Option<LearningStatus>> {
        let count = self.strategy_repo.count_all().await?;
        if count == 0 {
            return Ok(None);
        }
        Ok(Some(self.build_status().await?))
    }

    async fn analyze_now(&self) -> Result<LearningStatus> {
        self.build_status().await
    }

    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>> {
        let adaptive = self.adaptive.read().await;
        let history = &adaptive.state().threshold_history;
        let start = history.len().saturating_sub(limit);
        Ok(history[start..]
            .iter()
            .map(|c| ThresholdEntry {
                from: c.from,
                to: c.to,
                reason: c.reason.clone(),
                timestamp: c.timestamp,
            })
            .collect())
    }
}

impl LearningHandlerImpl {
    async fn build_status(&self) -> Result<LearningStatus> {
        let overall = self.strategy_repo.get_overall_stats().await?;
        let tool_rows = self.strategy_repo.get_tool_stats().await?;

        let per_tool: HashMap<String, ToolSummary> = tool_rows
            .into_iter()
            .map(|t| {
                (
                    t.tool_name,
                    ToolSummary {
                        total_calls: t.total_calls,
                        success_count: t.success_count,
                        avg_duration_ms: t.avg_duration_ms,
                    },
                )
            })
            .collect();

        let adaptive = self.adaptive.read().await;

        Ok(LearningStatus {
            current_threshold: adaptive.current_threshold(),
            total_strategy_records: overall.total_records,
            strategy_accuracy: overall.accuracy,
            avg_response_time_ms: overall.avg_response_time_ms,
            avg_satisfaction: overall.avg_satisfaction,
            suggested_threshold: adaptive.current_threshold(),
            per_tool,
        })
    }
}
```

**Step 3: Write tests**

Replace the test module in `learning_handler.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn make_handler() -> (LearningHandlerImpl, storage::StrategyRepo) {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let strategy_repo = storage::StrategyRepo::new(pool.inner().clone());
        let adaptive = Arc::new(RwLock::new(
            crate::learning::adaptive::AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50),
        ));
        let handler = LearningHandlerImpl::new(strategy_repo.clone(), adaptive);
        (handler, strategy_repo)
    }

    #[tokio::test]
    async fn test_get_status_empty() {
        let (handler, _) = make_handler().await;
        let status = handler.get_status().await.unwrap();
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_get_status_with_records() {
        let (handler, repo) = make_handler().await;

        let row = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-1".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: Some(1.0),
            response_time_ms: 500,
            chat_id: None,
            tool_name: Some("todo".to_string()),
            tool_success: Some(true),
            tool_duration_ms: Some(50),
        };
        repo.create(&row).await.unwrap();

        let status = handler.get_status().await.unwrap();
        assert!(status.is_some());
        let s = status.unwrap();
        assert_eq!(s.total_strategy_records, 1);
        assert!((s.strategy_accuracy - 1.0).abs() < 0.01);
        assert!(s.per_tool.contains_key("todo"));
    }
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(learning_handler)' --no-capture`
Expected: All PASS.

**Step 5: Commit**

```bash
git add crates/tools/src/learning_tool.rs crates/agent/src/learning_handler.rs
git commit -m "feat(agent): rewrite LearningHandlerImpl to read StrategyRepo"
```

---

### Task 6: Update Builder Wiring for LearningHandler

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:655-705` — pass `StrategyRepo` to `LearningHandlerImpl` instead of `OutcomeStore`

**Step 1: Update `LearningHandlerImpl::new()` call in builder**

At `builder.rs:669-672`, change:

```rust
// OLD:
let learning_handler = Arc::new(super::super::LearningHandlerImpl::new(
    Arc::clone(store),
    Arc::clone(&adaptive),
));

// NEW:
let learning_handler = Arc::new(super::super::LearningHandlerImpl::new(
    repos.strategies.clone(),
    Arc::clone(&adaptive),
));
```

**Step 2: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire StrategyRepo into LearningHandlerImpl in builder"
```

---

### Task 7: Remove Dead Outcome Code

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:77` — remove `outcome_recorder` field
- Modify: `crates/agent/src/agent_loop/builder.rs:822` — remove `outcome_recorder` from construction
- Modify: `crates/agent/src/plan_runner.rs:139-169` — remove `OutcomeRecorder` usage
- Modify: `crates/agent/src/learning/mod.rs` — remove `OutcomeStore`, `OutcomeRecorder` re-exports
- Modify: `crates/agent/src/lib.rs` — remove re-exports if any

Note: Do NOT delete `crates/agent/src/learning/outcome_store.rs` or `crates/agent/src/learning/recorder.rs` yet — they may still be referenced by `LearningService`. Just remove the `outcome_recorder` field from `AgentLoop` and the usage in `plan_runner.rs`.

**Step 1: Remove `outcome_recorder` field from `AgentLoop`**

In `crates/agent/src/agent_loop/mod.rs`, remove line 77:
```rust
pub(crate) outcome_recorder: Option<Arc<crate::learning::OutcomeRecorder>>,
```

**Step 2: Remove from builder construction**

In `crates/agent/src/agent_loop/builder.rs` at line ~822, remove:
```rust
outcome_recorder,
```

Also remove the `outcome_recorder` construction at lines 344-346:
```rust
let outcome_recorder = outcome_store
    .as_ref()
    .map(|store| Arc::new(crate::learning::OutcomeRecorder::new(Arc::clone(store))));
```

And remove the enrichment feedback handler wiring at lines 413-418 that references `outcome_recorder`.

**Step 3: Remove from `plan_runner.rs`**

In `crates/agent/src/plan_runner.rs`, remove the block at lines ~139-169 that calls `recorder.record_tool_outcome(...)`. Remove the `outcome_recorder` field from the `PlanRunner` struct (or `AgentLoop` — whichever holds it for plan execution).

**Step 4: Fix compilation**

Run: `cargo build --workspace 2>&1 | head -50`
Fix any remaining references to `outcome_recorder`. If `LearningService` still uses `OutcomeStore`, keep those files but remove the dead paths.

**Step 5: Run tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

**Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/plan_runner.rs
git commit -m "refactor(agent): remove dead OutcomeRecorder usage from AgentLoop and PlanRunner"
```

---

### Task 8: Add `preview_steps` to `PlanHandler` Trait

**Files:**
- Modify: `crates/tools/src/plan_tool.rs:37-57` — add `preview_steps` method to `PlanHandler` trait
- Modify: `crates/agent/src/plan_handler.rs` — implement `preview_steps`
- Test: `crates/agent/src/plan_handler.rs`

**Step 1: Add to `PlanHandler` trait**

In `crates/tools/src/plan_tool.rs`, add to the `PlanHandler` trait:

```rust
/// Generate plan steps as a preview without persisting.
/// Returns a list of step descriptions for user review.
async fn preview_steps(&self, description: &str) -> Result<Vec<String>>;
```

**Step 2: Implement in `PlanHandlerImpl`**

In `crates/agent/src/plan_handler.rs`, add:

```rust
async fn preview_steps(&self, description: &str) -> Result<Vec<String>> {
    let provider = match &self.provider {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    let model = self.model.as_deref().unwrap_or("gpt-4o-mini");

    let drafts = match generate_plan_steps(provider, model, description, &[], &[]).await {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("preview_steps: LLM call failed: {}", e);
            return Ok(Vec::new());
        }
    };

    Ok(drafts.iter().map(|d| d.description.clone()).collect())
}
```

**Step 3: Run workspace build**

Run: `cargo build --workspace`
Expected: Compiles (no tests yet for preview_steps specifically, just ensure trait matches).

**Step 4: Commit**

```bash
git add crates/tools/src/plan_tool.rs crates/agent/src/plan_handler.rs
git commit -m "feat(tools): add preview_steps to PlanHandler trait"
```

---

### Task 9: Rewrite `PlanTool::create` with Interactive Approval Loop

**Files:**
- Modify: `crates/tools/src/plan_tool.rs:126-158` — rewrite the `"create"` action
- Test: `crates/tools/src/plan_tool.rs`

**Step 1: Rewrite the `"create"` match arm**

Replace the `"create"` block in `PlanTool::execute()` with:

```rust
"create" => {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidParams("Missing title for create".into()))?;

    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let goal_id = args
        .get("goal_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let default_session_key = format!("{}:{}", ctx.channel, ctx.chat_id);
    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_session_key);

    // Preview steps via LLM (without persisting)
    let steps = handler.preview_steps(description).await?;

    // Format preview for user
    let mut preview = format!("**Plan: {}**\n", title);
    if !description.is_empty() {
        preview.push_str(&format!("Description: {}\n", description));
    }
    if steps.is_empty() {
        preview.push_str("\n(No steps generated — try a more specific description)\n");
    } else {
        preview.push_str(&format!("\n**Steps ({}):**\n", steps.len()));
        for (i, step) in steps.iter().enumerate() {
            preview.push_str(&format!("{}. {}\n", i + 1, step));
        }
    }

    // Ask user for approval via interaction channel (or fallback to text)
    let approval_result = ask_plan_approval(ctx, &preview).await;

    match approval_result {
        PlanApproval::Approved => {
            let plan = handler
                .create_plan(title, description, session_key, goal_id)
                .await?;
            // Generate and save steps
            let _ = handler.generate_steps(&plan.id).await;
            Ok(format!(
                "Created plan '{}' (id: {}, status: {:?})",
                plan.title, plan.id, plan.status
            ))
        }
        PlanApproval::Abandoned => {
            Ok("Plan abandoned — nothing was saved.".to_string())
        }
        PlanApproval::NoInteraction => {
            // Non-TTY: present preview and instruct LLM to ask conversationally
            Ok(format!(
                "{}\n\nPlease ask the user if they want to approve this plan, \
                 revise the description, or abandon it. Do NOT save the plan until \
                 the user explicitly approves.",
                preview
            ))
        }
    }
}
```

**Step 2: Add the helper types and function**

Add these above the `PlanTool` impl or in a private section:

```rust
/// Result of asking the user to approve a plan.
enum PlanApproval {
    Approved,
    Abandoned,
    /// No interaction channel — fall back to conversational approval.
    NoInteraction,
}

/// Ask the user to approve a plan preview via the interaction channel.
async fn ask_plan_approval(ctx: &RoutingContext, preview: &str) -> PlanApproval {
    use common::{
        AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
    };

    let interaction_tx = match &ctx.interaction_tx {
        Some(tx) => tx,
        None => return PlanApproval::NoInteraction,
    };

    let request = InteractionRequest {
        title: "Plan Review".to_string(),
        questions: vec![Question {
            id: "approval".to_string(),
            title: "Plan".to_string(),
            text: format!("{}\n\nDo you want to create this plan?", preview),
            answer_type: AnswerType::SingleSelect {
                options: vec![
                    AnswerOption {
                        value: "approve".to_string(),
                        label: "Approve".to_string(),
                        description: Some("Save and create this plan".to_string()),
                    },
                    AnswerOption {
                        value: "abandon".to_string(),
                        label: "Abandon".to_string(),
                        description: Some("Discard — nothing saved".to_string()),
                    },
                ],
            },
        }],
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    if interaction_tx
        .send(super::InteractionBundle {
            request,
            response_tx,
        })
        .await
        .is_err()
    {
        return PlanApproval::NoInteraction;
    }

    match response_rx.await {
        Ok(FormResponse::Completed(answers)) => {
            if let Some(answer) = answers.first() {
                match &answer.value {
                    AnswerValue::Selected { value } if value == "approve" => {
                        PlanApproval::Approved
                    }
                    _ => PlanApproval::Abandoned,
                }
            } else {
                PlanApproval::Abandoned
            }
        }
        Ok(FormResponse::Cancelled) => PlanApproval::Abandoned,
        Err(_) => PlanApproval::NoInteraction,
    }
}
```

**Step 3: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass (existing tests use `None` handler so they skip the create path).

**Step 4: Commit**

```bash
git add crates/tools/src/plan_tool.rs
git commit -m "feat(tools): interactive plan approval via ask_user in PlanTool create"
```

---

### Task 10: Integration Tests

**Files:**
- Modify: `tests/learning_loop_test.rs` — update existing tests for new LearningStatus fields, add learning tool consolidation test

**Step 1: Update existing tests**

Update `tests/learning_loop_test.rs` to:
1. Fix `StrategyRecordRow` construction to include `tool_name`, `tool_success`, `tool_duration_ms`
2. Add a test that verifies the learning tool now returns data from strategy records

```rust
#[tokio::test]
async fn test_learning_handler_reads_strategy_records() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);

    // Insert a strategy record with tool info
    let row = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "req-learn".to_string(),
        predicted_strategy: "ToolAssisted".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 0,
        iterations_used: 2,
        max_iterations: 5,
        success: true,
        user_satisfaction: Some(1.0),
        response_time_ms: 1500,
        chat_id: Some("tg:test".to_string()),
        tool_name: Some("todo".to_string()),
        tool_success: Some(true),
        tool_duration_ms: Some(45),
    };
    repos.strategies.create(&row).await.unwrap();

    // Verify count
    let count = repos.strategies.count_all().await.unwrap();
    assert_eq!(count, 1);

    // Verify overall stats
    let stats = repos.strategies.get_overall_stats().await.unwrap();
    assert_eq!(stats.total_records, 1);
    assert!((stats.accuracy - 1.0).abs() < 0.01);

    // Verify tool stats
    let tool_stats = repos.strategies.get_tool_stats().await.unwrap();
    assert_eq!(tool_stats.len(), 1);
    assert_eq!(tool_stats[0].tool_name, "todo");
    assert_eq!(tool_stats[0].success_count, 1);
}
```

**Step 2: Run integration tests**

Run: `cargo nextest run --test learning_loop_test --no-capture`
Expected: All pass.

**Step 3: Run full workspace**

Run: `cargo nextest run --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: All tests pass, zero clippy warnings.

**Step 4: Format and commit**

```bash
cargo fmt --all
git add tests/learning_loop_test.rs
git commit -m "test: update learning loop integration tests for strategy consolidation"
```

---

### Task 11: Final Cleanup and Formatting

**Step 1: Run full workspace checks**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo nextest run --workspace
cargo test --workspace --doc
```

**Step 2: Fix any issues**

If there are warnings or failures, fix them.

**Step 3: Commit any remaining changes**

```bash
cargo fmt --all
git add -A
git commit -m "chore: format and fix clippy warnings after learning consolidation + plan approval"
```
