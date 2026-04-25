//! Session-summary repo. Filled in by Task 3.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    /// Id.
    pub id: String,
    /// Session id.
    pub session_id: String,
    /// Repo id.
    pub repo_id: Option<String>,
    /// When summarised.
    pub summarised_at: Timestamp,
    /// Markdown body.
    pub summary_md: String,
    /// Estimated tokens.
    pub token_count: u32,
}

/// Repo.
#[derive(Debug, Clone)]
pub struct SessionSummaryRepo {
    pool: storage::StoragePool,
}

impl SessionSummaryRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref for fill-in.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}
