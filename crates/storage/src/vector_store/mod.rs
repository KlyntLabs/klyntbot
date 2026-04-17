//! LanceDB-backed vector store for embedding similarity search.
//!
//! Split into submodules by concern:
//!   - `schemas`     — Arrow schema definitions for all embedding tables
//!   - `crud`        — upsert, search, delete, count
//!   - `cognitive`   — cognitive fact upsert + domain-filtered search
//!   - `conv`        — conversation embedding search
//!   - `maintenance` — index creation, dedup

use std::path::Path;
use std::sync::Arc;

use dashmap::DashMap;
use lancedb::{Connection, Table};

use crate::error::StorageError;

/// Boxed record-batch reader accepted by lancedb 0.27+ write APIs.
pub(crate) type BatchReader = Box<dyn arrow_array::RecordBatchReader + Send>;

/// Index cache: 32 MB is enough for hot partitions across 12 tables in a single-user app.
/// Default is 6 GiB (designed for cloud/server).
const LANCE_INDEX_CACHE_BYTES: usize = 32 * 1024 * 1024;
/// Metadata cache: 8 MB is ample for 12 tables worth of manifests.
const LANCE_METADATA_CACHE_BYTES: usize = 8 * 1024 * 1024;

mod cognitive;
mod community;
mod conv;
mod crud;
mod entity_embedding;
mod maintenance;
pub(crate) mod schemas;
#[cfg(test)]
mod tests;
mod tree_node;

pub use cognitive::CognitiveFactParams;
pub use community::CommunitySearchResult;
pub use crud::sanitize_predicate_value;
pub use tree_node::TreeNodeSearchResult;

/// LanceDB-backed vector store for embedding similarity search.
///
/// Manages embedding tables (all share the convention: `id` first, `vector` second,
/// extra string fields, timestamp last):
///
/// - `todo_embeddings`              — id, vector(384), model, updated_at
/// - `task_embeddings`              — id, vector(384), model, updated_at
/// - `note_embeddings`              — id, vector(384), model, updated_at
/// - `conv_embeddings`              — id, vector(384), session_key, role, content_preview, created_at
/// - `cognitive_fact_embeddings`    — id, vector(384), domain, text, importance, stability, confidence, updated_at
/// - `activity_embeddings`          — id, vector(384), source, work_context_id, timestamp, updated_at
/// - `work_context_embeddings`      — id, vector(384), updated_at
/// - `flashcard_embeddings`         — id, vector(384), card_id, side, timestamp
/// - `tree_node_embeddings`         — id, vector(384), note_id, level, source_type, updated_at
/// - `community_embeddings`         — id, vector(384), member_count, source_note_count, updated_at
#[derive(Clone)]
pub struct VectorStore {
    pub(crate) db: Arc<Connection>,
    /// Cache of open table handles, keyed by table name.
    /// Avoids re-loading manifests from disk on every operation.
    table_cache: Arc<DashMap<String, Table>>,
}

impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore").finish_non_exhaustive()
    }
}

impl VectorStore {
    /// Open (or create) the LanceDB store at `{data_dir}/lance/`.
    ///
    /// Tables are opened lazily on first access via [`get_table`] — no table
    /// manifests are loaded here, keeping startup I/O near zero.
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError> {
        let lance_dir = data_dir.join("lance");
        std::fs::create_dir_all(&lance_dir)
            .map_err(|e| StorageError::Vector(format!("Failed to create lance dir: {e}")))?;
        let path_str = lance_dir
            .to_str()
            .ok_or_else(|| StorageError::Vector("lance dir path is not valid UTF-8".to_string()))?;
        // Default lance caches are 6GB+1GB — far too large for a single-user app.
        let session = Arc::new(lance::session::Session::new(
            LANCE_INDEX_CACHE_BYTES,
            LANCE_METADATA_CACHE_BYTES,
            Arc::new(lance_io::object_store::ObjectStoreRegistry::default()),
        ));
        let db = lancedb::connect(path_str)
            .session(session)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB connect failed: {e}")))?;
        Ok(Self {
            db: Arc::new(db),
            table_cache: Arc::new(DashMap::new()),
        })
    }

    /// Get a cached table handle, opening or creating the table on first access.
    ///
    /// Subsequent calls for the same table name return a cloned handle from the
    /// cache, avoiding repeated manifest loads from disk.
    ///
    /// On first access the sequence is:
    ///   1. Try `open_table` — succeeds for existing tables.
    ///   2. If `TableNotFound`, look up the schema and create an empty table.
    ///   3. Any other LanceDB error is propagated as `StorageError::Vector`.
    ///
    /// A pre-release schema migration for `conv_embeddings` is applied here:
    /// if the table exists but still has the legacy `full_content` column it is
    /// dropped and recreated with the current schema.
    pub(crate) async fn get_table(&self, name: &str) -> Result<Table, StorageError> {
        if let Some(tbl) = self.table_cache.get(name) {
            return Ok(tbl.clone());
        }

        let tbl = match self.db.open_table(name).execute().await {
            Ok(tbl) => {
                // Pre-release migration: drop conv_embeddings if it has the old `full_content` column.
                if name == "conv_embeddings" {
                    let schema = tbl.schema().await.map_err(|e| {
                        StorageError::Vector(format!("read conv_embeddings schema: {e}"))
                    })?;
                    if schema.column_with_name("full_content").is_some() {
                        self.db
                            .drop_table("conv_embeddings", &[])
                            .await
                            .map_err(|e| {
                                StorageError::Vector(format!("drop old conv_embeddings: {e}"))
                            })?;
                        tracing::info!(
                            "Dropped conv_embeddings table (removed full_content column)"
                        );
                        return self.create_empty_table(name).await;
                    }
                }
                tbl
            }
            Err(lancedb::Error::TableNotFound { .. }) => self.create_empty_table(name).await?,
            Err(e) => {
                return Err(StorageError::Vector(format!("open table {name}: {e}")));
            }
        };

        self.table_cache.insert(name.to_string(), tbl.clone());
        Ok(tbl)
    }

    /// Create an empty table with the schema registered for `name`.
    async fn create_empty_table(&self, name: &str) -> Result<Table, StorageError> {
        use arrow_array::{RecordBatch, RecordBatchIterator};
        use arrow_schema::{ArrowError, SchemaRef};

        let schema = schemas::schema_for_table(name).ok_or_else(|| {
            StorageError::Vector(format!("no schema registered for table '{name}'"))
        })?;
        let schema_ref: SchemaRef = Arc::new(schema);
        let reader = RecordBatchIterator::new(
            std::iter::empty::<Result<RecordBatch, ArrowError>>(),
            schema_ref,
        );
        let tbl = self
            .db
            .create_table(name, Box::new(reader) as BatchReader)
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB create table {name}: {e}")))?;
        self.table_cache.insert(name.to_string(), tbl.clone());
        Ok(tbl)
    }

    /// Clear all cached table handles.
    ///
    /// Should be called after `optimize_all_tables()` so that subsequent
    /// operations get fresh handles that reflect the compacted state.
    #[allow(dead_code)]
    pub(crate) fn invalidate_table_cache(&self) {
        self.table_cache.clear();
    }
}
