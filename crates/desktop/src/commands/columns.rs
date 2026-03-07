use desktop_shared::commands::{
    ColumnCreateParams, ColumnReorderParams, ColumnUpdateParams, ColumnValueSetParams,
    CustomColumnResponse, CustomColumnValueResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use storage::rows::custom_column::CustomColumnRow;
use tauri::State;

use crate::app_core::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn column_to_response(row: &CustomColumnRow) -> CustomColumnResponse {
    let options: Option<Vec<String>> = row
        .options_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    CustomColumnResponse {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        column_type: row.column_type.clone(),
        options,
        position: row.position,
        width: row.width.unwrap_or(150),
    }
}

fn value_to_response(
    row: &storage::rows::custom_column::CustomColumnValueRow,
) -> CustomColumnValueResponse {
    let value: serde_json::Value =
        serde_json::from_str(&row.value_json).unwrap_or(serde_json::Value::Null);

    CustomColumnValueResponse {
        task_id: row.task_id.clone(),
        column_id: row.column_id.clone(),
        value,
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn custom_column_list(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> Result<Vec<CustomColumnResponse>, ApiError> {
    let rows = state
        .repos
        .custom_columns
        .list_columns(&project_id)
        .await
        .map_err(super::map_storage_err)?;

    Ok(rows.iter().map(column_to_response).collect())
}

#[tauri::command]
pub async fn custom_column_create(
    state: State<'_, Arc<AppCore>>,
    params: ColumnCreateParams,
) -> Result<CustomColumnResponse, ApiError> {
    // Determine position: append after existing columns
    let existing = state
        .repos
        .custom_columns
        .list_columns(&params.project_id)
        .await
        .map_err(super::map_storage_err)?;
    let position = existing.len() as i32;

    let id = format!("cc_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let options_json = params
        .options
        .as_ref()
        .map(|opts| serde_json::to_string(opts).unwrap_or_default());

    let row = CustomColumnRow {
        id,
        project_id: params.project_id,
        name: params.name,
        column_type: params.column_type,
        options_json,
        position,
        width: params.width.or(Some(150)),
        created_at: chrono::Utc::now(),
    };

    let created = state
        .repos
        .custom_columns
        .create_column(&row)
        .await
        .map_err(super::map_storage_err)?;

    Ok(column_to_response(&created))
}

#[tauri::command]
pub async fn custom_column_update(
    state: State<'_, Arc<AppCore>>,
    params: ColumnUpdateParams,
) -> Result<CustomColumnResponse, ApiError> {
    let serialized;
    let options_json = match &params.options {
        None => None,
        Some(None) => Some(None),
        Some(Some(opts)) => {
            serialized = serde_json::to_string(opts).unwrap_or_default();
            Some(Some(serialized.as_str()))
        }
    };

    let updated = state
        .repos
        .custom_columns
        .update_column(
            &params.id,
            params.name.as_deref(),
            options_json,
            params.width,
        )
        .await
        .map_err(super::map_storage_err)?;

    Ok(column_to_response(&updated))
}

#[tauri::command]
pub async fn custom_column_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state
        .repos
        .custom_columns
        .delete_column(&id)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn custom_column_reorder(
    state: State<'_, Arc<AppCore>>,
    params: ColumnReorderParams,
) -> Result<(), ApiError> {
    state
        .repos
        .custom_columns
        .reorder_columns(&params.project_id, &params.ids)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn custom_column_values(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Vec<CustomColumnValueResponse>, ApiError> {
    let rows = state
        .repos
        .custom_columns
        .get_values(&task_id)
        .await
        .map_err(super::map_storage_err)?;

    Ok(rows.iter().map(value_to_response).collect())
}

#[tauri::command]
pub async fn custom_column_value_set(
    state: State<'_, Arc<AppCore>>,
    params: ColumnValueSetParams,
) -> Result<(), ApiError> {
    let value_json = serde_json::to_string(&params.value)
        .map_err(|e| ApiError::new("VALIDATION", format!("invalid value: {e}")))?;

    state
        .repos
        .custom_columns
        .set_value(&params.task_id, &params.column_id, &value_json)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn custom_column_value_delete(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
    column_id: String,
) -> Result<bool, ApiError> {
    state
        .repos
        .custom_columns
        .delete_value(&task_id, &column_id)
        .await
        .map_err(super::map_storage_err)
}
