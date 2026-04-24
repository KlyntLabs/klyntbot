//! FSRS write-back bridge — persists FSRS-5 parameters into the cognitive
//! `fsrs_parameters` table.
//!
//! Two concerns:
//! - `apply_fsrs_writeback` — commits the single `desired_retention` knob from
//!   a promoted autotuner trial. Runs every time a trial is promoted.
//! - `train_fsrs_weights` — runs the full 19-weight optimiser against the
//!   user's `review_log`. Holdout-validated; only persists when the trained
//!   weights beat the currently-persisted weights on unseen data. Intended
//!   for a weekly cron job, not for per-trial invocation.

use common::TrialParams;
use storage::StoragePool;
use tracing::{info, warn};

use cognitive::services::fsrs_optimizer::{self, OptimiserConfig};

/// Write `fsrs_desired_retention` from promoted trial params into the cognitive
/// `fsrs_parameters` table.
///
/// No-op when the param is `None`.
pub async fn apply_fsrs_writeback(
    repo: &cognitive::FsrsParamsRepo,
    params: &TrialParams,
) -> common::Result<()> {
    if let Some(retention) = params.fsrs_desired_retention {
        repo.update_desired_retention(retention).await?;
    }
    Ok(())
}

/// Train the 19 FSRS-5 weights against the user's review history and persist
/// them if the held-out loss improves over the currently-stored weights.
///
/// Returns `Ok(true)` when new weights were written, `Ok(false)` when training
/// ran but the baseline was not beaten (or there was not enough data), and an
/// error only when the underlying SQL write fails.
pub async fn train_fsrs_weights(
    pool: &StoragePool,
    repo: &cognitive::FsrsParamsRepo,
) -> common::Result<bool> {
    let baseline = repo.get_weights().await?;
    let config = OptimiserConfig::default();
    let outcome = fsrs_optimizer::optimize_weights(pool, &baseline, &config).await?;
    let Some(outcome) = outcome else {
        info!("fsrs weight training skipped: insufficient review history");
        return Ok(false);
    };
    info!(
        reviews = outcome.reviews_used,
        cards = outcome.cards_used,
        baseline_loss = outcome.baseline_holdout_loss,
        trained_loss = outcome.trained_holdout_loss,
        improved = outcome.improved,
        "fsrs weight training complete"
    );
    if !outcome.improved {
        warn!(
            "fsrs trained weights did not beat baseline on holdout — keeping current weights"
        );
        return Ok(false);
    }
    repo.update_weights(&outcome.trained_weights).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive::services::fsrs5::{
        initial_difficulty, initial_stability, next_difficulty, next_stability_failure,
        next_stability_success, retrievability,
    };

    /// Seed `review_log` with enough synthetic history to clear the optimiser
    /// gate. Generates deterministic review sequences under a given ground-truth
    /// weight vector — reviews succeed when predicted retrievability ≥ 0.5.
    async fn seed_reviews(pool: &StoragePool, card_count: usize, reviews_per_card: usize, w_true: &[f64; 19]) {
        let migrations = cognitive::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO knowledge_atoms (id, subject, atom_type, domain, created_at, updated_at)
             VALUES ('a_seed', 's', 'concept', 'd', datetime('now'), datetime('now'))",
        )
        .execute(pool.inner())
        .await
        .unwrap();

        for c in 0..card_count {
            let card_id = format!("card{c}");
            sqlx::query(
                "INSERT INTO flashcards (id, atom_id, front, back, stability, difficulty, state, created_at, updated_at)
                 VALUES (?, 'a_seed', 'f', 'b', 1.0, 5.0, 'new', datetime('now'), datetime('now'))",
            )
            .bind(&card_id)
            .execute(pool.inner())
            .await
            .unwrap();

            let first_rating: u8 = ((c % 3) + 2) as u8; // 2..=4
            let mut s = initial_stability(first_rating, w_true);
            let mut d = initial_difficulty(first_rating, w_true);
            for i in 0..reviews_per_card {
                let elapsed = if i == 0 { 0.0 } else { (i as f64) * 2.0 };
                let rating: u8 = if i == 0 {
                    first_rating
                } else {
                    let r = retrievability(elapsed, s);
                    if r >= 0.5 {
                        3
                    } else {
                        1
                    }
                };
                // Stagger timestamps so ORDER BY reviewed_at is stable.
                let ts = format!("2026-04-{:02}T{:02}:00:00Z", (c % 28) + 1, i % 24);
                sqlx::query(
                    "INSERT INTO review_log (id, card_id, rating, elapsed_days, scheduled_days, state, reviewed_at)
                     VALUES (?, ?, ?, ?, ?, 'Review', ?)",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&card_id)
                .bind(rating as i64)
                .bind(elapsed)
                .bind(elapsed.max(1.0))
                .bind(&ts)
                .execute(pool.inner())
                .await
                .unwrap();
                if i > 0 {
                    let r = retrievability(elapsed, s);
                    let new_s = if rating == 1 {
                        next_stability_failure(s, d, r, w_true)
                    } else {
                        next_stability_success(s, d, r, rating, w_true)
                    };
                    d = next_difficulty(d, rating, w_true);
                    s = new_s;
                }
            }
        }
    }

    #[tokio::test]
    async fn train_fsrs_weights_persists_when_improved() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let baseline = cognitive::services::fsrs_optimizer::BASELINE_WEIGHTS;
        // Seed under shifted ground truth so baseline has room to improve.
        let mut w_true = baseline;
        w_true[8] += 0.3;
        w_true[11] -= 0.2;
        seed_reviews(&pool, 30, 10, &w_true).await;

        let repo = cognitive::FsrsParamsRepo::new(pool.clone());
        let before = repo.get_weights().await.unwrap();

        let persisted = train_fsrs_weights(&pool, &repo).await.unwrap();

        let after = repo.get_weights().await.unwrap();
        if persisted {
            let changed = (0..19).any(|i| (before[i] - after[i]).abs() > 1e-6);
            assert!(changed, "persisted=true implies weights should change");
        } else {
            for i in 0..19 {
                assert!(
                    (before[i] - after[i]).abs() < 1e-9,
                    "persisted=false implies weights unchanged at i={i}"
                );
            }
        }
    }

    #[tokio::test]
    async fn train_fsrs_weights_noop_on_insufficient_data() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = cognitive::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();
        let repo = cognitive::FsrsParamsRepo::new(pool.clone());
        let before = repo.get_weights().await.unwrap();

        let persisted = train_fsrs_weights(&pool, &repo).await.unwrap();
        assert!(!persisted, "no history should result in no-op");

        let after = repo.get_weights().await.unwrap();
        for i in 0..19 {
            assert!((before[i] - after[i]).abs() < 1e-9);
        }
    }
}
