use async_trait::async_trait;

use crate::MemoryEntry;

/// Trait for searching domain-specific data (notes, tasks, finance, graph).
///
/// Feature crates (L4+) implement this trait; instances are injected at app
/// startup as `Arc<dyn DomainSearcher>`.
#[async_trait]
pub trait DomainSearcher: Send + Sync {
    /// Returns the human-readable name of this domain (e.g. "notes", "tasks").
    fn domain_name(&self) -> &str;

    /// Search this domain for entries relevant to `query`, returning up to `limit` results.
    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry>;
}
