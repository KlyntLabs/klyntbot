//! Pattern effectiveness EMA source. Filled in by Task 5.

/// Log repo for pattern effectiveness rows.
#[derive(Debug, Clone)]
pub struct PatternEffectivenessLogRepo {
    pool: storage::StoragePool,
}

impl PatternEffectivenessLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}
