pub mod entity_resolution;
pub mod gt_link;
pub mod tiptap_parser;
pub mod tree;
pub mod types;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use common::Result;
use dashmap::DashMap;

pub use gt_link::GTLinkRepo;
pub use tree::BookTreeRepo;
pub use types::*;

/// Local embedding trait for context_engine (L3 cannot import cognitive L5).
#[async_trait]
pub trait BookEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Trait wrapping the subset of EntityRepo operations BookIndex needs.
#[async_trait]
pub trait BookEntityRepo: Send + Sync {
    async fn find_by_name(&self, query: &str) -> Result<Vec<EntityInfo>>;
    async fn get_neighborhood_ids(
        &self,
        entity_id: &str,
        depth: u32,
    ) -> Result<Vec<(String, String, f64)>>;
}

/// Minimal entity info passed across the layer boundary.
#[derive(Debug, Clone)]
pub struct EntityInfo {
    pub id: String,
    pub name: String,
    pub entity_type: String,
}

pub struct BookIndex {
    tree_repo: Arc<dyn BookTreeRepo>,
    entity_repo: Arc<dyn BookEntityRepo>,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    embedder: Arc<dyn BookEmbedder>,
    has_content_flag: AtomicBool,
    root_sections_cache: DashMap<String, (Vec<TreeNode>, Instant)>,
    cache_ttl: Duration,
}

impl BookIndex {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        entity_repo: Arc<dyn BookEntityRepo>,
        gt_link_repo: Arc<dyn GTLinkRepo>,
        embedder: Arc<dyn BookEmbedder>,
    ) -> Self {
        Self {
            tree_repo,
            entity_repo,
            gt_link_repo,
            embedder,
            has_content_flag: AtomicBool::new(false),
            root_sections_cache: DashMap::new(),
            cache_ttl: Duration::from_secs(60),
        }
    }

    pub fn has_content(&self) -> bool {
        self.has_content_flag.load(Ordering::Relaxed)
    }

    pub fn tree_repo(&self) -> &dyn BookTreeRepo {
        self.tree_repo.as_ref()
    }

    pub fn entity_repo(&self) -> &dyn BookEntityRepo {
        self.entity_repo.as_ref()
    }

    pub fn gt_link_repo(&self) -> &dyn GTLinkRepo {
        self.gt_link_repo.as_ref()
    }

    pub fn embedder(&self) -> &dyn BookEmbedder {
        self.embedder.as_ref()
    }

    pub async fn refresh_has_content(&self) -> Result<()> {
        let has = self.tree_repo.has_any_nodes().await?;
        self.has_content_flag.store(has, Ordering::Release);
        self.invalidate_caches();
        Ok(())
    }

    /// Get root sections with 60s TTL cache.
    pub async fn get_root_sections_cached(
        &self,
        source_type: &SourceType,
    ) -> Result<Vec<TreeNode>> {
        let key = source_type.as_str().to_string();
        if let Some(entry) = self.root_sections_cache.get(&key) {
            let (nodes, inserted_at) = entry.value();
            if inserted_at.elapsed() < self.cache_ttl {
                return Ok(nodes.clone());
            }
        }
        let nodes = self.tree_repo.get_root_sections(source_type).await?;
        self.root_sections_cache
            .insert(key, (nodes.clone(), Instant::now()));
        Ok(nodes)
    }

    /// Invalidate all caches (call after tree modifications).
    pub fn invalidate_caches(&self) {
        self.root_sections_cache.clear();
    }
}
