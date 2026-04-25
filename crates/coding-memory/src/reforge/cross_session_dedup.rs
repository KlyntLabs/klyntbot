//! Phase 6.5 cross-session fact dedup.
//!
//! Find pairs of facts in the same `(scope_repo_id, subject, predicate)` bucket
//! whose `object` matches under exact-string equality (Phase 5 floor) or vector
//! similarity > threshold (Phase 6 will swap in `UnifiedMemoryService`). When
//! found, the older row's `valid_until` and the newer row's `supersedes` are
//! set bi-temporally — both rows remain queryable.

use cognitive::SemanticFactRepo;
use common::{KlyntbotError, Result};
use jiff::Timestamp;

/// Cross-session dedup pass.
#[derive(Debug, Default)]
pub struct CrossSessionDedup;

impl CrossSessionDedup {
    /// Run via vector similarity. Phase 5 ships the exact-match-only fallback
    /// because LanceDB embeddings live outside the in-memory test pool. Phase 6
    /// will replace this with similarity-based candidate pulls.
    pub async fn run(
        repo: &SemanticFactRepo,
        _similarity_threshold: f32,
    ) -> Result<u32> {
        Self::run_test_only_exact_match(repo, 0.92).await
    }

    /// Exact-match dedup — same `(scope_repo_id, subject, predicate, object)`
    /// across two distinct ids. Used as the Phase-5 floor and as the test seam.
    pub async fn run_test_only_exact_match(
        repo: &SemanticFactRepo,
        _similarity_threshold: f32,
    ) -> Result<u32> {
        let pool = repo.pool().clone();
        let pairs: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT older.id, older.valid_from, newer.id, newer.valid_from \
             FROM semantic_facts older \
             JOIN semantic_facts newer ON \
                 older.subject = newer.subject AND \
                 older.predicate = newer.predicate AND \
                 older.object = newer.object AND \
                 (older.scope_repo_id IS NEWER.scope_repo_id OR \
                  older.scope_repo_id = newer.scope_repo_id) AND \
                 older.id != newer.id AND \
                 older.valid_from < newer.valid_from \
             WHERE older.valid_until IS NULL AND newer.superseded_by IS NULL",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("dedup query: {e}")))?;

        let mut applied = 0_u32;
        for (older_id, _older_vf, newer_id, newer_vf) in pairs {
            let now = Timestamp::now().to_string();
            let cutoff = if newer_vf.is_empty() { now } else { newer_vf };
            sqlx::query(
                "UPDATE semantic_facts SET valid_until = ?1, superseded_by = ?2 WHERE id = ?3",
            )
            .bind(&cutoff)
            .bind(&newer_id)
            .bind(&older_id)
            .execute(&pool)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("dedup older update: {e}")))?;
            applied += 1;
        }
        Ok(applied)
    }
}
