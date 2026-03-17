use std::sync::Arc;

use async_trait::async_trait;
use common::Result;

use context_engine::book_index::{BookEmbedder, BookEntityRepo, BookIndex, EntityInfo, GTLinkRepo};
use context_engine::insight_forge::bookrag_searcher::BookRAGSearcher;
use context_engine::operators::OperatorLlm;
use context_engine::retrieval_planner::RetrievalPlanner;

// -- BookEmbedder adapter --

pub struct BookEmbedderAdapter {
    engine: Arc<tools::EmbeddingEngine>,
}

#[async_trait]
impl BookEmbedder for BookEmbedderAdapter {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let engine = self.engine.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || engine.embed(&text))
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
    }
}

// -- BookEntityRepo adapter --

pub struct BookEntityRepoAdapter {
    entity_repo: cognitive::repos::EntityRepo,
}

#[async_trait]
impl BookEntityRepo for BookEntityRepoAdapter {
    async fn find_by_name(&self, query: &str) -> Result<Vec<EntityInfo>> {
        let rows = self
            .entity_repo
            .find_by_name(query)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| EntityInfo {
                id: r.id,
                name: r.name,
                entity_type: r.entity_type,
            })
            .collect())
    }

    async fn get_neighborhood_ids(
        &self,
        entity_id: &str,
        depth: u32,
    ) -> Result<Vec<(String, String, f64)>> {
        let neighborhood = self
            .entity_repo
            .get_neighborhood(entity_id, depth)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let Some(neighborhood) = neighborhood else {
            return Ok(Vec::new());
        };
        let entity_id = entity_id.to_string();
        Ok(neighborhood
            .relationships
            .into_iter()
            .map(|r| {
                let target = if r.source_entity_id == entity_id {
                    r.target_entity_id
                } else {
                    r.source_entity_id
                };
                (target, r.relationship_type, r.strength)
            })
            .collect())
    }
}

// -- OperatorLlm adapter --

pub struct OperatorLlmAdapter {
    provider: providers::DynProvider,
    params: providers::ChatParams,
}

#[async_trait]
impl OperatorLlm for OperatorLlmAdapter {
    async fn complete(&self, system: &str, prompt: &str) -> Result<String> {
        let messages = vec![
            providers::Message::System {
                content: system.to_string(),
            },
            providers::Message::User {
                content: providers::UserContent::Text(prompt.to_string()),
            },
        ];
        let response = self.provider.chat(&messages, None, &self.params).await?;
        response
            .content
            .ok_or_else(|| common::KlyntbotError::Storage("empty LLM response".to_string()))
    }
}

// -- Builder functions --

pub fn build_book_index(
    tree_repo: Arc<dyn context_engine::book_index::BookTreeRepo>,
    entity_repo: cognitive::repos::EntityRepo,
    gt_link_repo: Arc<dyn GTLinkRepo>,
    engine: Arc<tools::EmbeddingEngine>,
) -> Arc<BookIndex> {
    Arc::new(BookIndex::new(
        tree_repo,
        Arc::new(BookEntityRepoAdapter { entity_repo }),
        gt_link_repo,
        Arc::new(BookEmbedderAdapter { engine }),
    ))
}

pub fn build_bookrag_searcher(
    book_index: Arc<BookIndex>,
    provider: providers::DynProvider,
    config: &config::BookRetrievalCfg,
) -> Arc<BookRAGSearcher> {
    let retrieval_config = context_engine::book_index::BookRetrievalConfig {
        max_nodes: config.max_nodes,
        max_map_nodes: config.max_map_nodes,
        operator_timeout_ms: config.operator_timeout_ms,
        pagerank_damping: config.pagerank_damping,
        pagerank_iterations: config.pagerank_iterations,
    };
    let params = providers::ChatParams::new("default")
        .with_temperature(0.1)
        .with_max_tokens(256);
    let llm: Arc<dyn OperatorLlm> = Arc::new(OperatorLlmAdapter { provider, params });
    let planner = Arc::new(RetrievalPlanner::new(book_index, llm, retrieval_config));
    Arc::new(BookRAGSearcher::new(
        planner,
        config.max_nodes,
        config.max_map_nodes,
        config.operator_timeout_ms,
    ))
}
