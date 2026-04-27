use desktop_shared::commands::{NotebookCreateParams, NotebookResponse, NotebookUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_notes::models::NotebookRow;
use feature_notes::repo::utc_now_str;

use super::converters::notebook_row_to_response;
use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

// ── Notebook handlers ───────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn notebook_list(&self) -> Result<Vec<NotebookResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_notebooks()
            .await
            .map_err(map_storage_err)?;

        let counts = self
            .note_repo
            .count_notes_by_notebook()
            .await
            .map_err(map_storage_err)?;

        Ok(rows
            .iter()
            .map(|row| notebook_row_to_response(row, counts.get(&row.id).copied().unwrap_or(0)))
            .collect())
    }

    #[tracing::instrument(skip(self))]
    pub async fn notebook_create(
        &self,
        params: NotebookCreateParams,
    ) -> HandlerResult<NotebookResponse> {
        if params.title.trim().is_empty() {
            return Err(ApiError::new(
                "VALIDATION",
                "notebook title must not be empty",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = utc_now_str();

        let row = NotebookRow {
            id: id.clone(),
            parent_id: params.parent_id,
            title: params.title,
            icon: params.icon,
            color: params.color,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .note_repo
            .create_notebook(&row)
            .await
            .map_err(map_storage_err)?;

        let response = notebook_row_to_response(&created, 0);
        let updates = vec![EntityUpdate {
            kind: EntityKind::Notebook,
            id,
        }];
        Ok((response, updates))
    }

    #[tracing::instrument(skip(self))]
    pub async fn notebook_update(
        &self,
        params: NotebookUpdateParams,
    ) -> HandlerResult<NotebookResponse> {
        // Check for cycles when changing parent
        if let Some(Some(new_parent_id)) = &params.parent_id {
            if self
                .note_repo
                .would_create_cycle(&params.id, new_parent_id)
                .await
                .map_err(map_storage_err)?
            {
                return Err(ApiError::new(
                    "VALIDATION",
                    "cannot set parent: would create a cycle",
                ));
            }
        }
        let parent_id_ref = params.parent_id.as_ref().map(|o| o.as_deref());
        let icon_ref = params.icon.as_ref().map(|o| o.as_deref());
        let color_ref = params.color.as_ref().map(|o| o.as_deref());
        let updated = self
            .note_repo
            .update_notebook(
                &params.id,
                params.title.as_deref(),
                icon_ref,
                color_ref,
                parent_id_ref,
            )
            .await
            .map_err(map_storage_err)?;

        let count = self
            .note_repo
            .count_notes_in_notebook(&params.id)
            .await
            .map_err(map_storage_err)?;

        let response = notebook_row_to_response(&updated, count);
        let updates = vec![EntityUpdate {
            kind: EntityKind::Notebook,
            id: params.id,
        }];
        Ok((response, updates))
    }

    #[tracing::instrument(skip(self))]
    pub async fn notebook_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .note_repo
            .delete_notebook(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Notebook,
                id,
            }]
        } else {
            vec![]
        };
        Ok((deleted, updates))
    }
}
