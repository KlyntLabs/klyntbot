//! LanceDB-backed vector store for embedding similarity search.

use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::Connection;

use crate::error::StorageError;

/// LanceDB-backed vector store for embedding similarity search.
///
/// Manages three tables (all share the convention: `id` first, `vector` second,
/// extra string fields, timestamp last):
///
/// - `todo_embeddings`         — id, vector(384), model, updated_at
/// - `conv_embeddings`         — id, vector(384), session_key, role, content_preview, full_content, created_at
/// - `memory_note_embeddings`  — id, vector(384), updated_at
#[derive(Clone)]
pub struct VectorStore {
    db: Arc<Connection>,
}

impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore").finish_non_exhaustive()
    }
}

impl VectorStore {
    /// Open (or create) the LanceDB store at `{data_dir}/lance/`.
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError> {
        let lance_dir = data_dir.join("lance");
        std::fs::create_dir_all(&lance_dir)
            .map_err(|e| StorageError::Vector(format!("Failed to create lance dir: {e}")))?;
        let path_str = lance_dir
            .to_str()
            .ok_or_else(|| StorageError::Vector("lance dir path is not valid UTF-8".to_string()))?;
        let db = lancedb::connect(path_str)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB connect failed: {e}")))?;
        let store = Self { db: Arc::new(db) };
        store.ensure_table("todo_embeddings", todo_schema()).await?;
        store.ensure_table("conv_embeddings", conv_schema()).await?;
        store
            .ensure_table("memory_note_embeddings", memory_note_schema())
            .await?;
        Ok(store)
    }

    /// Create the table if it does not already exist.
    async fn ensure_table(&self, name: &str, schema: Schema) -> Result<(), StorageError> {
        let table_names = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB list tables: {e}")))?;
        if !table_names.contains(&name.to_string()) {
            let schema_ref: SchemaRef = Arc::new(schema);
            let reader = RecordBatchIterator::new(
                std::iter::empty::<Result<RecordBatch, ArrowError>>(),
                schema_ref,
            );
            self.db
                .create_table(name, Box::new(reader))
                .execute()
                .await
                .map_err(|e| StorageError::Vector(format!("LanceDB create table {name}: {e}")))?;
        }
        Ok(())
    }

    /// Upsert an embedding vector (insert-then-delete-old for crash safety).
    ///
    /// `extra_fields` must be provided in **schema column order** (after `id` and
    /// `vector`), excluding the final timestamp column which is auto-populated.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // todo_embeddings: extra = [("model", "paraphrase-...")]
    /// store.upsert_embedding("todo_embeddings", "task-42", &vec, &[("model", "paraphrase-multilingual")]).await?;
    ///
    /// // conv_embeddings: extra = session_key, role, content_preview, full_content
    /// store.upsert_embedding("conv_embeddings", "msg-1", &vec,
    ///     &[("session_key", "sess"), ("role", "user"), ("content_preview", "Hi"), ("full_content", "Hi there")]).await?;
    ///
    /// // memory_note_embeddings: no extra fields
    /// store.upsert_embedding("memory_note_embeddings", "note-7", &vec, &[]).await?;
    /// ```
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
                Arc::new(Field::new("item", DataType::Float32, true)),
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
        let safe_id = id.replace('\'', "''");
        let safe_now = now.replace('\'', "''");
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
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

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
        // Escape single quotes to prevent predicate injection.
        let safe_id = id.replace('\'', "''");
        tbl.delete(&format!("id = '{safe_id}'"))
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB delete from {table}: {e}")))?;
        Ok(())
    }

    /// Delete all rows matching a SQL predicate.
    ///
    /// The caller is responsible for escaping string values in the predicate
    /// (replace `'` with `''`). Example predicates:
    /// - `"session_key = 'my-session'"`
    /// - `"created_at < '2024-01-01T00:00:00Z'"`
    /// - `"id IS NOT NULL"` (delete all)
    pub async fn delete_where(&self, table: &str, predicate: &str) -> Result<(), StorageError> {
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(_) => return Ok(()), // table may not exist yet
        };
        tbl.delete(predicate)
            .await
            .map_err(|e| StorageError::Vector(format!("delete_where in {table}: {e}")))?;
        Ok(())
    }

    /// Search `conv_embeddings` by nearest-neighbor and return full row data.
    ///
    /// Returns `(id, session_key, role, content_preview, full_content, score)` tuples
    /// where `score = 1.0 - distance` and `score >= threshold`.
    pub async fn search_conv_embeddings(
        &self,
        query: &[f32],
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(String, String, String, String, String, f64)>, StorageError> {
        let tbl = self
            .db
            .open_table("conv_embeddings")
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("open table conv_embeddings: {e}")))?;

        let results = tbl
            .query()
            .nearest_to(query)
            .map_err(|e| StorageError::Vector(format!("nearest_to: {e}")))?
            .limit(limit)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB query conv_embeddings: {e}")))?;

        let batches: Vec<RecordBatch> = results
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
                    out.push((
                        id_col.value(i).to_string(),
                        sk_col.value(i).to_string(),
                        role_col.value(i).to_string(),
                        preview_col.value(i).to_string(),
                        full_col.value(i).to_string(),
                        score,
                    ));
                }
            }
        }
        Ok(out)
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

// ── Table schemas ─────────────────────────────────────────────────────────────
//
// Column ordering convention (must match upsert_embedding column construction):
//   [0]   id         Utf8        row identifier
//   [1]   vector     FixedSizeList<Float32, 384>
//   [2..] extra      Utf8        table-specific string fields
//   last  timestamp  Utf8        updated_at / created_at

fn vector_field() -> Field {
    Field::new(
        "vector",
        DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 384),
        false,
    )
}

fn todo_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("model", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

fn conv_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("session_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("content_preview", DataType::Utf8, false),
        Field::new("full_content", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
    ])
}

fn memory_note_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn test_store() -> (VectorStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = VectorStore::connect(dir.path()).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn test_connect_creates_directory() {
        let dir = TempDir::new().unwrap();
        let lance_dir = dir.path().join("lance");
        assert!(!lance_dir.exists());
        VectorStore::connect(dir.path()).await.unwrap();
        assert!(lance_dir.exists());
    }

    #[tokio::test]
    async fn test_upsert_and_search() {
        let (store, _dir) = test_store().await;
        let vec1 = vec![1.0f32; 384];
        store
            .upsert_embedding(
                "todo_embeddings",
                "test-1",
                &vec1,
                &[("model", "test-model")],
            )
            .await
            .unwrap();

        let count = store.count("todo_embeddings").await.unwrap();
        assert_eq!(count, 1);

        let results = store
            .search_similar("todo_embeddings", &vec1, 5, 0.0)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "test-1");
    }

    #[tokio::test]
    async fn test_delete_removes_entry() {
        let (store, _dir) = test_store().await;
        let vec1 = vec![0.5f32; 384];
        store
            .upsert_embedding("todo_embeddings", "del-1", &vec1, &[("model", "test")])
            .await
            .unwrap();
        assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);
        store.delete("todo_embeddings", "del-1").await.unwrap();
        assert_eq!(store.count("todo_embeddings").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_upsert_replaces_existing() {
        let (store, _dir) = test_store().await;
        let v = vec![0.1f32; 384];
        store
            .upsert_embedding("todo_embeddings", "upsert-1", &v, &[("model", "m1")])
            .await
            .unwrap();
        store
            .upsert_embedding("todo_embeddings", "upsert-1", &v, &[("model", "m2")])
            .await
            .unwrap();
        assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_upsert_inserts_before_deleting() {
        let (store, _dir) = test_store().await;
        let v1 = vec![0.1f32; 384];
        let v2 = vec![0.9f32; 384];

        // Insert initial
        store
            .upsert_embedding("todo_embeddings", "safe-1", &v1, &[("model", "m1")])
            .await
            .unwrap();
        assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);

        // Upsert with new vector — should still have exactly 1 row after
        store
            .upsert_embedding("todo_embeddings", "safe-1", &v2, &[("model", "m2")])
            .await
            .unwrap();
        assert_eq!(store.count("todo_embeddings").await.unwrap(), 1);

        // Verify the new vector is searchable (v2 should match itself better)
        let results = store
            .search_similar("todo_embeddings", &v2, 5, 0.0)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "safe-1");
    }

    #[tokio::test]
    async fn test_conv_and_memory_note_tables_created() {
        let (store, _dir) = test_store().await;
        // These should return 0 (empty tables exist) rather than error.
        assert_eq!(store.count("conv_embeddings").await.unwrap(), 0);
        assert_eq!(store.count("memory_note_embeddings").await.unwrap(), 0);
    }
}
