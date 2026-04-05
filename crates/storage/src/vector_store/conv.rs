//! Conversation embedding search.

use arrow_array::{Float32Array, StringArray};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::error::StorageError;

use super::VectorStore;

impl VectorStore {
    /// Search `conv_embeddings` by nearest-neighbor and return full row data.
    ///
    /// Returns `(id, session_key, role, content_preview, full_content, created_at, score)` tuples
    /// where `score = 1.0 - distance` and `score >= threshold`.
    pub async fn search_conv_embeddings(
        &self,
        query: &[f32],
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(String, String, String, String, String, String, f64)>, StorageError> {
        let tbl = self.get_table("conv_embeddings").await?;

        let results = tbl
            .query()
            .nearest_to(query)
            .map_err(|e| StorageError::Vector(format!("nearest_to: {e}")))?
            .limit(limit)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB query conv_embeddings: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("collect conv results: {e}")))?;

        let mut out = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let sk_col = batch
                .column_by_name("session_key")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let role_col = batch
                .column_by_name("role")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let preview_col = batch
                .column_by_name("content_preview")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let full_col = batch
                .column_by_name("full_content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_col = batch
                .column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let (Some(id_col), Some(sk_col), Some(role_col), Some(preview_col), Some(full_col)) =
                (id_col, sk_col, role_col, preview_col, full_col)
            else {
                continue; // skip malformed batch
            };

            for i in 0..batch.num_rows() {
                let score = match dist_col {
                    Some(d) => 1.0 - d.value(i) as f64,
                    None => 1.0,
                };
                if score >= threshold {
                    let created_at = created_col
                        .map(|c| c.value(i).to_string())
                        .unwrap_or_default();
                    out.push((
                        id_col.value(i).to_string(),
                        sk_col.value(i).to_string(),
                        role_col.value(i).to_string(),
                        preview_col.value(i).to_string(),
                        full_col.value(i).to_string(),
                        created_at,
                        score,
                    ));
                }
            }
        }
        Ok(out)
    }
}
