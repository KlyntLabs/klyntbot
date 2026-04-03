//! NoteTreeBuilder — event subscriber that parses note content into tree nodes,
//! persists them to SQLite, embeds them in LanceDB, and pushes context updates.
//!
//! Listens for `DomainEvent::NoteContentChanged` on the domain event bus.
//! Tries `parse_tiptap_to_tree` on the content first (valid Tiptap JSON),
//! falls back to `parse_markdown_to_tree` if that returns no nodes.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Debounce window: ignore events for the same note within this duration.
/// Tree rebuild + LanceDB embed is expensive — avoid redundant work during rapid edits.
const DEBOUNCE_SECS: u64 = 3;

use bus::{
    ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, DomainEventBus,
    UpdatePriority,
};
use cognitive::TextEmbedder;
use common::truncate_at_boundary;
use context_engine::book_index::types::SourceType;
use context_engine::book_index::BookTreeRepo;

/// Subscribes to `NoteContentChanged` events and rebuilds the tree index for
/// the affected note: parse -> SQLite -> LanceDB embeddings -> context update.
pub struct NoteTreeBuilder {
    tree_repo: Arc<dyn BookTreeRepo>,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    note_repo: feature_notes::repo::NoteRepo,
}

impl NoteTreeBuilder {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn TextEmbedder>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
        domain_event_bus: Option<Arc<DomainEventBus>>,
        note_repo: feature_notes::repo::NoteRepo,
    ) -> Self {
        Self {
            tree_repo,
            vector_store,
            embedder,
            context_update_queue,
            domain_event_bus,
            note_repo,
        }
    }

    /// Event-driven subscriber loop. Listens for `NoteContentChanged` events
    /// and rebuilds the tree for each note.
    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("NoteTreeBuilder: subscriber started");
        let mut debounce_map: HashMap<String, tokio::time::Instant> = HashMap::new();

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("NoteTreeBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::NoteContentChanged { note_id }) => {
                            let now = tokio::time::Instant::now();
                            if let Some(last) = debounce_map.get(&note_id) {
                                if now.duration_since(*last).as_secs() < DEBOUNCE_SECS {
                                    debug!(note_id = %note_id, "NoteTreeBuilder: debounced");
                                    continue;
                                }
                            }
                            debounce_map.insert(note_id.clone(), now);
                            if debounce_map.len() > 500 {
                                let cutoff = now - tokio::time::Duration::from_secs(60);
                                debounce_map.retain(|_, ts| *ts > cutoff);
                            }

                            match self.note_repo.get_note(&note_id).await {
                                Ok(Some(note)) => {
                                    if let Err(e) = self.handle_note_changed(&note_id, &note.body).await {
                                        warn!(note_id = %note_id, "NoteTreeBuilder: failed to process note change: {e}");
                                    }
                                }
                                Ok(None) => {
                                    debug!(note_id = %note_id, "NoteTreeBuilder: note not found, skipping");
                                }
                                Err(e) => {
                                    warn!(note_id = %note_id, "NoteTreeBuilder: failed to fetch note: {e}");
                                }
                            }
                        }
                        Ok(_) => {
                            // Not a NoteContentChanged event — ignore.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("NoteTreeBuilder: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("NoteTreeBuilder: event channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Process a note content change: parse, persist tree nodes, embed, and push context update.
    ///
    /// Public so it can also be called from the migration/backfill job.
    pub async fn handle_note_changed(&self, note_id: &str, content: &str) -> common::Result<()> {
        debug!(note_id = %note_id, "NoteTreeBuilder: processing note change");

        // 1. Parse content into tree nodes.
        //    Try Tiptap JSON first (valid JSON with `type: "doc"` structure),
        //    fall back to markdown.
        let nodes = {
            let tiptap_nodes =
                cognitive::services::tiptap_parser::parse_tiptap_to_tree(note_id, content);
            if tiptap_nodes.is_empty() {
                cognitive::repos::markdown_parser::parse_markdown_to_tree(note_id, content)
            } else {
                tiptap_nodes
            }
        };

        if nodes.is_empty() {
            debug!(note_id = %note_id, "NoteTreeBuilder: no tree nodes parsed, skipping");
            return Ok(());
        }

        // 2. Delete existing tree nodes for this note (full rebuild).
        if let Err(e) = self
            .tree_repo
            .delete_by_source(&SourceType::Note, note_id)
            .await
        {
            warn!(note_id = %note_id, "NoteTreeBuilder: failed to delete old tree nodes: {e}");
        }

        // Delete old vector embeddings for this note.
        if let Err(e) = self
            .vector_store
            .delete_tree_node_embeddings_by_note(note_id)
            .await
        {
            warn!(note_id = %note_id, "NoteTreeBuilder: failed to delete old tree embeddings: {e}");
        }

        // 3. Insert new tree nodes into SQLite.
        self.tree_repo.insert_nodes(&nodes).await?;

        debug!(
            note_id = %note_id,
            node_count = nodes.len(),
            "NoteTreeBuilder: inserted tree nodes"
        );

        // 4. Embed each node and upsert to LanceDB.
        let mut embedded_count = 0_usize;
        for node in &nodes {
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
                            note_id,
                            &level_str,
                            node.source_type.as_str(),
                        )
                        .await
                    {
                        warn!(
                            node_id = %node.id,
                            "NoteTreeBuilder: failed to upsert tree node embedding: {e}"
                        );
                    } else {
                        embedded_count += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        node_id = %node.id,
                        "NoteTreeBuilder: failed to generate embedding: {e}"
                    );
                }
            }
        }

        debug!(
            note_id = %note_id,
            embedded_count,
            "NoteTreeBuilder: embedded tree nodes"
        );

        // 5. Push context update for live injection.
        if let Some(queue) = &self.context_update_queue {
            let section_titles: Vec<&str> =
                nodes.iter().filter_map(|n| n.title.as_deref()).collect();

            let payload = format!(
                "Note structure updated: {} nodes ({} sections: {})",
                nodes.len(),
                section_titles.len(),
                section_titles.join(" > "),
            );

            queue.push(ContextUpdate {
                reason: ContextUpdateReason::NoteStructureChanged,
                content: Some(payload),
                metadata: Some(serde_json::json!({
                    "note_id": note_id,
                    "node_count": nodes.len(),
                    "embedded_count": embedded_count,
                })),
                priority: UpdatePriority::Low,
                timestamp: chrono::Utc::now(),
            });
        }

        // 6. Emit TreeNodesRebuilt so EntityTreeLinker picks it up
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(DomainEvent::TreeNodesRebuilt {
                source_type: "note".to_string(),
                source_id: note_id.to_string(),
            });
        }

        info!(
            note_id = %note_id,
            node_count = nodes.len(),
            embedded_count,
            "NoteTreeBuilder: note tree rebuilt"
        );

        Ok(())
    }
}

/// Compose embedding text for a tree node based on its level:
/// - Level 0: title only (root/document node)
/// - Level 1-6: title + content preview (300 chars)
/// - Level 7+: content only (300 chars) — list items, deep nodes
fn compose_embedding_text(node: &context_engine::book_index::types::TreeNode) -> String {
    let title = node.title.as_deref().unwrap_or("");
    let content_preview = truncate_at_boundary(&node.content, 300);

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

#[cfg(test)]
mod tests {
    use super::*;
    use context_engine::book_index::types::{TreeNode, TreeNodeType};

    #[test]
    fn compose_embedding_level_0_title_only() {
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
        assert_eq!(compose_embedding_text(&node), "My Document");
    }

    #[test]
    fn compose_embedding_level_1_title_and_content() {
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
        assert_eq!(compose_embedding_text(&node), "Introduction");
    }

    #[test]
    fn compose_embedding_level_2_with_different_content() {
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
            compose_embedding_text(&node),
            "Subsection A This is a paragraph with some text."
        );
    }

    #[test]
    fn compose_embedding_level_7_content_only() {
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
        assert_eq!(compose_embedding_text(&node), "Item 1\nItem 2\nItem 3");
    }

    #[test]
    fn truncate_respects_char_boundary() {
        let s = "hello world";
        assert_eq!(truncate_at_boundary(s, 5), "hello");

        let unicode = "cafe\u{0301}"; // "cafe" + combining accent = "cafe\u{0301}"
                                      // The combining accent is 2 bytes at position 4..6
        let result = truncate_at_boundary(unicode, 5);
        assert!(result.len() <= 5);
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn truncate_short_string_unchanged() {
        let s = "short";
        assert_eq!(truncate_at_boundary(s, 300), "short");
    }
}
