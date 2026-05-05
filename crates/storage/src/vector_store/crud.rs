//! Core CRUD operations: upsert, search, delete, count.

use std::sync::Arc;

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{Field, SchemaRef};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::error::StorageError;

use super::VectorStore;

/// Escape a string value for use in a LanceDB SQL filter predicate.
///
/// LanceDB does not support parameterized queries, so string values must be
/// escaped before interpolation. This function escapes single quotes and
/// rejects values containing characters that should never appear in
/// identifiers or filter values (semicolons, line breaks, comment markers).
pub fn sanitize_predicate_value(value: &str) -> Result<String, StorageError> {
    if value.contains(';')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains("--")
        || value.contains("/*")
    {
        return Err(StorageError::Vector(format!(
            "Predicate value contains disallowed characters: {:?}",
            &value[..value.len().min(40)]
        )));
    }
    Ok(value.replace('\'', "''"))
}

/// Validate a full predicate expression for dangerous patterns.
///
/// Unlike [`sanitize_predicate_value`] (which escapes a single interpolated value),
/// this checks an assembled predicate for structural injection indicators:
/// semicolons (multi-statement), comment markers, and line breaks.
///
/// Single-quote safety is the caller's responsibility — use [`sanitize_predicate_value`]
/// for each interpolated value before assembling the predicate.
pub(crate) fn validate_predicate(predicate: &str) -> Result<(), StorageError> {
    if predicate.contains(';')
        || predicate.contains('\n')
        || predicate.contains('\r')
        || predicate.contains("--")
        || predicate.contains("/*")
    {
        return Err(StorageError::Vector(format!(
            "Predicate contains disallowed characters: {:?}",
            &predicate[..predicate.len().min(60)]
        )));
    }
    Ok(())
}

impl VectorStore {
    /// Upsert an embedding vector (insert-then-delete-old for crash safety).
    ///
    /// `extra_fields` must be provided in **schema column order** (after `id` and
    /// `vector`), excluding the final timestamp column which is auto-populated.
    pub async fn upsert_embedding(
        &self,
        table: &str,
        id: &str,
        vector: &[f32],
        extra_fields: &[(&str, &str)],
    ) -> Result<(), StorageError> {
        let tbl = self.get_table(table).await?;

        let schema: SchemaRef = tbl
            .schema()
            .await
            .map_err(|e| StorageError::Vector(format!("schema for {table}: {e}")))?;

        // Build column arrays in schema order: id, vector, extra..., timestamp.
        let id_arr = Arc::new(StringArray::from(vec![id])) as ArrayRef;

        let float_arr = Arc::new(Float32Array::from(vector.to_vec())) as ArrayRef;
        let vector_arr = Arc::new(
            FixedSizeListArray::try_new(
                Arc::new(Field::new("item", arrow_schema::DataType::Float32, true)),
                vector.len() as i32,
                float_arr,
                None,
            )
            .map_err(|e| StorageError::Vector(format!("build vector array: {e}")))?,
        ) as ArrayRef;

        let now = jiff::Timestamp::now().to_string();
        // Extract timestamp column name before schema is consumed below.
        let ts_col_name = schema
            .fields()
            .last()
            .map(|f| f.name().clone())
            .unwrap_or_else(|| "updated_at".into());
        let mut columns: Vec<ArrayRef> = vec![id_arr, vector_arr];
        for (_, v) in extra_fields {
            columns.push(Arc::new(StringArray::from(vec![*v])) as ArrayRef);
        }
        columns.push(Arc::new(StringArray::from(vec![now.as_str()])) as ArrayRef);

        let batch = RecordBatch::try_new(schema.clone(), columns)
            .map_err(|e| StorageError::Vector(format!("build record batch: {e}")))?;

        let reader = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);

        // Insert new row FIRST (safe: crash here = no change).
        tbl.add(Box::new(reader) as super::BatchReader)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB add to {table}: {e}")))?;

        // Delete old rows SECOND (best-effort cleanup). Crash or failure here
        // leaves a temporary duplicate, cleaned up on next upsert or maintenance.
        // We intentionally do NOT propagate delete errors — the new row is already
        // persisted, so the upsert semantically succeeded.
        let safe_id = sanitize_predicate_value(id)?;
        let safe_now = sanitize_predicate_value(&now)?;
        let predicate = format!("id = '{safe_id}' AND {ts_col_name} < '{safe_now}'");
        if let Err(e) = tbl.delete(&predicate).await {
            tracing::warn!("LanceDB cleanup old in {table} (non-fatal): {e}");
        }

        Ok(())
    }

    /// Batch-upsert multiple embeddings in a single write + single delete.
    ///
    /// Much more memory-efficient than N individual `upsert_embedding` calls,
    /// as LanceDB creates one fragment file instead of N.
    #[allow(clippy::type_complexity)]
    pub async fn upsert_embedding_batch(
        &self,
        table: &str,
        items: &[(&str, &[f32], &[(&str, &str)])],
    ) -> Result<(), StorageError> {
        if items.is_empty() {
            return Ok(());
        }
        let tbl = self.get_table(table).await?;
        let schema: SchemaRef = tbl
            .schema()
            .await
            .map_err(|e| StorageError::Vector(format!("schema for {table}: {e}")))?;

        let ts_col_name = schema
            .fields()
            .last()
            .map(|f| f.name().clone())
            .unwrap_or_else(|| "updated_at".into());

        let now = jiff::Timestamp::now().to_string();
        let n = items.len();

        // Build column arrays for all items at once
        let ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();
        let id_arr = Arc::new(StringArray::from(ids.clone())) as ArrayRef;

        // Build vector array: concatenate all vectors into one flat array,
        // then wrap in FixedSizeList
        let dim = items[0].1.len() as i32;
        let all_floats: Vec<f32> = items
            .iter()
            .flat_map(|(_, v, _)| v.iter().copied())
            .collect();
        let float_arr = Arc::new(Float32Array::from(all_floats)) as ArrayRef;
        let vector_arr = Arc::new(
            FixedSizeListArray::try_new(
                Arc::new(Field::new("item", arrow_schema::DataType::Float32, true)),
                dim,
                float_arr,
                None,
            )
            .map_err(|e| StorageError::Vector(format!("build vector array: {e}")))?,
        ) as ArrayRef;

        let mut columns: Vec<ArrayRef> = vec![id_arr, vector_arr];

        // Extra fields — assume all items share the same schema
        let extra_count = items[0].2.len();
        for col_idx in 0..extra_count {
            let vals: Vec<&str> = items
                .iter()
                .map(|(_, _, extras)| extras[col_idx].1)
                .collect();
            columns.push(Arc::new(StringArray::from(vals)) as ArrayRef);
        }

        // Timestamp column
        let ts_vals: Vec<&str> = (0..n).map(|_| now.as_str()).collect();
        columns.push(Arc::new(StringArray::from(ts_vals)) as ArrayRef);

        let batch = RecordBatch::try_new(schema.clone(), columns)
            .map_err(|e| StorageError::Vector(format!("build batch record: {e}")))?;

        let reader = RecordBatchIterator::new(vec![Ok(batch)].into_iter(), schema);

        // Single bulk insert
        tbl.add(Box::new(reader) as super::BatchReader)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB batch add to {table}: {e}")))?;

        // Single bulk delete of old rows
        let safe_now = sanitize_predicate_value(&now)?;
        let id_predicates: Vec<String> = ids
            .iter()
            .filter_map(|id| sanitize_predicate_value(id).ok())
            .map(|safe_id| format!("id = '{safe_id}'"))
            .collect();
        if !id_predicates.is_empty() {
            let predicate = format!(
                "({}) AND {ts_col_name} < '{safe_now}'",
                id_predicates.join(" OR ")
            );
            if let Err(e) = tbl.delete(&predicate).await {
                tracing::warn!("LanceDB batch cleanup old in {table} (non-fatal): {e}");
            }
        }

        Ok(())
    }

    /// Search for similar vectors, returning `(id, score)` pairs where `score >= threshold`.
    ///
    /// Score is computed as `1.0 - distance`. For cosine distance this equals cosine
    /// similarity; for L2 distance it is an inverse-distance measure.
    pub async fn search_similar(
        &self,
        table: &str,
        query: &[f32],
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let tbl = self.get_table(table).await?;

        let results = tbl
            .query()
            .nearest_to(query)
            .map_err(|e| StorageError::Vector(format!("nearest_to: {e}")))?
            .limit(limit)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB query {table}: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("collect results: {e}")))?;

        let mut out = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| StorageError::Vector("missing id column".to_string()))?;

            // LanceDB appends a `_distance` column to nearest-neighbor result sets.
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

            for i in 0..batch.num_rows() {
                let id = id_col.value(i).to_string();
                let score = match dist_col {
                    Some(d) => 1.0 - d.value(i) as f64,
                    None => 1.0,
                };
                if score >= threshold {
                    out.push((id, score));
                }
            }
        }
        Ok(out)
    }

    /// Delete an embedding by ID. Returns Ok(()) if the table does not exist.
    pub async fn delete(&self, table: &str, id: &str) -> Result<(), StorageError> {
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(lancedb::Error::TableNotFound { .. }) => return Ok(()),
            Err(e) => return Err(StorageError::Vector(format!("open table {table}: {e}"))),
        };
        let safe_id = sanitize_predicate_value(id)?;
        tbl.delete(&format!("id = '{safe_id}'"))
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB delete from {table}: {e}")))?;
        self.table_cache.remove(table);
        Ok(())
    }

    /// Delete all rows matching a SQL predicate.
    ///
    /// The predicate is validated for dangerous patterns (semicolons, comment
    /// markers, line breaks) before being passed to LanceDB. Individual values
    /// within the predicate should already be escaped via [`sanitize_predicate_value`].
    /// Returns Ok(()) if the table does not exist.
    pub async fn delete_where(&self, table: &str, predicate: &str) -> Result<(), StorageError> {
        validate_predicate(predicate)?;
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(lancedb::Error::TableNotFound { .. }) => return Ok(()),
            Err(e) => return Err(StorageError::Vector(format!("open table {table}: {e}"))),
        };
        tbl.delete(predicate)
            .await
            .map_err(|e| StorageError::Vector(format!("delete_where in {table}: {e}")))?;
        self.table_cache.remove(table);
        Ok(())
    }

    /// Fetch a raw embedding vector by ID from a table.
    ///
    /// Returns None if the ID is not found. Used for computing cosine
    /// similarity between specific embeddings (e.g., semantic drift).
    pub async fn get_embedding(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<Vec<f32>>, StorageError> {
        let tbl = self.get_table(table).await?;

        let predicate = format!("id = '{}'", sanitize_predicate_value(id)?);

        let results = tbl
            .query()
            .only_if(predicate)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("query {table} by id: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("collect results: {e}")))?;

        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }
            let vector_col = batch
                .column_by_name("vector")
                .ok_or_else(|| StorageError::Vector("missing vector column".to_string()))?;

            let list_arr = vector_col
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or_else(|| {
                    StorageError::Vector("vector column is not FixedSizeList".to_string())
                })?;

            let value_arr = list_arr.value(0);
            let values = value_arr
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| StorageError::Vector("vector values are not Float32".to_string()))?;

            return Ok(Some(values.values().to_vec()));
        }

        Ok(None)
    }

    /// Count the number of rows in a table.
    pub async fn count(&self, table: &str) -> Result<usize, StorageError> {
        let tbl = self.get_table(table).await?;
        let n = tbl
            .count_rows(None)
            .await
            .map_err(|e| StorageError::Vector(format!("count {table}: {e}")))?;
        Ok(n)
    }
}
