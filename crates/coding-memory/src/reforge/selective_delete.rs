//! Phase 6 selective-delete signal. Filled in by Task 10.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Selective-delete signal.
#[derive(Debug, Default)]
pub struct SelectiveDeleteSignal;

/// Repo for the audit log.
#[derive(Debug, Clone)]
pub struct SelectiveDeleteLogRepo {
    pool: storage::StoragePool,
}

impl SelectiveDeleteLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool getter (used by Phase 6 fill-in).
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}

impl SelectiveDeleteSignal {
    /// Apply the signal.
    pub async fn apply(_log: &SelectiveDeleteLogRepo) -> Result<u32> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
