# PARA + OKR Task System Redesign

## Summary

Redesign the task management system around the PARA method (Projects, Areas, Resources, Archive) with OKR-based goal tracking (Objectives, Key Results, Actions). This replaces the current flat project + loose goal system with a structured hierarchy.

**Scope**: Areas + Projects + OKR (Objectives + Key Results). Resources and Archive deferred.

**Breaking change**: No backward compatibility. Clean replacement of the legacy `goal` system.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Area requirement | Always required on tasks and projects | No implicit defaults. Agent asks which Area if not specified. |
| Project-Area relationship | Project belongs to exactly one Area (required FK) | Clean hierarchy. |
| OKR-Project relationship | Objectives belong to a Project. Multiple per project. | Standard OKR approach. |
| Plan linkage | Both Objectives and KRs can link to Plans | Maximum flexibility. Replaces `goal_id` on plans. |
| Task-OKR linkage | Optional `key_result_id` on tasks | Tasks are project/area-scoped primarily. OKR linkage is opt-in. |
| KR progress tracking | Hybrid: numeric target + auto from tasks | `auto_track` flag computes from linked task completion. Manual update always available. |
| OKR time-bounding | Optional `start_date`/`end_date` on Objectives | No enforced cadence. User can set quarterly boundaries if desired. |
| Tool architecture | Consolidated OKR Tool + separate Area Tool | Fewer tools for LLM reasoning. Related OKR ops grouped. 4 total tools: area, project, okr, todo. |
| Crate architecture | New `feature-okr` + `feature-area` crates | Follows established `feature-*` pattern. Delete `goal` crate entirely. |
| Unknown projects | Tasks with `project_id IS NULL` filtered as "Unassigned Tasks" | `todo list --project none` filter. |

## Data Model

### New Tables

```sql
CREATE TABLE areas (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    icon        TEXT,
    color       TEXT,
    sort_order  INTEGER DEFAULT 0,
    archived    BOOLEAN DEFAULT FALSE,
    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE objectives (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT DEFAULT 'active',  -- active/paused/achieved/abandoned
    priority    INTEGER,
    start_date  TEXT,
    end_date    TEXT,
    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE key_results (
    id              TEXT PRIMARY KEY,
    objective_id    TEXT NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT,
    target_value    REAL NOT NULL DEFAULT 100.0,
    current_value   REAL DEFAULT 0.0,
    unit            TEXT DEFAULT '%',
    status          TEXT DEFAULT 'active',  -- active/achieved/abandoned
    auto_track      BOOLEAN DEFAULT FALSE,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
```

### Modified Tables

```sql
-- projects: add required area_id
ALTER TABLE projects ADD COLUMN area_id TEXT NOT NULL DEFAULT '' REFERENCES areas(id);

-- todos: add required area_id, optional key_result_id
ALTER TABLE todos ADD COLUMN area_id TEXT NOT NULL DEFAULT '' REFERENCES areas(id);
ALTER TABLE todos ADD COLUMN key_result_id TEXT REFERENCES key_results(id) ON DELETE SET NULL;

-- plans: recreate to replace goal_id with objective_id + key_result_id
-- (SQLite cannot drop/rename FK columns, so table recreation is required)
```

### Removed Tables

```sql
DROP TABLE goal_project_links;
DROP TABLE goals;
```

### FK Cascade Rules

| Relationship | On Delete |
|-------------|-----------|
| objectives.project_id → projects | CASCADE (delete project removes objectives) |
| key_results.objective_id → objectives | CASCADE (delete objective removes KRs) |
| todos.key_result_id → key_results | SET NULL (delete KR unlinks tasks) |
| todos.area_id → areas | RESTRICT (cannot delete area with tasks) |
| projects.area_id → areas | RESTRICT (cannot delete area with projects) |
| plans.objective_id → objectives | SET NULL |
| plans.key_result_id → key_results | SET NULL |

## Crate Structure

### New Crates

```
crates/feature-area/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs        -- Area, AreaStatus
│   └── tool/
│       ├── mod.rs      -- AreaTool (Tool trait impl)
│       └── actions/    -- create, list, show, update, archive, delete

crates/feature-okr/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs        -- Objective, KeyResult, OkrStatus, KeyResultProgress
│   └── tool/
│       ├── mod.rs      -- OkrTool (Tool trait impl)
│       └── actions/    -- create-objective, list-objectives, add-kr, update-kr, etc.
```

### Deleted Crate

```
crates/goal/            -- Entire crate removed
```

### Storage Crate Changes

```
crates/storage/
├── migrations/005_para_okr.sql     -- NEW: full migration
├── src/repos/
│   ├── area_repo.rs                -- NEW
│   ├── okr_repo.rs                 -- NEW (ObjectiveRepo + KeyResultRepo)
│   ├── goal.rs                     -- DELETE
│   ├── mod.rs                      -- Update Repos aggregate
│   ├── project_repo.rs             -- Add area_id filtering
│   ├── todo_repo.rs                -- Add area_id, key_result_id
│   └── plan.rs                     -- Replace goal_id with objective_id + key_result_id
├── src/rows/
│   ├── area.rs                     -- NEW
│   ├── okr.rs                      -- NEW
│   └── goal.rs                     -- DELETE
```

## Tool Actions

### AreaTool (6 actions)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `create` | name, description?, icon?, color? | Create new area |
| `list` | archived? | List all areas with project/task counts |
| `show` | id | Show area details with projects |
| `update` | id, name?, description?, icon?, color?, sort_order? | Update area |
| `archive` | id | Archive/unarchive |
| `delete` | id | Delete (fails if has projects/tasks) |

### OkrTool (12 actions)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `create-objective` | project_id, title, description?, priority?, start_date?, end_date? | Create objective under project |
| `list-objectives` | project_id?, area_id?, status? | List objectives with filters |
| `show-objective` | id | Show objective with KRs and progress |
| `update-objective` | id, title?, description?, priority?, start_date?, end_date? | Update objective |
| `delete-objective` | id | Delete (cascades KRs) |
| `add-kr` | objective_id, title, description?, target_value?, unit?, auto_track? | Add key result |
| `list-krs` | objective_id | List KRs for an objective |
| `update-kr` | id, title?, description?, target_value?, unit?, auto_track? | Update KR |
| `update-kr-progress` | id, current_value | Manually update progress |
| `delete-kr` | id | Delete key result |
| `overview` | project_id? or area_id? | Full OKR tree view |
| `status` | id, status | Change objective status |

### TodoTool Modifications

- `add`: `area_id` required, `key_result_id` optional
- `list`: New filters `area_id`, `key_result_id`, `project_id: "none"` for unassigned
- `update`: Can set/change `area_id`, `key_result_id`

### ProjectTool Modifications

- `create`: `area_id` required
- `list`: New filter `area_id`
- **Register in AgentLoopBuilder** (currently unregistered)

## Context Sources

### AreaSource (priority 65)

```
# Areas
- Work (3 projects, 12 active tasks)
- Personal (2 projects, 5 active tasks)
```

### OkrSource (priority 60, replaces GoalSource)

```
# Active OKRs
## Work > Mobile App
- Objective: Launch v2.0 by March [2 KRs]
  - KR: Ship 5 core features (3/5, 60%)
  - KR: Reach 90% test coverage (78/90%, 87%)
## Personal > Health
- Objective: Run a marathon [1 KR]
  - KR: Complete 16-week training plan (8/16, 50%)
```

### TodoSource (modified, priority 70)

Tasks grouped by Area with optional KR linkage shown:
```
# Active tasks
## Work
- [doing][FOCUSED] P1 Fix auth bug (Mobile App)
- [todo] P3 Update API docs (Mobile App) -> KR: Ship 5 core features
## Personal
- [todo] P2 Book dentist appointment
```

## Agent Integration

### Auto-tracking KR Progress

When `TodoTool::handle_complete()` completes a task with `key_result_id`:
1. Check if linked KR has `auto_track = true`
2. Recompute: `current_value = (completed / total) * target_value`
3. Update via `KeyResultRepo::update_progress()`
4. If all KRs of an Objective reach 100%, suggest marking Objective as "achieved"

### Plan Integration

- `PlanTool` gets `objective_id` and `key_result_id` optional params (replaces `goal_id`)
- `PlanCompletionHandler` updates linked KR progress when plan completes
- `OkrTool::overview` shows linked plans alongside KRs

## Migration: 005_para_okr.sql

1. Create `areas`, `objectives`, `key_results` tables
2. Add `area_id` (NOT NULL DEFAULT '') and `key_result_id` to `todos`
3. Add `area_id` (NOT NULL DEFAULT '') to `projects`
4. Recreate `plans` table without `goal_id`, with `objective_id` + `key_result_id`
5. Drop `goal_project_links` and `goals`
6. Create indexes on new FK columns

## Legacy Code Removal

| File/Crate | Action |
|-----------|--------|
| `crates/goal/` | Delete entire crate |
| `crates/storage/src/repos/goal.rs` | Delete, replace with `okr_repo.rs` |
| `crates/storage/src/rows/goal.rs` | Delete, replace with `okr.rs` |
| `crates/tools/src/goal_tool.rs` | Delete, replaced by `feature-okr/OkrTool` |
| `crates/agent/src/goal_handler.rs` | Delete, logic moves to OkrHandler |
| `crates/agent/src/context_sources/goal.rs` | Delete, replace with `okr.rs` |
| `crates/agent/src/plan_completion_handler.rs` | Rewrite for OKR linkage |
| `crates/common/src/error.rs` Goal variant | Rename to `Okr` |
| `src/lib.rs` `pub use goal` | Replace with `pub use feature_okr` |
| Workspace `Cargo.toml` goal member | Replace with `feature-okr`, `feature-area` |

## Dashboard API

### New Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/areas` | List areas |
| POST | `/api/areas` | Create area |
| GET | `/api/areas/:id` | Get area |
| PATCH | `/api/areas/:id` | Update area |
| DELETE | `/api/areas/:id` | Delete area |
| GET | `/api/objectives` | List objectives (filter: projectId, areaId, status) |
| POST | `/api/objectives` | Create objective |
| GET | `/api/objectives/:id` | Get objective with KRs |
| PATCH | `/api/objectives/:id` | Update objective |
| DELETE | `/api/objectives/:id` | Delete objective |
| GET | `/api/objectives/:id/key-results` | List KRs |
| POST | `/api/objectives/:id/key-results` | Add KR |
| PATCH | `/api/key-results/:id` | Update KR |
| DELETE | `/api/key-results/:id` | Delete KR |

### Modified Endpoints

| Method | Path | Change |
|--------|------|--------|
| GET | `/api/tasks` | Add `areaId`, `keyResultId` query filters |
| POST | `/api/tasks` | `areaId` required |
| GET | `/api/projects` | Add `areaId` query filter |
| POST | `/api/projects` | `areaId` required |
