//! `ReforgeWriter` — safety wrapper that rejects DELETE-style mutations.
//!
//! All Reforge phases route writes through this wrapper so that:
//! - Raw DELETE is never issued.
//! - Supersede is the only valid removal path.
//! - Stability demotion is the soft-delete equivalent.

use common::{KlyntbotError, Result};

/** Wrapper enforcing Reforge's DELETE-free invariant.
 *
 * All Reforge phases that mutate `semantic_facts` or `episodic_memories`
 * route through this wrapper. Removal is exclusively expressed as
 * supersede-with-valid_until or stability-demotion; no raw DELETE is allowed.
 */
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

    /// Bi-temporal supersede: terminate `older_id` (`valid_until = at`,
    /// `superseded_by = newer_id`). Both rows remain physically present —
    /// this is the *only* sanctioned removal path.
    ///
    /// Distinct from `SemanticFactRepo::supersede`, which writes
    /// `superseded_at` (logical-time supersede, no `valid_until`) and
    /// appends a changelog entry. Use that path for in-distillation
    /// supersedes; this path is for cross-session dedup where the
    /// bi-temporal `valid_until` cutoff is what queries rely on.
    pub async fn set_superseded_by(
        &self,
        repo: &cognitive::SemanticFactRepo,
        older_id: &str,
        newer_id: &str,
        at: jiff::Timestamp,
    ) -> Result<()> {
        sqlx::query("UPDATE semantic_facts SET valid_until = ?1, superseded_by = ?2 WHERE id = ?3")
            .bind(at.to_string())
            .bind(newer_id)
            .bind(older_id)
            .execute(repo.pool())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("set_superseded_by: {e}")))?;
        Ok(())
    }
}

impl Default for ReforgeWriter {
    fn default() -> Self {
        Self::new()
    }
}
