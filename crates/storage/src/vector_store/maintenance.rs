//! Index management, compaction, and deduplication for LanceDB tables.

use arrow_array::StringArray;
use futures_util::TryStreamExt;
use lancedb::index::vector::IvfPqIndexBuilder;
use lancedb::index::Index;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::table::OptimizeAction;

use crate::error::StorageError;

use super::{crud::sanitize_predicate_value, VectorStore};

/// All embedding tables managed by VectorStore.
const ALL_TABLES: &[&str] = &[
    "todo_embeddings",
    "task_embeddings",
    "note_embeddings",
    "conv_embeddings",
    "cognitive_fact_embeddings",
    "activity_embeddings",
    "work_context_embeddings",
    "flashcard_embeddings",
    "tree_node_embeddings",
    "community_embeddings",
    "insight_embeddings",
    "entity_embeddings",
];

impl VectorStore {
    /// Create IVF-PQ vector indexes on all tables that have enough rows.
    ///
    /// Skips tables that already have a vector index or have fewer than
    /// `min_rows` rows (IVF-PQ needs data to train on). Safe to call
    /// repeatedly — it is a no-op if indexes already exist.
    pub async fn ensure_indexes(&self, min_rows: usize) -> Result<(), StorageError> {
        for &table_name in ALL_TABLES {
            let tbl = match self.get_table(table_name).await {
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
        let tbl = match self.get_table(table).await {
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

    /// Prune a table to at most `max_rows` by deleting the oldest entries.
    ///
    /// Uses `ts_column` (ISO-8601 string) to determine age. Scans only the
    /// timestamp column, finds a cutoff, and bulk-deletes everything older.
    /// Returns the number of rows deleted (approximate — concurrent writes
    /// may shift the exact count).
    pub async fn prune_table(
        &self,
        table: &str,
        ts_column: &str,
        max_rows: usize,
    ) -> Result<usize, StorageError> {
        let tbl = match self.get_table(table).await {
            Ok(t) => t,
            Err(_) => return Ok(0),
        };

        let row_count = tbl
            .count_rows(None)
            .await
            .map_err(|e| StorageError::Vector(format!("count {table}: {e}")))?;

        if row_count <= max_rows {
            return Ok(0);
        }

        let to_delete = row_count - max_rows;

        // Scan only the timestamp column for the oldest `to_delete` rows.
        let results = tbl
            .query()
            .select(Select::columns(&[ts_column]))
            .limit(to_delete)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("prune scan {table}: {e}")))?;

        let batches: Vec<arrow_array::RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| StorageError::Vector(format!("prune collect {table}: {e}")))?;

        // Find the maximum (newest) timestamp among the rows we want to delete.
        let mut cutoff: Option<String> = None;
        for batch in &batches {
            if let Some(col) = batch
                .column_by_name(ts_column)
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            {
                for i in 0..batch.num_rows() {
                    let ts = col.value(i).to_string();
                    if cutoff.as_ref().is_none_or(|c| ts > *c) {
                        cutoff = Some(ts);
                    }
                }
            }
        }

        if let Some(cutoff) = cutoff {
            let safe_cutoff = sanitize_predicate_value(&cutoff)?;
            let predicate = format!("{ts_column} <= '{safe_cutoff}'");
            tbl.delete(&predicate)
                .await
                .map_err(|e| StorageError::Vector(format!("prune delete {table}: {e}")))?;
            tracing::info!("Pruned ~{to_delete} rows from {table} (cutoff: {cutoff})");
        }

        Ok(to_delete)
    }

    /// Compact a single table to reclaim memory and disk space.
    ///
    /// Useful for high-write-frequency tables (e.g. `work_context_embeddings`)
    /// that accumulate fragments faster than periodic `optimize_all_tables`.
    pub async fn optimize_table(&self, table_name: &str) -> Result<(), StorageError> {
        let tbl = self.get_table(table_name).await?;
        tbl.optimize(OptimizeAction::All)
            .await
            .map_err(|e| StorageError::Vector(format!("optimize {table_name}: {e}")))?;
        self.table_cache.remove(table_name);
        Ok(())
    }

    /// Compact all vector tables to reclaim memory and disk space.
    ///
    /// LanceDB uses copy-on-write storage — every upsert/delete creates new
    /// fragment files. Without periodic compaction these accumulate and get
    /// memory-mapped, causing RSS to grow unboundedly.
    ///
    /// Runs `OptimizeAction::All` on each table which:
    /// - Merges small fragment files into larger ones (compaction)
    /// - Prunes old versions older than 7 days
    /// - Re-optimizes indices for unindexed data
    ///
    pub async fn optimize_all_tables(&self) -> Result<(), StorageError> {
        for &table_name in ALL_TABLES {
            let tbl = match self.get_table(table_name).await {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Skip empty tables — compaction is a no-op but still loads
            // the manifest and allocates Arrow buffers.
            let row_count = tbl.count_rows(None).await.unwrap_or(0);
            if row_count == 0 {
                self.table_cache.remove(table_name);
                continue;
            }

            let result = tbl.optimize(OptimizeAction::All).await;
            // Drop the cached handle so LanceDB can unmap fragment files.
            drop(tbl);
            self.table_cache.remove(table_name);

            match result {
                Ok(stats) => {
                    tracing::debug!(
                        "Optimized {table_name}: compaction={:?}, prune={:?}",
                        stats.compaction,
                        stats.prune,
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to optimize {table_name}: {e}");
                }
            }

            // Force the allocator to return freed pages to the OS immediately.
            // Without this, mimalloc retains large freed blocks for reuse,
            // causing RSS to stay elevated after compaction.
            common::memory::purge_freed_memory();
        }

        Ok(())
    }
}
