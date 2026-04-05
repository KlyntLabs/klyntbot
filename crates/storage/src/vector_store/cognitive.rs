//! Cognitive fact embedding operations.

use arrow_array::{Float32Array, StringArray};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::error::StorageError;

use super::{crud::sanitize_predicate_value, VectorStore};

/// Parameters for upserting a cognitive fact embedding.
pub struct CognitiveFactParams<'a> {
    pub fact_id: &'a str,
    pub vector: &'a [f32],
    pub domain: &'a str,
    pub text: &'a str,
    pub importance: f32,
    pub stability: f32,
    pub confidence: f32,
}

impl VectorStore {
    /// Upsert a cognitive fact embedding.
    pub async fn upsert_cognitive_fact(
        &self,
        params: CognitiveFactParams<'_>,
    ) -> Result<(), StorageError> {
        self.upsert_embedding(
            "cognitive_fact_embeddings",
            params.fact_id,
            params.vector,
            &[
                ("domain", params.domain),
                ("text", params.text),
                ("importance", &params.importance.to_string()),
                ("stability", &params.stability.to_string()),
                ("confidence", &params.confidence.to_string()),
            ],
        )
        .await
    }

    /// Search cognitive fact embeddings by vector similarity with domain filtering.
    ///
    /// Returns `(fact_id, similarity_score)` pairs sorted by similarity desc.
    pub async fn search_cognitive_facts(
        &self,
        query_vector: &[f32],
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let tbl = self.get_table("cognitive_fact_embeddings").await?;

        // Build domain filter: domain IN ('identity', 'energy', ...)
        let mut query = tbl
            .query()
            .nearest_to(query_vector)
            .map_err(|e| StorageError::Vector(format!("Vector search setup: {e}")))?
            .limit(top_k);

        if !domains.is_empty() {
            let mut quoted = Vec::with_capacity(domains.len());
            for d in domains {
                let safe = sanitize_predicate_value(d)?;
                quoted.push(format!("'{safe}'"));
            }
            let domain_filter = format!("domain IN ({})", quoted.join(", "));
            query = query.only_if(domain_filter);
        }

        let results = query
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("Vector search: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("Collect results: {e}")))?;

        let mut scored = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            if let (Some(ids), Some(dists)) = (id_col, dist_col) {
                for i in 0..batch.num_rows() {
                    let similarity = 1.0 - dists.value(i) as f64;
                    if similarity >= min_similarity {
                        scored.push((ids.value(i).to_string(), similarity));
                    }
                }
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored)
    }
}
