//! Search, search_semantic, and search_hybrid action handlers.

use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tracing::{debug, info};

use super::super::TodoTool;
use crate::types::Todo;

impl TodoTool {
    pub(crate) async fn handle_search(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?;
        let limit = p.optional_u64("limit")?;

        let rows = self
            .repo
            .search_by_keyword(query, limit.map(|l| l as i64))
            .await?;
        let results: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

        if results.is_empty() {
            return Ok(format!("No tasks found matching '{}'", query));
        }

        let mut output = format!("{} task(s) matching '{}':\n", results.len(), query);
        for todo in results {
            let priority_str = todo.priority.map(|p| format!("P{}", p)).unwrap_or_else(|| "P3".to_string());
            output.push_str(&format!(
                "\n- [{}] {} ({}, {:?})",
                todo.id, todo.title, priority_str, todo.status
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_search_semantic(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?.trim();
        let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;
        let threshold = p
            .optional_f64("threshold")?
            .unwrap_or(self.semantic_threshold);

        if query.is_empty() {
            return Err(ToolError::InvalidParams("Search query cannot be empty".to_string()).into());
        }
        if !(0.0..=1.0).contains(&threshold) {
            return Err(ToolError::InvalidParams(
                "Threshold must be between 0.0 and 1.0".to_string(),
            )
            .into());
        }

        let emb = self.embedding_handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "Semantic search unavailable: embedding engine not initialized. Use plain 'search' for keyword search.".to_string(),
            )
        })?;

        let emb_repo = self.embedding_repo.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "Semantic search unavailable: embedding repo not configured".to_string(),
            )
        })?;

        let total_summary = self.repo.summary().await?;
        let total_tasks = total_summary.total as usize;

        debug!(query = query, "Generating query embedding for semantic search");
        let query_vec = emb.embed_query(query).await?;

        let scored = emb_repo
            .search_similar_vec(&query_vec, limit as i64, threshold)
            .await?;

        let embedding_count = emb_repo.count().await.unwrap_or(0) as usize;

        if scored.is_empty() {
            let mut msg = format!(
                "No semantically similar tasks found for '{}' (threshold: {:.2}).",
                query, threshold
            );
            if threshold > 0.7 {
                msg.push_str(" Try lowering the threshold.");
            }
            return Ok(msg);
        }

        let mut output = format!(
            "{} task(s) matching '{}' (semantic, threshold: {:.2}):\n",
            scored.len(),
            query,
            threshold
        );

        for (id, sim) in &scored {
            if let Ok(Some(row)) = self.repo.get(id).await {
                let todo = Todo::from(row);
                let priority_str = todo.priority.map(|p| format!("P{}", p)).unwrap_or_else(|| "P3".to_string());
                output.push_str(&format!(
                    "\n- [{}] {} ({}, {:?}, sim: {:.2})",
                    todo.id, todo.title, priority_str, todo.status, sim
                ));
            }
        }

        if embedding_count < total_tasks {
            output.push_str(&format!(
                "\n\nNote: {} of {} tasks have embeddings.",
                embedding_count, total_tasks
            ));
        }

        info!(
            query = query,
            results = scored.len(),
            "Semantic search completed"
        );

        Ok(output)
    }

    pub(crate) async fn handle_search_hybrid(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?.trim();
        let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;

        if query.is_empty() {
            return Err(ToolError::InvalidParams("Search query cannot be empty".to_string()).into());
        }

        let keyword_rows = self.repo.search_by_keyword(query, None).await?;
        let keyword_results: Vec<Todo> = keyword_rows.into_iter().map(Todo::from).collect();
        let keyword_count = keyword_results.len();

        // Run semantic search if available
        let semantic_results: Vec<(String, f64)> = if let (Some(emb), Some(emb_repo)) =
            (&self.embedding_handler, &self.embedding_repo)
        {
            let query_vec = emb.embed_query(query).await?;
            emb_repo
                .search_similar_vec(&query_vec, 100, self.semantic_threshold)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let merged = crate::search::hybrid_merge(&keyword_results, &semantic_results, self.rrf_k);

        if merged.is_empty() {
            return Ok(format!(
                "No tasks found matching '{}' (hybrid: keyword + semantic).",
                query
            ));
        }

        let results: Vec<_> = merged.into_iter().take(limit).collect();
        let mut output = format!(
            "{} task(s) matching '{}' (hybrid: keyword + semantic):\n",
            results.len(),
            query
        );

        for (todo, _rrf_score, source) in &results {
            let priority_str = todo.priority.map(|p| format!("P{}", p)).unwrap_or_else(|| "P3".to_string());
            output.push_str(&format!(
                "\n- [{}] {} ({}, {:?}, match: {})",
                todo.id, todo.title, priority_str, todo.status, source
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
