//! Tree node embedding operations for hierarchical note RAG.

use arrow_array::{Float32Array, StringArray};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::error::StorageError;

use super::{crud::sanitize_predicate_value, VectorStore};

/// A search result from the `tree_node_embeddings` table.
pub struct TreeNodeSearchResult {
    pub node_id: String,
    pub note_id: String,
    pub level: String,
    pub score: f64,
}

impl VectorStore {
    /// Upsert a tree node embedding.
    pub async fn upsert_tree_node_embedding(
        &self,
        node_id: &str,
        embedding: &[f32],
        note_id: &str,
        level: &str,
        source_type: &str,
    ) -> Result<(), StorageError> {
        self.upsert_embedding(
            "tree_node_embeddings",
            node_id,
            embedding,
            &[
                ("note_id", note_id),
                ("level", level),
                ("source_type", source_type),
            ],
        )
        .await
    }

    /// Search tree node embeddings by vector similarity with optional note ID filtering.
    ///
    /// Returns `TreeNodeSearchResult` entries sorted by similarity desc,
    /// filtered to `score >= min_similarity`.
    pub async fn search_tree_node_embeddings(
        &self,
        query_vector: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> Result<Vec<TreeNodeSearchResult>, StorageError> {
        let tbl = self
            .db
            .open_table("tree_node_embeddings")
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("Open tree_node_embeddings table: {e}")))?;

        let mut query = tbl
            .query()
            .nearest_to(query_vector)
            .map_err(|e| StorageError::Vector(format!("Vector search setup: {e}")))?
            .limit(limit);

        if let Some(note_id) = note_id_filter {
            let safe = sanitize_predicate_value(note_id)?;
            query = query.only_if(format!("note_id = '{safe}'"));
        }

        let results = query
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("Vector search: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("Collect results: {e}")))?;

        let mut out = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let note_id_col = batch
                .column_by_name("note_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let level_col = batch
                .column_by_name("level")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let (Some(ids), Some(note_ids), Some(levels)) = (id_col, note_id_col, level_col) else {
                continue; // skip malformed batch
            };

            for i in 0..batch.num_rows() {
                let score = match dist_col {
                    Some(d) => 1.0 - d.value(i) as f64,
                    None => 1.0,
                };
                if score >= min_similarity {
                    out.push(TreeNodeSearchResult {
                        node_id: ids.value(i).to_string(),
                        note_id: note_ids.value(i).to_string(),
                        level: levels.value(i).to_string(),
                        score,
                    });
                }
            }
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        Ok(out)
    }

    /// Search tree node embeddings filtered to a specific `source_type` value.
    ///
    /// Returns `(id, score)` pairs where `score >= min_similarity`, sorted by
    /// similarity descending.  Used by the cross-domain searcher to find
    /// finance (or other domain) nodes without pulling in un-related source types.
    pub async fn search_tree_nodes_by_source_type(
        &self,
        query_vector: &[f32],
        source_type: &str,
        limit: usize,
        min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let tbl = match self.db.open_table("tree_node_embeddings").execute().await {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()), // table doesn't exist yet
        };

        let safe_source_type = sanitize_predicate_value(source_type)?;
        let results = tbl
            .query()
            .nearest_to(query_vector)
            .map_err(|e| StorageError::Vector(format!("Vector search setup: {e}")))?
            .limit(limit)
            .only_if(format!("source_type = '{safe_source_type}'"))
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("Vector search: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("Collect results: {e}")))?;

        let mut out = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let Some(ids) = id_col else {
                continue;
            };

            for i in 0..batch.num_rows() {
                let score = match dist_col {
                    Some(d) => 1.0 - d.value(i) as f64,
                    None => 1.0,
                };
                if score >= min_similarity {
                    out.push((ids.value(i).to_string(), score));
                }
            }
        }
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(out)
    }

    /// Delete all tree node embeddings for a given note.
    pub async fn delete_tree_node_embeddings_by_note(
        &self,
        note_id: &str,
    ) -> Result<(), StorageError> {
        let safe = sanitize_predicate_value(note_id)?;
        self.delete_where("tree_node_embeddings", &format!("note_id = '{safe}'"))
            .await
    }
}
