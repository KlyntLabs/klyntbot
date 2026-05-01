//! Shared infrastructure for domain tree builders.
//!
//! Extracts the duplicated `persist_nodes()`, event-loop shell, `slugify()`,
//! and `compose_embedding_text()` helpers that were copy-pasted across
//! `finance_tree_builder`, `learning_tree_builder`, `note_tree_builder`,
//! `okr_tree_builder`, `productivity_tree_builder`, and `task_tree_builder`.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{
    ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, DomainEventBus,
    UpdatePriority,
};
use cognitive::TextEmbedder;
use context_engine::book_index::types::TreeNode;
use context_engine::book_index::BookTreeRepo;

/// Common dependencies shared by all domain tree builders.
pub struct TreeBuilderCore {
    pub tree_repo: Arc<dyn BookTreeRepo>,
    pub vector_store: Arc<storage::VectorStore>,
    pub embedder: Arc<dyn TextEmbedder>,
    pub context_update_queue: Option<Arc<ContextUpdateQueue>>,
    #[allow(dead_code)]
    pub domain_event_bus: Option<Arc<DomainEventBus>>,
}

impl TreeBuilderCore {
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

    /// Persist nodes: insert each one (ignoring duplicate-ID errors for
    /// idempotent root/category nodes), embed leaf nodes, push context update.
    pub async fn persist_nodes(
        &self,
        name: &str,
        nodes: &[TreeNode],
        source_id: &str,
        reason: ContextUpdateReason,
        content: Option<String>,
    ) -> common::Result<()> {
        let mut inserted = Vec::new();

        for node in nodes {
            match self.tree_repo.insert_node(node).await {
                Ok(()) => inserted.push(node),
                Err(_) => {
                    // Duplicate key — node already exists. Expected for
                    // root/category nodes shared across multiple events.
                    debug!(
                        node_id = %node.id,
                        "{name}: node already exists, skipping insert"
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
                            "{name}: failed to upsert embedding: {e}"
                        );
                    } else {
                        embedded_count += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        node_id = %node.id,
                        "{name}: failed to generate embedding: {e}"
                    );
                }
            }
        }

        // Push context update for live injection.
        if let Some(queue) = &self.context_update_queue {
            queue.push(ContextUpdate {
                reason,
                content,
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
            "{name}: nodes persisted"
        );

        Ok(())
    }
}

/// Generic event-loop shell used by most tree builders.
///
/// Domain-specific event matching is supplied by the `on_event` callback.
pub async fn run_event_loop<F, Fut>(
    name: &str,
    mut rx: broadcast::Receiver<DomainEvent>,
    shutdown: CancellationToken,
    mut on_event: F,
) where
    F: FnMut(DomainEvent) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    info!("{name}: subscriber started");

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("{name}: shutdown received");
                break;
            }
            result = rx.recv() => {
                match result {
                    Ok(event) => on_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("{name}: lagged, skipped {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("{name}: event channel closed");
                        break;
                    }
                }
            }
        }
    }
}

/// Slugify a string for use in deterministic node IDs.
/// Lowercases and replaces non-alphanumeric chars with hyphens, collapsing runs.
pub fn slugify(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Compose embedding text for a tree node (simple variant).
/// Level 0: title only; Level 1: title fallback to content; Level 2+: content.
pub fn compose_embedding_text(node: &TreeNode) -> String {
    match node.level {
        0 => node.title.as_deref().unwrap_or("").to_string(),
        1 => node.title.as_deref().unwrap_or(&node.content).to_string(),
        _ => node.content.clone(),
    }
}

/// Compose embedding text for a tree node (note/task variant with truncation).
/// - Level 0: title only
/// - Level 1-6: title + content preview (300 chars)
/// - Level 7+: content only (300 chars)
pub fn compose_embedding_text_with_truncate(
    node: &context_engine::book_index::types::TreeNode,
) -> String {
    let title = node.title.as_deref().unwrap_or("");
    let content_preview = common::truncate_at_boundary(&node.content, 300);

    match node.level {
        0 => title.to_string(),
        1..=6 => {
            if title.is_empty() {
                content_preview.to_string()
            } else if content_preview.is_empty() || content_preview == title {
                title.to_string()
            } else {
                format!("{title} {content_preview}")
            }
        }
        _ => content_preview.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_engine::book_index::types::{SourceType, TreeNodeType};

    // --- slugify ---

    #[test]
    fn slugify_simple_word() {
        assert_eq!(slugify("food"), "food");
    }

    #[test]
    fn slugify_with_spaces() {
        assert_eq!(slugify("eating out"), "eating-out");
    }

    #[test]
    fn slugify_uppercase_and_specials() {
        assert_eq!(slugify("Coffee & Tea"), "coffee-tea");
    }

    #[test]
    fn slugify_collapses_multiple_separators() {
        assert_eq!(slugify("a  --  b"), "a-b");
    }

    // --- compose_embedding_text ---

    #[test]
    fn embedding_level_0_returns_title() {
        let node = TreeNode {
            id: "root".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "root".into(),
            title: Some("Root Title".into()),
            level: 0,
            source_type: SourceType::Finance,
            source_id: "root".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Root Title");
    }

    #[test]
    fn embedding_level_1_returns_title() {
        let node = TreeNode {
            id: "cat".into(),
            parent_id: Some("root".into()),
            node_type: TreeNodeType::Section,
            content: "category".into(),
            title: Some("Category Title".into()),
            level: 1,
            source_type: SourceType::Finance,
            source_id: "root".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Category Title");
    }

    #[test]
    fn embedding_level_1_fallback_to_content() {
        let node = TreeNode {
            id: "cat".into(),
            parent_id: Some("root".into()),
            node_type: TreeNodeType::Section,
            content: "category".into(),
            title: None,
            level: 1,
            source_type: SourceType::Finance,
            source_id: "root".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "category");
    }

    #[test]
    fn embedding_level_2_returns_content() {
        let node = TreeNode {
            id: "leaf".into(),
            parent_id: Some("cat".into()),
            node_type: TreeNodeType::Text,
            content: "leaf content".into(),
            title: None,
            level: 2,
            source_type: SourceType::Finance,
            source_id: "root".into(),
            position: 2,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "leaf content");
    }

    // --- compose_embedding_text_with_truncate ---

    #[test]
    fn truncate_embedding_level_0_title_only() {
        let node = TreeNode {
            id: "n1".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "note-123".into(),
            title: Some("My Document".into()),
            level: 0,
            source_type: SourceType::Note,
            source_id: "note-123".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text_with_truncate(&node), "My Document");
    }

    #[test]
    fn truncate_embedding_level_1_title_and_content() {
        let node = TreeNode {
            id: "n2".into(),
            parent_id: Some("n1".into()),
            node_type: TreeNodeType::Section,
            content: "Introduction".into(),
            title: Some("Introduction".into()),
            level: 1,
            source_type: SourceType::Note,
            source_id: "note-123".into(),
            position: 1,
            metadata: None,
        };
        // title == content, so just title
        assert_eq!(compose_embedding_text_with_truncate(&node), "Introduction");
    }

    #[test]
    fn truncate_embedding_level_2_with_different_content() {
        let node = TreeNode {
            id: "n3".into(),
            parent_id: Some("n2".into()),
            node_type: TreeNodeType::Text,
            content: "This is a paragraph with some text.".into(),
            title: Some("Subsection A".into()),
            level: 2,
            source_type: SourceType::Note,
            source_id: "note-123".into(),
            position: 2,
            metadata: None,
        };
        assert_eq!(
            compose_embedding_text_with_truncate(&node),
            "Subsection A This is a paragraph with some text."
        );
    }

    #[test]
    fn truncate_embedding_level_7_content_only() {
        let node = TreeNode {
            id: "n4".into(),
            parent_id: Some("n3".into()),
            node_type: TreeNodeType::ListItem,
            content: "Item 1\nItem 2\nItem 3".into(),
            title: None,
            level: 7,
            source_type: SourceType::Note,
            source_id: "note-123".into(),
            position: 3,
            metadata: None,
        };
        assert_eq!(
            compose_embedding_text_with_truncate(&node),
            "Item 1\nItem 2\nItem 3"
        );
    }
}
