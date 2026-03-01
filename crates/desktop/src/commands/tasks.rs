use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, ProjectResponse, TaskCreateParams, TaskResponse,
};
use desktop_shared::types::EntityKind;
use storage::{
    ActionFilter, ActionPatch, ActionRow, KeyResultRow, ObjectiveRow, ProjectFilter, ProjectRow,
};
use tauri::State;

use crate::app_core::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn priority_label(p: Option<i16>) -> Option<String> {
    p.map(|v| format!("P{v}"))
}

pub fn action_to_task(row: &ActionRow) -> TaskResponse {
    TaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        completed: row.status == "done",
        priority: priority_label(row.priority),
        status: row.status.clone(),
        due_date: row.due_date.map(|d| d.format("%b %-d").to_string()),
        tags: row.tags.clone(),
        project_id: row.project_id.clone(),
        area_id: row.area_id.clone(),
        objective_id: row.key_result_id.clone(),
        description: row.description.clone(),
    }
}

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

fn objective_to_response(
    row: &ObjectiveRow,
    key_results: Option<Vec<KeyResultResponse>>,
) -> ObjectiveResponse {
    ObjectiveResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        progress: row.progress,
        project_id: row.project_id.clone(),
        key_results,
    }
}

fn kr_to_response(row: &KeyResultRow) -> KeyResultResponse {
    KeyResultResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        progress: row.progress,
        current: row.current_value,
        target: row.target_value.unwrap_or(0.0),
        unit: row.unit.clone().unwrap_or_default(),
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn task_list(
    state: State<'_, AppCore>,
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<TaskResponse>, String> {
    let filter = ActionFilter {
        area_id,
        project_id,
        status,
        ..Default::default()
    };
    let rows = state
        .repos
        .actions
        .list(&filter)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(action_to_task).collect())
}

#[tauri::command]
pub async fn project_list(
    state: State<'_, AppCore>,
    area_id: Option<String>,
) -> Result<Vec<ProjectResponse>, String> {
    let filter = ProjectFilter {
        area_id,
        status: Some("active".to_string()),
        ..Default::default()
    };
    let projects = state
        .repos
        .projects
        .list(&filter)
        .await
        .map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(projects.len());
    for p in &projects {
        let counts = state
            .repos
            .projects
            .count_tasks_by_status(&p.id)
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
            .list(Some(&p.id), None)
            .await
            .map_err(|e| e.to_string())?;
        let objective_ids: Vec<String> = objectives.iter().map(|o| o.id.clone()).collect();

        results.push(project_to_response(
            p,
            task_count,
            completed_count,
            objective_ids,
        ));
    }
    Ok(results)
}

#[tauri::command]
pub async fn objective_list(
    state: State<'_, AppCore>,
    project_id: Option<String>,
) -> Result<Vec<ObjectiveResponse>, String> {
    let objectives = state
        .repos
        .objectives
        .list(project_id.as_deref(), None)
        .await
        .map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(objectives.len());
    for o in &objectives {
        let kr_rows = state
            .repos
            .key_results
            .list(Some(&o.id))
            .await
            .map_err(|e| e.to_string())?;

        let krs = if kr_rows.is_empty() {
            None
        } else {
            Some(kr_rows.iter().map(kr_to_response).collect())
        };

        results.push(objective_to_response(o, krs));
    }
    Ok(results)
}

#[tauri::command]
pub async fn task_toggle_complete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<TaskResponse, String> {
    let row = state
        .repos
        .actions
        .get_or_err(&id)
        .await
        .map_err(|e| e.to_string())?;

    let new_status = if row.status == "done" {
        "todo".to_string()
    } else {
        "done".to_string()
    };

    let patch = ActionPatch {
        id: id.clone(),
        status: Some(new_status),
        ..Default::default()
    };

    let updated = state
        .repos
        .actions
        .update(&patch)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Task, &id);

    Ok(action_to_task(&updated))
}

#[tauri::command]
pub async fn task_create(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let row = ActionRow {
        id: id.clone(),
        title: params.title,
        description: None,
        area_id: params.area_id.unwrap_or_else(|| "default".to_string()),
        project_id: params.project_id,
        key_result_id: None,
        parent_id: None,
        priority: params.priority,
        due_date: params
            .due_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc()),
        tags: params.tags.unwrap_or_default(),
        status: "todo".to_string(),
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
    };

    let created = state
        .repos
        .actions
        .add(&row)
        .await
        .map_err(|e| e.to_string())?;

    super::emit_entity_updated(&app, EntityKind::Task, &id);

    Ok(action_to_task(&created))
}
