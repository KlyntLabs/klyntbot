# feature-tasks Phase 1: Agentic Core MVP Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy `feature-todo` crate with a new `feature-tasks` crate that adds agentic task fields, activity logging, batch operations, and estimation history — while preserving all existing functionality.

**Architecture:** The new `feature-tasks` crate provides domain types (`Task`, `TaskExecution`, `TaskActivity`, etc.), handler traits (enrichment, embedding, decomposition, execution, planning), and a `TaskTool` with 20+ actions. Storage uses new `tasks`-prefixed tables in SQLite. The app-core handlers, desktop commands, and frontend types are updated to use the new schema. A SQL migration script copies data from legacy `actions` tables to the new `tasks` tables.

**Tech Stack:** Rust (async/await, sqlx, serde, chrono, async-trait), SQLite, TypeScript (React), Tailwind CSS, Tauri 2 IPC.

**Design Document:** The full design is in the conversation history preceding this plan. Key reference: the SQL schema (9 tables + 3 triggers), Rust domain types, 20 tool actions, 8 handler traits, and new DomainEvent variants.

---

## File Structure

### New Files (feature-tasks crate)

| File | Responsibility |
|---|---|
| `crates/feature-tasks/Cargo.toml` | Crate manifest with workspace dependencies |
| `crates/feature-tasks/migrations/001_create_tasks.sql` | Fresh schema: all 9 tables + indexes + triggers |
| `crates/feature-tasks/src/lib.rs` | `TasksFeature` implementing `FeaturePackage` |
| `crates/feature-tasks/src/types.rs` | All domain types: `Task`, `TaskExecution`, `TaskActivity`, `TaskSuggestion`, `DecompositionPlan`, `EstimationRecord`, enums, conversions |
| `crates/feature-tasks/src/config.rs` | `TasksConfig` (enhanced from `TodoConfig`) |
| `crates/feature-tasks/src/scoring.rs` | Urgency, priority weighting, plan scoring (from `tool/mod.rs`) |
| `crates/feature-tasks/src/complexity.rs` | `TaskComplexitySignals` (from `task_complexity.rs`) |
| `crates/feature-tasks/src/rrule.rs` | Recurrence utilities (from `rrule_utils.rs`) |
| `crates/feature-tasks/src/search.rs` | Hybrid search (from `search.rs`) |
| `crates/feature-tasks/src/handlers/mod.rs` | Re-exports all handler traits |
| `crates/feature-tasks/src/handlers/enrichment.rs` | `EnrichmentHandler` trait (enhanced: energy, task_type, acceptance_criteria) |
| `crates/feature-tasks/src/handlers/embedding.rs` | `EmbeddingHandler` trait (unchanged) |
| `crates/feature-tasks/src/handlers/progress.rs` | `ProgressHandler` re-export |
| `crates/feature-tasks/src/handlers/decomposition.rs` | `DecompositionHandler` trait (stub for Phase 2) |
| `crates/feature-tasks/src/handlers/execution.rs` | `TaskExecutionHandler` trait (stub for Phase 2) |
| `crates/feature-tasks/src/handlers/planning.rs` | `DayPlanningHandler` trait (stub for Phase 2) |
| `crates/feature-tasks/src/handlers/proactive.rs` | `ProactiveHandler` trait (stub for Phase 2) |
| `crates/feature-tasks/src/handlers/suggestion_applier.rs` | `SuggestionApplier` trait (stub for Phase 2, separate from ProactiveHandler) |
| `crates/feature-tasks/src/handlers/forecast.rs` | `ForecastHandler` trait (stub for Phase 2) |
| `crates/feature-tasks/src/forecast.rs` | Pure computation: similarity matching, deviation correction, velocity — no LLM needed (L4). Phase 2's LLM-enhanced impl goes in `agent/src/handlers/forecast.rs` (L5). |
| `crates/feature-tasks/src/cognitive_bridge.rs` | Typed helpers for extracting task-relevant cognitive facts (stub for Phase 2, L4-safe) |
| `crates/feature-tasks/src/tool/mod.rs` | `TaskTool` struct, `Tool` impl, builder pattern |
| `crates/feature-tasks/src/tool/actions/mod.rs` | Action module declarations |
| `crates/feature-tasks/src/tool/actions/create.rs` | `create` action handler |
| `crates/feature-tasks/src/tool/actions/mutate.rs` | `update`, `complete`, `delete` action handlers |
| `crates/feature-tasks/src/tool/actions/query.rs` | `show`, `list`, `summary`, `tree` action handlers |
| `crates/feature-tasks/src/tool/actions/focus.rs` | `focus`, `unfocus`, `log_time` action handlers |
| `crates/feature-tasks/src/tool/actions/deps.rs` | `add_dep`, `remove_dep` action handlers |
| `crates/feature-tasks/src/tool/actions/batch.rs` | `batch` action handler |
| `crates/feature-tasks/src/tool/actions/search.rs` | `search` (unified keyword/semantic/hybrid) |
| `crates/feature-tasks/src/tool/actions/recurrence.rs` | `recur`, `list_recurring`, `delete_recurring` |
| `crates/feature-tasks/src/tool/actions/plan.rs` | `plan_day` (enhanced scoring) |

### New Files (storage layer)

| File | Responsibility |
|---|---|
| `crates/storage/src/rows/task.rs` | `TaskRow`, `TaskActivityRow`, `TaskExecutionRow`, `TaskSuggestionRow`, `TaskDecompositionRow`, `TaskEstimationRow` |
| `crates/storage/src/repos/task_repo.rs` | `TaskRepo` with all CRUD + queries for new tables |

### Modified Files

| File | Changes |
|---|---|
| `Cargo.toml` (workspace root) | Add `feature-tasks` to `[workspace.members]` |
| `crates/storage/src/rows/mod.rs` | Add `pub mod task;` and re-exports |
| `crates/storage/src/repos/mod.rs` | Add `pub mod task_repo;`, add `tasks: TaskRepo` to `Repos` struct |
| `crates/storage/src/lib.rs` | Re-export new task types |
| `crates/bus/src/domain_events.rs` | Add 12 new `DomainEvent` variants (including `TaskExecutionProgress`) |
| `crates/cognitive/src/background.rs` | Handle new task events in observation conversion |
| `crates/cognitive/src/salience.rs` | Classify new events |
| `crates/desktop-shared/src/commands.rs` | Update `TaskResponse`, `TaskCreateParams`, `TaskUpdateParams` with new fields |
| `crates/app-core/src/handlers/tasks.rs` | Rewrite handlers to use `TaskRepo` instead of `ActionRepo` |
| `crates/desktop/src/commands/tasks.rs` | Update command wrappers for new params/responses |
| `desktop-ui/src/shared/types/tasks.ts` | Update `Task`, `TaskCreateParams`, `TaskUpdateParams` interfaces |

---

## Chunk 1: Storage Layer — Row Types & Migration SQL

### Task 1.1: Create TaskRow and related row types

**Files:**
- Create: `crates/storage/src/rows/task.rs`
- Modify: `crates/storage/src/rows/mod.rs`

- [ ] **Step 1: Create `task.rs` with `TaskRow` struct**

The `TaskRow` maps 1:1 to the `tasks` SQL table. All new agentic fields are included. Use the same patterns as `ActionRow` (sqlx `FromRow` derive, `#[sqlx(json)]` attribute on `Vec<String>` fields like tags, `Option<DateTime<Utc>>` for nullable timestamps). **Note:** There is no `JsonVec` wrapper type — use `#[sqlx(json)] pub tags: Vec<String>` directly.

```rust
// Key fields to add beyond existing ActionRow:
// task_type: String (default "manual")
// acceptance_criteria: Option<String>
// agent_config: Option<String> (JSON)
// execution_state: String (default "idle")
// spawned_execution_id: Option<String>
// context_snapshot: Option<String> (JSON)
// energy_level: Option<String> (default "medium")
// estimated_focus_blocks: Option<i32>
// actual_minutes: Option<i32>
// complexity_score: Option<i32>
// completed: bool (new explicit boolean)
```

Also define:
- `TaskActivityRow` — maps to `task_activity` table
- `TaskExecutionRow` — maps to `task_executions` table
- `TaskSuggestionRow` — maps to `task_suggestions` table
- `TaskDecompositionRow` — maps to `task_decompositions` table
- `TaskEstimationRow` — maps to `task_estimation_history` table
- `TaskAttachmentRow` — like `ActionAttachmentRow` but with `source` field
- `TaskTimeEntryRow` — like `ActionTimeEntryRow` but with `energy_level` field
- `TaskDependencyRow` — like `ActionDependencyRow` but with `dep_type` field

Reference: `crates/storage/src/rows/action.rs` for the existing pattern (sqlx `FromRow` derive, `#[sqlx(json)]` attribute for JSON-serialized Vec fields, etc.).

- [ ] **Step 2: Add module to `rows/mod.rs`**

Add `pub mod task;` and re-export all new row types. Keep existing `action` module untouched for backward compatibility during migration.

- [ ] **Step 3: Re-export from `crates/storage/src/lib.rs`**

Add re-exports for all new row types alongside existing action exports.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p storage`
Expected: PASS (no tests yet, just compilation)

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/rows/task.rs crates/storage/src/rows/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add TaskRow and related row types for feature-tasks"
```

### Task 1.2: Create TaskRepo with basic CRUD

**Files:**
- Create: `crates/storage/src/repos/task_repo.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: Write failing test for TaskRepo::add and TaskRepo::get**

In `task_repo.rs`, add `#[cfg(test)] mod tests` with:
```rust
#[tokio::test]
async fn test_add_and_get() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    // Run core migrations + feature-tasks migration
    let repo = TaskRepo::new(pool.inner().clone());
    let row = TaskRow { id: "test1234".into(), title: "Test task".into(), ... };
    let created = repo.add(&row).await.unwrap();
    assert_eq!(created.title, "Test task");
    let fetched = repo.get("test1234").await.unwrap();
    assert!(fetched.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(task_repo::tests::test_add_and_get)'`
Expected: FAIL (TaskRepo not defined)

- [ ] **Step 3: Implement TaskRepo struct with `add`, `get`, `get_or_err` methods**

Follow the same pattern as `ActionRepo`:
- `pub struct TaskRepo { pool: SqlitePool }`
- `pub fn new(pool: SqlitePool) -> Self`
- `pub async fn add(&self, row: &TaskRow) -> Result<TaskRow, StorageError>` — INSERT with all columns
- `pub async fn get(&self, id: &str) -> Result<Option<TaskRow>, StorageError>` — SELECT by id
- `pub async fn get_or_err(&self, id: &str) -> Result<TaskRow, StorageError>` — get + NotFound error

Key difference from ActionRepo: the INSERT SQL must include all new agentic columns.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p storage -E 'test(task_repo::tests::test_add_and_get)'`
Expected: PASS

- [ ] **Step 5: Write and implement `update` (TaskPatch), `delete`, `list` (TaskFilter)**

Define `TaskPatch` and `TaskFilter` structs in `task_repo.rs`. Follow the `ActionPatch`/`ActionFilter` pattern but include all new fields.

`TaskFilter` adds: `task_type`, `execution_state`, `energy_level`, `completed` filters.
`TaskPatch` adds: `task_type`, `acceptance_criteria`, `agent_config`, `execution_state`, `energy_level`, `complexity_score`, `completed`, `actual_minutes`.

Write tests for each:
```rust
#[tokio::test] async fn test_update_task() { ... }
#[tokio::test] async fn test_delete_task() { ... }
#[tokio::test] async fn test_list_with_filters() { ... }
```

- [ ] **Step 6: Run all TaskRepo tests**

Run: `cargo nextest run -p storage -E 'test(task_repo)'`
Expected: All PASS

- [ ] **Step 7: Implement child/subtask methods**

Port from ActionRepo:
- `get_children`, `count_children`, `count_children_bulk`, `get_subtree`
- `move_task` (with parent cycle detection)
- `cascade_complete`

Write tests for each.

- [ ] **Step 8: Implement focus/time/attachment/dependency methods**

Port from ActionRepo:
- `focus`, `unfocus`, `list_focused`
- `add_time_entry`, `close_time_entry`, `list_time_entries`
- `add_attachment`, `remove_attachment`, `list_attachments`
- `add_dependency`, `remove_dependency`, `get_blockers`, `incomplete_blockers`, `get_blocking`

New: dependency methods accept `dep_type` parameter (default "blocks").
New: time entry methods accept `energy_level` parameter.
New: attachment methods accept `source` parameter.

Write tests for each group.

- [ ] **Step 9: Implement new table methods (activity, execution, suggestion, estimation)**

These are new methods not in ActionRepo:
- `log_activity(task_id, activity_type, field_changed, old_value, new_value, actor_type, summary)` — INSERT into `task_activity`
- `list_activity(task_id, limit)` → `Vec<TaskActivityRow>`
- `create_execution(row: &TaskExecutionRow)` → `TaskExecutionRow`
- `update_execution(id, status, output_summary, error_message, metrics)` → `TaskExecutionRow`
- `list_executions(task_id)` → `Vec<TaskExecutionRow>`
- `create_suggestion(row: &TaskSuggestionRow)` → `TaskSuggestionRow`
- `resolve_suggestion(id, status)` → bool
- `list_pending_suggestions(task_id)` → `Vec<TaskSuggestionRow>`
- `record_estimation(row: &TaskEstimationRow)` → `TaskEstimationRow`
- `estimation_stats(tags)` → average deviation, count

Write tests for each group.

- [ ] **Step 10: Implement summary/aggregation methods**

- `summary()` → `TaskSummary` (counts by status group)
- `summary_by_group()` → `HashMap<String, i64>`
- `overdue()` → `Vec<TaskRow>`
- `search_by_keyword(query, limit)` → `Vec<TaskRow>`
- `to_context_string()` → `String`

Write tests.

- [ ] **Step 11: Add TaskRepo to Repos struct**

Modify `crates/storage/src/repos/mod.rs`:
```rust
pub struct Repos {
    // ... existing fields ...
    pub tasks: TaskRepo,  // NEW
}
```

Update `Repos::from_pool()` to initialize `TaskRepo`.

- [ ] **Step 12: Re-export new types from lib.rs**

Add `TaskRepo`, `TaskFilter`, `TaskPatch`, `TaskSummary` to storage re-exports.

- [ ] **Step 13: Run full storage test suite**

Run: `cargo nextest run -p storage`
Expected: All PASS (including existing tests unchanged)

- [ ] **Step 14: Commit**

```bash
git add crates/storage/src/repos/task_repo.rs crates/storage/src/repos/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add TaskRepo with full CRUD, activity log, execution, suggestion, and estimation support"
```

### Task 1.3: Write migration SQL

**Files:**
- Create: `crates/feature-tasks/migrations/001_create_tasks.sql`

- [ ] **Step 1: Write the full SQL schema**

Use the schema from the design document (9 tables, all indexes, 3 triggers). The SQL is in the conversation history under "2. Full SQL Schema".

Tables: `tasks`, `task_executions`, `task_activity`, `task_suggestions`, `task_attachments`, `task_time_entries`, `task_dependencies`, `task_decompositions`, `task_estimation_history`.

Triggers: `trg_tasks_updated_at`, `trg_tasks_completed`, `trg_execution_duration`.

- [ ] **Step 2: Verify SQL syntax**

Run in a test:
```rust
#[tokio::test]
async fn test_migration_sql_valid() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Run core migrations first (areas, projects, key_results must exist)
    // Then run feature-tasks migration
    sqlx::query(include_str!("../migrations/001_create_tasks.sql"))
        .execute(pool.inner())
        .await
        .unwrap();
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/migrations/
git commit -m "feat(feature-tasks): add migration 001 — full agentic task schema"
```

---

## Chunk 2: feature-tasks Crate — Types, Config, FeaturePackage

### Task 2.1: Create crate skeleton and Cargo.toml

**Files:**
- Create: `crates/feature-tasks/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `Cargo.toml`**

Based on `feature-todo/Cargo.toml` but with name `feature-tasks`:
```toml
[package]
name = "feature-tasks"
version.workspace = true
edition.workspace = true

[dependencies]
bus.workspace = true
common.workspace = true
tools-core.workspace = true
tools-core-macros.workspace = true
storage.workspace = true
async-trait.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono = { workspace = true }
uuid = { workspace = true }
rrule.workspace = true
sqlx.workspace = true
futures-util.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
```

- [ ] **Step 2: Add to workspace members AND workspace dependencies**

In root `Cargo.toml`:
1. Add `"crates/feature-tasks"` to `[workspace.members]`
2. Add `feature-tasks = { path = "crates/feature-tasks" }` to `[workspace.dependencies]` (required for downstream crates to use `feature-tasks.workspace = true`)

- [ ] **Step 3: Create minimal `src/lib.rs`**

```rust
//! feature-tasks: Agentic task management for klyntbot.
pub mod types;
pub mod config;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p feature-tasks`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/Cargo.toml crates/feature-tasks/src/lib.rs Cargo.toml
git commit -m "feat: create feature-tasks crate skeleton"
```

### Task 2.2: Implement domain types

**Files:**
- Create: `crates/feature-tasks/src/types.rs`

- [ ] **Step 1: Write tests for type conversions**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_task_type_default_is_manual() { ... }
    #[test]
    fn test_execution_state_default_is_idle() { ... }
    #[test]
    fn test_energy_level_default_is_medium() { ... }
    #[test]
    fn test_task_serde_round_trip() { ... }
    #[test]
    fn test_task_from_row_conversion() { ... }
    #[test]
    fn test_agent_config_serde() { ... }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-tasks -E 'test(types)'`
Expected: FAIL

- [ ] **Step 3: Implement all domain types**

Implement the types from the design document:

**Core task types:**
- `Task` (30+ fields, `From<TaskRow>` and `From<&Task> for TaskRow` conversions)
- `TaskStatus`, `TaskType`, `EnergyLevel` enums
- `ExecutionState` enum — the live task-level field: `Idle`, `AwaitingApproval`, `Queued`, `Running`, `Paused`, `Completed`, `Failed`
- `AgentConfig` struct (JSON-serializable) — includes `require_approval: bool`, `retry_policy: RetryPolicy`

**Execution types:**
- `TaskExecution`
- `ExecutionStatus` enum — the historical record status on `TaskExecution` rows: `Pending`, `Running`, `Completed`, `Failed`, `BudgetExceeded`, `Cancelled`, `TimedOut` (distinct from `ExecutionState` which is the live task field)
- `ExecuteResult` enum — `Started { execution_id: String }` | `AwaitingApproval { suggestion_id: String }`
- `ExecutionConfig` struct — `agent_profile`, `max_cost_usd`, `max_iterations`, `timeout_secs`, `allowed_tools`, `allowed_mcp`, `retry_policy`, `require_approval`, `progress_interval_secs`
- `RetryPolicy` struct with `default_for_task_type()` — agentic: `auto_retry=true, max_retries=2`; hybrid: `auto_retry=false`
- `ExecutionArtifact`
- `ContextSnapshot` struct — `facts: Vec<String>`, `parent_chain: Vec<String>`, `sibling_titles: Vec<String>`, `active_blockers: Vec<String>`, `captured_at: DateTime<Utc>`

**Activity types:**
- `TaskActivity`, `ActivityType`, `ActorType`

**Suggestion types:**
- `TaskSuggestion`
- `SuggestionType` enum (11 variants): `Reprioritize`, `Reschedule`, `Decompose`, `Delegate`, `Abandon`, `Merge`, `Unblock`, `AdjustEstimation`, `AdjustEnergy`, `WorkflowInsight`, `Execute`
- `SuggestionStatus` enum (6 variants): `Pending`, `Accepted`, `Applied`, `Dismissed`, `Expired`, `AutoApplied` — `Applied` = user-accepted then executed by SuggestionApplier; `AutoApplied` = confidence-gated automatic application
- `SuggestionCandidate` struct
- `SuggestionAction` enum (10 variants): `SetPriority`, `SetDueDate`, `TriggerDecomposition`, `ConvertToAgentic`, `Archive`, `MergeInto`, `RemoveBlocker`, `UpdateEstimationBaseline`, `SetEnergyLevel`, `Informational`
- `SuggestionTrigger` enum (9 variants): `TaskOverdue`, `TaskStale`, `ExecutionFailed`, `EstimationDeviation`, `WipLimitExceeded`, `BlockedChainStale`, `FocusAbandonedEarly`, `PeriodicScan`, `UserRequested`
- `SuggestionScope` struct

**Decomposition types:**
- `DecompositionContext` struct — `max_depth`, `max_subtasks_per_level`, `existing_subtasks`, `project_context`, `cognitive_facts: Vec<SemanticFact>`, `user_energy_profile: Option<EnergyProfile>`, `calendar_context: Option<Vec<CalendarBlock>>`
- `DecompositionResult` struct — `tree`, `confidence`, `reasoning`, `validation_warnings`
- `DecompositionTree`, `PlannedSubtask` (with `temp_id: String` and `dependencies: Vec<String>` — NOT index-based)
- `ValidationWarning`, `ValidationWarningKind` enum
- `DecompositionStatus` (for storage row)

**Planning types:**
- `PlanningContext` struct — `candidate_tasks: Vec<ScoredTask>` (pre-scored), `energy_profile`, `calendar_blocks`, `working_hours`, `cognitive_facts`, `now`, `max_tasks`, `completed_today`, `timezone`
- `ScoredTask` struct — `task`, `score`, `is_unblocked`, `dependent_count`
- `DayPlan` struct — `slots`, `locked_slots: Vec<PlanSlot>`, `total_work_mins`, `available_mins`, `utilization`, `reasoning`, `deferred`, `generated_at`
- `PlanSlot` struct — with `PlanSlotStatus` enum (`Pending`, `Active`, `Completed`, `Skipped`)
- `DeferredTask` struct
- `WorkingHours` struct — includes `lunch_start: NaiveTime` (default 12:00) for energy-matching time-of-day boundaries

**Forecast types:**
- `ForecastContext`, `TaskForecast`, `ProjectForecast`, `AccuracyReport`
- `ForecastRisk`, `RiskKind` enum, `ForecastMethodology`, `DataQuality` enum
- `AccuracyScope`, `AccuracyTrend` enum, `ComplexityBucket`, `TagAccuracy`, `EnergyAccuracy`

**Shared types (used across multiple handlers):**
- `EnergyProfile` struct — `peak_hours`, `low_energy_hours`, `avg_focus_duration_mins`, `preferred_task_size_mins`
- `CalendarBlock` struct — `title`, `start`, `end`, `is_busy`
- `ScopeOverrides` struct — per-project/area WIP limit and stale threshold overrides

**Existing types (ported from feature-todo):**
- `EstimationRecord`
- `Attachment`, `AttachmentType` (with new `Artifact` variant)
- `TimeEntry`, `TimeEntrySource` (with new `Agent` variant)

Key pattern: use `#[serde(rename_all = "camelCase")]` on structs and `#[serde(rename_all = "lowercase")]` on enums, matching existing codebase conventions.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p feature-tasks -E 'test(types)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/types.rs
git commit -m "feat(feature-tasks): implement all domain types with Row conversions"
```

### Task 2.3: Implement config

**Files:**
- Create: `crates/feature-tasks/src/config.rs`

- [ ] **Step 1: Write test for default config**

- [ ] **Step 2: Implement `TasksConfig`**

Based on `TodoConfig` but enhanced with all Phase 2-3 config fields (defined now to avoid config-breaking changes later):
```rust
pub struct TasksConfig {
    // Existing (from TodoConfig):
    pub max_focus_slots: usize,          // default 3
    pub focus_deadline_hours: u64,       // default 8
    pub timezone: String,                // default "UTC"
    pub enrichment: EnrichmentConfig,
    pub search: SearchConfig,
    // Phase 1 additions:
    pub auto_log_activity: bool,         // default true
    pub estimation_tracking: bool,       // default true
    pub default_energy_level: String,    // default "medium"
    // Phase 2-3 additions (defined now, consumed later):
    pub proactive_suggestions: bool,                    // default true
    pub suggestion_auto_apply_threshold: f64,           // default 0.83
    pub decomposition_auto_apply_threshold: f64,        // default 0.75
    pub working_hours: WorkingHours,                    // default 9-18
    pub max_plan_tasks: u32,                            // default 8
    pub auto_apply_day_plan: bool,                      // default false
    pub wip_limit: u32,                                 // default 5
    pub stale_task_days: u32,                           // default 5
    pub project_overrides: HashMap<String, ScopeOverrides>,  // default empty
    pub area_overrides: HashMap<String, ScopeOverrides>,     // default empty
    pub forecast_min_sample_size: u32,                  // default 5
    pub forecast_lookback_days: u32,                    // default 90
    pub cognitive_integration: bool,                    // default true
}
```

- [ ] **Step 3: Run test, verify pass**

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/config.rs
git commit -m "feat(feature-tasks): add TasksConfig with agentic settings"
```

### Task 2.4: Port utility modules

**Files:**
- Create: `crates/feature-tasks/src/scoring.rs`
- Create: `crates/feature-tasks/src/complexity.rs`
- Create: `crates/feature-tasks/src/rrule.rs`
- Create: `crates/feature-tasks/src/search.rs`

- [ ] **Step 1: Copy and adapt `scoring.rs` from `feature-todo/src/tool/mod.rs`**

Extract `calculate_urgency`, `priority_weight`, `calculate_age_days`, `calculate_score` as module-level functions (not impl methods). Update to take `&Task` instead of `&Action`.

- [ ] **Step 2: Copy and adapt `complexity.rs` from `feature-todo/src/task_complexity.rs`**

Update to use `TaskRepo` instead of `ActionRepo`, `Task` instead of `Action`.

- [ ] **Step 3: Copy and adapt `rrule.rs` from `feature-todo/src/rrule_utils.rs`**

No changes needed — this module is self-contained.

- [ ] **Step 4: Copy and adapt `search.rs` from `feature-todo/src/search.rs`**

Update to use `Task` instead of `Action`.

- [ ] **Step 5: Port all existing tests, run full suite**

Run: `cargo nextest run -p feature-tasks`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/src/scoring.rs crates/feature-tasks/src/complexity.rs crates/feature-tasks/src/rrule.rs crates/feature-tasks/src/search.rs
git commit -m "feat(feature-tasks): port scoring, complexity, rrule, search utilities"
```

### Task 2.5: Implement handler traits

**Files:**
- Create: `crates/feature-tasks/src/handlers/mod.rs`
- Create: `crates/feature-tasks/src/handlers/enrichment.rs`
- Create: `crates/feature-tasks/src/handlers/embedding.rs`
- Create: `crates/feature-tasks/src/handlers/progress.rs`
- Create: `crates/feature-tasks/src/handlers/decomposition.rs`
- Create: `crates/feature-tasks/src/handlers/execution.rs`
- Create: `crates/feature-tasks/src/handlers/planning.rs`
- Create: `crates/feature-tasks/src/handlers/proactive.rs`
- Create: `crates/feature-tasks/src/handlers/suggestion_applier.rs`
- Create: `crates/feature-tasks/src/handlers/forecast.rs`

- [ ] **Step 1: Implement existing handler traits (enrichment, embedding, progress)**

Port from `feature-todo` but enhance `EnrichmentResult` with new fields:
- `energy_level: Option<EnrichmentSuggestion<EnergyLevel>>`
- `task_type: Option<EnrichmentSuggestion<TaskType>>`
- `suggested_tags: Option<EnrichmentSuggestion<Vec<String>>>`
- `acceptance_criteria: Option<EnrichmentSuggestion<String>>`

- [ ] **Step 2: Define Phase 2 handler trait stubs**

For `DecompositionHandler`, `TaskExecutionHandler`, `DayPlanningHandler`, `ProactiveHandler`, `SuggestionApplier`, `ForecastHandler`: define the trait signatures with full doc comments. Include ALL supporting types required by the trait signatures — these must compile for the object-safety tests in Step 3.

**Required supporting types per handler** (see Phase 2-3 design spec for full field definitions):
- **DecompositionHandler**: `DecompositionContext`, `DecompositionResult`, `DecompositionTree`, `PlannedSubtask`, `ValidationWarning`, `ValidationWarningKind`
- **TaskExecutionHandler**: `ExecutionConfig`, `ExecuteResult`, `RetryPolicy`, `ContextSnapshot`
- **DayPlanningHandler**: `PlanningContext`, `ScoredTask`, `WorkingHours`, `DayPlan`, `PlanSlot`, `PlanSlotStatus`, `DeferredTask`
- **ProactiveHandler**: `SuggestionScope`, `SuggestionTrigger`, `SuggestionCandidate`, `SuggestionAction`
- **SuggestionApplier**: (uses `SuggestionAction` from ProactiveHandler)
- **ForecastHandler**: `ForecastContext`, `TaskForecast`, `ProjectForecast`, `AccuracyReport`, `AccuracyScope`, `ForecastRisk`, `RiskKind`, `ForecastMethodology`, `DataQuality`, `AccuracyTrend`
- **Shared**: `EnergyProfile`, `CalendarBlock`, `ScopeOverrides`

These are trait-only — no implementations yet (those come in Phase 2).

**Important:** `ForecastHandler` trait lives in L4 (`feature-tasks`) but must NOT import any L5 types. The LLM-enhanced implementation goes in `agent/src/handlers/forecast.rs` (L5) in Phase 2.

- [ ] **Step 3: Write object-safety tests for all traits**

```rust
#[tokio::test]
async fn test_decomposition_handler_is_object_safe() {
    struct NoOp;
    #[async_trait]
    impl DecompositionHandler for NoOp { ... }
    let _: Arc<dyn DecompositionHandler> = Arc::new(NoOp);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-tasks -E 'test(handlers)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/handlers/
git commit -m "feat(feature-tasks): define all handler traits (existing + Phase 2 stubs)"
```

### Task 2.6: Implement TasksFeature (FeaturePackage)

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs`

- [ ] **Step 1: Write test for FeaturePackage impl**

```rust
#[test]
fn test_feature_name() {
    // Verify TasksFeature::name() returns "tasks"
}
#[test]
fn test_migration_sql_not_empty() {
    let sql = TasksFeature::migration_sql();
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS tasks"));
}
```

- [ ] **Step 2: Implement `TasksFeature`**

```rust
pub struct TasksFeature {
    tool: Arc<TaskTool>,
}

impl FeaturePackage for TasksFeature {
    fn name(&self) -> &str { "tasks" }
    fn tools(&self) -> Vec<DynTool> { vec![self.tool.clone()] }
    fn migrations(&self) -> Vec<FeatureMigration> { /* version 1: fresh schema */ }
    fn config_key(&self) -> &str { "tasks" }
    fn default_config(&self) -> Value { serde_json::to_value(TasksConfig::default()).unwrap() }
    async fn health_check(&self) -> Result<HealthStatus> { /* summary check */ }
}
```

- [ ] **Step 3: Update `lib.rs` module declarations**

Add all module declarations and public re-exports.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-tasks`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/lib.rs
git commit -m "feat(feature-tasks): implement TasksFeature FeaturePackage"
```

---

## Chunk 3: Tool Actions — Core CRUD + Activity Logging

### Task 3.1: Implement TaskTool struct and create action

**Files:**
- Create: `crates/feature-tasks/src/tool/mod.rs`
- Create: `crates/feature-tasks/src/tool/actions/mod.rs`
- Create: `crates/feature-tasks/src/tool/actions/create.rs`

- [ ] **Step 1: Implement `TaskTool` struct**

```rust
pub struct TaskTool {
    pub(crate) repo: TaskRepo,
    pub(crate) max_focus_slots: usize,
    pub(crate) focus_deadline_hours: u64,
    pub(crate) timezone: String,
    pub(crate) enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    pub(crate) embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    pub(crate) embedding_store: Option<storage::VectorStore>,
    pub(crate) semantic_threshold: f64,
    pub(crate) rrf_k: u32,
    pub(crate) feedback_handler: Option<Arc<dyn EnrichmentFeedbackHandler>>,
    pub(crate) enrichment_threshold: f64,
    pub(crate) progress_handler: Option<Arc<dyn ProgressHandler>>,
    pub(crate) domain_bus: Option<Arc<DomainEventBus>>,
    // NEW Phase 2 handlers (Option for now):
    pub(crate) decomposition_handler: Option<Arc<dyn DecompositionHandler>>,
    pub(crate) execution_handler: Option<Arc<dyn TaskExecutionHandler>>,
    pub(crate) planning_handler: Option<Arc<dyn DayPlanningHandler>>,
    pub(crate) proactive_handler: Option<Arc<dyn ProactiveHandler>>,
    pub(crate) forecast_handler: Option<Arc<dyn ForecastHandler>>,
    // Config
    pub(crate) config: TasksConfig,
}
```

With builder methods: `new()`, `with_enrichment_handler()`, `with_embedding_handler()`, etc.

Implement `Tool` trait with `name() -> "task"`, `description()`, `parameters()` (JSON schema with all 20 actions), and `execute()` dispatch.

- [ ] **Step 2: Implement `handle_create` action**

Port from `feature-todo/src/tool/actions/add.rs` but:
- Accept new fields: `task_type`, `acceptance_criteria`, `energy_level`, `agent_config`
- Log activity: call `self.repo.log_activity(id, ActivityType::Created, ...)` after creation
- Emit `DomainEvent::TaskCreated` with `task_type` field

- [ ] **Step 3: Write integration test**

```rust
#[tokio::test]
async fn test_create_task() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Run migrations...
    let repo = TaskRepo::new(pool.inner().clone());
    let tool = TaskTool::new(repo, ...);
    let result = tool.execute(json!({
        "action": "create",
        "title": "Test task",
        "area_id": "area-1"
    }), &ctx).await.unwrap();
    assert!(result.contains("Task created"));
}
```

- [ ] **Step 4: Run test, verify pass**

Run: `cargo nextest run -p feature-tasks -E 'test(tool::tests::test_create)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/tool/
git commit -m "feat(feature-tasks): implement TaskTool struct and create action with activity logging"
```

### Task 3.2: Implement update, complete, delete actions

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/mutate.rs`

- [ ] **Step 1: Implement `handle_update`**

Port from `feature-todo` but:
- Accept new fields in patch: `task_type`, `acceptance_criteria`, `agent_config`, `energy_level`, `execution_state`
- For each changed field, log an activity entry: `self.repo.log_activity(id, ActivityType::Updated, field, old_value, new_value, ActorType::User, None)`
- To detect changes: fetch the row before update, compare with patch

- [ ] **Step 2: Implement `handle_complete`**

Port from `feature-todo` but:
- Record estimation history: if `estimated_minutes` was set, create `TaskEstimationRow` with actual_minutes from `total_tracked_secs` and calculate `deviation_pct`
- Log activity with `ActivityType::Completed`
- Emit `DomainEvent::TaskCompleted` with `deviation_pct`
- Emit `DomainEvent::EstimationRecorded` if estimation data available

- [ ] **Step 3: Implement `handle_delete`**

Same as existing, with activity log entry before deletion.

- [ ] **Step 4: Write tests for each action**

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo nextest run -p feature-tasks -E 'test(mutate)'`

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/mutate.rs
git commit -m "feat(feature-tasks): implement update, complete, delete with activity logging and estimation tracking"
```

### Task 3.3: Implement show, list, summary, tree actions

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/query.rs`

- [ ] **Step 1: Implement `handle_show`**

Port from `feature-todo` but:
- Include new fields in output: `task_type`, `execution_state`, `energy_level`, `acceptance_criteria`, `complexity_score`
- Include recent activity log entries (last 5)
- Include execution history summary (count of runs, last status)
- Include pending suggestions count

- [ ] **Step 2: Implement `handle_list`**

Port from `feature-todo` but add new filter parameters:
- `task_type` (manual/agentic/hybrid)
- `execution_state` (idle/queued/running/etc.)
- `energy_level` (low/medium/high/deep)
- `completed` (boolean)

- [ ] **Step 3: Implement `handle_summary`**

Enhanced: include agentic stats:
- Tasks by execution_state (running, queued, failed)
- Pending suggestions count
- Estimation accuracy (avg deviation from last 30 completions)

- [ ] **Step 4: Implement `handle_tree`** — port directly from `feature-todo`

- [ ] **Step 5: Write tests, verify pass**

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/query.rs
git commit -m "feat(feature-tasks): implement show, list, summary, tree with agentic fields"
```

### Task 3.4: Implement search action (unified)

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/search.rs`

- [ ] **Step 1: Implement unified `handle_search`**

Merge the three search actions from `feature-todo` into one smart action:
- If embedding handler is available and query looks like natural language → hybrid search
- Otherwise → keyword search
- Remove `search_semantic` and `search_hybrid` as separate actions
- Keep `threshold` and `limit` params

- [ ] **Step 2: Write tests**

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/search.rs
git commit -m "feat(feature-tasks): implement unified search action (auto keyword/semantic/hybrid)"
```

---

## Chunk 4: Ported Systems — Focus, Time, Deps, Recurrence, Batch

### Task 4.1: Port focus and time tracking

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/focus.rs`

- [ ] **Step 1: Implement `handle_focus`**

Port from `feature-todo` but:
- Accept `energy_level` parameter — recorded in time entry
- Log activity `TaskFocusStarted`
- Emit `DomainEvent::TaskFocusStarted { task_id, energy_level }`

- [ ] **Step 2: Implement `handle_unfocus`**

Port from `feature-todo` but:
- Log activity `TaskFocusEnded`
- Emit `DomainEvent::TaskFocusEnded { task_id, duration_secs }`

- [ ] **Step 3: Implement `handle_log_time`** — port directly

- [ ] **Step 4: Write tests, verify pass**

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/focus.rs
git commit -m "feat(feature-tasks): port focus/unfocus/log_time with energy level tracking"
```

### Task 4.2: Port dependency system

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/deps.rs`

- [ ] **Step 1: Implement `handle_add_dep` and `handle_remove_dep`**

Port from `feature-todo` but:
- Accept `dep_type` parameter (default "blocks", also supports "soft")
- Log activity `DependencyAdded` / `DependencyRemoved`
- Emit `DomainEvent::TaskBlocked` / `DomainEvent::TaskUnblocked`

- [ ] **Step 2: Write tests**

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/deps.rs
git commit -m "feat(feature-tasks): port dependency system with dep_type support"
```

### Task 4.3: Port recurrence system

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/recurrence.rs`

- [ ] **Step 1: Port `handle_recur`, `handle_list_recurring`, `handle_delete_recurring`**

Direct port from `feature-todo` — no changes needed except using `Task`/`TaskRepo` types.

- [ ] **Step 2: Write tests**

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/recurrence.rs
git commit -m "feat(feature-tasks): port recurrence system"
```

### Task 4.4: Implement batch operations

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/batch.rs`

- [ ] **Step 1: Implement `handle_batch`**

New action. Parameters:
- `operation`: "complete" | "delete" | "move" | "tag" | "reorder"
- `task_ids`: array of task IDs
- `project_id` (for move), `tags` (for tag), `positions` (for reorder)

Implementation:
```rust
match operation {
    "complete" => { for id in task_ids { self.handle_complete_single(id).await?; } }
    "delete" => { for id in task_ids { self.repo.delete(id).await?; } }
    "move" => { for id in task_ids { self.repo.move_task(id, None, project_id).await?; } }
    "tag" => { for id in task_ids { /* merge tags */ } }
    "reorder" => { /* update positions for all task_ids */ }
}
```

Log a single batch activity entry per operation.

- [ ] **Step 2: Write tests for each operation**

```rust
#[tokio::test] async fn test_batch_complete() { ... }
#[tokio::test] async fn test_batch_reorder() { ... }
```

- [ ] **Step 3: Run tests, verify pass**

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/batch.rs
git commit -m "feat(feature-tasks): implement batch operations (complete, delete, move, tag, reorder)"
```

### Task 4.5: Implement plan_day action

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/plan.rs`

- [ ] **Step 1: Implement `handle_plan_day`**

Port from `feature-todo`'s `handle_plan` but enhanced:
- Use the scoring system from `scoring.rs`
- Add energy-level matching: if time of day is known, prefer tasks whose `energy_level` matches
- Accept `count` parameter (default 5)
- Emit `DomainEvent::DayPlanGenerated`

Note: The full `DayPlanningHandler` integration comes in Phase 2. For Phase 1, this is the local scoring-based plan.

- [ ] **Step 2: Write tests**

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/plan.rs
git commit -m "feat(feature-tasks): implement plan_day with energy-aware scoring"
```

---

## Chunk 5: DomainEvent Integration

### Task 5.1: Add new DomainEvent variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add new variants to DomainEvent enum**

Add these variants (keeping all existing ones):
```rust
TaskDecomposed { source_task_id: String, subtask_ids: Vec<String>, total_estimated_mins: Option<i64> },
TaskExecutionStarted { task_id: String, execution_id: String, agent_profile: String },
TaskExecutionCompleted { task_id: String, execution_id: String, tokens_used: u64, cost_usd: Option<f64>, artifacts_count: u32 },
TaskExecutionFailed { task_id: String, execution_id: String, error: String, retry_count: u32 },
TaskBlocked { task_id: String, blocker_id: String },
TaskUnblocked { task_id: String, was_blocked_by: String },
DayPlanGenerated { task_count: u32, total_estimated_mins: u32 },
ProactiveSuggestionCreated { suggestion_id: String, suggestion_type: String, task_id: Option<String>, confidence: f64 },
TaskFocusStarted { task_id: String, energy_level: String },
TaskFocusEnded { task_id: String, duration_secs: u64 },
EstimationRecorded { task_id: String, estimated_mins: u32, actual_mins: u32, deviation_pct: f64 },
TaskExecutionProgress { task_id: String, execution_id: String, current_step: String, percentage: Option<u8>, latest_tool: Option<String>, reasoning_snippet: Option<String>, cost_so_far_usd: f64, elapsed_secs: u64 },
```

Also enhance existing `TaskCreated` to include `task_type: String`.
Enhance existing `TaskCompleted` to include `deviation_pct: Option<f64>`.

**⚠️ Breaking change:** Modifying `TaskCreated` and `TaskCompleted` variants will break pattern matches in **all** of these files — every one must be updated:
- `crates/cognitive/src/background.rs` — observation conversions
- `crates/cognitive/src/salience.rs` — salience classification
- `crates/activity-log/src/normalizers.rs` — event normalization
- `crates/activity-log/tests/integration.rs` — test fixtures constructing TaskCreated/TaskCompleted
- `crates/app-core/src/init.rs` — event subscription handler
- `crates/app-core/src/handlers/tasks.rs` — TaskCreated emission site
- `crates/desktop/src/app_core.rs` — event handler
- `crates/desktop/src/dev_server.rs` — event handler

- [ ] **Step 2: Verify bus crate compiles**

Run: `cargo build -p bus`

- [ ] **Step 3: Fix all downstream compilation errors**

Fix **every** file that pattern-matches or constructs `TaskCreated` or `TaskCompleted`:

In `salience.rs`, classify new events:
```rust
DomainEvent::TaskFocusStarted { .. } => SalienceVerdict::Accumulate,
DomainEvent::TaskFocusEnded { .. } => SalienceVerdict::Accumulate,
DomainEvent::EstimationRecorded { .. } => SalienceVerdict::Accumulate,
DomainEvent::TaskExecutionCompleted { .. } => SalienceVerdict::ExtractNow,
DomainEvent::TaskExecutionFailed { .. } => SalienceVerdict::ExtractNow,
DomainEvent::TaskExecutionProgress { .. } => SalienceVerdict::Accumulate,
// etc.
```

In `background.rs`, add observation conversions for new events.

In `activity-log/src/normalizers.rs`, update the `TaskCreated`/`TaskCompleted` match arms to destructure the new fields (can use `..` rest pattern).

In `activity-log/tests/integration.rs`, update all test fixtures that construct `TaskCreated` or `TaskCompleted` to include `task_type` / `deviation_pct`.

In `app-core/src/init.rs`, update the event subscription match arm for `TaskCreated`/`TaskCompleted`.

In `app-core/src/handlers/tasks.rs`, update the `TaskCreated` emission to include `task_type`.

In `desktop/src/app_core.rs` and `desktop/src/dev_server.rs`, update event handlers for the modified variants.

- [ ] **Step 4: Run full workspace build**

Run: `cargo build --workspace`
Expected: PASS

- [ ] **Step 5: Run bus, cognitive, activity-log, and app-core tests**

Run: `cargo nextest run -p bus -p cognitive -p activity-log -p app-core`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/cognitive/src/background.rs crates/cognitive/src/salience.rs crates/activity-log/src/normalizers.rs crates/activity-log/tests/integration.rs crates/app-core/src/init.rs crates/app-core/src/handlers/tasks.rs crates/desktop/src/app_core.rs crates/desktop/src/dev_server.rs
git commit -m "feat(bus): add 11 new DomainEvent variants for agentic task system"
```

---

## Chunk 6: App-Core & Desktop Integration

### Task 6.1: Update desktop-shared IPC types

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs`

- [ ] **Step 1: Update `TaskResponse` struct**

Add new fields (all optional to maintain backward compat during transition):
```rust
pub struct TaskResponse {
    // ... existing fields ...
    // NEW:
    pub task_type: Option<String>,           // "manual" | "agentic" | "hybrid"
    pub execution_state: Option<String>,      // "idle" | "queued" | "running" | etc.
    pub energy_level: Option<String>,         // "low" | "medium" | "high" | "deep"
    pub acceptance_criteria: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub complexity_score: Option<i32>,
    pub total_tracked_secs: Option<i64>,
    pub focused_at: Option<String>,
}
```

- [ ] **Step 2: Update `TaskCreateParams`**

Add:
```rust
pub task_type: Option<String>,
pub acceptance_criteria: Option<String>,
pub energy_level: Option<String>,
pub estimated_minutes: Option<i32>,
```

- [ ] **Step 3: Update `TaskUpdateParams`**

Add:
```rust
pub task_type: Option<String>,
pub acceptance_criteria: Option<Option<String>>,
pub energy_level: Option<String>,
pub execution_state: Option<String>,
pub estimated_minutes: Option<Option<i32>>,
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p desktop-shared`

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop-shared): add agentic fields to TaskResponse and mutation params"
```

### Task 6.2: Update app-core task handlers

**Files:**
- Modify: `crates/app-core/src/handlers/tasks.rs`

- [ ] **Step 1: Update `action_to_task` converter**

Map new `TaskRow` fields to `TaskResponse`:
```rust
task_type: Some(row.task_type.clone()),
execution_state: Some(row.execution_state.clone()),
energy_level: row.energy_level.clone(),
acceptance_criteria: row.acceptance_criteria.clone(),
estimated_minutes: row.estimated_minutes,
actual_minutes: row.actual_minutes,
complexity_score: row.complexity_score,
total_tracked_secs: Some(row.total_tracked_secs),
focused_at: row.focused_at.map(|dt| dt.to_rfc3339()),
```

- [ ] **Step 2: Update `task_create` handler**

Pass new fields from `TaskCreateParams` to storage:
```rust
let row = TaskRow {
    // ... existing mappings ...
    task_type: params.task_type.unwrap_or_else(|| "manual".to_string()),
    acceptance_criteria: params.acceptance_criteria,
    energy_level: params.energy_level,
    estimated_minutes: params.estimated_minutes,
    // defaults for other new fields
    agent_config: None,
    execution_state: "idle".to_string(),
    spawned_execution_id: None,
    context_snapshot: None,
    estimated_focus_blocks: None,
    actual_minutes: None,
    complexity_score: None,
    completed: false,
};
```

- [ ] **Step 3: Update `task_update` handler**

Map new fields from `TaskUpdateParams` to `TaskPatch`.

- [ ] **Step 4: Switch from `self.repos.actions` to `self.repos.tasks`**

Replace all `self.repos.actions.*` calls with `self.repos.tasks.*` calls.

**Important:** Do this incrementally. Before switching, verify `TaskRepo` implements **every** method called by app-core handlers. The required methods are (cross-reference with `crates/app-core/src/handlers/tasks.rs`):
- `get()`, `get_or_err()`, `add()`, `update()` (with `TaskPatch`), `delete()`
- `list()` (with `TaskFilter`), `overdue()`
- `get_children()`, `count_children_bulk()`
- `cascade_complete()`
- `summary()`
- `focus()`, `unfocus()`, `list_focused()`
- `add_time_entry()`, `close_time_entry()`
- `search_by_keyword()`

Steps:
1. First, verify all methods above exist on `TaskRepo` with compatible signatures
2. Then switch each handler method one at a time
3. Run `cargo build -p app-core` after each switch

- [ ] **Step 5: Update DomainEvent emissions**

Update `TaskCreated` emission to include `task_type`:
```rust
bus.publish(DomainEvent::TaskCreated {
    task_id: id.clone(),
    project: created.project_id.clone(),
    task_type: created.task_type.clone(),
    estimate_mins: created.estimated_minutes.map(|m| m as i64),
});
```

- [ ] **Step 6: Run app-core tests**

Run: `cargo nextest run -p app-core`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/tasks.rs
git commit -m "feat(app-core): migrate task handlers from ActionRepo to TaskRepo"
```

### Task 6.3: Update desktop commands

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`

- [ ] **Step 1: Update command handlers for new params**

The desktop commands are thin wrappers — they just need to pass through the new fields. Most changes are automatic since the underlying types (`TaskCreateParams`, `TaskUpdateParams`, `TaskResponse`) have been updated.

Verify each command still works by checking the serde deserialization handles new optional fields.

- [ ] **Step 2: Add new batch command (optional)**

```rust
#[tauri::command]
pub async fn task_batch(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    operation: String,
    task_ids: Vec<String>,
    params: Option<serde_json::Value>,
) -> Result<Vec<TaskResponse>, ApiError> {
    // Delegate to app-core batch handler
}
```

- [ ] **Step 3: Verify desktop crate compiles**

Run: `cargo build -p desktop`

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs
git commit -m "feat(desktop): update task commands for agentic fields"
```

### Task 6.4: Update frontend TypeScript types

**Files:**
- Modify: `desktop-ui/src/shared/types/tasks.ts`

- [ ] **Step 1: Update `Task` interface**

```typescript
export interface Task {
  // ... existing fields ...
  // NEW:
  taskType?: "manual" | "agentic" | "hybrid";
  executionState?: "idle" | "queued" | "running" | "paused" | "completed" | "failed";
  energyLevel?: "low" | "medium" | "high" | "deep";
  acceptanceCriteria?: string;
  estimatedMinutes?: number;
  actualMinutes?: number;
  complexityScore?: number;
  totalTrackedSecs?: number;
  focusedAt?: string;
}
```

- [ ] **Step 2: Update `TaskCreateParams`**

```typescript
export interface TaskCreateParams {
  // ... existing fields ...
  taskType?: "manual" | "agentic" | "hybrid";
  acceptanceCriteria?: string;
  energyLevel?: "low" | "medium" | "high" | "deep";
  estimatedMinutes?: number;
}
```

- [ ] **Step 3: Update `TaskUpdateParams`**

```typescript
export interface TaskUpdateParams {
  // ... existing fields ...
  taskType?: string;
  acceptanceCriteria?: string | null;
  energyLevel?: string;
  executionState?: string;
  estimatedMinutes?: number | null;
}
```

- [ ] **Step 4: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/types/tasks.ts
git commit -m "feat(desktop-ui): add agentic fields to Task TypeScript types"
```

---

## Chunk 7: Data Migration & Cleanup

### Task 7.1: Write legacy data migration

**Files:**
- Create: `crates/feature-tasks/migrations/002_migrate_from_legacy_todo.sql`

- [ ] **Step 1: Write migration SQL**

Use the migration SQL from the design document. This copies data from `actions` → `tasks`, `action_attachments` → `task_attachments`, `action_time_entries` → `task_time_entries`, `action_dependencies` → `task_dependencies`, and generates initial activity log entries.

Key: this migration should be idempotent (use `INSERT OR IGNORE` or check if data already exists).

- [ ] **Step 2: Register as FeatureMigration version 2**

In `TasksFeature::migrations()`:
```rust
vec![
    FeatureMigration {
        feature_name: "tasks".to_string(),
        version: 1,
        description: "Create agentic task tables".to_string(),
        sql: include_str!("../migrations/001_create_tasks.sql").to_string(),
    },
    FeatureMigration {
        feature_name: "tasks".to_string(),
        version: 2,
        description: "Migrate data from legacy todo system".to_string(),
        sql: include_str!("../migrations/002_migrate_from_legacy_todo.sql").to_string(),
    },
]
```

- [ ] **Step 3: Write integration test**

```rust
#[tokio::test]
async fn test_legacy_migration() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Run core migrations (areas, projects, etc.)
    // Run feature-todo migration (creates actions table)
    // Insert test data into actions table
    // Run feature-tasks migrations
    // Verify data exists in tasks table
    // Verify activity log has migration entries
}
```

- [ ] **Step 4: Run test, verify pass**

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/migrations/002_migrate_from_legacy_todo.sql
git commit -m "feat(feature-tasks): add legacy data migration from feature-todo"
```

### Task 7.2: Wire feature-tasks into agent initialization

**Files:**
- Modify: Files in `crates/agent/` that register `TodoFeature`

- [ ] **Step 1: Find where TodoFeature is instantiated**

Search for `TodoFeature::new` in the agent crate. Replace with `TasksFeature::new`.

- [ ] **Step 2: Update feature registration**

Replace `feature-todo` with `feature-tasks` in the agent's feature package list. Keep `feature-todo` as a dependency temporarily for the migration to work (both migrations need to run: todo's creates the old tables, tasks' creates new tables and migrates data).

- [ ] **Step 3: Update Cargo.toml dependencies**

In the agent crate's `Cargo.toml`, add `feature-tasks` dependency and keep `feature-todo` temporarily.

- [ ] **Step 4: Run full workspace build and tests**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(agent): wire feature-tasks into agent initialization"
```

### Task 7.3: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```
Expected: All PASS, 0 clippy warnings

- [ ] **Step 2: Run desktop app end-to-end**

```bash
cd desktop-ui && bun run dev &
cargo run -p dev-api
```

Open http://localhost:1420, verify:
- Tasks page loads
- Can create a task
- Can update/complete/delete a task
- Side panel shows task details with new fields
- Subtasks work

- [ ] **Step 3: Final commit**

```bash
git commit -m "feat(feature-tasks): Phase 1 complete — agentic core MVP"
```

---

## Summary

| Chunk | Tasks | Focus |
|---|---|---|
| **1** | 1.1–1.3 | Storage layer: row types, TaskRepo, migration SQL |
| **2** | 2.1–2.6 | feature-tasks crate: types, config, handlers, FeaturePackage |
| **3** | 3.1–3.4 | Core tool actions: create, update, complete, delete, show, list, summary, search |
| **4** | 4.1–4.5 | Ported systems: focus, time, deps, recurrence, batch, plan_day |
| **5** | 5.1 | DomainEvent integration: new variants + cognitive handling |
| **6** | 6.1–6.4 | App-core, desktop commands, frontend types |
| **7** | 7.1–7.3 | Data migration, agent wiring, final verification |

Total: ~25 tasks, ~100 steps. Estimated parallel execution with 4-5 subagents: Chunks 1-2 can run in parallel (storage + crate setup), then Chunks 3-4 in parallel (tool actions), then Chunks 5-7 sequential (integration).
