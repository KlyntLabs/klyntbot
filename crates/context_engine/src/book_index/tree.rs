use async_trait::async_trait;
use common::Result;

use super::types::{SourceType, TreeNode};

/// Abstract repo for tree node CRUD. Concrete impl in cognitive crate (SQLite).
#[async_trait]
pub trait BookTreeRepo: Send + Sync {
    async fn insert_node(&self, node: &TreeNode) -> Result<()>;
    async fn insert_nodes(&self, nodes: &[TreeNode]) -> Result<()>;
    async fn get_node(&self, id: &str) -> Result<Option<TreeNode>>;
    async fn get_children(&self, parent_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    async fn get_root_sections(&self, source_type: &SourceType) -> Result<Vec<TreeNode>>;
    async fn get_path_to_root(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    async fn delete_by_source(&self, source_type: &SourceType, source_id: &str) -> Result<u64>;
    async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<TreeNode>>;
    async fn has_any_nodes(&self) -> Result<bool>;
}
