//! Index management and deduplication for LanceDB tables.

use arrow_array::StringArray;
use futures_util::TryStreamExt;
use lancedb::index::vector::IvfPqIndexBuilder;
use lancedb::index::Index;
use lancedb::query::{ExecutableQuery, QueryBase, Select};

use crate::error::StorageError;

use super::{crud::sanitize_predicate_value, VectorStore};

impl VectorStore {
    /// Create IVF-PQ vector indexes on all tables that have enough rows.
    ///
    /// Skips tables that already have a vector index or have fewer than
    /// `min_rows` rows (IVF-PQ needs data to train on). Safe to call
    /// repeatedly — it is a no-op if indexes already exist.
    pub async fn ensure_indexes(&self, min_rows: usize) -> Result<(), StorageError> {
        let tables = [
            "todo_embeddings",
            "conv_embeddings",
            "cognitive_fact_embeddings",
            "activity_embeddings",
            "work_context_embeddings",
            "insight_embeddings",
        ];
        for table_name in tables {
            let tbl = match self.db.open_table(table_name).execute().await {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Skip if not enough data to train an index.
            let row_count = tbl
                .count_rows(None)
                .await
                .map_err(|e| StorageError::Vector(format!("count {table_name}: {e}")))?;
            if row_count < min_rows {
                continue;
            }

            // Skip if a vector index already exists.
            let indices = tbl
                .list_indices()
                .await
                .map_err(|e| StorageError::Vector(format!("list indices {table_name}: {e}")))?;
            let has_vector_index = indices
                .iter()
                .any(|idx| idx.columns.contains(&"vector".to_string()));
            if has_vector_index {
                continue;
            }

            let ivf_pq = IvfPqIndexBuilder::default().distance_type(lancedb::DistanceType::Cosine);
            tbl.create_index(&["vector"], Index::IvfPq(ivf_pq))
                .execute()
                .await
                .map_err(|e| {
                    StorageError::Vector(format!("create IVF-PQ index on {table_name}: {e}"))
                })?;

            tracing::info!("Created IVF-PQ index on {table_name} ({row_count} rows)");
        }
        Ok(())
    }

    /// Remove duplicate rows within a table, keeping only the newest row per ID.
    ///
    /// Performs a full table scan of the `id` and `ts_column` columns, groups by
    /// ID in memory, and deletes older rows for any ID that appears more than
    /// once. Returns the number of IDs that had duplicates removed.
    ///
    /// This is intended for background maintenance, not the hot path.
    pub async fn dedup_table(&self, table: &str, ts_column: &str) -> Result<usize, StorageError> {
        let tbl = match self.db.open_table(table).execute().await {
            Ok(t) => t,
            Err(_) => return Ok(0),
        };

        let results = tbl
            .query()
            .select(Select::columns(&["id", ts_column]))
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("dedup scan {table}: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("dedup collect {table}: {e}")))?;

        // Group by id, track the latest timestamp for each.
        let mut latest: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut duplicate_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let ts_col = batch
                .column_by_name(ts_column)
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let (Some(ids), Some(timestamps)) = (id_col, ts_col) else {
                continue;
            };

            for i in 0..batch.num_rows() {
                let id = ids.value(i).to_string();
                let ts = timestamps.value(i).to_string();
                match latest.entry(id.clone()) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(ts);
                    }
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        duplicate_ids.insert(id);
                        if ts > *e.get() {
                            e.insert(ts);
                        }
                    }
                }
            }
        }

        // Delete older rows for each duplicate ID.
        for id in &duplicate_ids {
            let max_ts = &latest[id];
            let safe_id = sanitize_predicate_value(id)?;
            let safe_ts = sanitize_predicate_value(max_ts)?;
            let predicate = format!("id = '{safe_id}' AND {ts_column} < '{safe_ts}'");
            tbl.delete(&predicate)
                .await
                .map_err(|e| StorageError::Vector(format!("dedup delete in {table}: {e}")))?;
        }

        Ok(duplicate_ids.len())
    }
}
