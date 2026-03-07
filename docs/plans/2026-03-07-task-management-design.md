# Task Management System — monday.com-style Design

**Date:** 2026-03-07
**Branch:** feat/task-management
**Status:** Approved

## Overview

Redesign klyntbot's task management to be a hybrid personal task manager — flexible board UI + AI-powered organization, fully synced. Inspired by monday.com's board/column/group model, adapted for a single-user personal assistant context.

### Design Principles

- **Hybrid interaction**: Board UI and AI chat are both first-class. Use whichever is faster in the moment.
- **Defaults that work instantly**: Ship with sensible defaults so the system is usable without configuration.
- **Customizable when needed**: Per-project workflows, custom columns, user-defined groups.
- **AI-aware**: The system always understands task semantics (done, blocked, active) regardless of custom label names, via status groups.

## 1. Status System

### Status Labels

User-defined, colored labels. Each label maps to a **status group** (semantic category):

| Status Group   | Semantic Meaning   | Default Labels              |
|----------------|--------------------|-----------------------------|
| `not_started`  | Work hasn't begun  | Backlog (gray), Todo (blue) |
| `active`       | In progress        | In Progress (yellow), In Review (orange) |
| `done`         | Completed          | Done (green)                |
| `stuck`        | Blocked/stalled    | Blocked (red)               |

### Key Behaviors

- Every status label maps to exactly one status group.
- The AI queries by group (e.g. "what's blocked?" queries `stuck` group), so custom labels don't break AI understanding.
- Up to ~20 labels per workflow.
- Labels have: id, name, color, group, position (for ordering).

### Workflows

A **status workflow** is an ordered collection of status labels.

- **Global default workflow** ships with the 6 labels above.
- **Per-project workflows** override the global default when assigned.
- Users can create, edit, and delete workflows.

### Workflow Templates

Preset workflows that can be applied to new projects:

- **Simple** — Todo, In Progress, Done
- **Software Dev** — Backlog, Todo, In Progress, In Review, Done, Blocked
- **Content Creation** — Idea, Drafting, Editing, Published

Users can save any workflow as a reusable template.

## 2. User-Defined Groups

Groups are collapsible sections within a project view. They are organizational — independent of status.

- A project starts with one default group: "Main".
- Users can add groups (e.g. "Sprint 1", "This Week", "Someday").
- Each task belongs to exactly one group.
- Groups have: id, project_id, name, color, position.
- Collapsed state persisted in the UI.
- Groups can be reordered via drag-and-drop.

## 3. Kanban with Drag-and-Drop

- Columns generated dynamically from the project's active status labels.
- Drag cards between columns to update status.
- Drag cards within a column to reorder.
- Card displays: title, priority badge, due date, tags, subtask progress.
- Optional WIP limit per column (personal focus limit).

## 4. Custom Columns

Users can add columns to any project board.

### Column Types

| Type         | Stores                  | UI Component          |
|--------------|-------------------------|-----------------------|
| Text         | `string`                | Inline text editor    |
| Number       | `f64`                   | Inline number input   |
| Date         | `date`                  | Date picker           |
| Dropdown     | `string` (from options) | Single-select popup   |
| Multi-select | `string[]`              | Multi-select popup    |
| Checkbox     | `bool`                  | Toggle                |
| Tags         | `string[]`              | Tag editor            |
| Link         | `{url, label}`          | Clickable link        |
| Rating       | `u8` (1-5)              | Star rating           |
| Progress     | `f64` (0-100)           | Progress bar          |
| Duration     | `i64` (seconds)         | Time display          |
| Currency     | `{amount, currency}`    | Formatted money       |

### Storage

Column definitions and values stored generically:

- `custom_columns` — per-project column definitions (id, project_id, name, column_type, options_json, position, width).
- `custom_column_values` — per-task values (task_id, column_id, value_json).

## 5. Data Model Changes

### New Tables

```sql
-- Status workflows and labels
status_workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    is_template BOOLEAN DEFAULT FALSE,
    is_global_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)

status_labels (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES status_workflows(id),
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    status_group TEXT NOT NULL CHECK(status_group IN ('not_started', 'active', 'done', 'stuck')),
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)

-- User-defined groups
task_groups (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    name TEXT NOT NULL,
    color TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)

-- Custom columns
custom_columns (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    name TEXT NOT NULL,
    column_type TEXT NOT NULL,
    options_json TEXT,          -- for dropdown/multi-select: list of options
    position INTEGER NOT NULL DEFAULT 0,
    width INTEGER DEFAULT 150,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)

custom_column_values (
    task_id TEXT NOT NULL REFERENCES actions(id),
    column_id TEXT NOT NULL REFERENCES custom_columns(id),
    value_json TEXT NOT NULL,
    PRIMARY KEY (task_id, column_id)
)
```

### Modified Tables

```sql
-- actions table changes:
-- ADD: group_id TEXT REFERENCES task_groups(id)
-- ADD: status_label_id TEXT REFERENCES status_labels(id)
-- ADD: position INTEGER DEFAULT 0  (for ordering within group)
-- KEEP: status TEXT (for backward compatibility during migration)

-- projects table changes:
-- ADD: workflow_id TEXT REFERENCES status_workflows(id)  (NULL = use global default)
```

## 6. AI Integration

The AI agent interacts with tasks through status groups, not raw labels:

- **Create**: "add a task to review the PR" — AI assigns `active` group status, suggests project.
- **Query**: "what's blocked?" — queries all tasks where status label's group = `stuck`.
- **Update**: "mark X as done" — finds first label in `done` group for that project's workflow.
- **Suggest**: AI can recommend workflow templates based on project description.
- **Focus**: "what should I work on?" — uses status groups + priority + due date to surface actionable tasks.

## 7. Implementation Phases

| Phase | Scope | Depends On |
|-------|-------|------------|
| **Phase 1: Status System** | Workflows, labels, per-project overrides, migration from string statuses, workflow templates, backend API + UI | — |
| **Phase 2: Groups** | Task groups CRUD, group assignment, collapsible UI sections, drag-to-reorder groups | Phase 1 |
| **Phase 3: Kanban DnD** | Dynamic kanban columns from status labels, drag-and-drop between columns and within columns, WIP limits | Phase 1 |
| **Phase 4: Custom Columns** | Column definitions CRUD, generic value storage, 12 column type renderers, column add/remove/reorder UI | Phase 1 |

Phases 2, 3, and 4 can be parallelized after Phase 1 is complete.
