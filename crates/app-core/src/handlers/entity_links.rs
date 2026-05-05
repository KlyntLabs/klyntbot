use desktop_shared::entity_link_types::*;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::EntityLinkRow;
use tracing::warn;

use super::project_sources::source_row_to_response;
use super::tasks::priority_label;

use crate::errors::map_storage_err;
use crate::state::{AppCore, HandlerResult};

fn row_to_response(row: &EntityLinkRow) -> EntityLinkResponse {
    EntityLinkResponse {
        id: row.id.clone(),
        source_kind: row.source_kind.clone(),
        source_id: row.source_id.clone(),
        target_kind: row.target_kind.clone(),
        target_id: row.target_id.clone(),
        link_type: row.link_type.clone(),
        metadata: row
            .metadata
            .as_deref()
            .and_then(|m| serde_json::from_str(m).ok()),
        created_at: row.created_at.to_string(),
    }
}

impl AppCore {
    #[tracing::instrument(skip(self))]
    pub async fn entity_link_create(
        &self,
        params: EntityLinkCreateParams,
    ) -> HandlerResult<EntityLinkResponse> {
        let link_type = params.link_type.as_deref().unwrap_or("related");
        let metadata_str = params.metadata.as_ref().map(|m| m.to_string());
        let row = self
            .repos
            .entity_links
            .create(
                &params.source_kind,
                &params.source_id,
                &params.target_kind,
                &params.target_id,
                link_type,
                metadata_str.as_deref(),
            )
            .await
            .map_err(map_storage_err)?;

        Ok((row_to_response(&row), vec![]))
    }

    #[tracing::instrument(skip(self))]
    pub async fn entity_link_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .entity_links
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        Ok((deleted, vec![]))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn entity_links_for_entity(
        &self,
        kind: String,
        id: String,
    ) -> Result<LinkedEntitiesResponse, ApiError> {
        let links = self
            .repos
            .entity_links
            .list_by_entity(&kind, &id)
            .await
            .map_err(map_storage_err)?;

        let mut tasks = vec![];
        let mut notes = vec![];
        let mut conversations = vec![];
        let mut objectives = vec![];
        let mut key_results = vec![];
        let mut sources = vec![];

        // Collect IDs by entity kind for batch fetching
        let mut task_ids: Vec<String> = Vec::new();
        let mut note_ids: Vec<String> = Vec::new();
        let mut conversation_keys: Vec<String> = Vec::new();
        let mut objective_ids: Vec<String> = Vec::new();
        let mut key_result_ids: Vec<String> = Vec::new();
        let mut source_ids: Vec<String> = Vec::new();

        for link in &links {
            let (other_kind, other_id) = if link.source_kind == kind && link.source_id == id {
                (&link.target_kind, &link.target_id)
            } else {
                (&link.source_kind, &link.source_id)
            };

            match EntityKind::parse(other_kind) {
                Some(EntityKind::Task) => task_ids.push(other_id.clone()),
                Some(EntityKind::Note) => note_ids.push(other_id.clone()),
                Some(EntityKind::Conversation) => conversation_keys.push(other_id.clone()),
                Some(EntityKind::Objective) => objective_ids.push(other_id.clone()),
                Some(EntityKind::KeyResult) => key_result_ids.push(other_id.clone()),
                Some(EntityKind::Source) => source_ids.push(other_id.clone()),
                _ => {}
            }
        }

        // Batch fetch all linked entities concurrently
        let (
            tasks_rows,
            notes_rows,
            conversations_rows,
            objectives_rows,
            key_results_rows,
            sources_rows,
        ) = tokio::join!(
            self.repos.tasks.get_by_ids(&task_ids),
            self.note_repo.get_notes_by_ids(&note_ids),
            self.repos.sessions.get_sessions_by_keys(&conversation_keys),
            self.repos.objectives.get_by_ids(&objective_ids),
            self.repos.key_results.get_by_ids(&key_result_ids),
            self.repos.project_sources.get_by_ids(&source_ids),
        );

        if let Ok(rows) = tasks_rows {
            tasks.extend(rows.into_iter().map(|task| ActionSummaryResponse {
                id: task.id,
                title: task.title,
                status: task.status,
                priority: priority_label(task.priority),
            }));
        } else if let Err(e) = tasks_rows {
            warn!(error = %e, "Failed to batch fetch linked tasks");
        }

        if let Ok(rows) = notes_rows {
            notes.extend(rows.into_iter().map(|note| NoteSummaryResponse {
                id: note.id,
                title: note.title,
                updated_at: note.updated_at,
            }));
        } else if let Err(e) = notes_rows {
            warn!(error = %e, "Failed to batch fetch linked notes");
        }

        if let Ok(rows) = conversations_rows {
            conversations.extend(rows.into_iter().map(|session| {
                SessionSummaryResponse {
                    key: session.key,
                    title: session
                        .metadata
                        .as_object()
                        .and_then(|m| m.get("title"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    conversation_type: session.conversation_type,
                    updated_at: session.updated_at.to_string(),
                }
            }));
        } else if let Err(e) = conversations_rows {
            warn!(error = %e, "Failed to batch fetch linked conversations");
        }

        if let Ok(rows) = objectives_rows {
            objectives.extend(rows.into_iter().map(|obj| ObjectiveSummaryResponse {
                id: obj.id,
                title: obj.title,
                progress: obj.progress,
                status: obj.status,
            }));
        } else if let Err(e) = objectives_rows {
            warn!(error = %e, "Failed to batch fetch linked objectives");
        }

        if let Ok(rows) = key_results_rows {
            key_results.extend(rows.into_iter().map(|kr| KeyResultSummaryResponse {
                id: kr.id,
                title: kr.title,
                progress: kr.progress,
            }));
        } else if let Err(e) = key_results_rows {
            warn!(error = %e, "Failed to batch fetch linked key results");
        }

        if let Ok(rows) = sources_rows {
            sources.extend(rows.into_iter().map(|src| source_row_to_response(&src)));
        } else if let Err(e) = sources_rows {
            warn!(error = %e, "Failed to batch fetch linked sources");
        }

        Ok(LinkedEntitiesResponse {
            tasks,
            notes,
            conversations,
            sources,
            objectives,
            key_results,
        })
    }
}
