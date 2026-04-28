//! Confirms CodeDomainSearcher domain name and InsightForge registration pattern.

use coding_memory::CodeDomainSearcher;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use context_engine::DomainSearcher;
use std::sync::Arc;
use storage::StoragePool;

struct NoopRetriever;
#[async_trait::async_trait]
impl context_engine::MemoryRetriever for NoopRetriever {
    async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<context_engine::MemoryEntry> {
        vec![]
    }
}

#[tokio::test]
async fn code_domain_searcher_name_is_coding() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let facts = SemanticFactRepo::new(pool.inner().clone());
    let episodes = EpisodicMemoryRepo::new(pool.inner().clone());
    let searcher = CodeDomainSearcher::new(facts, episodes);
    assert_eq!(searcher.domain_name(), "coding");
}

#[test]
fn insight_forge_searcher_names_works() {
    let mut forge = context_engine::InsightForge::new(
        context_engine::InsightForgeConfig::default(),
        Arc::new(context_engine::HeuristicDecomposer),
        Arc::new(NoopRetriever),
    );
    let pool = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { StoragePool::connect_in_memory().await.unwrap() });
    let facts = SemanticFactRepo::new(pool.inner().clone());
    let episodes = EpisodicMemoryRepo::new(pool.inner().clone());
    let searcher = CodeDomainSearcher::new(facts, episodes);
    forge.add_searcher(Arc::new(searcher));
    let names = forge.searcher_names();
    assert!(names.contains(&"coding"), "expected 'coding' in {names:?}");
}
