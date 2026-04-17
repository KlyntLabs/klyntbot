//! LearningTreeBuilder — event subscriber that creates tree nodes from learning
//! events (KnowledgeAtomAccepted, RetentionMilestoneReached) so knowledge atoms
//! participate in the entity graph, community detection, and Knowledge Fabric
//! Explorer.
//!
//! Uses **incremental appends**: each atom/milestone event creates leaf nodes
//! under a stable per-type root. Root and type-section nodes are idempotent —
//! duplicate-ID inserts are silently ignored because the node already exists.
//!
//! Tree structure:
//! ```text
//! learning-root  (root, level 0, Section, title: "Learning & Knowledge")
//!   └── learning-type-{atom_type}  (level 1, Section, title: e.g. "concept")
//!       ├── learning-atom-{atom_id}  (level 2, Text)
//!       │     — "Accepted: {atom_type} atom {atom_id}"
//!       └── learning-milestone-{atom_id}-{milestone}  (level 2, Text)
//!             — "Milestone: {milestone} at {new_retention_pct:.0}% retention"
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

pub struct LearningTreeBuilder {
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
}

impl LearningTreeBuilder {
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

    /// Backfill tree nodes from existing knowledge atoms.
    ///
    /// Queries all active atoms from the DB and calls `handle_atom_accepted` for
    /// each one. Insert errors for already-indexed atoms are silently ignored by
    /// the underlying `persist_nodes` logic, so this is safe to run on startup.
    pub async fn backfill_from_atoms(&self, pool: &sqlx::SqlitePool) -> common::Result<usize> {
        let atoms: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, atom_type, subject FROM knowledge_atoms WHERE status = 'active'",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| common::ToolError::ExecutionFailed(format!("learning backfill query: {e}")))?;

        let mut count = 0;
        for (atom_id, atom_type, subject) in &atoms {
            if self
                .handle_atom_accepted(atom_id, atom_type, Some(subject.as_str()))
                .await
                .is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Event-driven subscriber loop. Listens for `KnowledgeAtomAccepted` and
    /// `RetentionMilestoneReached` events and appends tree nodes for each.
    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("LearningTreeBuilder: subscriber started");

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("LearningTreeBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::KnowledgeAtomAccepted { atom_id, atom_type }) => {
                            if let Err(e) = self.handle_atom_accepted(&atom_id, &atom_type, None).await {
                                warn!(
                                    atom_id = %atom_id,
                                    "LearningTreeBuilder: failed to process atom accepted: {e}"
                                );
                            }
                        }
                        Ok(DomainEvent::RetentionMilestoneReached {
                            atom_id,
                            topic_id: _,
                            new_retention_pct,
                            milestone,
                            previous_pct: _,
                        }) => {
                            if let Err(e) = self
                                .handle_retention_milestone(&atom_id, &milestone, new_retention_pct)
                                .await
                            {
                                warn!(
                                    atom_id = %atom_id,
                                    "LearningTreeBuilder: failed to process retention milestone: {e}"
                                );
                            }
                        }
                        Ok(_) => {
                            // Not a learning event — ignore.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("LearningTreeBuilder: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("LearningTreeBuilder: event channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Handle a knowledge atom accepted event: upsert the global learning root
    /// and per-type section node, then insert a unique atom leaf node.
    pub async fn handle_atom_accepted(
        &self,
        atom_id: &str,
        atom_type: &str,
        subject: Option<&str>,
    ) -> common::Result<()> {
        let type_node_id = format!("learning-type-{}", slugify(atom_type));
        let atom_node_id = format!("learning-atom-{atom_id}");

        debug!(
            atom_id = %atom_id,
            atom_type = %atom_type,
            atom_node_id = %atom_node_id,
            "LearningTreeBuilder: processing atom accepted"
        );

        let nodes =
            build_atom_accepted_nodes(atom_id, atom_type, subject, &type_node_id, &atom_node_id);

        self.persist_nodes(&nodes, atom_id).await
    }

    /// Handle a retention milestone event: upsert the global learning root and
    /// the "milestone" type section, then insert a milestone leaf node.
    ///
    /// The milestone node is keyed by (atom_id, milestone) so repeated reaches
    /// of the same milestone are idempotent.
    pub async fn handle_retention_milestone(
        &self,
        atom_id: &str,
        milestone: &str,
        new_retention_pct: f64,
    ) -> common::Result<()> {
        let type_node_id = "learning-type-milestone".to_string();
        let milestone_node_id = format!("learning-milestone-{atom_id}-{}", slugify(milestone));

        debug!(
            atom_id = %atom_id,
            milestone = %milestone,
            new_retention_pct,
            milestone_node_id = %milestone_node_id,
            "LearningTreeBuilder: processing retention milestone"
        );

        let nodes = build_milestone_nodes(
            atom_id,
            milestone,
            new_retention_pct,
            &type_node_id,
            &milestone_node_id,
        );

        self.persist_nodes(&nodes, atom_id).await
    }

    /// Persist nodes: insert each one (ignoring duplicate-ID errors for
    /// idempotent root/type nodes), embed leaf nodes, push context update,
    /// and emit TreeNodesRebuilt.
    async fn persist_nodes(&self, nodes: &[TreeNode], source_id: &str) -> common::Result<()> {
        let mut inserted = Vec::new();

        for node in nodes {
            match self.tree_repo.insert_node(node).await {
                Ok(()) => inserted.push(node),
                Err(_) => {
                    // Duplicate key — node already exists. Expected for global
                    // learning root and per-type section nodes.
                    debug!(
                        node_id = %node.id,
                        "LearningTreeBuilder: node already exists, skipping insert"
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
                            "LearningTreeBuilder: failed to upsert embedding: {e}"
                        );
                    } else {
                        embedded_count += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        node_id = %node.id,
                        "LearningTreeBuilder: failed to generate embedding: {e}"
                    );
                }
            }
        }

        // Emit TreeNodesRebuilt so EntityTreeLinker picks up the new nodes.
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(DomainEvent::TreeNodesRebuilt {
                source_type: "learning".to_string(),
                source_id: source_id.to_string(),
            });
        }

        // Push context update for live injection.
        if let Some(queue) = &self.context_update_queue {
            queue.push(ContextUpdate {
                reason: ContextUpdateReason::Custom("learning_tree_updated".to_string()),
                content: Some(format!(
                    "Learning tree updated: {} new node(s) for atom {source_id}",
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
            "LearningTreeBuilder: nodes persisted"
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pure node construction helpers (public for unit testing)
// ---------------------------------------------------------------------------

/// Build nodes for a `KnowledgeAtomAccepted` event.
///
/// Returns up to 3 nodes:
/// - Level 0: global learning root (idempotent, shared across all atom types)
/// - Level 1: per-type section (idempotent by atom_type slug)
/// - Level 2: atom leaf (idempotent by atom_id — re-acceptance of the same
///   atom is silently deduplicated)
pub fn build_atom_accepted_nodes(
    atom_id: &str,
    atom_type: &str,
    subject: Option<&str>,
    type_node_id: &str,
    atom_node_id: &str,
) -> Vec<TreeNode> {
    let atom_content = match subject {
        Some(s) if !s.is_empty() => format!("{atom_type}: {s}"),
        _ => format!("Accepted: {atom_type} atom {atom_id}"),
    };

    vec![
        // Level 0 — global learning root
        TreeNode {
            id: "learning-root".to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "learning-root".to_string(),
            title: Some("Learning & Knowledge".to_string()),
            level: 0,
            source_type: SourceType::Learning,
            source_id: "learning-root".to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — per-type section
        TreeNode {
            id: type_node_id.to_string(),
            parent_id: Some("learning-root".to_string()),
            node_type: TreeNodeType::Section,
            content: atom_type.to_string(),
            title: Some(atom_type.to_string()),
            level: 1,
            source_type: SourceType::Learning,
            source_id: "learning-root".to_string(),
            position: 1,
            metadata: None,
        },
        // Level 2 — atom leaf
        TreeNode {
            id: atom_node_id.to_string(),
            parent_id: Some(type_node_id.to_string()),
            node_type: TreeNodeType::Text,
            content: atom_content,
            title: None,
            level: 2,
            source_type: SourceType::Learning,
            source_id: atom_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

/// Build nodes for a `RetentionMilestoneReached` event.
///
/// Returns up to 3 nodes:
/// - Level 0: global learning root (idempotent)
/// - Level 1: "milestone" type section (idempotent)
/// - Level 2: milestone leaf (idempotent per atom_id + milestone slug)
pub fn build_milestone_nodes(
    atom_id: &str,
    milestone: &str,
    new_retention_pct: f64,
    type_node_id: &str,
    milestone_node_id: &str,
) -> Vec<TreeNode> {
    let milestone_content =
        format!("Milestone: {milestone} at {new_retention_pct:.0}% retention for atom {atom_id}");

    vec![
        // Level 0 — global learning root
        TreeNode {
            id: "learning-root".to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "learning-root".to_string(),
            title: Some("Learning & Knowledge".to_string()),
            level: 0,
            source_type: SourceType::Learning,
            source_id: "learning-root".to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — milestone type section
        TreeNode {
            id: type_node_id.to_string(),
            parent_id: Some("learning-root".to_string()),
            node_type: TreeNodeType::Section,
            content: "milestone".to_string(),
            title: Some("milestone".to_string()),
            level: 1,
            source_type: SourceType::Learning,
            source_id: "learning-root".to_string(),
            position: 1,
            metadata: None,
        },
        // Level 2 — milestone leaf
        TreeNode {
            id: milestone_node_id.to_string(),
            parent_id: Some(type_node_id.to_string()),
            node_type: TreeNodeType::Text,
            content: milestone_content,
            title: None,
            level: 2,
            source_type: SourceType::Learning,
            source_id: atom_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Slugify a string for use in deterministic node IDs.
/// Lowercases and replaces non-alphanumeric chars with hyphens, collapsing runs.
fn slugify(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Compose embedding text for a learning tree node.
/// Level 0: title only; Level 1: type title; Level 2+: content.
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

    // --- slugify ---

    #[test]
    fn slugify_simple() {
        assert_eq!(slugify("concept"), "concept");
    }

    #[test]
    fn slugify_with_spaces() {
        assert_eq!(slugify("spaced repetition"), "spaced-repetition");
    }

    #[test]
    fn slugify_uppercase_and_specials() {
        assert_eq!(slugify("Key Idea!"), "key-idea");
    }

    #[test]
    fn slugify_collapses_separators() {
        assert_eq!(slugify("a  --  b"), "a-b");
    }

    // --- build_atom_accepted_nodes ---

    #[test]
    fn atom_nodes_structure() {
        let nodes = build_atom_accepted_nodes(
            "atom-abc-123",
            "concept",
            Some("Gut-brain axis mechanisms"),
            "learning-type-concept",
            "learning-atom-atom-abc-123",
        );

        assert_eq!(nodes.len(), 3);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "learning-root");
        assert_eq!(root.parent_id, None);
        assert_eq!(root.level, 0);
        assert!(matches!(root.node_type, TreeNodeType::Section));
        assert_eq!(root.title.as_deref(), Some("Learning & Knowledge"));
        assert_eq!(root.source_type.as_str(), "learning");

        // Type section
        let type_node = &nodes[1];
        assert_eq!(type_node.id, "learning-type-concept");
        assert_eq!(type_node.parent_id.as_deref(), Some("learning-root"));
        assert_eq!(type_node.level, 1);
        assert!(matches!(type_node.node_type, TreeNodeType::Section));
        assert_eq!(type_node.title.as_deref(), Some("concept"));
        assert_eq!(type_node.source_type.as_str(), "learning");

        // Atom leaf
        let leaf = &nodes[2];
        assert_eq!(leaf.id, "learning-atom-atom-abc-123");
        assert_eq!(leaf.parent_id.as_deref(), Some("learning-type-concept"));
        assert_eq!(leaf.level, 2);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        assert_eq!(leaf.title, None);
        assert_eq!(leaf.source_type.as_str(), "learning");
    }

    #[test]
    fn atom_content_includes_subject() {
        let nodes = build_atom_accepted_nodes(
            "atom-xyz",
            "fact",
            Some("Caffeine reduces deep sleep"),
            "learning-type-fact",
            "learning-atom-atom-xyz",
        );
        let leaf = &nodes[2];
        assert!(
            leaf.content.contains("fact"),
            "Expected atom_type in content: {}",
            leaf.content
        );
        assert!(
            leaf.content.contains("Caffeine reduces deep sleep"),
            "Expected subject in content: {}",
            leaf.content
        );
    }

    #[test]
    fn all_atom_nodes_have_learning_source_type() {
        let nodes = build_atom_accepted_nodes(
            "atom-1",
            "principle",
            None,
            "learning-type-principle",
            "learning-atom-atom-1",
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "learning");
        }
    }

    // --- build_milestone_nodes ---

    #[test]
    fn milestone_nodes_structure() {
        let nodes = build_milestone_nodes(
            "atom-abc",
            "90%",
            90.0,
            "learning-type-milestone",
            "learning-milestone-atom-abc-90",
        );

        assert_eq!(nodes.len(), 3);

        let root = &nodes[0];
        assert_eq!(root.id, "learning-root");
        assert_eq!(root.level, 0);

        let type_node = &nodes[1];
        assert_eq!(type_node.id, "learning-type-milestone");
        assert_eq!(type_node.parent_id.as_deref(), Some("learning-root"));
        assert_eq!(type_node.level, 1);
        assert_eq!(type_node.title.as_deref(), Some("milestone"));

        let leaf = &nodes[2];
        assert_eq!(leaf.id, "learning-milestone-atom-abc-90");
        assert_eq!(leaf.parent_id.as_deref(), Some("learning-type-milestone"));
        assert_eq!(leaf.level, 2);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        assert_eq!(leaf.source_id, "atom-abc");
    }

    #[test]
    fn milestone_content_includes_milestone_and_retention() {
        let nodes = build_milestone_nodes(
            "atom-def",
            "learned",
            85.5,
            "learning-type-milestone",
            "learning-milestone-atom-def-learned",
        );
        let leaf = &nodes[2];
        assert!(
            leaf.content.contains("learned"),
            "Expected milestone name in content: {}",
            leaf.content
        );
        assert!(
            leaf.content.contains("86%")
                || leaf.content.contains("85%")
                || leaf.content.contains("85."),
            "Expected retention pct in content: {}",
            leaf.content
        );
    }

    #[test]
    fn all_milestone_nodes_have_learning_source_type() {
        let nodes = build_milestone_nodes(
            "atom-2",
            "mastered",
            95.0,
            "learning-type-milestone",
            "learning-milestone-atom-2-mastered",
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "learning");
        }
    }

    // --- compose_embedding_text ---

    #[test]
    fn embedding_level_0_returns_title() {
        let node = TreeNode {
            id: "learning-root".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "learning-root".into(),
            title: Some("Learning & Knowledge".into()),
            level: 0,
            source_type: SourceType::Learning,
            source_id: "learning-root".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Learning & Knowledge");
    }

    #[test]
    fn embedding_level_1_returns_type_title() {
        let node = TreeNode {
            id: "learning-type-concept".into(),
            parent_id: Some("learning-root".into()),
            node_type: TreeNodeType::Section,
            content: "concept".into(),
            title: Some("concept".into()),
            level: 1,
            source_type: SourceType::Learning,
            source_id: "learning-root".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "concept");
    }

    #[test]
    fn embedding_level_2_returns_content() {
        let node = TreeNode {
            id: "learning-atom-abc".into(),
            parent_id: Some("learning-type-concept".into()),
            node_type: TreeNodeType::Text,
            content: "Accepted: concept atom abc-123".into(),
            title: None,
            level: 2,
            source_type: SourceType::Learning,
            source_id: "abc-123".into(),
            position: 2,
            metadata: None,
        };
        assert_eq!(
            compose_embedding_text(&node),
            "Accepted: concept atom abc-123"
        );
    }
}
