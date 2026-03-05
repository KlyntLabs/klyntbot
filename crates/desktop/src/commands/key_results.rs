use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use std::sync::Arc;
use storage::KeyResultRow;
use tauri::State;

use super::tasks::kr_to_response;
use crate::app_core::AppCore;

#[tauri::command]
pub async fn key_result_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultCreateParams,
) -> Result<KeyResultResponse, ApiError> {
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
        .map_err(super::map_storage_err)?;

    // Recalculate parent objective progress
    if let Err(e) = state
        .repos
        .objectives
        .recalculate_progress(&params.objective_id)
        .await
    {
        tracing::warn!("failed to recalculate objective progress: {e}");
    }

    super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
    super::emit_entity_updated(&app, EntityKind::Objective, &params.objective_id);

    Ok(kr_to_response(&created))
}

#[tauri::command]
pub async fn key_result_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultUpdateParams,
) -> Result<KeyResultResponse, ApiError> {
    let due_date = params
        .due_date
        .map(|opt| opt.and_then(|d| super::parse_date(&d)));

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
        .map_err(super::map_storage_err)?;

    // Recalculate parent objective progress
    if let Err(e) = state
        .repos
        .objectives
        .recalculate_progress(&updated.objective_id)
        .await
    {
        tracing::warn!("failed to recalculate objective progress: {e}");
    }

    super::emit_entity_updated(&app, EntityKind::KeyResult, &params.id);
    super::emit_entity_updated(&app, EntityKind::Objective, &updated.objective_id);

    Ok(kr_to_response(&updated))
}

#[tauri::command]
pub async fn key_result_update_metric(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    current_value: f64,
) -> Result<KeyResultResponse, ApiError> {
    let updated = state
        .repos
        .key_results
        .update_metric(&id, current_value)
        .await
        .map_err(super::map_storage_err)?;

    // Recalculate parent objective progress
    if let Err(e) = state
        .repos
        .objectives
        .recalculate_progress(&updated.objective_id)
        .await
    {
        tracing::warn!("failed to recalculate objective progress: {e}");
    }

    super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
    super::emit_entity_updated(&app, EntityKind::Objective, &updated.objective_id);

    Ok(kr_to_response(&updated))
}

#[tauri::command]
pub async fn key_result_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    // Get the KR first to know the parent objective
    let kr = state
        .repos
        .key_results
        .get_or_err(&id)
        .await
        .map_err(super::map_storage_err)?;

    let deleted = state
        .repos
        .key_results
        .delete(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        // Recalculate parent objective progress
        if let Err(e) = state
            .repos
            .objectives
            .recalculate_progress(&kr.objective_id)
            .await
        {
            tracing::warn!("failed to recalculate objective progress: {e}");
        }

        super::emit_entity_updated(&app, EntityKind::KeyResult, &id);
        super::emit_entity_updated(&app, EntityKind::Objective, &kr.objective_id);
    }

    Ok(deleted)
}
