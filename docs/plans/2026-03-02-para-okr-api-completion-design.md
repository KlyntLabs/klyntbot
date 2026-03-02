# PARA+OKR API Completion Design

**Date:** 2026-03-02
**Status:** Approved
**Approach:** Backend-first (wire all Tauri commands, remove mocks, then build detail pages)

## Context

The PARA+OKR domain migration is complete — Areas, Projects, Objectives, Key Results, and Actions all have domain types, SQLite migrations, and full repository implementations. The desktop-ui frontend is built with Tauri IPC hooks (`useQuery`, `useMutation`, `useEvent`) but currently falls back to mock data because most Tauri commands only cover read operations.

**What exists today:**
- Read commands: `task_list`, `project_list`, `objective_list`, `area_list`, `today_tasks`
- Task mutations: `task_create`, `task_toggle_complete`
- Chat: `chat_threads`, `chat_messages`, `chat_send`, `chat_cancel`
- Calendar: `calendar_events`
- Status: `agent_status`
- Event system: `entity:updated` for frontend cache invalidation

**What's missing:** All CRUD mutations for Areas, Projects, Objectives, Key Results, and full Task editing.

## Design

### 1. Backend — Tauri Command Layer

All repos are complete. Add `#[tauri::command]` wrappers + Params/Response types in `desktop-shared`.

#### Tasks (extend `commands/tasks.rs`)

| Command | Params | Description |
|---------|--------|-------------|
| `task_update` | `id`, partial fields (title, priority, status, due_date, project_id, area_id, tags, description) | General task editing |
| `task_delete` | `id` | Delete a task |

#### Areas (extend `commands/areas.rs`)

| Command | Params | Description |
|---------|--------|-------------|
| `area_create` | `name`, `color`, `icon?` | Create new area |
| `area_update` | `id`, partial fields | Edit area |
| `area_delete` | `id` | Delete area (cascades) |
| `area_reorder` | `id`, `position` | Reorder in sidebar |

#### Projects (new `commands/projects.rs`)

| Command | Params | Description |
|---------|--------|-------------|
| `project_create` | `name`, `area_id`, `color?` | Create project |
| `project_get` | `id` | Get single project with counts |
| `project_update` | `id`, partial fields | Edit project |
| `project_delete` | `id` | Delete project (cascades) |
| `project_archive` | `id` | Archive project |

#### Objectives (new `commands/objectives.rs`)

| Command | Params | Description |
|---------|--------|-------------|
| `objective_create` | `title`, `project_id`, `description?`, `priority?`, `due_date?` | Create objective |
| `objective_get` | `id` | Single objective with KRs |
| `objective_update` | `id`, partial fields | Edit objective |
| `objective_delete` | `id` | Delete objective (cascades KRs) |

#### Key Results (new `commands/key_results.rs`)

| Command | Params | Description |
|---------|--------|-------------|
| `key_result_create` | `objective_id`, `title`, `target_value?`, `unit?`, `tracking_mode?` | Create KR |
| `key_result_update` | `id`, partial fields | Edit KR |
| `key_result_update_metric` | `id`, `current_value` | Update progress (auto-recalculates objective progress) |
| `key_result_delete` | `id` | Delete KR |

Every mutation emits `entity:updated` so the frontend auto-refetches. All use existing `Repos` from `AppCore`.

### 2. Frontend — Navigation & Detail Pages

#### Navigation Model

Detail pages use a **view stack** within the main content area (replace, not overlay):

```
Sidebar (always visible) + Chat Panel (toggleable, persists across all views)
+-- Tasks view (list, grouped by project)
|   +-- TaskDetail view (replaces list)
+-- OKR view (objectives list)
|   +-- ObjectiveDetail view (replaces list)
+-- ProjectDetail view (enhance existing)
|   +-- TaskDetail view (replaces project detail)
+-- Calendar view
+-- Settings view
```

Back navigation via breadcrumb/back button. No deep nesting beyond 2 levels.

#### Detail Pages

**TaskDetail:** Title (inline editable), Status (dropdown), Priority (cycle click), Due date (date picker), Project (dropdown), Area (derived from project or manual), Tags (inline editor), Description (markdown textarea), Linked Objective/KR (link), Delete (inline confirmation — "click again to confirm").

**ObjectiveDetail:** Title (inline editable), Progress bar (auto-calculated, read-only), Project link, Status, Priority, Due date, Key Results list (each KR inline editable: title, current value input, target, unit, auto-recalculating progress), "Add Key Result" button, Delete (inline confirmation).

**ProjectDetail (enhance existing):** Add inline editable name, color picker, status, archive button, "New Task" and "New Objective" buttons.

#### Inline Editing in Lists

- Task rows: click title to rename, click priority to cycle, checkbox to toggle, click row to navigate to TaskDetail
- Project headers: click name to rename, click to navigate to ProjectDetail
- Objective rows: click title to rename, click to navigate to ObjectiveDetail

#### Create Flows (no modals)

- **New Task:** Empty row at top of task list. Type title, Enter. Defaults applied, edit on TaskDetail.
- **New Project:** Empty row at bottom of project list. Type name, Enter.
- **New Objective:** Empty row in OKR view. Type title, select project, Enter.
- **New Area:** Button in sidebar/settings area management.

### 3. Data Flow & Mock Removal

- Remove `mockData.ts` entirely
- `useQuery` in browser dev mode returns empty arrays/null — app shows empty states
- Add empty state messages per view ("No tasks yet", etc.)

**Mutation flow:**
```
User action -> useMutation(cmd, params) -> Tauri invoke -> Repo -> SQLite
  -> emit entity:updated -> useEvent triggers -> useQuery.refetch() -> re-render
```

Optimistic updates for toggle/cycle actions. Creates/deletes wait for `entity:updated` refetch.

**Error handling:** Inline error banners on detail pages (no toasts, no modals). Failed mutations preserve form state for retry.

**EntityKind extension:** Add `Area` and `KeyResult` variants to the enum in `desktop-shared/src/types.rs`.

## Implementation Order

1. Backend: Add all Params/Response types to `desktop-shared`
2. Backend: Implement all Tauri commands (areas, projects, objectives, key_results, task_update, task_delete)
3. Backend: Register commands in `main.rs` invoke_handler
4. Frontend: Remove `mockData.ts`, add empty states
5. Frontend: Build TaskDetail, ObjectiveDetail pages
6. Frontend: Enhance ProjectDetail with edit/create capabilities
7. Frontend: Add inline editing to list views
8. Frontend: Add create flows (empty row pattern)
9. Frontend: Wire navigation stack (view push/pop)
