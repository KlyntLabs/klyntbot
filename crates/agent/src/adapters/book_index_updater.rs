use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use cognitive::repos::{parse_markdown_to_tree, SqliteBookTreeRepo};
use context_engine::book_index::{BookIndex, BookTreeRepo, SourceType};

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
                                if let Err(e) = handle_event(&tree_repo, &book_index, event).await {
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
) -> common::Result<()> {
    match event {
        bus::DomainEvent::NoteContentChanged { note_id, content } => {
            debug!("BookIndex: rebuilding tree for note {note_id}");
            // Delete existing tree for this note
            tree_repo
                .delete_by_source(&SourceType::Note, &note_id)
                .await?;
            // Parse markdown and insert new tree
            let nodes = parse_markdown_to_tree(&note_id, &content);
            if !nodes.is_empty() {
                tree_repo.insert_nodes(&nodes).await?;
                // Refresh the has_content flag
                book_index.refresh_has_content().await?;
                debug!(
                    "BookIndex: inserted {} tree nodes for note {note_id}",
                    nodes.len()
                );
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
            debug!("BookIndex: task hierarchy changed for project {project_id}");
            // Task tree building is a future enhancement — for now just log
        }
        _ => {} // Ignore other events
    }
    Ok(())
}
