# PARA + OKR Task System Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the flat project + goals system with PARA (Areas) + OKR (Objectives, Key Results) hierarchy, giving users structured life management through AI chat.

**Architecture:** New `feature-area` and `feature-okr` crates following the established `feature-*` pattern. A single migration (`005_para_okr.sql`) creates new tables, modifies existing ones, and drops legacy `goals`/`goal_project_links`. The `goal` crate is deleted entirely. Four tools total: `area`, `project` (modified), `okr`, `todo` (modified).

**Tech Stack:** Rust, SQLite (sqlx), async-trait, serde, chrono, uuid, Axum (dashboard)

**Design doc:** `docs/plans/2026-02-28-para-okr-redesign-design.md`

---

## Task 1: Write the SQL migration

**Files:**
- Create: `crates/storage/migrations/005_para_okr.sql`

**Step 1: Write the migration**

```sql
-- 005_para_okr.sql: PARA + OKR restructuring
-- Creates: areas, objectives, key_results
-- Modifies: projects (add area_id), todos (add area_id, key_result_id), plans (replace goal_id)
-- Drops: goal_project_links, goals

-- ============================================================
-- Areas (PARA top-level organizer)
-- ============================================================
CREATE TABLE areas (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    icon        TEXT,
    color       TEXT,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    archived    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Add area_id to projects (required FK)
-- ============================================================
ALTER TABLE projects ADD COLUMN area_id TEXT NOT NULL DEFAULT '' REFERENCES areas(id);
CREATE INDEX idx_projects_area ON projects(area_id);

-- ============================================================
-- Add area_id and key_result_id to todos
-- ============================================================
ALTER TABLE todos ADD COLUMN area_id TEXT NOT NULL DEFAULT '' REFERENCES areas(id);
ALTER TABLE todos ADD COLUMN key_result_id TEXT REFERENCES key_results(id) ON DELETE SET NULL;
CREATE INDEX idx_todos_area ON todos(area_id);
CREATE INDEX idx_todos_key_result ON todos(key_result_id) WHERE key_result_id IS NOT NULL;

-- ============================================================
-- Objectives (OKR - belongs to a Project)
-- ============================================================
CREATE TABLE objectives (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'active',
    priority    INTEGER,
    start_date  TEXT,
    end_date    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_objectives_project ON objectives(project_id);
CREATE INDEX idx_objectives_status ON objectives(status);

-- ============================================================
-- Key Results (belongs to an Objective)
-- ============================================================
CREATE TABLE key_results (
    id              TEXT PRIMARY KEY,
    objective_id    TEXT NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT,
    target_value    REAL NOT NULL DEFAULT 100.0,
    current_value   REAL NOT NULL DEFAULT 0.0,
    unit            TEXT NOT NULL DEFAULT '%',
    status          TEXT NOT NULL DEFAULT 'active',
    auto_track      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_key_results_objective ON key_results(objective_id);
CREATE INDEX idx_key_results_status ON key_results(status);

-- ============================================================
-- Recreate plans table (replace goal_id with objective_id + key_result_id)
-- ============================================================
CREATE TABLE plans_new (
    id                 TEXT PRIMARY KEY,
    session_key        TEXT NOT NULL,
    objective_id       TEXT REFERENCES objectives(id) ON DELETE SET NULL,
    key_result_id      TEXT REFERENCES key_results(id) ON DELETE SET NULL,
    title              TEXT NOT NULL,
    description        TEXT NOT NULL DEFAULT '',
    status             TEXT NOT NULL DEFAULT 'Draft',
    current_step_index INTEGER NOT NULL DEFAULT 0,
    iteration_limit    INTEGER NOT NULL DEFAULT 50,
    backtrack_history  TEXT NOT NULL DEFAULT '[]',
    visibility         TEXT NOT NULL DEFAULT 'Transparent',
    task_id            TEXT REFERENCES todos(id) ON DELETE SET NULL,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at       TEXT
);

INSERT INTO plans_new (id, session_key, title, description, status, current_step_index,
    iteration_limit, backtrack_history, visibility, task_id, created_at, updated_at, completed_at)
SELECT id, session_key, title, description, status, current_step_index,
    iteration_limit, backtrack_history, visibility, task_id, created_at, updated_at, completed_at
FROM plans;

DROP TABLE plans;
ALTER TABLE plans_new RENAME TO plans;

CREATE INDEX idx_plans_session_status ON plans(session_key, status);
CREATE INDEX idx_plans_objective ON plans(objective_id) WHERE objective_id IS NOT NULL;
CREATE INDEX idx_plans_key_result ON plans(key_result_id) WHERE key_result_id IS NOT NULL;

-- Recreate plan_steps FK (required after plans table recreation)
CREATE TABLE plan_steps_new (
    id             TEXT PRIMARY KEY,
    plan_id        TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    step_index     INTEGER NOT NULL,
    description    TEXT NOT NULL,
    reasoning      TEXT NOT NULL DEFAULT '',
    expected_tools TEXT NOT NULL DEFAULT '[]',
    status         TEXT NOT NULL DEFAULT 'Pending',
    attempt_count  INTEGER NOT NULL DEFAULT 0,
    max_attempts   INTEGER NOT NULL DEFAULT 3,
    result         TEXT,
    started_at     TEXT,
    completed_at   TEXT
);

INSERT INTO plan_steps_new SELECT * FROM plan_steps;
DROP TABLE plan_steps;
ALTER TABLE plan_steps_new RENAME TO plan_steps;
CREATE INDEX idx_plan_steps_plan_id ON plan_steps(plan_id);

-- ============================================================
-- Drop legacy goal tables
-- ============================================================
DROP TABLE IF EXISTS goal_project_links;
DROP TABLE IF EXISTS goals;
```

**Note:** The `ALTER TABLE todos ADD COLUMN key_result_id` references `key_results(id)` which is created later in the migration. SQLite defers FK checks, but the `key_results` table must be created before any inserts happen. Since `ALTER TABLE ADD COLUMN` doesn't insert rows, this is safe. However, if SQLite rejects forward FK references in ALTER TABLE, reorder to create `key_results` before the ALTER.

**Step 2: Verify migration order is correct**

Run: `cargo build -p storage 2>&1 | head -20`
Expected: Compiles (migrations are embedded at compile time via `sqlx::migrate!`)

**Step 3: Run tests to verify migration applies cleanly**

Run: `cargo nextest run -p storage 2>&1 | tail -20`
Expected: All storage tests pass (they use `StoragePool::connect_in_memory()` which runs all migrations)

**Step 4: Commit**

```bash
git add crates/storage/migrations/005_para_okr.sql
git commit -m "feat(storage): add PARA + OKR migration (005_para_okr.sql)"
```

---

## Task 2: Storage layer — row structs and repos for Areas

**Files:**
- Create: `crates/storage/src/rows/area.rs`
- Create: `crates/storage/src/repos/area_repo.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/lib.rs`

**Step 1: Write the AreaRow struct**

Create `crates/storage/src/rows/area.rs`:

```rust
//! Row struct for the `areas` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Step 2: Write the AreaRepo**

Create `crates/storage/src/repos/area_repo.rs` with CRUD operations:
- `new(pool)`, `create(&AreaRow) -> AreaRow`, `get(id) -> AreaRow`
- `list(archived: Option<bool>) -> Vec<AreaRow>`
- `update(id, &AreaPatch) -> AreaRow` (using COALESCE/CASE WHEN pattern)
- `delete(id)` (check for referencing projects/todos first)
- `count_projects(id) -> i64`, `count_tasks(id) -> i64`

Follow the same patterns as `ProjectRepo`: parameterized queries, `RETURNING *`, `QueryBuilder` for dynamic filters.

**`AreaPatch` struct:**

```rust
#[derive(Debug, Clone, Default)]
pub struct AreaPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub color: Option<Option<String>>,
    pub sort_order: Option<i32>,
    pub archived: Option<bool>,
}
```

**Step 3: Wire into rows/mod.rs and repos/mod.rs**

In `crates/storage/src/rows/mod.rs`, add:
```rust
pub mod area;
```

In `crates/storage/src/repos/mod.rs`:
- Add `pub mod area_repo;`
- Add `pub use area_repo::{AreaPatch, AreaRepo};`
- Add `pub areas: AreaRepo,` field to `Repos` struct
- Add `areas: AreaRepo::new(db.clone()),` to `Repos::from_pool()`

In `crates/storage/src/lib.rs`, add re-exports:
```rust
pub use repos::area_repo::{AreaPatch, AreaRepo};
pub use rows::area::AreaRow;
```

**Step 4: Write tests for AreaRepo**

Add inline `#[cfg(test)] mod tests` in `area_repo.rs`:
- `test_create_and_get_area`
- `test_list_areas_with_archive_filter`
- `test_update_area`
- `test_delete_area_fails_with_references` (create area, create project referencing it, try delete → should fail)
- `test_count_projects_and_tasks`

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(area)' --nocapture`
Expected: All area tests pass

**Step 6: Commit**

```bash
git add crates/storage/src/rows/area.rs crates/storage/src/repos/area_repo.rs
git add crates/storage/src/rows/mod.rs crates/storage/src/repos/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add AreaRepo and AreaRow for PARA areas"
```

---

## Task 3: Storage layer — row structs and repos for OKR (Objectives + Key Results)

**Files:**
- Create: `crates/storage/src/rows/okr.rs`
- Create: `crates/storage/src/repos/okr_repo.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/lib.rs`

**Step 1: Write OKR row structs**

Create `crates/storage/src/rows/okr.rs`:

```rust
//! Row structs for `objectives` and `key_results` tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<i16>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultRow {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub target_value: f64,
    pub current_value: f64,
    pub unit: String,
    pub status: String,
    pub auto_track: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Step 2: Write ObjectiveRepo and KeyResultRepo**

Create `crates/storage/src/repos/okr_repo.rs`:

**ObjectiveRepo** methods:
- `create(&ObjectiveRow) -> ObjectiveRow`
- `get(id) -> ObjectiveRow`
- `list(&ObjectiveFilter) -> Vec<ObjectiveRow>` (filter by project_id, status)
- `update(id, &ObjectivePatch) -> ObjectiveRow`
- `delete(id)`

**KeyResultRepo** methods:
- `create(&KeyResultRow) -> KeyResultRow`
- `get(id) -> KeyResultRow`
- `list_by_objective(objective_id) -> Vec<KeyResultRow>`
- `update(id, &KeyResultPatch) -> KeyResultRow`
- `update_progress(id, current_value) -> KeyResultRow`
- `delete(id)`
- `count_tasks(id) -> (total: i64, completed: i64)` — for auto-tracking
- `recompute_auto_tracked(id)` — recalculates current_value from linked tasks

**Patch structs:**

```rust
#[derive(Debug, Clone, Default)]
pub struct ObjectivePatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<Option<i16>>,
    pub start_date: Option<Option<DateTime<Utc>>>,
    pub end_date: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, Default)]
pub struct ObjectiveFilter {
    pub project_id: Option<String>,
    pub area_id: Option<String>,  // joins through projects.area_id
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct KeyResultPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub target_value: Option<f64>,
    pub unit: Option<String>,
    pub status: Option<String>,
    pub auto_track: Option<bool>,
}
```

**Step 3: Wire into mod.rs and lib.rs**

Same pattern as Task 2: add module declarations, re-exports, Repos fields.
- Add `pub objectives: ObjectiveRepo,` and `pub key_results: KeyResultRepo,` to `Repos`
- Wire in `from_pool()`

**Step 4: Write tests**

- `test_create_objective_under_project`
- `test_list_objectives_by_project`
- `test_objective_cascades_on_project_delete`
- `test_create_key_result`
- `test_update_kr_progress`
- `test_kr_cascades_on_objective_delete`
- `test_count_tasks_for_kr` (create tasks with key_result_id, verify counts)

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(objective) | test(key_result)' --nocapture`
Expected: All OKR tests pass

**Step 6: Commit**

```bash
git add crates/storage/src/rows/okr.rs crates/storage/src/repos/okr_repo.rs
git add crates/storage/src/rows/mod.rs crates/storage/src/repos/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add ObjectiveRepo and KeyResultRepo for OKR"
```

---

## Task 4: Modify existing storage — projects, todos, plans

**Files:**
- Modify: `crates/storage/src/rows/project.rs` — add `area_id` field
- Modify: `crates/storage/src/rows/todo.rs` — add `area_id`, `key_result_id` fields
- Modify: `crates/storage/src/rows/plan.rs` — replace `goal_id` with `objective_id` + `key_result_id`
- Modify: `crates/storage/src/repos/project_repo.rs` — area_id in create/update/list
- Modify: `crates/storage/src/repos/todo_repo.rs` — area_id in create/update/list/context_string
- Modify: `crates/storage/src/repos/plan.rs` — replace goal_id references

**Step 1: Update ProjectRow**

In `crates/storage/src/rows/project.rs`, add field:
```rust
pub area_id: String,
```

**Step 2: Update TodoRow**

In `crates/storage/src/rows/todo.rs`, add fields:
```rust
pub area_id: String,
pub key_result_id: Option<String>,
```

**Step 3: Update PlanRow**

In `crates/storage/src/rows/plan.rs`, replace `goal_id`:
```rust
// Remove: pub goal_id: Option<uuid::Uuid>,
pub objective_id: Option<String>,
pub key_result_id: Option<String>,
```

**Step 4: Update ProjectRepo**

In `crates/storage/src/repos/project_repo.rs`:
- Add `area_id` to INSERT in `create()` — must be provided
- Add `area_id` to `ProjectPatch` and UPDATE in `update()`
- Add `area_id` filter to `list()` via `ProjectFilter.area_id: Option<String>`
- Add `area_id` to column lists in all SELECT queries

**Step 5: Update TodoRepo**

In `crates/storage/src/repos/todo_repo.rs`:
- Add `area_id` and `key_result_id` to INSERT in `add()`
- Add `area_id` and `key_result_id` to `TodoPatch` and UPDATE in `update()`
- Add `area_id` and `key_result_id` to `TodoFilter`
- Add `project_id: "none"` filter option for unassigned tasks (WHERE project_id IS NULL)
- Update `to_context_string()` to group tasks by area (join areas table for name)

**Step 6: Update PlanRepo**

In `crates/storage/src/repos/plan.rs`:
- Replace all `goal_id` references with `objective_id` and `key_result_id`
- Update INSERT, UPDATE, SELECT queries
- Update any methods that filter/query by `goal_id`

**Step 7: Run all storage tests**

Run: `cargo nextest run -p storage --nocapture 2>&1 | tail -30`
Expected: All storage tests pass (existing tests may need updates for new required fields)

**Step 8: Commit**

```bash
git add crates/storage/src/rows/ crates/storage/src/repos/
git commit -m "feat(storage): add area_id/key_result_id to projects, todos, plans"
```

---

## Task 5: Delete legacy goal crate and all references

**Files:**
- Delete: `crates/goal/` (entire directory)
- Delete: `crates/storage/src/rows/goal.rs`
- Delete: `crates/storage/src/repos/goal.rs`
- Delete: `crates/tools/src/goal_tool.rs`
- Delete: `crates/agent/src/goal_handler.rs`
- Delete: `crates/agent/src/plan_completion_handler.rs`
- Delete: `crates/agent/src/context_sources/goal.rs`
- Modify: `Cargo.toml` (root) — remove `goal` from workspace members and dependencies
- Modify: `src/lib.rs` — remove `pub use goal;`
- Modify: `crates/common/src/error.rs` — rename `Goal(String)` to `Okr(String)`
- Modify: `crates/storage/src/rows/mod.rs` — remove `pub mod goal;`
- Modify: `crates/storage/src/repos/mod.rs` — remove `pub mod goal;`, `pub use goal::GoalRepo;`, `goals` field from `Repos`
- Modify: `crates/storage/src/lib.rs` — remove `GoalRepo`, `GoalRow`, `GoalProjectLinkRow` re-exports
- Modify: `crates/tools/src/lib.rs` — remove `goal_tool` module and re-exports
- Modify: `crates/agent/src/lib.rs` — remove `goal_handler` and `plan_completion_handler` module declarations
- Modify: `crates/agent/src/context_sources/mod.rs` — remove `pub mod goal;` and `pub use goal::GoalSource;`
- Modify: `crates/agent/src/agent_loop/builder.rs` — remove GoalTool/GoalHandler/PlanCompletionHandler wiring (lines ~442-468)
- Modify: every `Cargo.toml` that depends on `goal` crate

**Step 1: Delete crate directory**

```bash
rm -rf crates/goal/
```

**Step 2: Delete storage goal files**

```bash
rm crates/storage/src/rows/goal.rs
rm crates/storage/src/repos/goal.rs
```

**Step 3: Delete tool and agent files**

```bash
rm crates/tools/src/goal_tool.rs
rm crates/agent/src/goal_handler.rs
rm crates/agent/src/plan_completion_handler.rs
rm crates/agent/src/context_sources/goal.rs
```

**Step 4: Update all module declarations and imports**

Remove all `goal` references from:
- Root `Cargo.toml`: remove `"crates/goal"` from workspace members, remove `goal = { path = "crates/goal" }` from workspace.dependencies, remove `goal.workspace = true` from `[dependencies]`
- `src/lib.rs`: remove `pub use goal;`
- `crates/common/src/error.rs`: rename `Goal(String)` to `Okr(String)`, update error message to `"OKR error: {0}"`, update test
- `crates/storage/src/rows/mod.rs`: remove `pub mod goal;`
- `crates/storage/src/repos/mod.rs`: remove `pub mod goal;`, `pub use goal::GoalRepo;`, remove `goals: GoalRepo` field and its construction in `from_pool()`
- `crates/storage/src/lib.rs`: remove `pub use repos::GoalRepo;`, remove `pub use rows::goal::*` re-exports
- `crates/tools/src/lib.rs`: remove `pub mod goal_tool;` and any `GoalHandler` re-exports
- `crates/agent/src/lib.rs`: remove `pub mod goal_handler;`, `pub mod plan_completion_handler;`
- `crates/agent/src/context_sources/mod.rs`: remove `pub mod goal;`, `pub use goal::GoalSource;`
- `crates/agent/src/agent_loop/builder.rs`: remove the `// Goal tool` section (~L442-468), remove `PlanCompletionHandler` wiring (~L462-468), remove related imports
- Any other `Cargo.toml` files that depend on `goal`

**Step 5: Update plan crate**

In `crates/plan/src/types.rs`: replace `pub goal_id: Option<Uuid>` with:
```rust
pub objective_id: Option<String>,
pub key_result_id: Option<String>,
```
Update `Plan` conversions in `crates/plan/src/conversions.rs` and any `goal_id` references in `crates/goal/src/conversions.rs` (already deleted, so just the plan side).

In `crates/plan/src/conversions.rs` (or wherever `Plan ↔ PlanRow` conversion lives), update to use `objective_id`/`key_result_id` instead of `goal_id`.

**Step 6: Fix compilation errors iteratively**

Run: `cargo build --workspace 2>&1 | head -50`
Expected: Fix all remaining references to `goal`, `GoalRepo`, `GoalRow`, `GoalHandler`, `GoalSource`, `GoalStatus`, `GoalTool`, `PlanCompletionHandler`, `goal_id` across the workspace. This is a search-and-destroy pass.

Use: `rg 'goal|Goal' --type rust -l` to find remaining references.

**Step 7: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | head -30`
Expected: 0 warnings

**Step 8: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`
Expected: All tests pass

**Step 9: Commit**

```bash
git add -A
git commit -m "refactor: remove legacy goal system (replaced by OKR)"
```

---

## Task 6: Create feature-area crate with AreaTool

**Files:**
- Create: `crates/feature-area/Cargo.toml`
- Create: `crates/feature-area/src/lib.rs`
- Create: `crates/feature-area/src/types.rs`
- Create: `crates/feature-area/src/tool/mod.rs`
- Create: `crates/feature-area/src/tool/actions/mod.rs`
- Create: `crates/feature-area/src/tool/actions/create.rs`
- Create: `crates/feature-area/src/tool/actions/list.rs`
- Create: `crates/feature-area/src/tool/actions/show.rs`
- Create: `crates/feature-area/src/tool/actions/update.rs`
- Create: `crates/feature-area/src/tool/actions/archive.rs`
- Create: `crates/feature-area/src/tool/actions/delete.rs`
- Modify: `Cargo.toml` (root) — add workspace member and dependency

**Step 1: Create Cargo.toml**

```toml
[package]
name = "feature-area"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
tools-core.workspace = true
storage.workspace = true
async-trait.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
```

**Step 2: Create domain types**

Create `crates/feature-area/src/types.rs` with `Area` domain struct and `AreaRow ↔ Area` conversions. Follow the same pattern as `feature-todo/src/types.rs`:
- `Area` struct with typed fields (id, name, description, icon, color, sort_order, archived)
- `Area::generate_id()` → 8-char UUID prefix
- `impl From<AreaRow> for Area` and `impl From<&Area> for AreaRow`

**Step 3: Create AreaTool with 6 actions**

Follow the `feature-todo` tool pattern:
- `AreaTool` struct holding `AreaRepo`
- `impl Tool for AreaTool` with `execute()` dispatching on `action` param
- 6 actions: create, list, show, update, archive, delete
- Each action in its own file under `tool/actions/`
- Tool parameters JSON schema matches the design doc

**Step 4: Create lib.rs with FeaturePackage impl**

Follow `feature-todo/src/lib.rs` pattern:
- `AreaFeature` struct wrapping `Arc<AreaTool>`
- `impl FeaturePackage` with name "area", tools, config_key

**Step 5: Add to workspace**

In root `Cargo.toml`:
- Add `"crates/feature-area"` to `[workspace] members`
- Add `feature-area = { path = "crates/feature-area" }` to `[workspace.dependencies]`

**Step 6: Write tests**

Test each action: create area, list areas, show area with counts, update area, archive area, delete area (expect failure if referenced).

**Step 7: Run tests**

Run: `cargo nextest run -p feature-area --nocapture`
Expected: All tests pass

**Step 8: Commit**

```bash
git add crates/feature-area/ Cargo.toml
git commit -m "feat: add feature-area crate with AreaTool (6 actions)"
```

---

## Task 7: Create feature-okr crate with OkrTool

**Files:**
- Create: `crates/feature-okr/Cargo.toml`
- Create: `crates/feature-okr/src/lib.rs`
- Create: `crates/feature-okr/src/types.rs`
- Create: `crates/feature-okr/src/tool/mod.rs`
- Create: `crates/feature-okr/src/tool/actions/mod.rs`
- Create: `crates/feature-okr/src/tool/actions/create_objective.rs`
- Create: `crates/feature-okr/src/tool/actions/list_objectives.rs`
- Create: `crates/feature-okr/src/tool/actions/show_objective.rs`
- Create: `crates/feature-okr/src/tool/actions/update_objective.rs`
- Create: `crates/feature-okr/src/tool/actions/delete_objective.rs`
- Create: `crates/feature-okr/src/tool/actions/add_kr.rs`
- Create: `crates/feature-okr/src/tool/actions/list_krs.rs`
- Create: `crates/feature-okr/src/tool/actions/update_kr.rs`
- Create: `crates/feature-okr/src/tool/actions/update_kr_progress.rs`
- Create: `crates/feature-okr/src/tool/actions/delete_kr.rs`
- Create: `crates/feature-okr/src/tool/actions/overview.rs`
- Create: `crates/feature-okr/src/tool/actions/status.rs`
- Modify: `Cargo.toml` (root) — add workspace member and dependency

**Step 1: Create Cargo.toml**

Same pattern as feature-area. Dependencies: `common`, `tools-core`, `storage`, `async-trait`, `serde`, `serde_json`, `tokio`, `tracing`, `chrono`, `uuid`.

**Step 2: Create domain types**

Create `crates/feature-okr/src/types.rs`:
- `Objective` struct: id, project_id, title, description, status (OkrStatus), priority, start_date, end_date, created_at, updated_at
- `KeyResult` struct: id, objective_id, title, description, target_value, current_value, unit, status, auto_track, created_at, updated_at
- `OkrStatus` enum: Active, Paused, Achieved, Abandoned — with `validate_transition()` (same state machine as old GoalStatus)
- `KeyResultProgress` struct: key_result_id, percentage (current/target*100), summary
- `Objective::generate_id()`, `KeyResult::generate_id()` → 8-char UUID prefix
- Row ↔ Domain conversions

**Step 3: Create OkrTool with 12 actions**

Follow the same patterns as AreaTool/TodoTool:
- `OkrTool` struct holding `ObjectiveRepo`, `KeyResultRepo`, and optionally `PlanRepo` (for overview action showing linked plans)
- `impl Tool for OkrTool` with `execute()` dispatching on `action`
- 12 actions as defined in design doc, each in its own file
- `add-kr` and `update-kr-progress` emit EntityCard to `ctx.entity_tx`
- `overview` action: builds hierarchical tree view (Area > Project > Objective > KRs with progress)

**Key action: `update-kr-progress`**
```rust
// In update_kr_progress.rs
// 1. Parse id, current_value from args
// 2. Call KeyResultRepo::update_progress(id, current_value)
// 3. Check if all KRs of parent objective are at 100%
// 4. If so, suggest marking objective as achieved
// 5. Return formatted progress string
```

**Key action: `overview`**
```rust
// In overview.rs
// 1. If project_id: list objectives for project
// 2. If area_id: list projects for area, then objectives for each
// 3. For each objective: list KRs with progress
// 4. For each KR: show progress bar (current/target, %)
// 5. Format as hierarchical tree string
```

**Step 4: Create lib.rs with FeaturePackage impl**

`OkrFeature` struct wrapping `Arc<OkrTool>`. Implement `FeaturePackage`.

**Step 5: Add to workspace**

In root `Cargo.toml`, add `feature-okr` member and dependency.

**Step 6: Write tests**

- `test_create_objective` — requires project and area to exist first
- `test_list_objectives_by_project_and_status`
- `test_add_key_result`
- `test_update_kr_progress_manual`
- `test_overview_tree_format`
- `test_objective_status_transitions` (valid and invalid)
- `test_delete_objective_cascades_krs`

**Step 7: Run tests**

Run: `cargo nextest run -p feature-okr --nocapture`
Expected: All tests pass

**Step 8: Commit**

```bash
git add crates/feature-okr/ Cargo.toml
git commit -m "feat: add feature-okr crate with OkrTool (12 actions)"
```

---

## Task 8: Modify feature-todo for area_id and key_result_id

**Files:**
- Modify: `crates/feature-todo/src/types.rs` — add `area_id`, `key_result_id` fields to `Todo`
- Modify: `crates/feature-todo/src/tool/actions/add.rs` — require `area_id`, accept `key_result_id`
- Modify: `crates/feature-todo/src/tool/actions/update.rs` — support changing `area_id`, `key_result_id`; add KR auto-tracking on complete
- Modify: `crates/feature-todo/src/tool/actions/search.rs` — add area_id, key_result_id filters to list
- Modify: `crates/feature-todo/src/tool/mod.rs` — update parameters schema

**Step 1: Update Todo domain type**

In `crates/feature-todo/src/types.rs`, add to `Todo` struct:
```rust
pub area_id: String,
#[serde(default)]
pub key_result_id: Option<String>,
```

Update `default_instance()` to include `area_id: String::new()` and `key_result_id: None`.
Update `From<TodoRow> for Todo` and `From<&Todo> for TodoRow` conversions.

**Step 2: Update handle_add**

In `crates/feature-todo/src/tool/actions/add.rs`:
- Parse `area_id` as required param (return error if missing)
- Parse `key_result_id` as optional param
- Set on the `Todo` before saving

**Step 3: Update handle_complete for KR auto-tracking**

In `crates/feature-todo/src/tool/actions/update.rs`, after successful completion:
```rust
// After task is marked done, check for KR auto-tracking
if let Some(kr_id) = &completed_todo.key_result_id {
    if let Some(ref handler) = self.kr_progress_handler {
        handler.on_task_completed(kr_id).await;
    }
}
```

Define a new trait `KrProgressHandler` in `feature-todo` (dependency inversion):
```rust
#[async_trait]
pub trait KrProgressHandler: Send + Sync {
    async fn on_task_completed(&self, key_result_id: &str) -> Result<()>;
}
```

This trait will be implemented in the `agent` crate using `KeyResultRepo`.

**Step 4: Update list/search filters**

In `crates/feature-todo/src/tool/actions/search.rs` (or wherever `handle_list` lives):
- Parse `area_id` filter from params
- Parse `key_result_id` filter from params
- Parse `project_id: "none"` as a special filter for unassigned tasks
- Pass to `TodoFilter`

**Step 5: Update tool parameters schema**

In `crates/feature-todo/src/tool/mod.rs`, update the `parameters()` JSON schema to include:
- `area_id` in `add` action (required)
- `key_result_id` in `add` action (optional)
- `area_id` filter in `list` action
- `key_result_id` filter in `list` action

**Step 6: Run tests**

Run: `cargo nextest run -p feature-todo --nocapture`
Expected: All tests pass (update existing tests to include `area_id`)

**Step 7: Commit**

```bash
git add crates/feature-todo/
git commit -m "feat(todo): add area_id (required) and key_result_id (optional) to tasks"
```

---

## Task 9: Modify ProjectTool for area_id

**Files:**
- Modify: `crates/tools/src/project_types.rs` — add `area_id` to `Project`, `ProjectPatch`, `ProjectFilter`
- Modify: `crates/tools/src/project_tool.rs` — require `area_id` on create, add filter to list

**Step 1: Update Project domain type**

In `crates/tools/src/project_types.rs`, add:
```rust
pub area_id: String,
```
to `Project` struct. Add `area_id` to `ProjectPatch` and `ProjectFilter`. Update `From<ProjectRow>` conversion.

**Step 2: Update ProjectTool create action**

In `crates/tools/src/project_tool.rs`:
- Parse `area_id` as required param in create action
- Add `area_id` filter support in list action

**Step 3: Run tests**

Run: `cargo nextest run -p tools --nocapture`
Expected: Pass (update existing project tests to include `area_id`)

**Step 4: Commit**

```bash
git add crates/tools/src/project_types.rs crates/tools/src/project_tool.rs
git commit -m "feat(project): add area_id (required) to projects"
```

---

## Task 10: Wire everything into the agent

**Files:**
- Create: `crates/agent/src/context_sources/area.rs` — AreaSource
- Create: `crates/agent/src/context_sources/okr.rs` — OkrSource
- Create: `crates/agent/src/kr_progress_handler.rs` — KrProgressHandler impl
- Modify: `crates/agent/src/context_sources/mod.rs` — add area, okr modules
- Modify: `crates/agent/src/context_sources/todo.rs` — group by area
- Modify: `crates/agent/src/agent_loop/builder.rs` — register AreaTool, OkrTool, ProjectTool; wire KrProgressHandler
- Modify: `crates/agent/src/lib.rs` — add new module declarations
- Modify: `crates/agent/Cargo.toml` — add feature-area, feature-okr dependencies

**Step 1: Create AreaSource (priority 65)**

Create `crates/agent/src/context_sources/area.rs`:
Follow exact same pattern as the deleted `GoalSource` (TTL cache, `ContextSource` impl):
```rust
pub struct AreaSource {
    repo: storage::AreaRepo,
    cache: Mutex<Option<CachedValue>>,
}
```
Output format:
```
# Areas
- Work (3 projects, 12 active tasks)
- Personal (2 projects, 5 active tasks)
```

**Step 2: Create OkrSource (priority 60)**

Create `crates/agent/src/context_sources/okr.rs`:
Query objectives (active only) with their KRs. Join through projects to get area names.
Output format:
```
# Active OKRs
## Work > Mobile App
- Objective: Launch v2.0 [2 KRs]
  - KR: Ship 5 features (3/5, 60%)
  - KR: Test coverage (78/90%, 87%)
```

**Step 3: Create KrProgressHandler impl**

Create `crates/agent/src/kr_progress_handler.rs`:
```rust
pub struct KrProgressHandlerImpl {
    key_result_repo: storage::KeyResultRepo,
}

#[async_trait]
impl feature_todo::KrProgressHandler for KrProgressHandlerImpl {
    async fn on_task_completed(&self, key_result_id: &str) -> Result<()> {
        // 1. Get the KR
        // 2. If auto_track, recompute from linked tasks
        // 3. Update current_value
        self.key_result_repo.recompute_auto_tracked(key_result_id).await
    }
}
```

**Step 4: Update TodoSource for area grouping**

In `crates/agent/src/context_sources/todo.rs`, the `TodoRepo::to_context_string()` already generates the context. Update it (in `crates/storage/src/repos/todo_repo.rs`) to JOIN with `areas` and group tasks by area name.

**Step 5: Wire into builder.rs**

In `crates/agent/src/agent_loop/builder.rs`:

```rust
// ── Area tool ──
{
    let area_tool = feature_area::AreaTool::new(repos.areas.clone());
    tool_registry.register(area_tool);
}

// ── OKR tool ──
{
    let okr_tool = feature_okr::OkrTool::new(
        repos.objectives.clone(),
        repos.key_results.clone(),
    );
    tool_registry.register(okr_tool);
}

// ── Project tool (was previously unregistered!) ──
{
    let project_tool = tools::ProjectTool::new(repos.projects.clone(), repos.todos.clone());
    tool_registry.register(project_tool);
}

// ── KR progress handler (inject into TodoTool) ──
let kr_handler = Arc::new(KrProgressHandlerImpl::new(repos.key_results.clone()));
// Pass to TodoTool via .with_kr_progress_handler(kr_handler)
```

Add `AreaSource` and `OkrSource` to context engine registration (wherever context sources are added).

**Step 6: Update context_sources/mod.rs**

```rust
pub mod area;
pub mod okr;

pub use area::AreaSource;
pub use okr::OkrSource;
```

**Step 7: Update agent/src/lib.rs and Cargo.toml**

Add `pub mod kr_progress_handler;` to `lib.rs`.
Add `feature-area` and `feature-okr` to `crates/agent/Cargo.toml` dependencies.

**Step 8: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`
Expected: All tests pass

**Step 9: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 10: Commit**

```bash
git add crates/agent/ crates/storage/src/repos/todo_repo.rs
git commit -m "feat(agent): wire AreaTool, OkrTool, ProjectTool, and context sources"
```

---

## Task 11: Update facade crate and root Cargo.toml

**Files:**
- Modify: `src/lib.rs` — add `pub use feature_area;` and `pub use feature_okr;`
- Modify: `Cargo.toml` (root) — add `feature-area` and `feature-okr` to `[dependencies]`

**Step 1: Update src/lib.rs**

Add after other re-exports:
```rust
pub use feature_area;
pub use feature_okr;
```

Remove any remaining `pub use goal;` if not already removed.

**Step 2: Update root Cargo.toml dependencies**

Add:
```toml
feature-area.workspace = true
feature-okr.workspace = true
```

**Step 3: Build full workspace**

Run: `cargo build --workspace`
Expected: Clean build

**Step 4: Commit**

```bash
git add src/lib.rs Cargo.toml
git commit -m "feat: re-export feature-area and feature-okr from facade crate"
```

---

## Task 12: Dashboard API endpoints for Areas and OKR

**Files:**
- Create: `crates/dashboard/src/api/areas.rs`
- Create: `crates/dashboard/src/api/objectives.rs`
- Create: `crates/dashboard/src/api/key_results.rs`
- Modify: `crates/dashboard/src/api/mod.rs` — add modules
- Modify: `crates/dashboard/src/router.rs` — register new routes
- Modify: `crates/dashboard/src/api/tasks.rs` — add areaId, keyResultId query filters
- Modify: `crates/dashboard/src/api/projects.rs` — add areaId filter, require areaId on create

**Step 1: Create Areas API handlers**

Create `crates/dashboard/src/api/areas.rs`:
Follow exact same pattern as `projects.rs`:
- `list_areas(State, Query<ListAreasParams>)` → `AreaRepo::list()`
- `create_area(State, Json<CreateAreaBody>)` → `AreaRepo::create()`
- `get_area(State, Path<id>)` → `AreaRepo::get()` + count_projects + count_tasks
- `patch_area(State, Path<id>, Json<PatchBody>)` → `AreaRepo::update()`
- `delete_area(State, Path<id>)` → check references, then `AreaRepo::delete()`

**Step 2: Create Objectives API handlers**

Create `crates/dashboard/src/api/objectives.rs`:
- `list_objectives(State, Query<params>)` — filter by projectId, areaId, status
- `create_objective(State, Json<body>)` — requires project_id
- `get_objective(State, Path<id>)` — returns objective with key_results array
- `patch_objective(State, Path<id>, Json<body>)`
- `delete_objective(State, Path<id>)` — CASCADE on KRs

**Step 3: Create Key Results API handlers**

Create `crates/dashboard/src/api/key_results.rs`:
- `list_key_results(State, Path<objective_id>)` — nested under objective
- `create_key_result(State, Path<objective_id>, Json<body>)`
- `patch_key_result(State, Path<id>, Json<body>)` — includes progress updates
- `delete_key_result(State, Path<id>)`

**Step 4: Register routes**

In `crates/dashboard/src/router.rs`, add:
```rust
// Areas
.route("/api/areas", get(areas::list_areas).post(areas::create_area))
.route("/api/areas/{id}", get(areas::get_area).patch(areas::patch_area).delete(areas::delete_area))
// Objectives
.route("/api/objectives", get(objectives::list_objectives).post(objectives::create_objective))
.route("/api/objectives/{id}", get(objectives::get_objective).patch(objectives::patch_objective).delete(objectives::delete_objective))
.route("/api/objectives/{id}/key-results", get(key_results::list_key_results).post(key_results::create_key_result))
// Key Results
.route("/api/key-results/{id}", patch(key_results::patch_key_result).delete(key_results::delete_key_result))
```

**Step 5: Update existing task/project endpoints**

In `tasks.rs`: add `areaId` and `keyResultId` to `ListTasksParams` query struct.
In `projects.rs`: add `areaId` to `ListProjectsParams` and require `areaId` in `CreateProjectBody`.

**Step 6: Run dashboard tests**

Run: `cargo nextest run -p dashboard --nocapture`
Expected: All tests pass

**Step 7: Commit**

```bash
git add crates/dashboard/
git commit -m "feat(dashboard): add REST API for areas, objectives, key results"
```

---

## Task 13: Update frontend TypeScript types

**Files:**
- Modify: `crates/dashboard/frontend/src/lib/types.ts` — add Area, Objective, KeyResult types; add area_id to Task/Project

**Step 1: Add new TypeScript interfaces**

```typescript
export interface Area {
  id: string;
  name: string;
  description: string | null;
  icon: string | null;
  color: string | null;
  sortOrder: number;
  archived: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface Objective {
  id: string;
  projectId: string;
  title: string;
  description: string | null;
  status: 'active' | 'paused' | 'achieved' | 'abandoned';
  priority: number | null;
  startDate: string | null;
  endDate: string | null;
  keyResults?: KeyResult[];
  createdAt: string;
  updatedAt: string;
}

export interface KeyResult {
  id: string;
  objectiveId: string;
  title: string;
  description: string | null;
  targetValue: number;
  currentValue: number;
  unit: string;
  status: 'active' | 'achieved' | 'abandoned';
  autoTrack: boolean;
  createdAt: string;
  updatedAt: string;
}
```

**Step 2: Update Task and Project interfaces**

Add to `Task`:
```typescript
areaId: string;
keyResultId: string | null;
```

Add to `Project`:
```typescript
areaId: string;
```

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/lib/types.ts
git commit -m "feat(dashboard): add Area, Objective, KeyResult TypeScript types"
```

---

## Task 14: Final verification and CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md` — update architecture docs

**Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Clean build, 0 errors

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

**Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: Clean

**Step 5: Update CLAUDE.md**

Update the workspace layout to reflect:
- New layer 1.5 crates: `feature-area`, `feature-okr`
- Removed: `goal` crate
- Updated `Repos` aggregate to list `areas`, `objectives`, `key_results` instead of `goals`
- Updated extension traits table: remove `GoalHandler`, add `KrProgressHandler`
- Update error enum: `Goal` → `Okr`
- Add OKR section explaining the hierarchy

**Step 6: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for PARA + OKR architecture"
```
