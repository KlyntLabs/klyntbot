# Wire Cognitive Architecture Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire three existing but disconnected cognitive components: pending memory UI, community_intelligence.rs git tracking, and trial preview messages_scored metric.

**Architecture:** All components already exist in code. Task 1 adds Tauri/dev-server commands to surface `PendingMemoryRepo` + routes low-confidence facts there during consolidation. Task 2 is a trivial git commit. Task 3 adds a `count_since` query to `StrategyRepo` and implements `EarlyTrialEvaluator` to populate `messages_scored`.

**Tech Stack:** Rust, SQLite (sqlx), Tauri 2, tokio, async_trait

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/desktop-shared/src/types.rs` | Modify | Add `PendingMemory` to `EntityKind` |
| `crates/app-core/src/state.rs` | Modify | Add `pending_memory_repo` field to `AppCore` |
| `crates/app-core/src/init/mod.rs` | Modify | Construct and store `PendingMemoryRepo` |
| `crates/desktop/src/commands/pending_memory.rs` | Create | Tauri commands for list/approve/dismiss |
| `crates/desktop/src/commands/mod.rs` | Modify | Register new command module |
| `crates/desktop/src/lib.rs` | Modify | Register commands with Tauri |
| `crates/cognitive/src/services/consolidation.rs` | Modify | Add `pending_repo` param, route low-confidence facts |
| `crates/cognitive/src/services/background.rs` | Modify | Pass `PendingMemoryRepo` to consolidation |
| `crates/cognitive/src/lib.rs` | Modify | Re-export updated `execute_memory_ops` signature |
| `crates/storage/src/repos/strategy.rs` | Modify | Add `count_since` query |
| `crates/app-core/src/adapters/trial_evaluator.rs` | Create | Implement `EarlyTrialEvaluator` trait |
| `crates/app-core/src/adapters/mod.rs` | Modify | Export new module |
| `crates/cognitive/src/mirror/subscribers/trial.rs` | Modify | Wire evaluator to populate `messages_scored` |
| `tests/integration/cognitive.rs` | Modify | Add pending memory integration test |

---

### Task 1: Commit `community_intelligence.rs`

**Files:**
- Stage: `crates/cognitive/src/services/community_intelligence.rs`

- [ ] **Step 1: Verify the file compiles**

Run: `cargo check -p cognitive`
Expected: compiles with 0 errors (the module is already declared in `mod.rs`)

- [ ] **Step 2: Stage and commit**

```bash
git add crates/cognitive/src/services/community_intelligence.rs
git commit -m "feat(cognitive): add community intelligence types and execution logic

Phase 6.5b types (CommunityContext, CommunityIntelligenceInput/Output,
CommunityRename/Merge/Split) and apply_intelligence/build_intelligence_input
functions. Already wired into Reforge service but was missing from git."
```

---

### Task 2: Add `PendingMemory` EntityKind variant

**Files:**
- Modify: `crates/desktop-shared/src/types.rs:48-63` (EntityKind enum)
- Test: `cargo check -p desktop-shared`

- [ ] **Step 1: Add the variant to EntityKind**

In `crates/desktop-shared/src/types.rs`, add `PendingMemory` to the enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityKind {
    Task,
    Project,
    Objective,
    Area,
    KeyResult,
    FocusSession,
    Productivity,
    Note,
    Notebook,
    Finance,
    Source,
    Conversation,
    MirrorSnippet,
    BrainVersion,
    PendingMemory,
}
```

- [ ] **Step 2: Add parse case for the new variant**

In the `EntityKind::parse` method, add the match arm:

```rust
"pending_memory" | "pendingmemory" => Some(Self::PendingMemory),
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p desktop-shared`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/types.rs
git commit -m "feat(desktop-shared): add PendingMemory entity kind"
```

---

### Task 3: Store `PendingMemoryRepo` in AppCore

**Files:**
- Modify: `crates/app-core/src/state.rs` (add field + accessor)
- Modify: `crates/app-core/src/init/mod.rs` (construct repo)

- [ ] **Step 1: Add field to AppCore**

In `crates/app-core/src/state.rs`, add after the `mirror_facade` field (~line 132):

```rust
/// Pending memory repo for user-confirmable facts (None when cognitive unavailable).
pub pending_memory_repo: Option<cognitive::repos::PendingMemoryRepo>,
```

- [ ] **Step 2: Add accessor method**

In the `impl AppCore` block (near the `mirror_facade()` method ~line 291), add:

```rust
/// Return pending memory repo or a "not available" error.
pub fn pending_memory_repo(&self) -> Result<&cognitive::repos::PendingMemoryRepo, ApiError> {
    self.pending_memory_repo
        .as_ref()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Pending memory repo not available"))
}
```

- [ ] **Step 3: Construct repo during init**

In `crates/app-core/src/init/mod.rs`, in the Phase 9 block (near line 389 where mirror is initialized), add after the MirrorRepo creation:

```rust
let pending_memory_repo = {
    let repo = cognitive::repos::PendingMemoryRepo::new(storage_pool.inner().clone());
    if let Err(e) = repo.migrate().await {
        tracing::warn!("Pending memory migration failed: {e}");
    }
    Some(repo)
};
```

- [ ] **Step 4: Wire into AppCore struct literal**

In the `AppCore { ... }` struct literal (near line 576), add:

```rust
pending_memory_repo,
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p app-core`
Expected: 0 errors (may need to add `None` for pending_memory_repo in test builders if any exist)

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): wire PendingMemoryRepo into AppCore"
```

---

### Task 4: Create Tauri commands for pending memory

**Files:**
- Create: `crates/desktop/src/commands/pending_memory.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Write the failing test — verify DEV_COMMANDS coverage**

The codebase has an existing test `dev_server_covers_all_tauri_commands` that verifies every module's `DEV_COMMANDS` are registered. First, create the command file so the test can find it. We'll verify the test catches our new module.

- [ ] **Step 2: Create the command file**

Create `crates/desktop/src/commands/pending_memory.rs`:

```rust
use std::sync::Arc;

use cognitive::repos::PendingMemoryRow;
use cognitive::types::SemanticFact;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use tauri::State;

use crate::app_core::AppCore;

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "list_pending_memories",
    "approve_pending_memory",
    "dismiss_pending_memory",
];

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMemoryResponse {
    pub id: String,
    pub fact: serde_json::Value,
    pub reason: String,
    pub created_at: String,
}

impl From<PendingMemoryRow> for PendingMemoryResponse {
    fn from(row: PendingMemoryRow) -> Self {
        Self {
            id: row.id.clone(),
            fact: serde_json::from_str(&row.fact_json).unwrap_or(serde_json::Value::Null),
            reason: row.reason,
            created_at: row.created_at,
        }
    }
}

#[tauri::command]
pub async fn list_pending_memories(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<PendingMemoryResponse>, ApiError> {
    let repo = state.pending_memory_repo()?;
    let rows = repo.list_pending(limit.unwrap_or(20)).await;
    Ok(rows.into_iter().map(PendingMemoryResponse::from).collect())
}

#[tauri::command]
pub async fn approve_pending_memory(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    let pending_repo = state.pending_memory_repo()?;

    // Load the pending row
    let rows = pending_repo.list_pending(100).await;
    let row = rows
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::new("NOT_FOUND", "pending memory not found"))?;

    // Deserialize the fact and upsert to semantic facts
    let fact: SemanticFact = serde_json::from_str(&row.fact_json)
        .map_err(|e| ApiError::new("PARSE_ERROR", &format!("invalid fact JSON: {e}")))?;

    let fact_repo = state.repos.cognitive_facts();
    fact_repo
        .upsert(&fact)
        .await
        .map_err(|e| ApiError::new("STORAGE", &e.to_string()))?;

    // Remove from pending
    pending_repo
        .remove(&id)
        .await
        .map_err(|e| ApiError::new("STORAGE", &e.to_string()))?;

    super::emit_entity_updated(&app, EntityKind::PendingMemory, &id);
    Ok(())
}

#[tauri::command]
pub async fn dismiss_pending_memory(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    let repo = state.pending_memory_repo()?;
    repo.remove(&id)
        .await
        .map_err(|e| ApiError::new("STORAGE", &e.to_string()))?;
    super::emit_entity_updated(&app, EntityKind::PendingMemory, &id);
    Ok(())
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};

    let repo = match core.pending_memory_repo() {
        Ok(r) => r,
        Err(e) => return Some(Err(e)),
    };

    Some(match cmd {
        "list_pending_memories" => {
            let limit: Option<i64> = dev::get(body, "limit");
            let rows = repo.list_pending(limit.unwrap_or(20)).await;
            let resp: Vec<PendingMemoryResponse> =
                rows.into_iter().map(PendingMemoryResponse::from).collect();
            dev::val(Ok::<_, ApiError>(resp))
        }
        "approve_pending_memory" => {
            let id: String = try_field!(dev::get(body, "id")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: id")));

            let rows = repo.list_pending(100).await;
            let row = match rows.iter().find(|r| r.id == id) {
                Some(r) => r,
                None => return Some(Err(ApiError::new("NOT_FOUND", "pending memory not found"))),
            };

            let fact: SemanticFact = match serde_json::from_str(&row.fact_json) {
                Ok(f) => f,
                Err(e) => {
                    return Some(Err(ApiError::new(
                        "PARSE_ERROR",
                        &format!("invalid fact JSON: {e}"),
                    )))
                }
            };

            let fact_repo = core.repos.cognitive_facts();
            if let Err(e) = fact_repo.upsert(&fact).await {
                return Some(Err(ApiError::new("STORAGE", &e.to_string())));
            }
            dev::val(repo.remove(&id).await.map_err(|e| ApiError::new("STORAGE", &e.to_string())))
        }
        "dismiss_pending_memory" => {
            let id: String = try_field!(dev::get(body, "id")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: id")));
            dev::val(repo.remove(&id).await.map_err(|e| ApiError::new("STORAGE", &e.to_string())))
        }
        _ => return None,
    })
}
```

- [ ] **Step 3: Register module in commands/mod.rs**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod pending_memory;
```

And in the `dispatch_dev` aggregator function (if it delegates to sub-modules), add the pending_memory dispatch. The pattern varies — check `mod.rs` for how mirror commands are routed and replicate.

- [ ] **Step 4: Register Tauri commands in lib.rs**

In `crates/desktop/src/lib.rs`, in the `.invoke_handler(tauri::generate_handler![...])` macro invocation, add:

```rust
commands::pending_memory::list_pending_memories,
commands::pending_memory::approve_pending_memory,
commands::pending_memory::dismiss_pending_memory,
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p desktop`
Expected: 0 errors

- [ ] **Step 6: Verify DEV_COMMANDS test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: PASS (the test should pick up the new module's DEV_COMMANDS)

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/pending_memory.rs crates/desktop/src/commands/mod.rs crates/desktop/src/lib.rs
git commit -m "feat(desktop): add pending memory Tauri commands

list_pending_memories, approve_pending_memory, dismiss_pending_memory
with dev server dispatch for browser-only dev."
```

---

### Task 5: Route low-confidence facts to pending during consolidation

**Files:**
- Modify: `crates/cognitive/src/services/consolidation.rs:58-106`
- Modify: `crates/cognitive/src/services/background.rs:596-602`
- Modify: `crates/cognitive/src/lib.rs:21`
- Test: `crates/cognitive/src/services/consolidation.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

In `crates/cognitive/src/services/consolidation.rs`, in the `#[cfg(test)] mod tests` block, add:

```rust
#[tokio::test]
async fn test_low_confidence_routed_to_pending() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool.clone());
    let pending_repo = crate::repos::PendingMemoryRepo::new(pool.clone());
    pending_repo.migrate().await.unwrap();

    let mut candidate = test_fact("f-low", "peak_hours", "maybe 10am");
    candidate.confidence = 0.35; // Below LOW_CONFIDENCE_THRESHOLD (0.5)

    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![],
    }];
    let ops = vec![MemoryOp::Add { id: "f-low".into() }];

    execute_memory_ops(&ops, &candidates, &repo, None, Some(&pending_repo)).await;

    // Should NOT be in semantic_facts
    let stored = repo.get("f-low").await.unwrap();
    assert!(stored.is_none(), "low-confidence fact should not be in semantic_facts");

    // Should be in pending_memories
    let pending = pending_repo.list_pending(10).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "f-low");
    assert!(pending[0].reason.contains("low_confidence"));
}

#[tokio::test]
async fn test_high_confidence_bypasses_pending() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool.clone());
    let pending_repo = crate::repos::PendingMemoryRepo::new(pool.clone());
    pending_repo.migrate().await.unwrap();

    let candidate = test_fact("f-high", "peak_hours", "10am-12pm"); // confidence 0.8

    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![],
    }];
    let ops = vec![MemoryOp::Add { id: "f-high".into() }];

    execute_memory_ops(&ops, &candidates, &repo, None, Some(&pending_repo)).await;

    // Should be in semantic_facts
    let stored = repo.get("f-high").await.unwrap();
    assert!(stored.is_some(), "high-confidence fact should be stored directly");

    // Should NOT be in pending_memories
    let pending = pending_repo.list_pending(10).await;
    assert!(pending.is_empty(), "high-confidence fact should not be pending");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(low_confidence_routed_to_pending)'`
Expected: FAIL (compile error — `execute_memory_ops` doesn't accept 5th arg yet)

- [ ] **Step 3: Add pending_repo parameter to execute_memory_ops**

In `crates/cognitive/src/services/consolidation.rs`, change the signature and add routing logic:

```rust
/// Facts with confidence below this threshold are routed to pending review.
const LOW_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Execute consolidation decisions against the repo and embedder.
///
/// Low-confidence Add operations are diverted to `pending_repo` (if provided)
/// instead of being stored directly, giving the user a chance to review.
pub async fn execute_memory_ops(
    ops: &[MemoryOp],
    candidates: &[ConsolidationCandidate],
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    pending_repo: Option<&crate::repos::PendingMemoryRepo>,
) {
    for (op, entry) in ops.iter().zip(candidates.iter()) {
        match op {
            MemoryOp::Add { .. } => {
                // Route low-confidence new facts to pending review
                if entry.candidate.confidence < LOW_CONFIDENCE_THRESHOLD {
                    if let Some(pending) = pending_repo {
                        if let Err(e) =
                            pending.insert(&entry.candidate, "low_confidence").await
                        {
                            warn!("Failed to insert pending memory '{}': {e}", entry.candidate.id);
                        } else {
                            debug!(
                                "Routed low-confidence fact '{}' to pending review",
                                entry.candidate.id
                            );
                        }
                        continue;
                    }
                }
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
                debug!("Consolidated: NOOP for candidate '{}'", entry.candidate.id);
            }
        }
    }
}
```

- [ ] **Step 4: Update the re-export in lib.rs**

In `crates/cognitive/src/lib.rs`, the re-export at line 21 doesn't need to change (same function name, just new optional param).

- [ ] **Step 5: Update the call site in background.rs**

In `crates/cognitive/src/services/background.rs` at line 596, add the pending_repo parameter. First, the pending_repo needs to be available in the background service. Find where the background service is constructed and add it as an optional field.

Add to the background service's field/closure captures (near where `repo`, `embedder`, etc. are captured):

```rust
let pending_repo_ref = pending_repo.as_ref();
```

Then update the call at line 596:

```rust
crate::consolidation::execute_memory_ops(
    &ops,
    &candidates,
    &repo,
    embedder.as_deref(),
    pending_repo_ref,
)
.await;
```

The `pending_repo` needs to be threaded into the background service from `CognitiveServiceBuilder`. Check how `embedder` is passed and replicate the pattern.

- [ ] **Step 6: Update existing consolidation tests**

The existing tests (`test_execute_memory_ops_add`, `test_execute_memory_ops_update`, `test_execute_memory_ops_noop`) call `execute_memory_ops` with 4 args. Add `None` as the 5th argument to each:

```rust
execute_memory_ops(&ops, &candidates, &repo, None, None).await;
```

- [ ] **Step 7: Run all consolidation tests**

Run: `cargo nextest run -p cognitive -E 'test(execute_memory_ops)'`
Expected: ALL PASS (existing + new tests)

- [ ] **Step 8: Run full cognitive test suite**

Run: `cargo nextest run -p cognitive`
Expected: ALL PASS

- [ ] **Step 9: Commit**

```bash
git add crates/cognitive/src/services/consolidation.rs crates/cognitive/src/services/background.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): route low-confidence facts to pending memory

Facts with confidence < 0.5 during consolidation are diverted to
pending_memories table for user review instead of direct storage.
High-confidence facts bypass pending and are stored immediately."
```

---

### Task 6: Add `count_since` to StrategyRepo

**Files:**
- Modify: `crates/storage/src/repos/strategy.rs`
- Test: inline test in same file

- [ ] **Step 1: Write the failing test**

In `crates/storage/src/repos/strategy.rs`, add a test module (or append to existing tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_count_since() {
        let pool = setup().await;
        let repo = StrategyRepo::new(pool);
        let since = Utc::now() - chrono::Duration::hours(1);

        let count = repo.count_since(since).await.unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(count_since)'`
Expected: FAIL (no method `count_since`)

- [ ] **Step 3: Implement count_since**

In `crates/storage/src/repos/strategy.rs`, add after `count_all()`:

```rust
/// Count strategy records since a given timestamp.
pub async fn count_since(&self, since: DateTime<Utc>) -> Result<i64, StorageError> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM strategy_records WHERE timestamp >= ?1")
            .bind(since.to_rfc3339())
            .fetch_one(&self.pool)
            .await?;
    Ok(count)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p storage -E 'test(count_since)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/strategy.rs
git commit -m "feat(storage): add StrategyRepo::count_since for trial metrics"
```

---

### Task 7: Implement EarlyTrialEvaluator

**Files:**
- Create: `crates/app-core/src/adapters/trial_evaluator.rs`
- Modify: `crates/app-core/src/adapters/mod.rs`

- [ ] **Step 1: Write the test**

The evaluator will be tested via the mirror integration test. First, create the implementation.

- [ ] **Step 2: Create the evaluator implementation**

Create `crates/app-core/src/adapters/trial_evaluator.rs`:

```rust
//! EarlyTrialEvaluator implementation — queries strategy_records and
//! domain_event_log to compute early trial signals at the 4-hour mark.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use storage::repos::{EventLogRepo, StrategyRepo};

use cognitive::mirror::types::{EarlyTrialEvaluator, TrendDirection, TrialEarlySignals};

/// Production evaluator that reads from strategy_records and domain_event_log.
pub struct AppTrialEvaluator {
    strategy_repo: StrategyRepo,
    event_log_repo: EventLogRepo,
}

impl AppTrialEvaluator {
    pub fn new(strategy_repo: StrategyRepo, event_log_repo: EventLogRepo) -> Self {
        Self {
            strategy_repo,
            event_log_repo,
        }
    }
}

#[async_trait]
impl EarlyTrialEvaluator for AppTrialEvaluator {
    async fn evaluate_trial_early(
        &self,
        _trial_id: &str,
        since: DateTime<Utc>,
    ) -> common::Result<TrialEarlySignals> {
        // Count messages (strategy records) since trial started
        let message_count = self
            .strategy_repo
            .count_since(since)
            .await
            .unwrap_or(0);

        // Count user corrections since trial started
        let corrections = self
            .event_log_repo
            .count_by_event_type_since("UserCorrectedAI", &since.to_rfc3339())
            .await
            .unwrap_or(0);

        // Compute correction rate delta (vs baseline of ~5% correction rate)
        let baseline_correction_rate = 0.05;
        let trial_correction_rate = if message_count > 0 {
            corrections as f64 / message_count as f64
        } else {
            0.0
        };
        let correction_rate_delta = baseline_correction_rate - trial_correction_rate;

        // Determine confidence trend from recent strategy records
        let confidence_trend = if message_count < 3 {
            TrendDirection::Stable
        } else {
            // Compare first-half vs second-half correction rates
            let midpoint = since + (Utc::now() - since) / 2;
            let first_half = self.strategy_repo.count_since(midpoint).await.unwrap_or(0);
            let second_half = message_count - first_half;

            if second_half > first_half + 2 {
                TrendDirection::Rising // More activity = rising engagement
            } else if first_half > second_half + 2 {
                TrendDirection::Falling
            } else {
                TrendDirection::Stable
            }
        };

        Ok(TrialEarlySignals {
            correction_rate_delta,
            confidence_trend,
            dominant_skill_shift: None,
        })
    }
}
```

- [ ] **Step 3: Export the module**

In `crates/app-core/src/adapters/mod.rs`, add:

```rust
pub mod trial_evaluator;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p app-core`
Expected: 0 errors. If `EventLogRepo::count_by_event_type_since` doesn't exist, we need to add it (see Step 5).

- [ ] **Step 5: Add EventLogRepo::count_by_event_type_since if missing**

Check if the method exists. If not, add to the event log repo (likely in `crates/storage/` or `crates/cognitive/src/repos/`):

```rust
pub async fn count_by_event_type_since(
    &self,
    event_type: &str,
    since: &str,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM domain_event_log WHERE event_type = ?1 AND timestamp >= ?2",
    )
    .bind(event_type)
    .bind(since)
    .fetch_one(&self.pool)
    .await?;
    Ok(count)
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/adapters/trial_evaluator.rs crates/app-core/src/adapters/mod.rs
git commit -m "feat(app-core): implement EarlyTrialEvaluator for trial preview metrics

Queries strategy_records for message count and domain_event_log for
correction count since trial start. Computes correction_rate_delta
and confidence_trend from first-half vs second-half activity."
```

---

### Task 8: Wire evaluator into MirrorEngine and fix messages_scored

**Files:**
- Modify: `crates/cognitive/src/mirror/subscribers/trial.rs:82-90`
- Modify: `crates/app-core/src/init/mod.rs` (mirror init block)

- [ ] **Step 1: Write the failing test**

In `crates/cognitive/src/mirror/subscribers/trial.rs`, add to the test module:

```rust
#[test]
fn test_compute_recommendation_with_real_message_count() {
    let signals = TrialEarlySignals {
        correction_rate_delta: 0.03,
        confidence_trend: TrendDirection::Rising,
        dominant_skill_shift: None,
    };
    // With enough messages (>= MIN_MESSAGES_FOR_KILL) and positive delta + rising
    assert_eq!(
        compute_recommendation(&signals, 10),
        PreviewRecommendation::Continue
    );
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(real_message_count)'`
Expected: PASS (this validates the recommendation logic works with non-zero messages)

- [ ] **Step 3: Update trial.rs to return messages_scored from evaluator**

In `crates/cognitive/src/mirror/subscribers/trial.rs`, add `messages_scored` to the `EarlyTrialEvaluator` trait return. But the trait is already defined — we need to add `messages_scored` as a field.

**Option A (simpler):** Add `messages_scored` to `TrialEarlySignals`:

In `crates/cognitive/src/mirror/types.rs`, add the field:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialEarlySignals {
    pub correction_rate_delta: f64,
    pub confidence_trend: TrendDirection,
    pub dominant_skill_shift: Option<String>,
    #[serde(default)]
    pub messages_scored: u32,
}
```

- [ ] **Step 4: Update the evaluator to set messages_scored**

In `crates/app-core/src/adapters/trial_evaluator.rs`, update the return:

```rust
Ok(TrialEarlySignals {
    correction_rate_delta,
    confidence_trend,
    dominant_skill_shift: None,
    messages_scored: message_count as u32,
})
```

- [ ] **Step 5: Update trial.rs to use signals.messages_scored**

In `crates/cognitive/src/mirror/subscribers/trial.rs`, replace line 90:

```rust
// Before:
let messages_scored = 0; // TODO: get from evaluator in Phase 5

// After:
let messages_scored = signals.messages_scored;
```

- [ ] **Step 6: Wire AppTrialEvaluator into MirrorEngine::start**

In `crates/app-core/src/init/mod.rs`, in the Phase 9 mirror initialization block, construct the evaluator and pass it:

```rust
let trial_evaluator: Option<Arc<dyn cognitive::mirror::EarlyTrialEvaluator>> = {
    let strategy_repo = storage::repos::StrategyRepo::new(storage_pool.inner().clone());
    let event_log_repo = repos.event_log.clone(); // or however EventLogRepo is accessed
    Some(Arc::new(
        crate::adapters::trial_evaluator::AppTrialEvaluator::new(
            strategy_repo,
            event_log_repo,
        ),
    ))
};
```

Then pass `trial_evaluator` to wherever `MirrorEngine::start` constructs the `TrialPreviewSubscriber`. This may require adding a parameter to `MirrorEngine::start` — check its current signature and add `evaluator: Option<Arc<dyn EarlyTrialEvaluator>>`.

- [ ] **Step 7: Run full test suite**

Run: `cargo nextest run -p cognitive -E 'test(trial)'`
Expected: ALL PASS

Run: `cargo nextest run -p app-core`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/mirror/subscribers/trial.rs crates/cognitive/src/mirror/types.rs crates/app-core/src/init/mod.rs crates/app-core/src/adapters/trial_evaluator.rs
git commit -m "feat(mirror): wire EarlyTrialEvaluator to populate messages_scored

Adds messages_scored field to TrialEarlySignals, implements
AppTrialEvaluator using strategy_records count, and wires it
into MirrorEngine init. Replaces the hardcoded 0 TODO."
```

---

### Task 9: Integration test for pending memory flow

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add integration test**

In `tests/integration/cognitive.rs`, add:

```rust
#[tokio::test]
async fn test_pending_memory_approve_flow() {
    let pool = common::test_pool().await;
    let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
    let pending_repo = cognitive::repos::PendingMemoryRepo::new(pool.clone());
    pending_repo.migrate().await.unwrap();

    // Insert a fact into pending
    let fact = cognitive::types::SemanticFact {
        id: "pending-test-1".into(),
        domain: "test".into(),
        subject: "user".into(),
        predicate: "might_prefer".into(),
        object: "dark mode".into(),
        confidence: 0.3,
        stability: 1.0,
        access_count: 0,
        convergence_score: 0.0,
        source: "test".into(),
        ..Default::default()
    };
    pending_repo.insert(&fact, "low_confidence").await.unwrap();

    // Verify it's pending
    let pending = pending_repo.list_pending(10).await;
    assert_eq!(pending.len(), 1);

    // Approve it
    let row = &pending[0];
    let approved: cognitive::types::SemanticFact =
        serde_json::from_str(&row.fact_json).unwrap();
    fact_repo.upsert(&approved).await.unwrap();
    pending_repo.remove(&row.id).await.unwrap();

    // Verify it's now in semantic_facts
    let stored = fact_repo.get("pending-test-1").await.unwrap();
    assert!(stored.is_some());

    // Verify it's no longer pending
    let pending = pending_repo.list_pending(10).await;
    assert!(pending.is_empty());
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo nextest run -E 'test(pending_memory_approve_flow)'`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/integration/cognitive.rs
git commit -m "test: add integration test for pending memory approve flow"
```

---

### Task 10: Final verification

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 2: Run format check**

Run: `cargo fmt --all --check`
Expected: 0 formatting issues

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: ALL PASS

- [ ] **Step 4: Verify git status is clean**

Run: `git status`
Expected: All changes committed, working tree clean
