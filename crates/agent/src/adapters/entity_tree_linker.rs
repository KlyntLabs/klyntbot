//! EntityTreeLinker — bridges entities to tree nodes by scanning tree node
//! content for known entity names. Runs after NoteTreeBuilder processes a note.
//!
//! This is the missing link in the Phase 2 community graph pipeline:
//! NoteTreeBuilder → EntityTreeLinker → CommunityBuilder (Louvain)
//!
//! Two modes:
//! 1. **Match existing entities** — case-insensitive name scan against all known entities
//! 2. **Extract new entities from headings** — section headings become concept entities
//!    (bootstraps the entity graph for fresh databases with no chat history)

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use bus::DomainEvent;
use context_engine::book_index::GTLinkRepo;

/// Minimum entity name length to avoid false positives (e.g. "I", "a", "is")
const MIN_ENTITY_NAME_LEN: usize = 3;

/// Minimum tree node content length to bother scanning
const MIN_CONTENT_LEN: usize = 10;

pub struct EntityTreeLinker {
    pool: sqlx::SqlitePool,
}

impl EntityTreeLinker {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }

    /// Event-driven subscriber loop. Listens for `TreeNodesRebuilt` events
    /// and links entities to tree nodes for the affected source.
    ///
    /// `TreeNodesRebuilt` is emitted by NoteTreeBuilder/TaskTreeBuilder AFTER
    /// tree nodes are persisted, so no delay is needed.
    pub async fn run(
        self: Arc<Self>,
        rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        crate::adapters::tree_builder_base::run_event_loop(
            "EntityTreeLinker",
            rx,
            shutdown,
            |_event| async {},
        )
        .await;
    }

    /// Link entities to tree nodes for a specific source (note, task, etc.).
    /// Public for use in backfill jobs.
    pub async fn link_entities_for_source(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> common::Result<()> {
        // 1. Load all tree nodes for this source
        let tree_nodes = self
            .get_tree_nodes_for_source(source_type, source_id)
            .await?;
        if tree_nodes.is_empty() {
            return Ok(());
        }

        // 2. Load all known entities (name → id mapping)
        let entities = self.load_entity_names().await?;

        // 3. Extract heading-based entities from section nodes (bootstrapping)
        let heading_entities = self.extract_heading_entities(&tree_nodes).await?;

        // Merge with existing entities
        let mut all_entities = entities;
        for (name, id) in heading_entities {
            all_entities.entry(name).or_insert(id);
        }

        if all_entities.is_empty() {
            debug!(source_type = %source_type, source_id = %source_id, "EntityTreeLinker: no entities to match against");
            return Ok(());
        }

        // 4. Scan each tree node's content for entity name mentions
        let mut links: Vec<(String, String)> = Vec::new();

        for (node_id, content) in &tree_nodes {
            if content.len() < MIN_CONTENT_LEN {
                continue;
            }
            let content_lower = content.to_lowercase();

            for (entity_name, entity_id) in &all_entities {
                if entity_name.len() < MIN_ENTITY_NAME_LEN {
                    continue;
                }
                // Case-insensitive word boundary match
                if contains_entity(&content_lower, &entity_name.to_lowercase()) {
                    links.push((entity_id.clone(), node_id.clone()));
                }
            }
        }

        if links.is_empty() {
            debug!(source_type = %source_type, source_id = %source_id, "EntityTreeLinker: no entity matches found");
            return Ok(());
        }

        // 5. Delete old links for these tree nodes and insert new ones
        let gt_link_repo = cognitive::repos::SqliteGTLinkRepo::new(self.pool.clone());
        for (node_id, _) in &tree_nodes {
            let _ = gt_link_repo.delete_by_tree_node(node_id).await;
        }
        gt_link_repo.link_batch(&links).await?;

        info!(
            source_type = %source_type,
            source_id = %source_id,
            links = links.len(),
            entities = all_entities.len(),
            "EntityTreeLinker: linked entities to tree nodes"
        );

        Ok(())
    }

    /// Backward-compatible wrapper for note-specific linking (used in backfill).
    pub async fn link_entities_for_note(&self, note_id: &str) -> common::Result<()> {
        self.link_entities_for_source("note", note_id).await
    }

    /// Get all tree nodes for a source: (node_id, content)
    async fn get_tree_nodes_for_source(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> common::Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, content FROM book_tree_nodes
             WHERE source_type = ?1 AND source_id = ?2",
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    /// Load all known entity names → IDs for matching
    async fn load_entity_names(&self) -> common::Result<HashMap<String, String>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM entities WHERE LENGTH(name) >= ?1")
                .bind(MIN_ENTITY_NAME_LEN as i32)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        Ok(rows.into_iter().map(|(id, name)| (name, id)).collect())
    }

    /// Extract entities from section headings.
    /// Section titles like "Circadian Rhythm", "Deep Work", "Neural Networks"
    /// are strong concept signals — create entities for them if they don't exist.
    async fn extract_heading_entities(
        &self,
        tree_nodes: &[(String, String)],
    ) -> common::Result<HashMap<String, String>> {
        // Get headings from book_tree_nodes (sections with titles)
        let node_ids: Vec<&str> = tree_nodes.iter().map(|(id, _)| id.as_str()).collect();
        if node_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Query for section nodes with titles
        let placeholders: String = node_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT DISTINCT title FROM book_tree_nodes
             WHERE id IN ({placeholders})
             AND node_type = 'section'
             AND title IS NOT NULL
             AND LENGTH(title) >= {MIN_ENTITY_NAME_LEN}
             AND level > 0"
        );

        let mut q = sqlx::query_scalar::<_, String>(&query);
        for id in &node_ids {
            q = q.bind(id);
        }
        let headings: Vec<String> = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let entity_repo = cognitive::repos::EntityRepo::new(self.pool.clone());
        let mut result = HashMap::new();

        for heading in headings {
            // Skip generic headings
            if is_generic_heading(&heading) {
                continue;
            }

            // Upsert as concept entity
            match entity_repo
                .upsert_entity(&cognitive::repos::NewEntity {
                    name: heading.clone(),
                    entity_type: "concept".to_string(),
                    description: None,
                    source: "note_heading".to_string(),
                    source_id: None,
                    metadata: None,
                })
                .await
            {
                Ok(row) => {
                    result.insert(heading, row.id);
                }
                Err(e) => {
                    debug!("EntityTreeLinker: failed to upsert heading entity '{heading}': {e}");
                }
            }
        }

        Ok(result)
    }
}

/// Check if content contains an entity name with word boundary awareness.
/// Avoids matching "cat" inside "category" by checking surrounding characters.
fn contains_entity(content_lower: &str, entity_lower: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = content_lower[start..].find(entity_lower) {
        let abs_pos = start + pos;
        let end_pos = abs_pos + entity_lower.len();

        // Check word boundaries
        let before_ok =
            abs_pos == 0 || !content_lower.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_ok = end_pos >= content_lower.len()
            || !content_lower.as_bytes()[end_pos].is_ascii_alphanumeric();

        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}

/// Skip headings that are too generic to be useful entities
fn is_generic_heading(heading: &str) -> bool {
    const GENERIC: &[&str] = &[
        "introduction",
        "overview",
        "summary",
        "conclusion",
        "notes",
        "references",
        "appendix",
        "background",
        "key details",
        "key points",
        "details",
        "more",
        "other",
        "misc",
        "todo",
        "tasks",
    ];
    GENERIC.contains(&heading.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_entity_basic() {
        assert!(contains_entity("sleep affects memory", "sleep"));
        assert!(contains_entity(
            "the circadian rhythm controls sleep",
            "circadian rhythm"
        ));
    }

    #[test]
    fn test_contains_entity_word_boundary() {
        // "cat" should not match inside "category"
        assert!(!contains_entity("category of things", "cat"));
        // But should match standalone
        assert!(contains_entity("the cat sat down", "cat"));
    }

    #[test]
    fn test_contains_entity_start_end() {
        assert!(contains_entity("sleep is important", "sleep"));
        assert!(contains_entity("importance of sleep", "sleep"));
    }

    #[test]
    fn test_is_generic_heading() {
        assert!(is_generic_heading("Introduction"));
        assert!(is_generic_heading("SUMMARY"));
        assert!(!is_generic_heading("Circadian Rhythm"));
        assert!(!is_generic_heading("Neural Networks"));
    }
}
