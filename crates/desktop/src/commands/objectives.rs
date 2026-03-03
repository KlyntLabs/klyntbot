use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::ObjectiveRow;
use tauri::State;

use super::tasks::{kr_to_response, objective_to_response};
use crate::app_core::AppCore;

async fn build_objective_response(
    state: &AppCore,
    row: &ObjectiveRow,
) -> Result<ObjectiveResponse, ApiError> {
    let kr_rows = state
        .repos
        .key_results
        .list(Some(&row.id))
        .await
        .map_err(super::map_storage_err)?;

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
) -> Result<ObjectiveResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let due_date = params.due_date.and_then(|d| super::parse_date(&d));

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
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Objective, &id);

    Ok(objective_to_response(&created, None))
}

#[tauri::command]
pub async fn objective_get(
    state: State<'_, AppCore>,
    id: String,
) -> Result<ObjectiveResponse, ApiError> {
    let row = state
        .repos
        .objectives
        .get_or_err(&id)
        .await
        .map_err(super::map_storage_err)?;

    build_objective_response(&state, &row).await
}

#[tauri::command]
pub async fn objective_update(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    params: ObjectiveUpdateParams,
) -> Result<ObjectiveResponse, ApiError> {
    let due_date = params
        .due_date
        .map(|opt| opt.and_then(|d| super::parse_date(&d)));

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
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Objective, &params.id);

    build_objective_response(&state, &updated).await
}

#[tauri::command]
pub async fn objective_delete(
    state: State<'_, AppCore>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let deleted = state
        .repos
        .objectives
        .delete(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Objective, &id);
    }

    Ok(deleted)
}
