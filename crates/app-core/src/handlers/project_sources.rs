use desktop_shared::entity_link_types::*;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

fn row_to_response(row: &storage::ProjectSourceRow) -> ProjectSourceResponse {
    ProjectSourceResponse {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        source_type: row.source_type.clone(),
        title: row.title.clone(),
        content: row.content.clone(),
        url: row.url.clone(),
        file_path: row.file_path.clone(),
        metadata: row
            .metadata
            .as_deref()
            .and_then(|m| serde_json::from_str(m).ok()),
        tags: serde_json::from_str(&row.tags).unwrap_or_default(),
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

impl AppCore {
    pub async fn project_source_create(
        &self,
        params: ProjectSourceCreateParams,
    ) -> HandlerResult<ProjectSourceResponse> {
        let row = self
            .repos
            .project_sources
            .create(
                &params.project_id,
                &params.source_type,
                &params.title,
                params.content.as_deref(),
                params.url.as_deref(),
                params.file_path.as_deref(),
            )
            .await
            .map_err(map_storage_err)?;

        let response = row_to_response(&row);
        let updates = vec![EntityUpdate {
            kind: EntityKind::Source,
            id: response.id.clone(),
        }];
        Ok((response, updates))
    }

    pub async fn project_source_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .project_sources
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Source,
                id,
            }]
        } else {
            vec![]
        };
        Ok((deleted, updates))
    }

    pub async fn project_source_list(
        &self,
        project_id: String,
    ) -> Result<Vec<ProjectSourceResponse>, ApiError> {
        let rows = self
            .repos
            .project_sources
            .list_by_project(&project_id)
            .await
            .map_err(map_storage_err)?;

        Ok(rows.iter().map(row_to_response).collect())
    }
}
