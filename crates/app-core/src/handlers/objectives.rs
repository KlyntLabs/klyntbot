use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::ObjectiveRow;

use super::tasks::{kr_to_response, objective_to_response};
use crate::errors::{map_storage_err, parse_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

pub(crate) async fn build_objective_response(
    state: &AppCore,
    row: &ObjectiveRow,
) -> Result<ObjectiveResponse, ApiError> {
    let kr_rows = state
        .repos
        .key_results
        .list(Some(&row.id))
        .await
        .map_err(map_storage_err)?;

    let krs = if kr_rows.is_empty() {
        None
    } else {
        Some(kr_rows.iter().map(kr_to_response).collect())
    };

    Ok(objective_to_response(row, krs))
}

impl AppCore {
    pub async fn objective_create(
        &self,
        params: ObjectiveCreateParams,
    ) -> HandlerResult<ObjectiveResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = common::time::bridge::chrono_to_jiff(chrono::Utc::now()).into();

        let due_date: Option<storage::SqlTs> = params
            .due_date
            .and_then(|d| parse_date(&d))
            .map(|d| common::time::bridge::chrono_to_jiff(d).into());

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

        let created = self
            .repos
            .objectives
            .create(&row)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Objective,
            id: id.clone(),
        }];

        Ok((objective_to_response(&created, None), updates))
    }

    pub async fn objective_get(&self, id: String) -> Result<ObjectiveResponse, ApiError> {
        let row = self
            .repos
            .objectives
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;

        build_objective_response(self, &row).await
    }

    pub async fn objective_update(
        &self,
        params: ObjectiveUpdateParams,
    ) -> HandlerResult<ObjectiveResponse> {
        let due_date = params.due_date.map(|opt| {
            opt.and_then(|d| parse_date(&d))
                .map(common::time::bridge::chrono_to_jiff)
        });

        let updated = self
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
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Objective,
            id: params.id,
        }];

        let response = build_objective_response(self, &updated).await?;
        Ok((response, updates))
    }

    pub async fn objective_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .objectives
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Objective,
                id,
            }]
        } else {
            vec![]
        };

        Ok((deleted, updates))
    }
}
