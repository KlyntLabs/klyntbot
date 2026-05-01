//! TaskTreeBuilder — event subscriber that rebuilds task hierarchy tree nodes,
//! persists them to SQLite, embeds them in LanceDB, and emits TreeNodesRebuilt.
//!
//! Listens for `DomainEvent::TaskHierarchyChanged` on the domain event bus.

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, UpdatePriority};
use context_engine::book_index::types::SourceType;

use super::tree_builder_base::{
    compose_embedding_text_with_truncate, run_event_loop, TreeBuilderCore,
};

pub struct TaskTreeBuilder {
    core: TreeBuilderCore,
    pool: SqlitePool,
}

impl TaskTreeBuilder {
    pub fn new(
        tree_repo: Arc<dyn context_engine::book_index::BookTreeRepo>,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn cognitive::TextEmbedder>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
        domain_event_bus: Option<Arc<bus::DomainEventBus>>,
        pool: SqlitePool,
    ) -> Self {
        Self {
            core: TreeBuilderCore::new(
                tree_repo,
                vector_store,
                embedder,
                context_update_queue,
                domain_event_bus,
            ),
            pool,
        }
    }

    /// Event-driven subscriber loop. Listens for `TaskHierarchyChanged` events
    /// and rebuilds the task tree for the affected project.
    pub async fn run(
        self: Arc<Self>,
        rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        // Currently no events are handled; the loop preserves the subscription
        // so future task event wiring can be added here without changing the
        // outer shell.
        run_event_loop("TaskTreeBuilder", rx, shutdown, |_event| async {}).await
    }

    /// Rebuild the task tree for a project. Public for backfill use.
    pub async fn handle_project_changed(&self, project_id: &str) -> common::Result<()> {
        debug!(project_id = %project_id, "TaskTreeBuilder: processing project change");

        // 1. Load project name
        let project_name: Option<(String,)> =
            sqlx::query_as("SELECT name FROM projects WHERE id = ?1")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let project_name = project_name
            .map(|(name,)| name)
            .unwrap_or_else(|| "Unknown Project".to_string());

        // 2. Load tasks for this project
        let tasks: Vec<storage::TaskRow> =
            sqlx::query_as("SELECT * FROM tasks WHERE project_id = ?1")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        // 3. Build tree nodes
        let nodes =
            super::book_index_task_builder::build_task_tree(project_id, &project_name, &tasks);

        if nodes.is_empty() {
            return Ok(());
        }

        // 4. Delete existing tree nodes + embeddings for this project
        if let Err(e) = self
            .core
            .tree_repo
            .delete_by_source(&SourceType::Task, project_id)
            .await
        {
            warn!(project_id = %project_id, "TaskTreeBuilder: failed to delete old tree nodes: {e}");
        }
        if let Err(e) = self
            .core
            .vector_store
            .delete_tree_node_embeddings_by_note(project_id)
            .await
        {
            warn!(project_id = %project_id, "TaskTreeBuilder: failed to delete old embeddings: {e}");
        }

        // 5. Insert new tree nodes
        self.core.tree_repo.insert_nodes(&nodes).await?;

        // 6. Embed each node
        let mut embedded_count = 0_usize;
        for node in &nodes {
            let embed_text = compose_embedding_text_with_truncate(node);
            if embed_text.is_empty() {
                continue;
            }

            match self.core.embedder.embed(&embed_text).await {
                Ok(embedding) => {
                    let level_str = node.level.to_string();
                    if let Err(e) = self
                        .core
                        .vector_store
                        .upsert_tree_node_embedding(
                            &node.id,
                            &embedding,
                            project_id,
                            &level_str,
                            node.source_type.as_str(),
                        )
                        .await
                    {
                        warn!(node_id = %node.id, "TaskTreeBuilder: embedding upsert failed: {e}");
                    } else {
                        embedded_count += 1;
                    }
                }
                Err(e) => {
                    warn!(node_id = %node.id, "TaskTreeBuilder: embedding failed: {e}");
                }
            }
        }

        // 8. Push context update
        if let Some(queue) = &self.core.context_update_queue {
            queue.push(ContextUpdate {
                reason: ContextUpdateReason::NoteStructureChanged,
                content: Some(format!(
                    "Task tree updated for project '{}': {} nodes",
                    project_name,
                    nodes.len()
                )),
                metadata: Some(serde_json::json!({
                    "project_id": project_id,
                    "node_count": nodes.len(),
                    "embedded_count": embedded_count,
                })),
                priority: UpdatePriority::Low,
                timestamp: jiff::Timestamp::now(),
            });
        }

        info!(
            project_id = %project_id,
            node_count = nodes.len(),
            embedded_count,
            "TaskTreeBuilder: task tree rebuilt"
        );

        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
