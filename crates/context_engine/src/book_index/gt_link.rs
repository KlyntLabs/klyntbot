use async_trait::async_trait;
use common::Result;

use super::types::TreeNode;

/// Abstract repo for entity <-> tree node links.
#[async_trait]
pub trait GTLinkRepo: Send + Sync {
    async fn link(&self, entity_id: &str, tree_node_id: &str) -> Result<()>;
    async fn link_batch(&self, links: &[(String, String)]) -> Result<()>;
    async fn get_linked_nodes(&self, entity_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_entities_in_subtree(&self, node_id: &str) -> Result<Vec<String>>;
    async fn delete_by_tree_node(&self, tree_node_id: &str) -> Result<u64>;
    async fn migrate_entity_links(
        &self,
        source_entity_id: &str,
        target_entity_id: &str,
    ) -> Result<()>;
}
