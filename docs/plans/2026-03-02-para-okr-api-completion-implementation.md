# PARA+OKR API Completion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire all CRUD Tauri commands for Areas, Projects, Objectives, Key Results, and full Task editing. Remove mock data. Build detail pages with inline editing and create flows.

**Architecture:** Backend-first approach. The storage layer (repos) is 100% complete. We add Tauri command wrappers in `crates/desktop/src/commands/`, shared types in `crates/desktop-shared/src/commands.rs`, then build frontend detail pages and inline editing. All mutations emit `entity:updated` events for auto-refetch.

**Tech Stack:** Rust (Tauri 2, sqlx, chrono, uuid), TypeScript (React 18, react-router, Tailwind v4, lucide-react)

---

## Task 1: Extend EntityKind and Add ActionPatch.project_id

**Files:**
- Modify: `crates/desktop-shared/src/types.rs`
- Modify: `crates/storage/src/repos/action_repo.rs:926-941` (ActionPatch struct)
- Modify: `crates/storage/src/repos/action_repo.rs:142-182` (update SQL)

**Step 1: Add Area and KeyResult to EntityKind**

In `crates/desktop-shared/src/types.rs`, add two variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityKind {
    Task,
    Project,
    Objective,
    Area,
    KeyResult,
}
```

**Step 2: Add project_id to ActionPatch**

In `crates/storage/src/repos/action_repo.rs`, add to the `ActionPatch` struct after `area_id`:

```rust
pub project_id: Option<Option<String>>,
```

**Step 3: Update the ActionRepo::update SQL**

Add `project_id` binding to the UPDATE query. After the `area_id` line:

```rust
                project_id         = CASE WHEN ?24 THEN ?25 ELSE project_id END,
```

And add bindings:

```rust
        .bind(patch.project_id.is_some())
        .bind(patch.project_id.as_ref().and_then(|v| v.as_deref()))
```

**Step 4: Build and verify**

Run: `cargo build --workspace`
Expected: Compiles with 0 errors.

**Step 5: Commit**

```bash
git add crates/desktop-shared/src/types.rs crates/storage/src/repos/action_repo.rs
git commit -m "feat(storage): extend EntityKind, add project_id to ActionPatch"
```

---

## Task 2: Add All Params Types to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs`

**Step 1: Add params structs**

Append after the existing `TaskCreateParams`:

```rust
// ── Task Update ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
    pub project_id: Option<Option<String>>,
    pub area_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub key_result_id: Option<Option<String>>,
}

// ── Area Params ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaCreateParams {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<Option<String>>,
}

// ── Project Params ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateParams {
    pub name: String,
    pub area_id: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub area_id: Option<String>,
    pub color: Option<String>,
    pub description: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
}

// ── Objective Params ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveCreateParams {
    pub title: String,
    pub project_id: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<String>>,
}

// ── Key Result Params ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultCreateParams {
    pub objective_id: String,
    pub title: String,
    pub target_value: Option<f64>,
    pub unit: Option<String>,
    pub tracking_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
}
```

**Step 2: Build and verify**

Run: `cargo build -p desktop-shared`
Expected: Compiles with 0 errors.

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop-shared): add all CRUD params types"
```

---

## Task 3: Implement Task Update and Delete Commands

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`

**Step 1: Add imports**

Add `TaskUpdateParams` to the import line:

```rust
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, ProjectResponse, TaskCreateParams, TaskUpdateParams,
    TaskResponse, TodayTaskResponse,
};
```

**Step 2: Add task_update command**

After `task_create`, add:

```rust
#[tauri::command]
pub async fn task_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: TaskUpdateParams,
) -> Result<TaskResponse, String> {
    let patch = ActionPatch {
        id: params.id.clone(),
        title: params.title,
        description: params.description,
        priority: params.priority,
        status: params.status,
        due_date: params.due_date.map(|opt| {
            opt.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        }),
        tags: params.tags,
        area_id: params.area_id,
        project_id: params.project_id,
        key_result_id: params.key_result_id,
        ..Default::default()
    };

    let updated = state
        .repos
        .actions
        .update(&patch)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Task, &params.id);

    Ok(action_to_task(&updated))
}
```

**Step 3: Add task_delete command**

```rust
#[tauri::command]
pub async fn task_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, String> {
    let deleted = state
        .repos
        .actions
        .delete(&id)
        .await
        .map_err(|e| e.to_string())?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Task, &id);
    }

    Ok(deleted)
}
```

**Step 4: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors.

**Step 5: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs
git commit -m "feat(desktop): add task_update and task_delete commands"
```

---

## Task 4: Implement Area CRUD Commands

**Files:**
- Modify: `crates/desktop/src/commands/areas.rs`

**Step 1: Replace the file contents**

```rust
use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};
use desktop_shared::types::EntityKind;
use storage::AreaRow;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn area_list(state: State<'_, AppCore>) -> Result<Vec<AreaResponse>, String> {
    let areas = state
        .repos
        .areas
        .list(Some("active"))
        .await
        .map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(areas.len());
    for a in &areas {
        let project_count = state
            .repos
            .areas
            .count_projects(&a.id)
            .await
            .map_err(|e| e.to_string())?;
        let task_count = state
            .repos
            .areas
            .count_actions(&a.id)
            .await
            .map_err(|e| e.to_string())?;

        results.push(AreaResponse {
            id: a.id.clone(),
            name: a.name.clone(),
            color: a.color.clone(),
            icon: a.icon.clone(),
            project_count,
            task_count,
        });
    }
    Ok(results)
}

#[tauri::command]
pub async fn area_create(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: AreaCreateParams,
) -> Result<AreaResponse, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let row = AreaRow {
        id: id.clone(),
        name: params.name,
        description: None,
        color: params.color.unwrap_or_else(|| "blue".to_string()),
        icon: params.icon,
        position: 0,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    state
        .repos
        .areas
        .create(&row)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Area, &id);

    Ok(AreaResponse {
        id: row.id,
        name: row.name,
        color: row.color,
        icon: row.icon,
        project_count: 0,
        task_count: 0,
    })
}

#[tauri::command]
pub async fn area_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: AreaUpdateParams,
) -> Result<AreaResponse, String> {
    let updated = state
        .repos
        .areas
        .update(
            &params.id,
            params.name.as_deref(),
            None, // description not exposed in desktop UI
            params.color.as_deref(),
            params.icon.as_ref().map(|o| o.as_deref()),
            None, // status not changed via update
        )
        .await
        .map_err(|e| e.to_string())?;

    let project_count = state
        .repos
        .areas
        .count_projects(&params.id)
        .await
        .map_err(|e| e.to_string())?;
    let task_count = state
        .repos
        .areas
        .count_actions(&params.id)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Area, &params.id);

    Ok(AreaResponse {
        id: updated.id,
        name: updated.name,
        color: updated.color,
        icon: updated.icon,
        project_count,
        task_count,
    })
}

#[tauri::command]
pub async fn area_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, String> {
    let deleted = state
        .repos
        .areas
        .delete(&id)
        .await
        .map_err(|e| e.to_string())?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Area, &id);
    }

    Ok(deleted)
}

#[tauri::command]
pub async fn area_reorder(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
    position: i32,
) -> Result<AreaResponse, String> {
    let updated = state
        .repos
        .areas
        .reorder(&id, position)
        .await
        .map_err(|e| e.to_string())?;

    let project_count = state
        .repos
        .areas
        .count_projects(&id)
        .await
        .map_err(|e| e.to_string())?;
    let task_count = state
        .repos
        .areas
        .count_actions(&id)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Area, &id);

    Ok(AreaResponse {
        id: updated.id,
        name: updated.name,
        color: updated.color,
        icon: updated.icon,
        project_count,
        task_count,
    })
}
```

**Step 2: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors.

**Step 3: Commit**

```bash
git add crates/desktop/src/commands/areas.rs
git commit -m "feat(desktop): add area CRUD commands"
```

---

## Task 5: Implement Project CRUD Commands

**Files:**
- Create: `crates/desktop/src/commands/projects.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod projects;`)

**Step 1: Add module declaration**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod projects;
```

**Step 2: Create projects.rs**

```rust
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, ProjectCreateParams, ProjectResponse,
    ProjectUpdateParams,
};
use desktop_shared::types::EntityKind;
use storage::{ProjectFilter, ProjectPatch, ProjectRow};
use tauri::State;

use crate::app_core::AppCore;
use super::tasks::{objective_to_response, kr_to_response};

fn project_to_response(
    row: &ProjectRow,
    task_count: u32,
    completed_count: u32,
    objective_ids: Vec<String>,
) -> ProjectResponse {
    ProjectResponse {
        id: row.id.clone(),
        name: row.name.clone(),
        color: row.color.clone(),
        area_id: row.area_id.clone(),
        task_count,
        completed_count,
        objective_ids: if objective_ids.is_empty() {
            None
        } else {
            Some(objective_ids)
        },
    }
}

async fn build_project_response(
    state: &AppCore,
    row: &ProjectRow,
) -> Result<ProjectResponse, String> {
    let counts = state
        .repos
        .projects
        .count_tasks_by_status(&row.id)
        .await
        .map_err(|e| e.to_string())?;

    let mut task_count: u32 = 0;
    let mut completed_count: u32 = 0;
    for (status, count) in &counts {
        task_count += *count as u32;
        if status == "done" {
            completed_count = *count as u32;
        }
    }

    let objectives = state
        .repos
        .objectives
        .list(Some(&row.id), None)
        .await
        .map_err(|e| e.to_string())?;
    let objective_ids: Vec<String> = objectives.iter().map(|o| o.id.clone()).collect();

    Ok(project_to_response(row, task_count, completed_count, objective_ids))
}

#[tauri::command]
pub async fn project_create(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: ProjectCreateParams,
) -> Result<ProjectResponse, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let row = ProjectRow {
        id: id.clone(),
        area_id: params.area_id,
        name: params.name,
        description: params.description,
        color: params.color.unwrap_or_else(|| "blue".to_string()),
        tags: params.tags.unwrap_or_default(),
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = state
        .repos
        .projects
        .create(&row)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Project, &id);

    Ok(project_to_response(&created, 0, 0, vec![]))
}

#[tauri::command]
pub async fn project_get(
    state: State<'_, AppCore>,
    id: String,
) -> Result<ProjectResponse, String> {
    let row = state
        .repos
        .projects
        .get_or_err(&id)
        .await
        .map_err(|e| e.to_string())?;

    build_project_response(&state, &row).await
}

#[tauri::command]
pub async fn project_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: ProjectUpdateParams,
) -> Result<ProjectResponse, String> {
    let patch = ProjectPatch {
        id: params.id.clone(),
        name: params.name,
        area_id: params.area_id,
        color: params.color,
        description: params.description,
        tags: params.tags,
        status: params.status,
    };

    let updated = state
        .repos
        .projects
        .update(&patch)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Project, &params.id);

    build_project_response(&state, &updated).await
}

#[tauri::command]
pub async fn project_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, String> {
    let deleted = state
        .repos
        .projects
        .delete(&id)
        .await
        .map_err(|e| e.to_string())?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Project, &id);
    }

    Ok(deleted)
}

#[tauri::command]
pub async fn project_archive(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<ProjectResponse, String> {
    let archived = state
        .repos
        .projects
        .archive(&id)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Project, &id);

    build_project_response(&state, &archived).await
}
```

**Step 3: Make converters in tasks.rs public**

In `crates/desktop/src/commands/tasks.rs`, change these functions from private to `pub(super)`:

```rust
pub(super) fn objective_to_response(...)
pub(super) fn kr_to_response(...)
```

**Step 4: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors.

**Step 5: Commit**

```bash
git add crates/desktop/src/commands/projects.rs crates/desktop/src/commands/mod.rs crates/desktop/src/commands/tasks.rs
git commit -m "feat(desktop): add project CRUD commands"
```

---

## Task 6: Implement Objective CRUD Commands

**Files:**
- Create: `crates/desktop/src/commands/objectives.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod objectives;`)

**Step 1: Add module declaration**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod objectives;
```

**Step 2: Create objectives.rs**

```rust
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams,
};
use desktop_shared::types::EntityKind;
use storage::ObjectiveRow;
use tauri::State;

use crate::app_core::AppCore;
use super::tasks::{objective_to_response, kr_to_response};

async fn build_objective_response(
    state: &AppCore,
    row: &ObjectiveRow,
) -> Result<ObjectiveResponse, String> {
    let kr_rows = state
        .repos
        .key_results
        .list(Some(&row.id))
        .await
        .map_err(|e| e.to_string())?;

    let krs = if kr_rows.is_empty() {
        None
    } else {
        Some(kr_rows.iter().map(kr_to_response).collect())
    };

    Ok(objective_to_response(row, krs))
}

#[tauri::command]
pub async fn objective_create(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: ObjectiveCreateParams,
) -> Result<ObjectiveResponse, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let due_date = params
        .due_date
        .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());

    let row = ObjectiveRow {
        id: id.clone(),
        project_id: params.project_id,
        title: params.title,
        description: params.description,
        status: "active".to_string(),
        priority: params.priority,
        due_date,
        progress: 0.0,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    let created = state
        .repos
        .objectives
        .create(&row)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Objective, &id);

    Ok(objective_to_response(&created, None))
}

#[tauri::command]
pub async fn objective_get(
    state: State<'_, AppCore>,
    id: String,
) -> Result<ObjectiveResponse, String> {
    let row = state
        .repos
        .objectives
        .get_or_err(&id)
        .await
        .map_err(|e| e.to_string())?;

    build_objective_response(&state, &row).await
}

#[tauri::command]
pub async fn objective_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: ObjectiveUpdateParams,
) -> Result<ObjectiveResponse, String> {
    let due_date = params.due_date.map(|opt| {
        opt.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
    });

    let updated = state
        .repos
        .objectives
        .update(
            &params.id,
            params.title.as_deref(),
            params.description.as_ref().map(|o| o.as_deref()),
            params.status.as_deref(),
            params.priority.as_ref().map(|o| *o),
            due_date,
        )
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Objective, &params.id);

    build_objective_response(&state, &updated).await
}

#[tauri::command]
pub async fn objective_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, String> {
    let deleted = state
        .repos
        .objectives
        .delete(&id)
        .await
        .map_err(|e| e.to_string())?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Objective, &id);
    }

    Ok(deleted)
}
```

**Step 3: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors.

**Step 4: Commit**

```bash
git add crates/desktop/src/commands/objectives.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(desktop): add objective CRUD commands"
```

---

## Task 7: Implement Key Result CRUD Commands

**Files:**
- Create: `crates/desktop/src/commands/key_results.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod key_results;`)

**Step 1: Add module declaration**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod key_results;
```

**Step 2: Create key_results.rs**

```rust
use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::types::EntityKind;
use storage::KeyResultRow;
use tauri::State;

use crate::app_core::AppCore;
use super::tasks::kr_to_response;

#[tauri::command]
pub async fn key_result_create(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: KeyResultCreateParams,
) -> Result<KeyResultResponse, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let row = KeyResultRow {
        id: id.clone(),
        objective_id: params.objective_id.clone(),
        title: params.title,
        description: None,
        status: "active".to_string(),
        tracking_mode: params.tracking_mode.unwrap_or_else(|| "metric".to_string()),
        target_value: params.target_value,
        current_value: 0.0,
        unit: params.unit,
        progress: 0.0,
        due_date: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    let created = state
        .repos
        .key_results
        .create(&row)
        .await
        .map_err(|e| e.to_string())?;

    // Recalculate parent objective progress
    let _ = state
        .repos
        .objectives
        .recalculate_progress(&params.objective_id)
        .await;

    super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
    super::emit_entity_updated(&app, EntityKind::Objective, &params.objective_id);

    Ok(kr_to_response(&created))
}

#[tauri::command]
pub async fn key_result_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: KeyResultUpdateParams,
) -> Result<KeyResultResponse, String> {
    let due_date = params.due_date.map(|opt| {
        opt.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
    });

    let updated = state
        .repos
        .key_results
        .update(
            &params.id,
            params.title.as_deref(),
            params.description.as_ref().map(|o| o.as_deref()),
            params.status.as_deref(),
            due_date,
        )
        .await
        .map_err(|e| e.to_string())?;

    // Recalculate parent objective progress
    let _ = state
        .repos
        .objectives
        .recalculate_progress(&updated.objective_id)
        .await;

    super::emit_entity_updated(&app, EntityKind::KeyResult, &params.id);
    super::emit_entity_updated(&app, EntityKind::Objective, &updated.objective_id);

    Ok(kr_to_response(&updated))
}

#[tauri::command]
pub async fn key_result_update_metric(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
    current_value: f64,
) -> Result<KeyResultResponse, String> {
    let updated = state
        .repos
        .key_results
        .update_metric(&id, current_value)
        .await
        .map_err(|e| e.to_string())?;

    // Recalculate parent objective progress
    let _ = state
        .repos
        .objectives
        .recalculate_progress(&updated.objective_id)
        .await;

    super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
    super::emit_entity_updated(&app, EntityKind::Objective, &updated.objective_id);

    Ok(kr_to_response(&updated))
}

#[tauri::command]
pub async fn key_result_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, String> {
    // Get the KR first to know the parent objective
    let kr = state
        .repos
        .key_results
        .get_or_err(&id)
        .await
        .map_err(|e| e.to_string())?;

    let deleted = state
        .repos
        .key_results
        .delete(&id)
        .await
        .map_err(|e| e.to_string())?;

    if deleted {
        // Recalculate parent objective progress
        let _ = state
            .repos
            .objectives
            .recalculate_progress(&kr.objective_id)
            .await;

        super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
        super::emit_entity_updated(&app, EntityKind::Objective, &kr.objective_id);
    }

    Ok(deleted)
}
```

**Step 3: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors.

**Step 4: Commit**

```bash
git add crates/desktop/src/commands/key_results.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(desktop): add key_result CRUD commands"
```

---

## Task 8: Register All New Commands in main.rs

**Files:**
- Modify: `crates/desktop/src/main.rs:104-119` (invoke_handler)

**Step 1: Update invoke_handler**

Replace the `invoke_handler` block:

```rust
        .invoke_handler(tauri::generate_handler![
            // Tasks
            commands::tasks::today_tasks,
            commands::tasks::task_list,
            commands::tasks::task_create,
            commands::tasks::task_update,
            commands::tasks::task_delete,
            commands::tasks::task_toggle_complete,
            commands::tasks::project_list,
            commands::tasks::objective_list,
            // Areas
            commands::areas::area_list,
            commands::areas::area_create,
            commands::areas::area_update,
            commands::areas::area_delete,
            commands::areas::area_reorder,
            // Projects
            commands::projects::project_create,
            commands::projects::project_get,
            commands::projects::project_update,
            commands::projects::project_delete,
            commands::projects::project_archive,
            // Objectives
            commands::objectives::objective_create,
            commands::objectives::objective_get,
            commands::objectives::objective_update,
            commands::objectives::objective_delete,
            // Key Results
            commands::key_results::key_result_create,
            commands::key_results::key_result_update,
            commands::key_results::key_result_update_metric,
            commands::key_results::key_result_delete,
            // Chat
            commands::chat::chat_threads,
            commands::chat::chat_messages,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            // Calendar
            commands::calendar::calendar_events,
            // Status
            commands::status::agent_status,
            // Window
            commands::window::resize_window,
        ])
```

**Step 2: Build the full workspace**

Run: `cargo build --workspace`
Expected: Compiles with 0 errors, 0 warnings.

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 4: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): register all CRUD commands in invoke_handler"
```

---

## Task 9: Remove Mock Data and Add Empty States

**Files:**
- Delete: `desktop-ui/src/data/mockData.ts`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx`
- Modify: `desktop-ui/src/components/views/SystemTray.tsx`
- Modify: `desktop-ui/src/components/views/Chat.tsx`
- Modify: `desktop-ui/src/hooks/useQuery.ts`

**Step 1: Update useQuery to use empty defaults**

In `desktop-ui/src/hooks/useQuery.ts`, change the initial state to handle no fallback:

```typescript
import { useState, useEffect, useCallback, useRef } from 'react';
import { ipc } from './useIpc';
import { isTauri } from '../lib/utils';

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Fetches data from a Tauri command. Falls back to `fallback` in browser dev mode.
 */
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown>,
  fallback?: T,
): QueryResult<T> {
  const [data, setData] = useState<T>((fallback ?? null) as T);
  const [loading, setLoading] = useState(isTauri);
  const [error, setError] = useState<string | null>(null);
  const argsRef = useRef(args);
  argsRef.current = args;

  const fetch = useCallback(() => {
    if (!isTauri) return;

    setLoading(true);
    setError(null);

    ipc<T>(cmd, argsRef.current)
      .then(setData)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [cmd]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { data, loading, error, refetch: fetch };
}
```

**Step 2: Update MainApp.tsx — remove mock imports, use empty arrays**

Remove the mock import line and replace with empty array defaults:

```typescript
// Remove this line:
// import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';

// Change these:
const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>('task_list', undefined, []);
const { data: projects, refetch: refetchProjects } = useQuery<Project[]>('project_list', undefined, []);
const { data: objectives } = useQuery<Objective[]>('objective_list', undefined, []);
```

Add an empty state after the `<TaskTable>` component when tasks are empty:

```tsx
{filteredTasks.length === 0 && (
  <div className="flex flex-col items-center justify-center py-20 text-center">
    <p className="text-muted text-sm font-light">No tasks yet</p>
    <p className="text-dim text-xs font-light mt-1">Create a task to get started</p>
  </div>
)}
```

**Step 3: Update ProjectDetail.tsx — remove mock imports**

Remove mock imports and computed mock data:

```typescript
// Remove these lines:
// import { mockProjects, mockTasks, mockObjectives } from '../../data/mockData';
// const mockProjectTasks = useMemo(...)
// const mockProjectObjectives = useMemo(...)

// Change to:
const { data: allProjects } = useQuery<Project[]>('project_list', undefined, []);
const { data: tasks, refetch: refetchTasks } = useQuery<Task[]>(
  'task_list',
  id ? { project_id: id } : undefined,
  [],
);
const { data: objectives } = useQuery<Objective[]>(
  'objective_list',
  id ? { project_id: id } : undefined,
  [],
);
```

**Step 4: Update SystemTray.tsx and Chat.tsx similarly**

Remove mock imports, use empty array fallbacks.

**Step 5: Delete mockData.ts**

Run: `rm desktop-ui/src/data/mockData.ts`

If the `data/` directory is now empty, remove it too.

**Step 6: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds with no errors.

**Step 7: Commit**

```bash
git add -A desktop-ui/src/
git commit -m "feat(desktop-ui): remove mock data, use real Tauri IPC with empty state fallbacks"
```

---

## Task 10: Add Frontend TypeScript Types for New Commands

**Files:**
- Modify: `desktop-ui/src/lib/types.ts`

**Step 1: Add mutation param types**

Append to `types.ts`:

```typescript
// ── Mutation Params ─────────────────────────────────────────────────────

export interface TaskUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  priority?: number | null;
  status?: string;
  dueDate?: string | null;
  projectId?: string | null;
  areaId?: string;
  tags?: string[];
  keyResultId?: string | null;
}

export interface AreaCreateParams {
  name: string;
  color?: string;
  icon?: string;
}

export interface AreaUpdateParams {
  id: string;
  name?: string;
  color?: string;
  icon?: string | null;
}

export interface ProjectCreateParams {
  name: string;
  areaId: string;
  color?: string;
  description?: string;
  tags?: string[];
}

export interface ProjectUpdateParams {
  id: string;
  name?: string;
  areaId?: string;
  color?: string;
  description?: string | null;
  tags?: string[];
  status?: string;
}

export interface ObjectiveCreateParams {
  title: string;
  projectId: string;
  description?: string;
  priority?: number;
  dueDate?: string;
}

export interface ObjectiveUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  priority?: number | null;
  dueDate?: string | null;
}

export interface KeyResultCreateParams {
  objectiveId: string;
  title: string;
  targetValue?: number;
  unit?: string;
  trackingMode?: string;
}

export interface KeyResultUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  dueDate?: string | null;
}
```

**Step 2: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/types.ts
git commit -m "feat(desktop-ui): add TypeScript mutation param types"
```

---

## Task 11: Build TaskDetail Page

**Files:**
- Create: `desktop-ui/src/components/views/TaskDetail.tsx`
- Modify: `desktop-ui/src/App.tsx` (add route)

**Step 1: Add route in App.tsx**

Add after the project detail route:

```tsx
import { TaskDetail } from "./components/views/TaskDetail";

// In router:
{ path: "/task/:id", element: <TaskDetail /> },
```

**Step 2: Create TaskDetail.tsx**

Build a full-page detail view with:
- Back button (navigate to `/`)
- Inline editable title (click to edit, Enter to save, Escape to cancel)
- Status dropdown (todo/doing/done)
- Priority cycle (click badge to cycle P1→P2→P3→P4→none)
- Due date input (type="date")
- Project dropdown (select from loaded projects)
- Tags display
- Description textarea (inline editable)
- Delete button with inline "click again" confirmation
- Chat panel toggle (same as MainApp)
- All mutations via `useMutation` calling `task_update` / `task_delete`
- `useEvent` for auto-refresh

The component should:
- Load task via `task_list` with filter `{ id }` (or we can add a `task_get` — but filtering the list works fine for now since tasks.rs already supports filtering)
- Actually, load all tasks and find by id, OR call task_list with no filter and find. Simpler: use the existing `task_list` command and find the task in the result.
- Use `useMutation('task_update')` for all field changes
- Use `useMutation('task_delete')` for deletion, navigate back after

This is a substantial component. The implementing engineer should build it following the design doc patterns: inline editing with `useState` for edit mode per field, `onBlur` or `onKeyDown` Enter to save.

**Step 3: Make TaskRow navigate to TaskDetail on click**

In `desktop-ui/src/components/tasks/TaskRow.tsx`, add:
- `onClick` handler on the row div (excluding checkbox) that navigates to `/task/${task.id}`
- Use `useNavigate` from react-router

**Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 5: Commit**

```bash
git add desktop-ui/src/components/views/TaskDetail.tsx desktop-ui/src/App.tsx desktop-ui/src/components/tasks/TaskRow.tsx
git commit -m "feat(desktop-ui): add TaskDetail page with inline editing"
```

---

## Task 12: Build ObjectiveDetail Page

**Files:**
- Create: `desktop-ui/src/components/views/ObjectiveDetail.tsx`
- Modify: `desktop-ui/src/App.tsx` (add route)

**Step 1: Add route**

```tsx
import { ObjectiveDetail } from "./components/views/ObjectiveDetail";

{ path: "/objective/:id", element: <ObjectiveDetail /> },
```

**Step 2: Create ObjectiveDetail.tsx**

Build a detail view with:
- Back button
- Inline editable title
- Progress bar (read-only, auto-calculated)
- Project link (breadcrumb to `/project/:projectId`)
- Status dropdown (active/paused/completed/abandoned)
- Key Results list — each KR row shows:
  - Title (inline editable)
  - Current value (number input, editable) — calls `key_result_update_metric` on change
  - Target value display
  - Unit display
  - Progress bar per KR (auto-calculated)
- "Add Key Result" row at bottom (type title, press Enter)
- Delete KR button per row
- Delete objective button with inline confirmation
- Uses `useMutation` for `objective_update`, `key_result_update_metric`, `key_result_create`, `key_result_delete`, `objective_delete`

**Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 4: Commit**

```bash
git add desktop-ui/src/components/views/ObjectiveDetail.tsx desktop-ui/src/App.tsx
git commit -m "feat(desktop-ui): add ObjectiveDetail page with KR inline editing"
```

---

## Task 13: Enhance ProjectDetail with Edit/Create Capabilities

**Files:**
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx`

**Step 1: Add inline editing to project name**

- Click project name in header → switches to input
- Enter saves via `useMutation('project_update')`
- Escape cancels

**Step 2: Add color picker**

- Click color dot → shows a simple color palette dropdown (preset colors matching ProjectColor enum: red, orange, yellow, green, blue, purple, gray)
- Select saves via `project_update`

**Step 3: Add "New Task" button**

- Button below task table header
- Click shows an inline input row
- Type title, Enter → calls `useMutation('task_create')` with `project_id`
- Refetches on `entity:updated`

**Step 4: Add "New Objective" button**

- Button below OKR section header
- Same pattern: inline input, Enter to create via `objective_create`

**Step 5: Add archive button**

- In header area, small archive button
- Click → inline "Archive? Click again" confirmation
- Second click → `useMutation('project_archive')` → navigate back to `/`

**Step 6: Make objective rows clickable → navigate to ObjectiveDetail**

**Step 7: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 8: Commit**

```bash
git add desktop-ui/src/components/views/ProjectDetail.tsx
git commit -m "feat(desktop-ui): enhance ProjectDetail with edit, create, archive"
```

---

## Task 14: Add Inline Editing to Task List Views

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx`
- Modify: `desktop-ui/src/components/tasks/ProjectHeader.tsx`
- Modify: `desktop-ui/src/components/tasks/Toolbar.tsx`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`

**Step 1: TaskRow — priority cycling**

- Click priority badge → cycle through P1→P2→P3→P4→null
- Calls `useMutation('task_update')` with new priority
- Prevent event propagation (don't navigate to detail)

**Step 2: TaskRow — click title to rename**

- Double-click title → inline input
- Enter saves, Escape cancels
- Single click → navigate to TaskDetail

**Step 3: ProjectHeader — click name to rename**

- Click project name → inline input
- Enter saves via `project_update`

**Step 4: Toolbar — "Add task" creates inline row**

- Click "Add task" → adds an empty input row at top of task list
- Type title, Enter → calls `task_create`
- Escape cancels

**Step 5: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 6: Commit**

```bash
git add desktop-ui/src/components/tasks/ desktop-ui/src/components/views/MainApp.tsx
git commit -m "feat(desktop-ui): add inline editing to task list views"
```

---

## Task 15: Add OKR List View with Navigation

**Files:**
- Create: `desktop-ui/src/components/views/OkrView.tsx`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`
- Modify: `desktop-ui/src/App.tsx`

**Step 1: Create OkrView**

A standalone view showing all objectives grouped by project:
- Each objective row: title, progress bar, project badge, status
- Click objective → navigate to `/objective/:id`
- "New Objective" inline row at bottom of each project group
- Uses `useQuery('objective_list')` and `useQuery('project_list')`

**Step 2: Wire sidebar navigation**

In MainApp, when `activeSidebar === 'OKR'`, show `<OkrView />` instead of the task table.

**Step 3: Add route for direct OKR access**

```tsx
{ path: "/okr", element: <OkrView /> },
```

**Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 5: Commit**

```bash
git add desktop-ui/src/components/views/OkrView.tsx desktop-ui/src/components/views/MainApp.tsx desktop-ui/src/App.tsx
git commit -m "feat(desktop-ui): add OKR list view with objective navigation"
```

---

## Task 16: Final Integration Test and Cleanup

**Step 1: Run full backend test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 3: Check formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 4: Build frontend**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

**Step 5: Verify no stale mock references**

Run: `grep -r "mockData\|mockTasks\|mockProjects\|mockObjectives\|mockTodayTasks\|mockCalendarEvents\|mockChatMessages" desktop-ui/src/ --include="*.ts" --include="*.tsx"`
Expected: No matches (all mock references removed).

**Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final cleanup after PARA+OKR API completion"
```
