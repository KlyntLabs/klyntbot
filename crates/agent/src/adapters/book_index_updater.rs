use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use cognitive::repos::{parse_markdown_to_tree, SqliteBookTreeRepo};
use context_engine::book_index::{BookIndex, BookTreeRepo, SourceType};

use super::book_index_entity_extractor::BookIndexEntityExtractor;

/// Background service that updates the BookIndex tree when notes/tasks change.
pub struct BookIndexUpdater {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BookIndexUpdater {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        tree_repo: Arc<SqliteBookTreeRepo>,
        book_index: Arc<BookIndex>,
        cancel: CancellationToken,
        entity_extractor: Option<Arc<BookIndexEntityExtractor>>,
        task_repo: Option<storage::TaskRepo>,
        project_repo: Option<storage::ProjectRepo>,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            info!("BookIndexUpdater started");
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("BookIndexUpdater shutting down");
                        break;
                    }
                    result = event_rx.recv() => {
                        match result {
                            Ok(event) => {
                                if let Err(e) = handle_event(
                                    &tree_repo,
                                    &book_index,
                                    event,
                                    entity_extractor.as_ref(),
                                    task_repo.as_ref(),
                                    project_repo.as_ref(),
                                ).await {
                                    warn!("BookIndexUpdater event handling failed: {e}");
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BookIndexUpdater lagged {n} events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                debug!("BookIndexUpdater channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel_clone,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }
}

async fn handle_event(
    tree_repo: &SqliteBookTreeRepo,
    book_index: &BookIndex,
    event: bus::DomainEvent,
    entity_extractor: Option<&Arc<BookIndexEntityExtractor>>,
    task_repo: Option<&storage::TaskRepo>,
    project_repo: Option<&storage::ProjectRepo>,
) -> common::Result<()> {
    match event {
        bus::DomainEvent::NoteContentChanged { note_id, content } => {
            debug!("BookIndex: rebuilding tree for note {note_id}");
            tree_repo
                .delete_by_source(&SourceType::Note, &note_id)
                .await?;
            let nodes = parse_markdown_to_tree(&note_id, &content);
            if !nodes.is_empty() {
                tree_repo.insert_nodes(&nodes).await?;
                book_index.refresh_has_content().await?;
                debug!(
                    "BookIndex: inserted {} tree nodes for note {note_id}",
                    nodes.len()
                );

                if let Some(extractor) = entity_extractor {
                    super::book_index_entity_extractor::spawn_extract_and_link(
                        extractor,
                        nodes.clone(),
                    );
                }
            }
        }
        bus::DomainEvent::NoteDeleted { note_id } => {
            debug!("BookIndex: deleting tree for note {note_id}");
            tree_repo
                .delete_by_source(&SourceType::Note, &note_id)
                .await?;
            book_index.refresh_has_content().await?;
        }
        bus::DomainEvent::TaskHierarchyChanged { project_id } => {
            debug!("BookIndex: rebuilding task tree for project {project_id}");
            if let Some(task_repo) = task_repo {
                tree_repo
                    .delete_by_source(&SourceType::Task, &project_id)
                    .await?;

                let project_name = if let Some(project_repo) = project_repo {
                    project_repo
                        .get(&project_id)
                        .await
                        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
                        .ok()
                        .flatten()
                        .map(|p| p.name)
                        .unwrap_or_else(|| project_id.clone())
                } else {
                    project_id.clone()
                };

                let tasks = task_repo
                    .list(&storage::TaskFilter {
                        project_id: Some(project_id.clone()),
                        ..Default::default()
                    })
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
                    .unwrap_or_default();

                if !tasks.is_empty() {
                    let nodes = crate::adapters::book_index_task_builder::build_task_tree(
                        &project_id,
                        &project_name,
                        &tasks,
                    );
                    tree_repo.insert_nodes(&nodes).await?;
                    book_index.refresh_has_content().await?;
                    debug!(
                        "BookIndex: inserted {} task tree nodes for project {project_id}",
                        nodes.len()
                    );
                }
            }
        }
        _ => {}
    }
    Ok(())
}
