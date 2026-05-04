//! Community embedding operations for Phase 2 community graph RAG.

use arrow_array::{Float32Array, StringArray};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::error::StorageError;

use super::{crud::sanitize_predicate_value, VectorStore};

/// A search result from the `community_embeddings` table.
pub struct CommunitySearchResult {
    pub community_id: String,
    pub member_count: String,
    pub source_note_count: String,
    pub score: f64,
}

impl VectorStore {
    /// Upsert a community embedding.
    pub async fn upsert_community_embedding(
        &self,
        community_id: &str,
        embedding: &[f32],
        member_count: &str,
        source_note_count: &str,
    ) -> Result<(), StorageError> {
        self.upsert_embedding(
            "community_embeddings",
            community_id,
            embedding,
            &[
                ("member_count", member_count),
                ("source_note_count", source_note_count),
            ],
        )
        .await
    }

    /// Search community embeddings by vector similarity.
    ///
    /// Returns `CommunitySearchResult` entries sorted by similarity desc,
    /// filtered to `score >= min_similarity`.
    pub async fn search_community_embeddings(
        &self,
        query_vector: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> Result<Vec<CommunitySearchResult>, StorageError> {
        let tbl = self.get_table("community_embeddings").await?;

        let query = tbl
            .query()
            .nearest_to(query_vector)
            .map_err(|e| StorageError::Vector(format!("Vector search setup: {e}")))?
            .limit(limit);

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
            let member_count_col = batch
                .column_by_name("member_count")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let source_note_count_col = batch
                .column_by_name("source_note_count")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let (Some(ids), Some(member_counts), Some(source_note_counts)) =
                (id_col, member_count_col, source_note_count_col)
            else {
                continue;
            };

            for i in 0..batch.num_rows() {
                let score = match dist_col {
                    Some(d) => 1.0 - d.value(i) as f64,
                    None => 1.0,
                };
                if score >= min_similarity {
                    out.push(CommunitySearchResult {
                        community_id: ids.value(i).to_string(),
                        member_count: member_counts.value(i).to_string(),
                        source_note_count: source_note_counts.value(i).to_string(),
                        score,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Delete a community embedding.
    pub async fn delete_community_embedding(&self, community_id: &str) -> Result<(), StorageError> {
        let safe = sanitize_predicate_value(community_id)?;
        self.delete_where("community_embeddings", &format!("id = '{safe}'"))
            .await
    }
}
