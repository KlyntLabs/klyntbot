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

    pub async fn entity_link_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .entity_links
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        Ok((deleted, vec![]))
    }

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

        for link in &links {
            let (other_kind, other_id) = if link.source_kind == kind && link.source_id == id {
                (&link.target_kind, &link.target_id)
            } else {
                (&link.source_kind, &link.source_id)
            };

            match EntityKind::parse(other_kind) {
                Some(EntityKind::Task) => match self.repos.tasks.get(other_id).await {
                    Ok(Some(task)) => {
                        tasks.push(ActionSummaryResponse {
                            id: task.id,
                            title: task.title,
                            status: task.status,
                            priority: priority_label(task.priority),
                        });
                    }
                    Err(e) => {
                        warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked task")
                    }
                    _ => {}
                },
                Some(EntityKind::Note) => match self.note_repo.get_note(other_id).await {
                    Ok(Some(note)) => {
                        notes.push(NoteSummaryResponse {
                            id: note.id,
                            title: note.title,
                            updated_at: note.updated_at,
                        });
                    }
                    Err(e) => {
                        warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked note")
                    }
                    _ => {}
                },
                Some(EntityKind::Conversation) => {
                    match self.repos.sessions.get_session(other_id).await {
                        Ok(session) => {
                            conversations.push(SessionSummaryResponse {
                                key: session.key,
                                title: session
                                    .metadata
                                    .as_object()
                                    .and_then(|m| m.get("title"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                conversation_type: session.conversation_type,
                                updated_at: session.updated_at.to_string(),
                            });
                        }
                        Err(e) => {
                            warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked conversation")
                        }
                    }
                }
                Some(EntityKind::Objective) => match self.repos.objectives.get(other_id).await {
                    Ok(Some(obj)) => {
                        objectives.push(ObjectiveSummaryResponse {
                            id: obj.id,
                            title: obj.title,
                            progress: obj.progress,
                            status: obj.status,
                        });
                    }
                    Err(e) => {
                        warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked objective")
                    }
                    _ => {}
                },
                Some(EntityKind::KeyResult) => match self.repos.key_results.get(other_id).await {
                    Ok(Some(kr)) => {
                        key_results.push(KeyResultSummaryResponse {
                            id: kr.id,
                            title: kr.title,
                            progress: kr.progress,
                        });
                    }
                    Err(e) => {
                        warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked key result")
                    }
                    _ => {}
                },
                Some(EntityKind::Source) => match self.repos.project_sources.get(other_id).await {
                    Ok(Some(src)) => {
                        sources.push(source_row_to_response(&src));
                    }
                    Err(e) => {
                        warn!(kind = other_kind, id = other_id, error = %e, "Failed to fetch linked source")
                    }
                    _ => {}
                },
                _ => {}
            }
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
