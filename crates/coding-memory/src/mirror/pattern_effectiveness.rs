//! Pattern effectiveness EMA log repo. Filled in by Task 19.

/// Repo for the `pattern_effectiveness_log` table.
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
