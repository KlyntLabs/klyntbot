//! `ReforgeWriter` — safety wrapper that rejects DELETE-style mutations.
//!
//! All Reforge phases route writes through this wrapper so that:
//! - Raw DELETE is never issued.
//! - Supersede is the only valid removal path.
//! - Stability demotion is the soft-delete equivalent.

use common::{KlyntbotError, Result};

/** Wrapper enforcing Reforge's DELETE-free invariant. */
#[derive(Debug, Clone)]
pub struct ReforgeWriter;

impl ReforgeWriter {
    /// Create a new `ReforgeWriter`.
    pub fn new() -> Self {
        Self
    }

    /// Reject any raw DELETE operation.
    pub fn reject_delete(&self, _table: &str, _reason: &str) -> Result<()> {
        Err(KlyntbotError::Storage(
            "reforge writer rejected DELETE".into(),
        ))
    }

    /// Acceptable soft-removal: demote stability to near-zero.
    pub async fn demote_stability(
        &self,
        repo: &cognitive::SemanticFactRepo,
        id: &str,
    ) -> Result<()> {
        repo.update_convergence(id, 0.01)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("demote stability: {e}")))?;
        Ok(())
    }
}

impl Default for ReforgeWriter {
    fn default() -> Self {
        Self::new()
    }
}
