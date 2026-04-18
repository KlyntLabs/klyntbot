use desktop_shared::commands::{
    ColumnCreateParams, ColumnReorderParams, ColumnUpdateParams, ColumnValueSetParams,
    CustomColumnResponse, CustomColumnValueResponse,
};
use desktop_shared::errors::ApiError;
use storage::rows::custom_column::CustomColumnRow;
use tracing::warn;

use crate::errors::map_storage_err;
use crate::state::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn column_to_response(row: &CustomColumnRow) -> CustomColumnResponse {
    let options: Option<Vec<String>> = match row.options_json.as_deref() {
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(column_id = %row.id, error = %e, "failed to parse options_json");
                None
            }
        },
        None => None,
    };

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
    let value: serde_json::Value = match serde_json::from_str(&row.value_json) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                task_id = %row.task_id, column_id = %row.column_id,
                error = %e, "failed to parse custom column value_json"
            );
            serde_json::Value::Null
        }
    };

    CustomColumnValueResponse {
        task_id: row.task_id.clone(),
        column_id: row.column_id.clone(),
        value,
    }
}

// ── Handler methods ─────────────────────────────────────────────────────

impl AppCore {
    pub async fn custom_column_list(
        &self,
        project_id: String,
    ) -> Result<Vec<CustomColumnResponse>, ApiError> {
        let rows = self
            .repos
            .custom_columns
            .list_columns(&project_id)
            .await
            .map_err(map_storage_err)?;

        Ok(rows.iter().map(column_to_response).collect())
    }

    pub async fn custom_column_create(
        &self,
        params: ColumnCreateParams,
    ) -> Result<CustomColumnResponse, ApiError> {
        let existing = self
            .repos
            .custom_columns
            .list_columns(&params.project_id)
            .await
            .map_err(map_storage_err)?;
        let position = existing.len() as i32;

        let id = format!("cc_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let options_json = params
            .options
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                ApiError::new("VALIDATION", format!("failed to serialize options: {e}"))
            })?;

        let row = CustomColumnRow {
            id,
            project_id: params.project_id,
            name: params.name,
            column_type: params.column_type,
            options_json,
            position,
            width: params.width.or(Some(150)),
            created_at: jiff::Timestamp::now().into(),
        };

        let created = self
            .repos
            .custom_columns
            .create_column(&row)
            .await
            .map_err(map_storage_err)?;

        Ok(column_to_response(&created))
    }

    pub async fn custom_column_update(
        &self,
        params: ColumnUpdateParams,
    ) -> Result<CustomColumnResponse, ApiError> {
        let serialized;
        let options_json = match &params.options {
            None => None,
            Some(None) => Some(None),
            Some(Some(opts)) => {
                serialized = serde_json::to_string(opts).map_err(|e| {
                    ApiError::new("VALIDATION", format!("failed to serialize options: {e}"))
                })?;
                Some(Some(serialized.as_str()))
            }
        };

        let updated = self
            .repos
            .custom_columns
            .update_column(
                &params.id,
                params.name.as_deref(),
                options_json,
                params.width,
            )
            .await
            .map_err(map_storage_err)?;

        Ok(column_to_response(&updated))
    }

    pub async fn custom_column_delete(&self, id: String) -> Result<bool, ApiError> {
        self.repos
            .custom_columns
            .delete_column(&id)
            .await
            .map_err(map_storage_err)
    }

    pub async fn custom_column_reorder(&self, params: ColumnReorderParams) -> Result<(), ApiError> {
        self.repos
            .custom_columns
            .reorder_columns(&params.project_id, &params.ids)
            .await
            .map_err(map_storage_err)
    }

    pub async fn custom_column_values(
        &self,
        task_id: String,
    ) -> Result<Vec<CustomColumnValueResponse>, ApiError> {
        let rows = self
            .repos
            .custom_columns
            .get_values(&task_id)
            .await
            .map_err(map_storage_err)?;

        Ok(rows.iter().map(value_to_response).collect())
    }

    pub async fn custom_column_value_set(
        &self,
        params: ColumnValueSetParams,
    ) -> Result<(), ApiError> {
        let value_json = serde_json::to_string(&params.value)
            .map_err(|e| ApiError::new("VALIDATION", format!("invalid value: {e}")))?;

        self.repos
            .custom_columns
            .set_value(&params.task_id, &params.column_id, &value_json)
            .await
            .map_err(map_storage_err)
    }

    pub async fn custom_column_value_delete(
        &self,
        task_id: String,
        column_id: String,
    ) -> Result<bool, ApiError> {
        self.repos
            .custom_columns
            .delete_value(&task_id, &column_id)
            .await
            .map_err(map_storage_err)
    }
}
