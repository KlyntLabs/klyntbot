//! Unified search action handler (keyword / semantic / hybrid).

use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tracing::{debug, info};

use super::super::TaskTool;
use crate::types::Task;

impl TaskTool {
    /// Unified search: auto-selects keyword, semantic, or hybrid mode
    /// based on whether embedding handler + store are available.
    pub(crate) async fn handle_search(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?.trim();
        let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;

        if query.is_empty() {
            return Err(
                ToolError::InvalidParams("Search query cannot be empty".to_string()).into(),
            );
        }

        // If we have embedding support, do hybrid search
        if let (Some(emb), Some(vs)) = (&self.embedding_handler, &self.embedding_store) {
            return self
                .search_hybrid(query, limit, emb.as_ref(), vs)
                .await;
        }

        // Otherwise, keyword-only search
        self.search_keyword(query, limit).await
    }

    async fn search_keyword(&self, query: &str, limit: usize) -> Result<String> {
        let rows = self
            .repo
            .search_by_keyword(query, Some(limit as i64))
            .await?;
        let results: Vec<Task> = rows.into_iter().map(Task::from).collect();

        if results.is_empty() {
            return Ok(format!("No tasks found matching '{}'", query));
        }

        let mut output = format!("{} task(s) matching '{}':\n", results.len(), query);
        for task in results {
            let priority_str = task
                .priority
                .map(|p| format!("P{}", p))
                .unwrap_or_else(|| "P3".to_string());
            output.push_str(&format!(
                "\n- [{}] {} ({}, {})",
                task.id, task.title, priority_str, task.status
            ));
        }
        Ok(output)
    }

    async fn search_hybrid(
        &self,
        query: &str,
        limit: usize,
        emb: &dyn crate::handlers::EmbeddingHandler,
        vs: &storage::VectorStore,
    ) -> Result<String> {
        let keyword_rows = self.repo.search_by_keyword(query, None).await?;
        let keyword_results: Vec<Task> = keyword_rows.into_iter().map(Task::from).collect();
        let keyword_count = keyword_results.len();

        // Run semantic search
        debug!(query = query, "Generating query embedding for hybrid search");
        let query_vec = emb.embed_query(query).await?;

        let semantic_results = vs
            .search_similar("task_embeddings", &query_vec, 100, self.semantic_threshold)
            .await
            .unwrap_or_default();

        let merged = crate::search::hybrid_merge(&keyword_results, &semantic_results, self.rrf_k);

        if merged.is_empty() {
            return Ok(format!("No tasks found matching '{}' (hybrid search).", query));
        }

        let results: Vec<_> = merged.into_iter().take(limit).collect();
        let mut output = format!(
            "{} task(s) matching '{}' (hybrid: keyword + semantic):\n",
            results.len(),
            query
        );

        for (task, _rrf_score, source) in &results {
            let priority_str = task
                .priority
                .map(|p| format!("P{}", p))
                .unwrap_or_else(|| "P3".to_string());
            output.push_str(&format!(
                "\n- [{}] {} ({}, {}, match: {})",
                task.id, task.title, priority_str, task.status, source
            ));
        }

        info!(
            query = query,
            results = results.len(),
            keyword_results = keyword_count,
            "Hybrid search completed"
        );

        Ok(output)
    }
}
