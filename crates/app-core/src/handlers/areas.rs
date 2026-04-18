use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::AreaRow;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

pub async fn build_area_response(state: &AppCore, row: &AreaRow) -> Result<AreaResponse, ApiError> {
    let (project_count, task_count) = tokio::try_join!(
        state.repos.areas.count_projects(&row.id),
        state.repos.areas.count_actions(&row.id),
    )
    .map_err(map_storage_err)?;

    Ok(AreaResponse {
        id: row.id.clone(),
        name: row.name.clone(),
        color: row.color.clone(),
        icon: row.icon.clone(),
        project_count,
        task_count,
    })
}

impl AppCore {
    pub async fn area_list(&self) -> Result<Vec<AreaResponse>, ApiError> {
        let areas = self
            .repos
            .areas
            .list(Some("active"))
            .await
            .map_err(map_storage_err)?;

        let mut results = Vec::with_capacity(areas.len());
        for a in &areas {
            results.push(build_area_response(self, a).await?);
        }
        Ok(results)
    }

    pub async fn area_create(&self, params: AreaCreateParams) -> HandlerResult<AreaResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();

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

        self.repos
            .areas
            .create(&row)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Area,
            id: id.clone(),
        }];

        let response = AreaResponse {
            id: row.id,
            name: row.name,
            color: row.color,
            icon: row.icon,
            project_count: 0,
            task_count: 0,
        };

        Ok((response, updates))
    }

    pub async fn area_update(&self, params: AreaUpdateParams) -> HandlerResult<AreaResponse> {
        let updated = self
            .repos
            .areas
            .update(
                &params.id,
                params.name.as_deref(),
                None,
                params.color.as_deref(),
                params.icon.as_ref().map(|o| o.as_deref()),
                None,
            )
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Area,
            id: params.id,
        }];

        let response = build_area_response(self, &updated).await?;
        Ok((response, updates))
    }

    pub async fn area_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .areas
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Area,
                id,
            }]
        } else {
            vec![]
        };

        Ok((deleted, updates))
    }

    pub async fn area_reorder(&self, id: String, position: i32) -> HandlerResult<AreaResponse> {
        let updated = self
            .repos
            .areas
            .reorder(&id, position)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Area,
            id,
        }];

        let response = build_area_response(self, &updated).await?;
        Ok((response, updates))
    }
}
