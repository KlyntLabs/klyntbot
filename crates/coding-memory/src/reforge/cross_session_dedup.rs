//! Phase 6.5 cross-session fact dedup.
//!
//! Find pairs of facts in the same `(scope_repo_id, subject, predicate)` bucket
//! whose `object` matches under exact-string equality (Phase 5 floor) or vector
//! similarity > threshold. When found, the older row's `valid_until` and the
//! newer row's `superseded_by` are set bi-temporally — both rows remain queryable.

use crate::reforge::writer::ReforgeWriter;
use cognitive::SemanticFactRepo;
use common::{KlyntbotError, Result};
use jiff::Timestamp;

/// Cross-session dedup pass.
#[derive(Debug, Default)]
pub struct CrossSessionDedup;

impl CrossSessionDedup {
    /// Run via vector similarity when an embedder is available; otherwise fall
    /// back to exact-string match.
    pub async fn run(
        repo: &SemanticFactRepo,
        similarity_threshold: f32,
        embedder: Option<&dyn cognitive::TextEmbedder>,
    ) -> Result<u32> {
        match embedder {
            Some(emb) => Self::run_vector_similarity(repo, similarity_threshold, emb).await,
            None => Self::run_test_only_exact_match(repo, similarity_threshold).await,
        }
    }

    /// Vector-similarity dedup using a `TextEmbedder`.
    async fn run_vector_similarity(
        repo: &SemanticFactRepo,
        threshold: f32,
        embedder: &dyn cognitive::TextEmbedder,
    ) -> Result<u32> {
        let pool = repo.pool().clone();
        let cutoff = Timestamp::now()
            .checked_sub(jiff::ToSpan::days(7))
            .unwrap_or(Timestamp::MIN)
            .to_string();
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT id, subject, predicate, object FROM semantic_facts \
             WHERE valid_until IS NULL AND valid_from > ?1",
        )
        .bind(&cutoff)
        .fetch_all(&pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("dedup query: {e}")))?;

        let writer = ReforgeWriter::new();
        let now = Timestamp::now();
        let mut applied = 0_u32;
        let mut processed = std::collections::HashSet::new();

        for (i, (id_a, subj_a, pred_a, obj_a)) in rows.iter().enumerate() {
            if processed.contains(id_a) {
                continue;
            }
            let text_a = format!("{subj_a} {pred_a} {obj_a}");
            let emb_a = embedder.embed(&text_a).await?;
            for (id_b, subj_b, pred_b, obj_b) in rows.iter().skip(i + 1) {
                if processed.contains(id_b) || id_a == id_b {
                    continue;
                }
                let text_b = format!("{subj_b} {pred_b} {obj_b}");
                let emb_b = embedder.embed(&text_b).await?;
                let sim = cosine(&emb_a, &emb_b);
                if sim > threshold {
                    writer.set_superseded_by(repo, id_b, id_a, now).await?;
                    processed.insert(id_a.clone());
                    processed.insert(id_b.clone());
                    applied += 1;
                    break;
                }
            }
        }
        Ok(applied)
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

        let writer = ReforgeWriter::new();
        let mut applied = 0_u32;
        for (older_id, _older_vf, newer_id, newer_vf) in pairs {
            let cutoff = if newer_vf.is_empty() {
                Timestamp::now()
            } else {
                newer_vf
                    .parse::<Timestamp>()
                    .unwrap_or_else(|_| Timestamp::now())
            };
            writer
                .set_superseded_by(repo, &older_id, &newer_id, cutoff)
                .await?;
            applied += 1;
        }
        Ok(applied)
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}
