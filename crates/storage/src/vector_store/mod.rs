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

use lancedb::Connection;

use crate::error::StorageError;

/// Index cache: holds loaded IVF partition data, HNSW graphs, scalar index details.
/// 128 MB fits hot partitions for all ~12 tables without forcing re-reads on every query.
/// Default is 6 GiB (designed for cloud/server), way too much for a single-user desktop app.
const LANCE_INDEX_CACHE_BYTES: usize = 128 * 1024 * 1024;
/// Metadata cache: holds manifests, deletion vectors, page locations.
/// 32 MB is generous for 12 tables worth of metadata.
const LANCE_METADATA_CACHE_BYTES: usize = 32 * 1024 * 1024;

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
/// - `conv_embeddings`              — id, vector(384), session_key, role, content_preview, full_content, created_at
/// - `cognitive_fact_embeddings`    — id, vector(384), domain, text, importance, stability, confidence, updated_at
/// - `activity_embeddings`          — id, vector(384), source, work_context_id, timestamp, updated_at
/// - `work_context_embeddings`      — id, vector(384), updated_at
/// - `flashcard_embeddings`         — id, vector(384), card_id, side, timestamp
/// - `tree_node_embeddings`         — id, vector(384), note_id, level, source_type, updated_at
/// - `community_embeddings`         — id, vector(384), member_count, source_note_count, updated_at
#[derive(Clone)]
pub struct VectorStore {
    pub(crate) db: Arc<Connection>,
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
        let store = Self { db: Arc::new(db) };

        // Fetch table list once instead of per-table (saves 11 round-trips).
        let existing = store
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| StorageError::Vector(format!("LanceDB list tables: {e}")))?;

        store
            .ensure_table("todo_embeddings", schemas::todo_schema(), &existing)
            .await?;
        store
            .ensure_table("conv_embeddings", schemas::conv_schema(), &existing)
            .await?;
        store
            .ensure_table(
                "cognitive_fact_embeddings",
                schemas::cognitive_fact_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "activity_embeddings",
                schemas::activity_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "work_context_embeddings",
                schemas::work_context_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "task_embeddings",
                schemas::task_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "note_embeddings",
                schemas::note_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "insight_embeddings",
                schemas::insight_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "entity_embeddings",
                schemas::entity_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "flashcard_embeddings",
                schemas::flashcard_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "tree_node_embeddings",
                schemas::tree_node_embedding_schema(),
                &existing,
            )
            .await?;
        store
            .ensure_table(
                "community_embeddings",
                schemas::community_embedding_schema(),
                &existing,
            )
            .await?;
        Ok(store)
    }

    /// Create the table if it does not already exist.
    pub(crate) async fn ensure_table(
        &self,
        name: &str,
        schema: arrow_schema::Schema,
        existing: &[String],
    ) -> Result<(), StorageError> {
        use arrow_array::{RecordBatch, RecordBatchIterator};
        use arrow_schema::{ArrowError, SchemaRef};

        if !existing.iter().any(|t| t == name) {
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
}
