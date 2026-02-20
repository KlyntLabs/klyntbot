//! Search, search_semantic, and search_hybrid action handlers.

use crate::params::ParamExtractor;
use crate::todo_types::Todo;
use common::{Result, ToolError};
use std::collections::HashMap;
use tracing::{debug, info};

use super::super::TodoTool;

impl TodoTool {
    pub(crate) async fn handle_search(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?;

        let rows = self.repo.search_by_keyword(query, None).await?;
        let results: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

        if results.is_empty() {
            return Ok(format!("No tasks found matching '{}'", query));
        }

        let mut output = format!("{} task(s) matching '{}':\n", results.len(), query);
        for todo in results {
            output.push_str(&format!(
                "\n- [{}] {} (P{}, {:?})",
                todo.id,
                todo.title,
                todo.priority.unwrap_or(3),
                todo.status
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_search_semantic(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?;
        let query = query.trim();
        let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;
        let threshold = p
            .optional_f64("threshold")?
            .unwrap_or(self.semantic_threshold);

        // Validation
        if query.is_empty() {
            return Err(
                ToolError::InvalidParams("Search query cannot be empty".to_string()).into(),
            );
        }
        if query.len() > 1000 {
            return Err(ToolError::InvalidParams(
                "Query too long (max 1000 characters)".to_string(),
            )
            .into());
        }
        if !(0.0..=1.0).contains(&threshold) {
            return Err(ToolError::InvalidParams(
                "Threshold must be between 0.0 and 1.0".to_string(),
            )
            .into());
        }
        if limit == 0 || limit > 1000 {
            return Err(
                ToolError::InvalidParams("Limit must be between 1 and 1000".to_string()).into(),
            );
        }

        let search_start = std::time::Instant::now();

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

        // Get total task count for partial-coverage note
        let total_summary = self.repo.summary().await?;
        let total_tasks = total_summary.total as usize;

        // 1. Embed query
        debug!(
            query = query,
            "Generating query embedding for semantic search"
        );
        let query_vec = emb.embed_query(query).await?;

        // 2. Search via pgvector
        let scored = emb_repo
            .search_similar_vec(&query_vec, limit as i64, threshold)
            .await?;

        let embedding_count = emb_repo.count().await.unwrap_or(0) as usize;

        if scored.is_empty() {
            let mut msg = format!(
                "No semantically similar tasks found for '{}' (threshold: {:.2}).",
                query, threshold,
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
            threshold,
        );

        for (id, sim) in &scored {
            if let Ok(Some(row)) = self.repo.get(id).await {
                let todo = Todo::from(row);
                output.push_str(&format!(
                    "\n- [{}] {} (P{}, {:?}, sim: {:.2})",
                    todo.id,
                    todo.title,
                    todo.priority.unwrap_or(3),
                    todo.status,
                    sim,
                ));
            }
        }

        if embedding_count < total_tasks {
            output.push_str(&format!(
                "\n\nNote: {} of {} tasks have embeddings. Run `todo backfill-embeddings` to index all tasks.",
                embedding_count, total_tasks,
            ));
        }

        info!(
            query = query,
            results = scored.len(),
            embeddings_searched = embedding_count,
            elapsed_ms = search_start.elapsed().as_millis() as u64,
            "Semantic search completed"
        );

        Ok(output)
    }

    pub(crate) async fn handle_search_hybrid(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.required_str("query")?;
        let query_trimmed = query.trim();
        let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;

        if query_trimmed.is_empty() {
            return Err(
                ToolError::InvalidParams("Search query cannot be empty".to_string()).into(),
            );
        }
        if query_trimmed.len() > 1000 {
            return Err(ToolError::InvalidParams(
                "Query too long (max 1000 characters)".to_string(),
            )
            .into());
        }
        if limit == 0 || limit > 1000 {
            return Err(
                ToolError::InvalidParams("Limit must be between 1 and 1000".to_string()).into(),
            );
        }

        let search_start = std::time::Instant::now();

        // 1. Run keyword search
        let keyword_rows = self.repo.search_by_keyword(query_trimmed, None).await?;
        let keyword_results: Vec<Todo> = keyword_rows.into_iter().map(Todo::from).collect();

        // Build ID-to-Todo map for RRF lookups
        let all_rows = self.repo.list(&storage::TodoFilter::default()).await?;
        let all_todos: Vec<Todo> = all_rows.into_iter().map(Todo::from).collect();
        let todos_by_id: HashMap<String, crate::search_utils::SearchResult> = all_todos
            .into_iter()
            .map(|t| {
                (
                    t.id.clone(),
                    crate::search_utils::SearchResult::Todo(Box::new(t)),
                )
            })
            .collect();

        // 2. Run semantic search via pgvector (if available)
        let semantic_results: Vec<(String, f64)> =
            if let (Some(emb), Some(emb_repo)) = (&self.embedding_handler, &self.embedding_repo) {
                let query_vec = emb.embed_query(query_trimmed).await?;
                emb_repo
                    .search_similar_vec(&query_vec, 100, self.semantic_threshold)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

        // 3. Merge via Reciprocal Rank Fusion
        let keyword_count = keyword_results.len();
        let keyword_search_results: Vec<crate::search_utils::SearchResult> = keyword_results
            .into_iter()
            .map(|t| crate::search_utils::SearchResult::Todo(Box::new(t)))
            .collect();

        let merged = crate::search_utils::rrf_merge(
            &keyword_search_results,
            &semantic_results,
            self.rrf_k,
            &todos_by_id,
        );

        if merged.is_empty() {
            return Ok(format!(
                "No tasks found matching '{}' (hybrid: keyword + semantic).",
                query_trimmed,
            ));
        }

        let results: Vec<_> = merged.into_iter().take(limit).collect();
        let mut output = format!(
            "{} task(s) matching '{}' (hybrid: keyword + semantic):\n",
            results.len(),
            query_trimmed,
        );

        for (search_result, _rrf_score, source) in &results {
            if let crate::search_utils::SearchResult::Todo(todo) = search_result {
                output.push_str(&format!(
                    "\n- [{}] {} (P{}, {:?}, match: {})",
                    todo.id,
                    todo.title,
                    todo.priority.unwrap_or(3),
                    todo.status,
                    source,
                ));
            }
        }

        let has_semantic = self.embedding_handler.is_some();
        info!(
            query = query_trimmed,
            results = results.len(),
            keyword_results = keyword_count,
            semantic_available = has_semantic,
            elapsed_ms = search_start.elapsed().as_millis() as u64,
            "Hybrid search completed"
        );

        Ok(output)
    }
}
