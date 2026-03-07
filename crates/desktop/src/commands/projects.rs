use desktop_shared::commands::{ProjectCreateParams, ProjectResponse, ProjectUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use std::sync::Arc;
use storage::{ProjectPatch, ProjectRow};
use tauri::State;

use crate::app_core::AppCore;

pub(crate) fn project_to_response(
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
        workflow_id: row.workflow_id.clone(),
    }
}

pub(crate) async fn build_project_response(
    state: &AppCore,
    row: &ProjectRow,
) -> Result<ProjectResponse, ApiError> {
    let (counts, objectives) = tokio::try_join!(
        state.repos.projects.count_tasks_by_status(&row.id),
        state.repos.objectives.list(Some(&row.id), None),
    )
    .map_err(super::map_storage_err)?;

    let mut task_count: u32 = 0;
    let mut completed_count: u32 = 0;
    for (status, count) in &counts {
        task_count += *count as u32;
        if status == "done" {
            completed_count = *count as u32;
        }
    }

    let objective_ids: Vec<String> = objectives.iter().map(|o| o.id.clone()).collect();

    Ok(project_to_response(
        row,
        task_count,
        completed_count,
        objective_ids,
    ))
}

#[tauri::command]
pub async fn project_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ProjectCreateParams,
) -> Result<ProjectResponse, ApiError> {
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
        workflow_id: None,
    };

    let created = state
        .repos
        .projects
        .create(&row)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Project, &id);

    Ok(project_to_response(&created, 0, 0, vec![]))
}

#[tauri::command]
pub async fn project_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<ProjectResponse, ApiError> {
    let row = state
        .repos
        .projects
        .get_or_err(&id)
        .await
        .map_err(super::map_storage_err)?;

    build_project_response(&state, &row).await
}

#[tauri::command]
pub async fn project_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ProjectUpdateParams,
) -> Result<ProjectResponse, ApiError> {
    let patch = ProjectPatch {
        id: params.id.clone(),
        name: params.name,
        area_id: params.area_id,
        color: params.color,
        description: params.description,
        tags: params.tags,
        status: params.status,
        workflow_id: params.workflow_id,
    };

    let updated = state
        .repos
        .projects
        .update(&patch)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Project, &params.id);

    build_project_response(&state, &updated).await
}

#[tauri::command]
pub async fn project_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let deleted = state
        .repos
        .projects
        .delete(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Project, &id);
    }

    Ok(deleted)
}

#[tauri::command]
pub async fn project_archive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<ProjectResponse, ApiError> {
    let archived = state
        .repos
        .projects
        .archive(&id)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Project, &id);

    build_project_response(&state, &archived).await
}
