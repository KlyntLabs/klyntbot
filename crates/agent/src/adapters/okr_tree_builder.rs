//! OkrTreeBuilder — event subscriber that creates tree nodes from OKR/goal
//! events (GoalProgress) so objectives data participates in the entity graph,
//! community detection, and Knowledge Fabric Explorer.
//!
//! Uses **incremental appends**: each progress event creates/updates leaf nodes
//! under a stable per-objective root. Root nodes are idempotent — duplicate-ID
//! inserts are silently ignored because the node already exists.
//!
//! Tree structure:
//! ```text
//! okr-root  (root, level 0, Section, title: "Goals & Objectives")
//!   └── okr-obj-{objective_id}  (level 1, Section, title: objective_id)
//!       └── okr-progress-{objective_id}-{date}  (level 2, Text)
//!             — "Progress: 75% toward 1.0"
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{
    ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, DomainEventBus,
    UpdatePriority,
};
use cognitive::TextEmbedder;
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use context_engine::book_index::BookTreeRepo;

pub struct OkrTreeBuilder {
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
}

impl OkrTreeBuilder {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn TextEmbedder>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
        domain_event_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self {
            tree_repo,
            vector_store,
            embedder,
            context_update_queue,
            domain_event_bus,
        }
    }

    /// Event-driven subscriber loop. Listens for `GoalProgress` events and
    /// appends tree nodes for each progress update.
    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("OkrTreeBuilder: subscriber started");

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("OkrTreeBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::GoalProgress { objective_id, progress, target }) => {
                            if let Err(e) = self.handle_goal_progress(&objective_id, progress, target).await {
                                warn!(
                                    objective_id = %objective_id,
                                    "OkrTreeBuilder: failed to process goal progress: {e}"
                                );
                            }
                        }
                        Ok(_) => {
                            // Not an OKR event — ignore.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("OkrTreeBuilder: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("OkrTreeBuilder: event channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Handle a goal progress event: upsert the global OKR root and per-objective
    /// section node, then insert a dated progress leaf node.
    pub async fn handle_goal_progress(
        &self,
        objective_id: &str,
        progress: f64,
        target: f64,
    ) -> common::Result<()> {
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let obj_id = format!("okr-obj-{objective_id}");
        let progress_id = format!("okr-progress-{objective_id}-{date}");

        debug!(
            objective_id = %objective_id,
            progress,
            target,
            progress_id = %progress_id,
            "OkrTreeBuilder: processing goal progress"
        );

        let nodes =
            build_goal_progress_nodes(objective_id, &obj_id, &progress_id, progress, target);

        self.persist_nodes(&nodes, objective_id).await
    }

    /// Persist nodes: insert each one (ignoring duplicate-ID errors for
    /// idempotent root/objective nodes), embed leaf nodes, push context update,
    /// and emit TreeNodesRebuilt.
    async fn persist_nodes(&self, nodes: &[TreeNode], source_id: &str) -> common::Result<()> {
        let mut inserted = Vec::new();

        for node in nodes {
            match self.tree_repo.insert_node(node).await {
                Ok(()) => inserted.push(node),
                Err(_) => {
                    // Duplicate key — node already exists. Expected for the global
                    // OKR root and per-objective section nodes.
                    debug!(
                        node_id = %node.id,
                        "OkrTreeBuilder: node already exists, skipping insert"
                    );
                }
            }
        }

        // Embed newly inserted nodes.
        let mut embedded_count = 0_usize;
        for node in &inserted {
            let embed_text = compose_embedding_text(node);
            if embed_text.is_empty() {
                continue;
            }
            match self.embedder.embed(&embed_text).await {
                Ok(embedding) => {
                    let level_str = node.level.to_string();
                    if let Err(e) = self
                        .vector_store
                        .upsert_tree_node_embedding(
                            &node.id,
                            &embedding,
                            source_id,
                            &level_str,
                            node.source_type.as_str(),
                        )
                        .await
                    {
                        warn!(
                            node_id = %node.id,
                            "OkrTreeBuilder: failed to upsert embedding: {e}"
                        );
                    } else {
                        embedded_count += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        node_id = %node.id,
                        "OkrTreeBuilder: failed to generate embedding: {e}"
                    );
                }
            }
        }

        // Emit TreeNodesRebuilt so EntityTreeLinker picks up the new nodes.
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(DomainEvent::TreeNodesRebuilt {
                source_type: "okr".to_string(),
                source_id: source_id.to_string(),
            });
        }

        // Push context update for live injection.
        if let Some(queue) = &self.context_update_queue {
            queue.push(ContextUpdate {
                reason: ContextUpdateReason::Custom("goal_progress_updated".to_string()),
                content: Some(format!(
                    "OKR tree updated: {} new node(s) for objective {source_id}",
                    inserted.len()
                )),
                metadata: Some(serde_json::json!({
                    "source_id": source_id,
                    "inserted_count": inserted.len(),
                    "embedded_count": embedded_count,
                })),
                priority: UpdatePriority::Low,
                timestamp: jiff::Timestamp::now(),
            });
        }

        info!(
            source_id = %source_id,
            inserted_count = inserted.len(),
            embedded_count,
            "OkrTreeBuilder: nodes persisted"
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pure node construction helpers (public for unit testing)
// ---------------------------------------------------------------------------

/// Build the nodes for a `GoalProgress` event.
///
/// Returns up to 3 nodes:
/// - Level 0: global OKR root (idempotent, shared across all objectives)
/// - Level 1: per-objective section (idempotent by objective_id)
/// - Level 2: dated progress leaf (deterministic per objective + date — last
///   progress event on a given day wins via the duplicate-ignore logic)
pub fn build_goal_progress_nodes(
    objective_id: &str,
    obj_node_id: &str,
    progress_node_id: &str,
    progress: f64,
    target: f64,
) -> Vec<TreeNode> {
    let pct = if target > 0.0 {
        (progress / target * 100.0).round() as u64
    } else {
        0
    };
    let progress_content = format!("Progress: {pct}% toward {target} (current: {progress:.2})");

    vec![
        // Level 0 — global OKR root (shared across all objectives)
        TreeNode {
            id: "okr-root".to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "okr-root".to_string(),
            title: Some("Goals & Objectives".to_string()),
            level: 0,
            source_type: SourceType::Okr,
            source_id: "okr-root".to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — per-objective section
        TreeNode {
            id: obj_node_id.to_string(),
            parent_id: Some("okr-root".to_string()),
            node_type: TreeNodeType::Section,
            content: objective_id.to_string(),
            title: Some(objective_id.to_string()),
            level: 1,
            source_type: SourceType::Okr,
            source_id: objective_id.to_string(),
            position: 1,
            metadata: None,
        },
        // Level 2 — dated progress leaf
        TreeNode {
            id: progress_node_id.to_string(),
            parent_id: Some(obj_node_id.to_string()),
            node_type: TreeNodeType::Text,
            content: progress_content,
            title: None,
            level: 2,
            source_type: SourceType::Okr,
            source_id: objective_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compose embedding text for an OKR tree node.
/// Level 0: title only; Level 1: objective title; Level 2+: progress content.
fn compose_embedding_text(node: &TreeNode) -> String {
    match node.level {
        0 => node.title.as_deref().unwrap_or("").to_string(),
        1 => node.title.as_deref().unwrap_or(&node.content).to_string(),
        _ => node.content.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- build_goal_progress_nodes ---

    #[test]
    fn progress_nodes_structure() {
        let nodes = build_goal_progress_nodes(
            "obj-launch-v2",
            "okr-obj-obj-launch-v2",
            "okr-progress-obj-launch-v2-2024-01-15",
            0.75,
            1.0,
        );

        assert_eq!(nodes.len(), 3);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "okr-root");
        assert_eq!(root.parent_id, None);
        assert_eq!(root.level, 0);
        assert!(matches!(root.node_type, TreeNodeType::Section));
        assert_eq!(root.title.as_deref(), Some("Goals & Objectives"));
        assert_eq!(root.source_type.as_str(), "okr");

        // Objective section
        let obj = &nodes[1];
        assert_eq!(obj.id, "okr-obj-obj-launch-v2");
        assert_eq!(obj.parent_id.as_deref(), Some("okr-root"));
        assert_eq!(obj.level, 1);
        assert!(matches!(obj.node_type, TreeNodeType::Section));
        assert_eq!(obj.title.as_deref(), Some("obj-launch-v2"));
        assert_eq!(obj.source_type.as_str(), "okr");

        // Progress leaf
        let leaf = &nodes[2];
        assert_eq!(leaf.id, "okr-progress-obj-launch-v2-2024-01-15");
        assert_eq!(leaf.parent_id.as_deref(), Some("okr-obj-obj-launch-v2"));
        assert_eq!(leaf.level, 2);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        assert_eq!(leaf.title, None);
        assert_eq!(leaf.source_type.as_str(), "okr");
    }

    #[test]
    fn progress_content_shows_percentage() {
        let nodes = build_goal_progress_nodes(
            "obj-revenue",
            "okr-obj-obj-revenue",
            "okr-progress-obj-revenue-2024-01-15",
            0.5,
            1.0,
        );
        let leaf = &nodes[2];
        assert!(
            leaf.content.contains("50%"),
            "Expected 50% in: {}",
            leaf.content
        );
        assert!(
            leaf.content.contains("1"),
            "Expected target in: {}",
            leaf.content
        );
    }

    #[test]
    fn progress_content_zero_target_is_zero_pct() {
        let nodes = build_goal_progress_nodes(
            "obj-x",
            "okr-obj-obj-x",
            "okr-progress-obj-x-2024-01-15",
            0.5,
            0.0,
        );
        let leaf = &nodes[2];
        assert!(
            leaf.content.contains("0%"),
            "Expected 0% for zero target: {}",
            leaf.content
        );
    }

    #[test]
    fn all_nodes_have_okr_source_type() {
        let nodes = build_goal_progress_nodes(
            "obj-foo",
            "okr-obj-obj-foo",
            "okr-progress-obj-foo-2024-01-15",
            0.3,
            1.0,
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "okr");
        }
    }

    // --- compose_embedding_text ---

    #[test]
    fn embedding_level_0_returns_title() {
        let node = TreeNode {
            id: "okr-root".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "okr-root".into(),
            title: Some("Goals & Objectives".into()),
            level: 0,
            source_type: SourceType::Okr,
            source_id: "okr-root".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Goals & Objectives");
    }

    #[test]
    fn embedding_level_1_returns_objective_title() {
        let node = TreeNode {
            id: "okr-obj-foo".into(),
            parent_id: Some("okr-root".into()),
            node_type: TreeNodeType::Section,
            content: "foo-objective".into(),
            title: Some("foo-objective".into()),
            level: 1,
            source_type: SourceType::Okr,
            source_id: "foo-objective".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "foo-objective");
    }

    #[test]
    fn embedding_level_2_returns_content() {
        let node = TreeNode {
            id: "okr-progress-foo-2024-01-15".into(),
            parent_id: Some("okr-obj-foo".into()),
            node_type: TreeNodeType::Text,
            content: "Progress: 75% toward 1 (current: 0.75)".into(),
            title: None,
            level: 2,
            source_type: SourceType::Okr,
            source_id: "foo-objective".into(),
            position: 2,
            metadata: None,
        };
        assert_eq!(
            compose_embedding_text(&node),
            "Progress: 75% toward 1 (current: 0.75)"
        );
    }
}
