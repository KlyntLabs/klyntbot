use std::sync::Arc;

use cognitive::repos::{EntityRepo, NewEntity};
use context_engine::book_index::types::{TreeNode, TreeNodeType};
use context_engine::book_index::GTLinkRepo;
use context_engine::operators::OperatorLlm;
use tracing::{debug, warn};

/// Extracts entities from tree nodes and creates GT-Links.
/// Runs in background after tree insertion — does NOT block note save.
pub struct BookIndexEntityExtractor {
    entity_repo: EntityRepo,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    llm: Arc<dyn OperatorLlm>,
}

impl BookIndexEntityExtractor {
    pub fn new(
        entity_repo: EntityRepo,
        gt_link_repo: Arc<dyn GTLinkRepo>,
        llm: Arc<dyn OperatorLlm>,
    ) -> Self {
        Self {
            entity_repo,
            gt_link_repo,
            llm,
        }
    }

    /// Extract entities from all leaf nodes and create GT-Links.
    /// Call this after tree nodes have been inserted.
    pub async fn extract_and_link(&self, nodes: &[TreeNode]) -> common::Result<usize> {
        let leaf_nodes: Vec<&TreeNode> = nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.node_type,
                    TreeNodeType::Text | TreeNodeType::Code | TreeNodeType::Task
                )
            })
            .filter(|n| n.content.len() > 20)
            .collect();

        if leaf_nodes.is_empty() {
            return Ok(0);
        }

        let mut total_links = 0;

        for node in &leaf_nodes {
            match self.extract_entities_from_node(node).await {
                Ok(count) => total_links += count,
                Err(e) => {
                    warn!("Entity extraction failed for node {}: {e}", node.id);
                }
            }
        }

        Ok(total_links)
    }

    async fn extract_entities_from_node(&self, node: &TreeNode) -> common::Result<usize> {
        let prompt = format!(
            "Extract named entities from this text. Return one entity per line in format: NAME|TYPE\n\
             Types: Person, Project, Tool, Concept, Organization, Location\n\
             Only extract proper nouns and specific named things. Skip generic words.\n\n\
             Text:\n{}",
            &node.content[..node.content.len().min(500)]
        );

        let response = self
            .llm
            .complete(
                "You extract named entities. Return NAME|TYPE per line, nothing else.",
                &prompt,
            )
            .await?;

        let mut links = Vec::new();

        for line in response.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() != 2 {
                continue;
            }
            let name = parts[0].trim();
            let entity_type = parts[1].trim();
            if name.is_empty() || entity_type.is_empty() || name.len() < 2 {
                continue;
            }

            match self
                .entity_repo
                .upsert_entity(&NewEntity {
                    name: name.to_string(),
                    entity_type: entity_type.to_string(),
                    description: None,
                    source: "bookindex".to_string(),
                    source_id: Some(node.source_id.clone()),
                    metadata: None,
                })
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
            {
                Ok(entity) => {
                    links.push((entity.id, node.id.clone()));
                }
                Err(e) => {
                    debug!("Entity upsert failed for '{name}': {e}");
                }
            }
        }

        if !links.is_empty() {
            self.gt_link_repo.link_batch(&links).await?;
        }

        Ok(links.len())
    }
}

/// Fire-and-forget entity extraction in a background task.
/// Used by both the updater (per-event) and backfill (per-note).
pub fn spawn_extract_and_link(extractor: &Arc<BookIndexEntityExtractor>, nodes: Vec<TreeNode>) {
    let extractor = extractor.clone();
    tokio::spawn(async move {
        match extractor.extract_and_link(&nodes).await {
            Ok(n) if n > 0 => debug!("BookIndex: linked {n} entities"),
            Ok(_) => {}
            Err(e) => warn!("BookIndex entity extraction failed: {e}"),
        }
    });
}
