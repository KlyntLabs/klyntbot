use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::types::EntityKind;
use storage::KeyResultRow;

use super::tasks::kr_to_response;
use crate::errors::{map_storage_err, parse_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

impl AppCore {
    pub async fn key_result_create(
        &self,
        params: KeyResultCreateParams,
    ) -> HandlerResult<KeyResultResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();

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

        let created = self
            .repos
            .key_results
            .create(&row)
            .await
            .map_err(map_storage_err)?;

        // Recalculate parent objective progress
        if let Err(e) = self
            .repos
            .objectives
            .recalculate_progress(&params.objective_id)
            .await
        {
            tracing::warn!("failed to recalculate objective progress: {e}");
        }

        let updates = vec![
            EntityUpdate {
                kind: EntityKind::KeyResult,
                id: id.clone(),
            },
            EntityUpdate {
                kind: EntityKind::Objective,
                id: params.objective_id,
            },
        ];

        Ok((kr_to_response(&created), updates))
    }

    pub async fn key_result_update(
        &self,
        params: KeyResultUpdateParams,
    ) -> HandlerResult<KeyResultResponse> {
        let due_date = params.due_date.map(|opt| {
            opt.and_then(|d| parse_date(&d))
        });

        let updated = self
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
            .map_err(map_storage_err)?;

        // Recalculate parent objective progress
        if let Err(e) = self
            .repos
            .objectives
            .recalculate_progress(&updated.objective_id)
            .await
        {
            tracing::warn!("failed to recalculate objective progress: {e}");
        }

        let updates = vec![
            EntityUpdate {
                kind: EntityKind::KeyResult,
                id: params.id,
            },
            EntityUpdate {
                kind: EntityKind::Objective,
                id: updated.objective_id.clone(),
            },
        ];

        Ok((kr_to_response(&updated), updates))
    }

    pub async fn key_result_update_metric(
        &self,
        id: String,
        current_value: f64,
    ) -> HandlerResult<KeyResultResponse> {
        let updated = self
            .repos
            .key_results
            .update_metric(&id, current_value)
            .await
            .map_err(map_storage_err)?;

        // Recalculate parent objective progress
        if let Err(e) = self
            .repos
            .objectives
            .recalculate_progress(&updated.objective_id)
            .await
        {
            tracing::warn!("failed to recalculate objective progress: {e}");
        }

        let updates = vec![
            EntityUpdate {
                kind: EntityKind::KeyResult,
                id,
            },
            EntityUpdate {
                kind: EntityKind::Objective,
                id: updated.objective_id.clone(),
            },
        ];

        Ok((kr_to_response(&updated), updates))
    }

    pub async fn key_result_delete(&self, id: String) -> HandlerResult<bool> {
        // Get the KR first to know the parent objective
        let kr = self
            .repos
            .key_results
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;

        let deleted = self
            .repos
            .key_results
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            // Recalculate parent objective progress
            if let Err(e) = self
                .repos
                .objectives
                .recalculate_progress(&kr.objective_id)
                .await
            {
                tracing::warn!("failed to recalculate objective progress: {e}");
            }

            vec![
                EntityUpdate {
                    kind: EntityKind::KeyResult,
                    id,
                },
                EntityUpdate {
                    kind: EntityKind::Objective,
                    id: kr.objective_id,
                },
            ]
        } else {
            vec![]
        };

        Ok((deleted, updates))
    }
}
