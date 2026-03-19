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
        let tbl = self
            .db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table {table}: {e}")))?;

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

        let now = chrono::Utc::now().to_rfc3339();
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
        tbl.add(Box::new(reader))
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB add to {table}: {e}")))?;

        // Delete old rows SECOND (safe: crash here = temporary duplicate,
        // cleaned up on next upsert). We delete rows with matching ID that
        // have an older timestamp than `now`.
        let safe_id = sanitize_predicate_value(id)?;
        let safe_now = sanitize_predicate_value(&now)?;
        let predicate = format!("id = '{safe_id}' AND {ts_col_name} < '{safe_now}'");
        tbl.delete(&predicate)
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB cleanup old in {table}: {e}")))?;

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
        let tbl = self
            .db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table {table}: {e}")))?;

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

    /// Delete an embedding by ID.
    pub async fn delete(&self, table: &str, id: &str) -> Result<(), StorageError> {
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(_) => return Ok(()), // table may not exist yet
        };
        let safe_id = sanitize_predicate_value(id)?;
        tbl.delete(&format!("id = '{safe_id}'"))
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB delete from {table}: {e}")))?;
        Ok(())
    }

    /// Delete all rows matching a SQL predicate.
    ///
    /// The predicate is validated for dangerous patterns (semicolons, comment
    /// markers, line breaks) before being passed to LanceDB. Individual values
    /// within the predicate should already be escaped via [`sanitize_predicate_value`].
    pub async fn delete_where(&self, table: &str, predicate: &str) -> Result<(), StorageError> {
        validate_predicate(predicate)?;
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(_) => return Ok(()), // table may not exist yet
        };
        tbl.delete(predicate)
            .await
            .map_err(|e| StorageError::Vector(format!("delete_where in {table}: {e}")))?;
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
        let tbl = self
            .db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table {table}: {e}")))?;

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
        let tbl = self
            .db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table {table}: {e}")))?;
        let n = tbl
            .count_rows(None)
            .await
            .map_err(|e| StorageError::Vector(format!("count {table}: {e}")))?;
        Ok(n)
    }
}
