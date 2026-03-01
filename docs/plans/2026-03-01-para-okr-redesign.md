# PARA + OKR Task System Redesign

**Date:** 2026-03-01
**Status:** Approved

## Overview

Redesign the task management system from flat tasks + loose projects + goals + plans into a PARA-method hierarchy with OKR structure inside projects.

**Current system:** Tasks (independent) → Projects (optional grouping) → Goals (strategic) → Plans (execution)
**New system:** Areas (required) → Projects → Objectives → Key Results → Actions (tasks)

## Decisions

- **Breaking changes acceptable** — no production users yet
- **Approach:** Flat tables (Approach A) — each entity gets its own table with FK relationships
- **Plans system:** Removed entirely, replaced by OKR hierarchy
- **Goals system:** Removed entirely, replaced by Objectives + Key Results
- **KR progress tracking:** Both modes — metric-based (target/current) and action-completion-based
- **Objective depth:** Full entity with own status, progress (aggregated from KRs), due dates
- **Task features:** Full parity — all current Todo features preserved (focus, time tracking, recurrence, semantic search, dependencies, attachments, enrichment)
- **Crate strategy:** Modify in-place (rewrite existing crates, no new feature crates)
- **Schema scope:** Full PARA schema (Resources + Archive tables created but no tool support yet)
- **Tool surface:** 4 tools — AreaTool, ProjectTool, OkrTool (objectives + key results), TaskTool

## Data Model

### Entity Relationships

```
areas  1──*  projects  1──*  objectives  1──*  key_results
  │                                               │ 0..*
  │            ┌──────────────────────────────────▼────────┐
  │   ┌──────▶│  actions (tasks)                           │
  │   │       │  area_id: required                         │
  └───┘       │  project_id: optional                      │
  0..*        │  key_result_id: optional                   │
              │  parent_id: optional (subtasks)             │
              └────────────────────────────────────────────┘
```

### Schema

```sql
-- PARA Layer
CREATE TABLE areas (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'blue',
    icon        TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'active',  -- active | archived
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    area_id     TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'orange',
    tags        TEXT NOT NULL DEFAULT '[]',
    status      TEXT NOT NULL DEFAULT 'active',  -- active | paused | completed | archived
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX idx_projects_area_id ON projects(area_id);
CREATE INDEX idx_projects_status ON projects(status);

-- OKR Layer
CREATE TABLE objectives (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT,
    status       TEXT NOT NULL DEFAULT 'active',  -- active | paused | completed | abandoned
    priority     INTEGER,
    due_date     TEXT,
    progress     REAL NOT NULL DEFAULT 0.0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    completed_at TEXT
);
CREATE INDEX idx_objectives_project_id ON objectives(project_id);
CREATE INDEX idx_objectives_status ON objectives(status);

CREATE TABLE key_results (
    id            TEXT PRIMARY KEY,
    objective_id  TEXT NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
    title         TEXT NOT NULL,
    description   TEXT,
    status        TEXT NOT NULL DEFAULT 'active',  -- active | completed | abandoned
    tracking_mode TEXT NOT NULL DEFAULT 'action',  -- metric | action
    target_value  REAL,
    current_value REAL NOT NULL DEFAULT 0.0,
    unit          TEXT,
    progress      REAL NOT NULL DEFAULT 0.0,
    due_date      TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    completed_at  TEXT
);
CREATE INDEX idx_key_results_objective_id ON key_results(objective_id);
CREATE INDEX idx_key_results_status ON key_results(status);

-- Actions (replaces todos)
CREATE TABLE actions (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    area_id              TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    project_id           TEXT REFERENCES projects(id) ON DELETE SET NULL,
    key_result_id        TEXT REFERENCES key_results(id) ON DELETE SET NULL,
    parent_id            TEXT REFERENCES actions(id) ON DELETE SET NULL,
    priority             INTEGER,
    due_date             TEXT,
    tags                 TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'todo',  -- todo | doing | done | archived
    focused_at           TEXT,
    focus_deadline       TEXT,
    focus_expired_count  INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    completed_at         TEXT,
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     TEXT,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   TEXT,
    blocked_by           TEXT NOT NULL DEFAULT '[]',
    blocks               TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX idx_actions_area_id ON actions(area_id);
CREATE INDEX idx_actions_project_id ON actions(project_id);
CREATE INDEX idx_actions_key_result_id ON actions(key_result_id);
CREATE INDEX idx_actions_parent_id ON actions(parent_id);
CREATE INDEX idx_actions_status ON actions(status);
CREATE INDEX idx_actions_due_date ON actions(due_date);
CREATE INDEX idx_actions_focused_at ON actions(focused_at);
CREATE INDEX idx_actions_is_template ON actions(is_template);

-- Supporting tables
CREATE TABLE action_attachments (
    id              TEXT PRIMARY KEY,
    action_id       TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_action_attachments_action_id ON action_attachments(action_id);

CREATE TABLE action_time_entries (
    id            TEXT PRIMARY KEY,
    action_id     TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    note          TEXT
);
CREATE INDEX idx_action_time_entries_action_id ON action_time_entries(action_id);

CREATE TABLE action_dependencies (
    action_id  TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    blocker_id TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    PRIMARY KEY (action_id, blocker_id),
    CHECK (action_id != blocker_id)
);
CREATE INDEX idx_action_dependencies_blocker_id ON action_dependencies(blocker_id);

-- PARA: Resources & Archive (schema-only, no tool support yet)
CREATE TABLE resources (
    id            TEXT PRIMARY KEY,
    area_id       TEXT REFERENCES areas(id) ON DELETE SET NULL,
    title         TEXT NOT NULL,
    description   TEXT,
    resource_type TEXT NOT NULL DEFAULT 'note',
    content       TEXT,
    url           TEXT,
    tags          TEXT NOT NULL DEFAULT '[]',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE archive_items (
    id              TEXT PRIMARY KEY,
    source_type     TEXT NOT NULL,
    source_id       TEXT NOT NULL,
    title           TEXT NOT NULL,
    snapshot        TEXT NOT NULL,
    archived_at     TEXT NOT NULL,
    archived_reason TEXT
);
CREATE INDEX idx_archive_items_source ON archive_items(source_type, source_id);
```

## Domain Types

### Area (new, `domain` crate)

```rust
pub struct Area {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: AreaColor,        // Blue | Green | Purple | Orange | Red | Yellow | Gray
    pub icon: Option<String>,
    pub position: i32,
    pub status: AreaStatus,      // Active | Archived
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Project (rewritten, `domain` crate)

```rust
pub struct Project {
    pub id: String,
    pub area_id: String,          // required
    pub name: String,
    pub description: Option<String>,
    pub color: ProjectColor,
    pub tags: Vec<String>,
    pub status: ProjectStatus,    // Active | Paused | Completed | Archived
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Objective (new, replaces Goal, `domain` crate)

```rust
pub struct Objective {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: ObjectiveStatus,  // Active | Paused | Completed | Abandoned
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub progress: f64,            // 0.0-100.0, derived from KRs
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

### KeyResult (new, `domain` crate)

```rust
pub struct KeyResult {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: KeyResultStatus,      // Active | Completed | Abandoned
    pub tracking_mode: TrackingMode,  // Metric | Action
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub progress: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum TrackingMode {
    Metric,  // progress = current_value / target_value * 100
    Action,  // progress = completed_actions / total_actions * 100
}
```

### Action (evolution of Todo, `feature-todo` crate)

```rust
pub struct Action {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub area_id: String,              // required
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub parent_id: Option<String>,
    // ... all existing Todo fields preserved
}
```

## Tool Interfaces

### AreaTool (5 actions)

| Action | Params | Description |
|--------|--------|-------------|
| `create` | name, description?, color?, icon? | Create area |
| `list` | status? | List areas (default: active) |
| `show` | id | Area details + project/action counts |
| `update` | id, name?, description?, color?, icon?, status? | Update area |
| `reorder` | id, position | Change display order |

### ProjectTool (8 actions)

| Action | Params | Description |
|--------|--------|-------------|
| `create` | name, area_id, description?, color?, tags? | Create project in area |
| `list` | area_id?, status?, tag? | List projects |
| `show` | id | Project + objective count + action stats |
| `update` | id, name?, description?, color?, tags?, status?, area_id? | Update project |
| `archive` | id | Archive project |
| `tasks` | id | List actions in project |
| `objectives` | id | List objectives with KR summaries |
| `stats` | id | Full OKR + action breakdown |

### OkrTool (14 actions)

| Action | Params | Description |
|--------|--------|-------------|
| `objective.create` | project_id, title, description?, priority?, due_date? | Create objective |
| `objective.list` | project_id?, status? | List objectives |
| `objective.show` | id | Objective + all KRs with progress |
| `objective.update` | id, title?, description?, priority?, due_date?, status? | Update objective |
| `objective.delete` | id | Delete objective |
| `objective.progress` | id | Detailed progress per KR |
| `kr.create` | objective_id, title, description?, tracking_mode?, target_value?, unit?, due_date? | Create key result |
| `kr.list` | objective_id? | List key results |
| `kr.show` | id | KR details + linked actions or metric |
| `kr.update` | id, title?, description?, status?, due_date? | Update key result |
| `kr.update_metric` | id, current_value | Update metric value, recalculate progress chain |
| `kr.add_action` | kr_id, action_id | Link action to KR |
| `kr.remove_action` | kr_id, action_id | Unlink action from KR |
| `kr.delete` | id | Delete key result |

### TaskTool (~22 actions, replaces TodoTool)

Core: `add`, `list`, `show`, `update`, `complete`, `delete`
Subtasks: `add_subtask`, `move`, `add_dependency`, `remove_dependency`, `tree`
Rich features: `focus`, `unfocus`, `attach`, `detach`, `log_time`, `search`, `search_semantic`, `search_hybrid`, `enrich`, `report`, `recur`, `list_recurring`, `delete_recurring`

Key change: `add` requires `area_id`. `complete` triggers progress recalculation up the KR → Objective chain.

## Removals

### Removed systems
- **Goal system**: domain types, repo, tool, handler trait, migrations
- **Plan system**: domain types, repo, tool, executor, handler, step generator, PlannedEngine, PlanCleanupService

### Removed handler traits
- `GoalHandler`, `PlanHandler`, `PlanCompletionHandler`

### New handler trait
- `ProgressHandler` — defined in `tools`, implemented in `agent`. Handles action completion → KR progress → Objective progress recalculation chain.

## Intent Pipeline Changes

- Remove `ExecutionMode::Planned` variant
- Remove `PlannedEngine`
- Remove escalation to Planned (Reactive is the ceiling)
- Simplify `IntentClassifier` — no plan-related classification

## Crate-Level Change Map

### Layer 2 — `domain`
- Remove: `goal.rs`, `plan.rs`
- Add: `area.rs`, `objective.rs`, `key_result.rs`
- Move: Project types from `tools/src/project_types.rs` → `domain/src/project.rs`

### Layer 2 — `storage`
- Remove: `repos/goal.rs`, `repos/plan.rs`
- Add: `repos/area.rs`, `repos/objective.rs`, `repos/key_result.rs`
- Rewrite: `repos/project_repo.rs` (add area_id)
- Rename: `repos/todo_repo.rs` → `repos/action_repo.rs`
- Rewrite: `migrations/001_initial.sql` (full new schema)
- Delete: migrations 002-005

### Layer 4 — `tools`
- Remove: `goal_tool.rs`, `goal_types.rs`, `plan_tool.rs`
- Add: `area_tool.rs`, `okr_tool.rs`, `progress_handler.rs`
- Rewrite: `project_tool.rs`

### Layer 4 — `feature-todo`
- Rename: Todo → Action throughout
- Rename: TodoTool → TaskTool
- Update: add/list/complete/move actions for area_id, key_result_id

### Layer 5 — `agent`
- Remove: plan_executor.rs, plan_handler.rs, plan_step_generator.rs, PlannedEngine, PlanCleanupService
- Update: ExecutionRouter, IntentAnalyzer, IntentClassifier
- Add: ProgressHandler impl
- Update: tool registration

### Progress Recalculation Chain

```
Action completed
  → if action.key_result_id is Some:
    → KeyResult(action mode): count completed/total actions → update progress
    → Parent Objective: average of all KR progresses → update progress
```
