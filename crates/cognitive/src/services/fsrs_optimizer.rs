//! FSRS-5 weight optimizer.
//!
//! Trains the 19 FSRS-5 weights against the user's `review_log` by minimising
//! binary cross-entropy between predicted retrievability and observed
//! success/failure. Intended to run on a scheduled cadence (e.g. weekly) once
//! the user has accumulated enough review history.
//!
//! Pipeline:
//!   1. Load per-card review sequences ordered by `reviewed_at`.
//!   2. Forward-simulate each sequence under candidate weights `w`, computing
//!      predicted retrievability at every review after the first.
//!   3. Aggregate binary cross-entropy against observed success (rating >= 2).
//!   4. Adam optimiser with numerical (finite-difference) gradients.
//!   5. Chronological 80/20 holdout validation — persist only when holdout
//!      loss improves over the baseline (current weights).
//!
//! The forward simulator (`super::fsrs5`) is reused as-is. This module adds
//! only the loss, gradient, and optimiser on top.

use common::Result;
use storage::StoragePool;

use super::fsrs5::{
    initial_difficulty, initial_stability, next_difficulty, next_stability_failure,
    next_stability_success, retrievability, DEFAULT_WEIGHTS,
};

/// One card's chronological review sequence as `(rating, elapsed_days)` pairs.
#[derive(Debug, Clone)]
pub struct ReviewSequence {
    pub reviews: Vec<(u8, f64)>,
}

/// Holdout entry: full review history plus an index marking where the train
/// window ended. The forward simulator replays `[0, score_from)` to establish
/// state, then scores loss only on `[score_from..]`. This is the honest
/// measurement — predictions on reviews the optimiser never saw.
#[derive(Debug, Clone)]
struct HoldoutSequence {
    reviews: Vec<(u8, f64)>,
    score_from: usize,
}

/// Result of a training attempt.
#[derive(Debug, Clone)]
pub struct TrainingOutcome {
    pub trained_weights: [f64; 19],
    pub baseline_holdout_loss: f64,
    pub trained_holdout_loss: f64,
    pub reviews_used: usize,
    pub cards_used: usize,
    pub improved: bool,
}

/// Optimiser hyper-parameters. Defaults are conservative — favour no-op over
/// fitting noise on small datasets.
#[derive(Debug, Clone)]
pub struct OptimiserConfig {
    pub learning_rate: f64,
    pub max_epochs: usize,
    pub early_stop_eps: f64,
    pub min_total_reviews: usize,
    pub min_cards_with_two_plus_reviews: usize,
    pub holdout_fraction: f64,
    pub finite_diff_h_relative: f64,
}

impl Default for OptimiserConfig {
    fn default() -> Self {
        Self {
            learning_rate: 4e-2,
            max_epochs: 30,
            early_stop_eps: 1e-5,
            min_total_reviews: 200,
            min_cards_with_two_plus_reviews: 20,
            holdout_fraction: 0.20,
            finite_diff_h_relative: 1e-4,
        }
    }
}

/// FSRS-5 weight bounds. Conservative ranges from the FSRS community reference
/// implementation; keeps the optimiser inside physically-meaningful territory.
const WEIGHT_LOWER: [f64; 19] = [
    0.001, 0.001, 0.001, 0.001, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.001, 0.001, 0.001, 0.001, 0.0, 0.0,
    1.0, 0.0, 0.0,
];
const WEIGHT_UPPER: [f64; 19] = [
    100.0, 100.0, 100.0, 100.0, 10.0, 4.0, 4.0, 0.75, 4.5, 0.8, 3.5, 5.0, 0.25, 0.9, 4.0, 1.0, 6.0,
    2.0, 2.0,
];

/// Top-level entry: load review history, attempt training, return the trained
/// weights only if holdout loss improves over the baseline.
pub async fn optimize_weights(
    pool: &StoragePool,
    baseline_weights: &[f64; 19],
    config: &OptimiserConfig,
) -> Result<Option<TrainingOutcome>> {
    let sequences = load_review_sequences(pool).await?;
    Ok(train(&sequences, baseline_weights, config))
}

/// Pure training entry point — exposed for unit tests with synthetic data.
pub fn train(
    sequences: &[ReviewSequence],
    baseline_weights: &[f64; 19],
    config: &OptimiserConfig,
) -> Option<TrainingOutcome> {
    let total_reviews: usize = sequences.iter().map(|s| s.reviews.len()).sum();
    let cards_with_two_plus = sequences.iter().filter(|s| s.reviews.len() >= 2).count();

    if total_reviews < config.min_total_reviews
        || cards_with_two_plus < config.min_cards_with_two_plus_reviews
    {
        return None;
    }

    let (train_set, holdout_set) = chronological_split(sequences, config.holdout_fraction);

    let baseline_holdout_loss = holdout_loss(&holdout_set, baseline_weights);

    let mut w = *baseline_weights;
    let mut m = [0.0; 19];
    let mut v = [0.0; 19];
    let mut prev_loss = f64::INFINITY;

    for t in 1..=config.max_epochs {
        let loss = sequence_loss(&train_set, &w);
        if !loss.is_finite() {
            break;
        }
        if (prev_loss - loss).abs() < config.early_stop_eps {
            break;
        }
        prev_loss = loss;

        let grad = finite_diff_gradient(&train_set, &w, config.finite_diff_h_relative);
        adam_step(&mut w, &grad, &mut m, &mut v, t, config.learning_rate);
        clamp_weights(&mut w);
    }

    let trained_holdout_loss = holdout_loss(&holdout_set, &w);
    let improved = trained_holdout_loss.is_finite()
        && baseline_holdout_loss.is_finite()
        && trained_holdout_loss < baseline_holdout_loss;

    Some(TrainingOutcome {
        trained_weights: w,
        baseline_holdout_loss,
        trained_holdout_loss,
        reviews_used: total_reviews,
        cards_used: cards_with_two_plus,
        improved,
    })
}

async fn load_review_sequences(pool: &StoragePool) -> Result<Vec<ReviewSequence>> {
    let rows: Vec<(String, i64, f64)> = sqlx::query_as(
        "SELECT card_id, rating, elapsed_days
         FROM review_log
         ORDER BY card_id, reviewed_at",
    )
    .fetch_all(pool.inner())
    .await?;

    let mut sequences: Vec<ReviewSequence> = Vec::new();
    let mut current_card: Option<String> = None;

    for (card_id, rating, elapsed_days) in rows {
        let rating = rating.clamp(1, 4) as u8;
        match &current_card {
            Some(id) if id == &card_id => {
                sequences
                    .last_mut()
                    .unwrap()
                    .reviews
                    .push((rating, elapsed_days));
            }
            _ => {
                current_card = Some(card_id);
                sequences.push(ReviewSequence {
                    reviews: vec![(rating, elapsed_days)],
                });
            }
        }
    }
    Ok(sequences)
}

/// Chronological split — the train set carries only the first `1 - holdout_fraction`
/// of each sequence. The holdout carries the full sequence plus a `score_from`
/// index; the forward sim replays the training prefix to rebuild state, then
/// scores loss only on the held-out tail.
///
/// Sequences shorter than 2 reviews are dropped (no prediction possible).
fn chronological_split(
    sequences: &[ReviewSequence],
    holdout_fraction: f64,
) -> (Vec<ReviewSequence>, Vec<HoldoutSequence>) {
    let mut train = Vec::with_capacity(sequences.len());
    let mut hold = Vec::with_capacity(sequences.len());
    for seq in sequences {
        if seq.reviews.len() < 2 {
            continue;
        }
        let n = seq.reviews.len();
        let hold_count = ((n as f64) * holdout_fraction).round() as usize;
        let hold_count = hold_count.clamp(1, n.saturating_sub(1));
        let split_idx = n - hold_count;
        train.push(ReviewSequence {
            reviews: seq.reviews[..split_idx].to_vec(),
        });
        hold.push(HoldoutSequence {
            reviews: seq.reviews.clone(),
            score_from: split_idx,
        });
    }
    (train, hold)
}

/// Aggregate binary cross-entropy across all sequences. The first review in
/// each sequence has no predictor and is skipped.
fn sequence_loss(sequences: &[ReviewSequence], w: &[f64; 19]) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for seq in sequences {
        if seq.reviews.len() < 2 {
            continue;
        }
        let (mut s, mut d) = initial_state(seq.reviews[0].0, w);
        for &(rating, elapsed) in &seq.reviews[1..] {
            let r_pred = retrievability(elapsed, s);
            let y = if rating >= 2 { 1.0 } else { 0.0 };
            total += binary_cross_entropy(y, r_pred);
            count += 1;
            let r_at_review = r_pred;
            let new_s = if rating == 1 {
                next_stability_failure(s, d, r_at_review, w)
            } else {
                next_stability_success(s, d, r_at_review, rating, w)
            };
            d = next_difficulty(d, rating, w);
            s = new_s;
        }
    }
    if count == 0 {
        return f64::INFINITY;
    }
    total / count as f64
}

fn initial_state(first_rating: u8, w: &[f64; 19]) -> (f64, f64) {
    (initial_stability(first_rating, w), initial_difficulty(first_rating, w))
}

/// Holdout loss: replays `[0, score_from)` to rebuild per-card state, then
/// aggregates binary cross-entropy only over reviews at index `>= score_from`.
fn holdout_loss(sequences: &[HoldoutSequence], w: &[f64; 19]) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for seq in sequences {
        if seq.reviews.len() < 2 || seq.score_from == 0 {
            continue;
        }
        let (mut s, mut d) = initial_state(seq.reviews[0].0, w);
        for (i, &(rating, elapsed)) in seq.reviews.iter().enumerate().skip(1) {
            let r_pred = retrievability(elapsed, s);
            if i >= seq.score_from {
                let y = if rating >= 2 { 1.0 } else { 0.0 };
                total += binary_cross_entropy(y, r_pred);
                count += 1;
            }
            let new_s = if rating == 1 {
                next_stability_failure(s, d, r_pred, w)
            } else {
                next_stability_success(s, d, r_pred, rating, w)
            };
            d = next_difficulty(d, rating, w);
            s = new_s;
        }
    }
    if count == 0 {
        return f64::INFINITY;
    }
    total / count as f64
}

fn binary_cross_entropy(y: f64, p: f64) -> f64 {
    let eps = 1e-9;
    let p = p.clamp(eps, 1.0 - eps);
    -(y * p.ln() + (1.0 - y) * (1.0 - p).ln())
}

fn finite_diff_gradient(
    sequences: &[ReviewSequence],
    w: &[f64; 19],
    h_relative: f64,
) -> [f64; 19] {
    let mut grad = [0.0; 19];
    for i in 0..19 {
        let h = (w[i].abs().max(1.0)) * h_relative;
        let mut wp = *w;
        let mut wm = *w;
        wp[i] += h;
        wm[i] -= h;
        let lp = sequence_loss(sequences, &wp);
        let lm = sequence_loss(sequences, &wm);
        let g = (lp - lm) / (2.0 * h);
        grad[i] = if g.is_finite() { g } else { 0.0 };
    }
    grad
}

fn adam_step(
    w: &mut [f64; 19],
    grad: &[f64; 19],
    m: &mut [f64; 19],
    v: &mut [f64; 19],
    t: usize,
    lr: f64,
) {
    let b1 = 0.9_f64;
    let b2 = 0.999_f64;
    let eps = 1e-8_f64;
    let b1_t = b1.powi(t as i32);
    let b2_t = b2.powi(t as i32);
    for i in 0..19 {
        m[i] = b1 * m[i] + (1.0 - b1) * grad[i];
        v[i] = b2 * v[i] + (1.0 - b2) * grad[i] * grad[i];
        let m_hat = m[i] / (1.0 - b1_t);
        let v_hat = v[i] / (1.0 - b2_t);
        w[i] -= lr * m_hat / (v_hat.sqrt() + eps);
    }
}

fn clamp_weights(w: &mut [f64; 19]) {
    for i in 0..19 {
        w[i] = w[i].clamp(WEIGHT_LOWER[i], WEIGHT_UPPER[i]);
    }
}

/// Re-export for callers that want to construct a `Default`-less baseline.
pub const BASELINE_WEIGHTS: [f64; 19] = DEFAULT_WEIGHTS;

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward-simulate under weights `w_true` and return synthetic review
    /// sequences. Ratings are deterministic: success (3) when predicted R ≥ 0.7,
    /// failure (1) otherwise. Not randomised — the test is about whether the
    /// optimiser can recover the weights that produced the data.
    fn synth_sequences(
        card_count: usize,
        reviews_per_card: usize,
        w_true: &[f64; 19],
    ) -> Vec<ReviewSequence> {
        let mut out = Vec::with_capacity(card_count);
        for c in 0..card_count {
            let mut reviews = Vec::with_capacity(reviews_per_card);
            let first_rating: u8 = ((c % 3) + 2) as u8; // 2..=4
            reviews.push((first_rating, 0.0));
            let (mut s, mut d) = initial_state(first_rating, w_true);
            for i in 1..reviews_per_card {
                let elapsed = (i as f64) * 2.0;
                let r = retrievability(elapsed, s);
                let rating: u8 = if r >= 0.7 { 3 } else { 1 };
                reviews.push((rating, elapsed));
                let r_now = r;
                let new_s = if rating == 1 {
                    next_stability_failure(s, d, r_now, w_true)
                } else {
                    next_stability_success(s, d, r_now, rating, w_true)
                };
                d = next_difficulty(d, rating, w_true);
                s = new_s;
            }
            out.push(ReviewSequence { reviews });
        }
        out
    }

    #[test]
    fn sequence_loss_is_finite_and_positive() {
        let seqs = synth_sequences(10, 8, &DEFAULT_WEIGHTS);
        let loss = sequence_loss(&seqs, &DEFAULT_WEIGHTS);
        assert!(loss.is_finite() && loss > 0.0, "loss={loss}");
    }

    #[test]
    fn gate_refuses_too_few_reviews() {
        let seqs = synth_sequences(5, 3, &DEFAULT_WEIGHTS);
        let out = train(&seqs, &DEFAULT_WEIGHTS, &OptimiserConfig::default());
        assert!(out.is_none(), "must refuse to train on too-small dataset");
    }

    #[test]
    fn gate_accepts_with_enough_reviews() {
        let seqs = synth_sequences(30, 10, &DEFAULT_WEIGHTS);
        let total: usize = seqs.iter().map(|s| s.reviews.len()).sum();
        assert!(total >= 200, "test setup sanity: total={total}");
        let cfg = OptimiserConfig {
            max_epochs: 2,
            ..Default::default()
        };
        let out = train(&seqs, &DEFAULT_WEIGHTS, &cfg);
        assert!(out.is_some(), "should run training on adequate dataset");
    }

    #[test]
    fn optimizer_reduces_train_loss_from_perturbed_start() {
        let w_true = DEFAULT_WEIGHTS;
        let mut w_start = DEFAULT_WEIGHTS;
        w_start[0] += 0.5;
        w_start[8] += 0.3;
        let seqs = synth_sequences(30, 10, &w_true);
        let initial_loss = sequence_loss(&seqs, &w_start);
        let cfg = OptimiserConfig {
            max_epochs: 10,
            learning_rate: 1e-2,
            ..Default::default()
        };
        let out = train(&seqs, &w_start, &cfg).expect("should train");
        let final_loss = sequence_loss(&seqs, &out.trained_weights);
        assert!(
            final_loss <= initial_loss,
            "training should not worsen loss: {initial_loss} -> {final_loss}"
        );
    }

    #[test]
    fn clamp_keeps_weights_in_bounds() {
        let mut w = DEFAULT_WEIGHTS;
        w[4] = 1000.0;
        w[7] = -1.0;
        clamp_weights(&mut w);
        for i in 0..19 {
            assert!(
                w[i] >= WEIGHT_LOWER[i] && w[i] <= WEIGHT_UPPER[i],
                "weight {i} out of bounds: {}",
                w[i]
            );
        }
    }

    #[test]
    fn adam_step_moves_against_gradient() {
        let mut w = [1.0_f64; 19];
        let grad = [1.0_f64; 19];
        let mut m = [0.0; 19];
        let mut v = [0.0; 19];
        adam_step(&mut w, &grad, &mut m, &mut v, 1, 0.1);
        for i in 0..19 {
            assert!(w[i] < 1.0, "Adam step with positive grad must decrease w[{i}]");
        }
    }

    #[tokio::test]
    async fn load_review_sequences_groups_by_card() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO knowledge_atoms (id, subject, atom_type, domain, created_at, updated_at)
             VALUES ('a1', 's', 'concept', 'd', datetime('now'), datetime('now'))",
        )
        .execute(pool.inner())
        .await
        .unwrap();
        for cid in ["cardA", "cardB"] {
            sqlx::query(
                "INSERT INTO flashcards (id, atom_id, front, back, stability, difficulty, state, created_at, updated_at)
                 VALUES (?, 'a1', 'f', 'b', 1.0, 5.0, 'new', datetime('now'), datetime('now'))",
            )
            .bind(cid)
            .execute(pool.inner())
            .await
            .unwrap();
        }

        for (card, rating, elapsed, ts) in [
            ("cardA", 3_i64, 0.0_f64, "2026-04-01T00:00:00Z"),
            ("cardA", 3, 2.0, "2026-04-03T00:00:00Z"),
            ("cardB", 4, 0.0, "2026-04-02T00:00:00Z"),
            ("cardA", 1, 5.0, "2026-04-08T00:00:00Z"),
        ] {
            sqlx::query(
                "INSERT INTO review_log (id, card_id, rating, elapsed_days, scheduled_days, state, reviewed_at)
                 VALUES (?, ?, ?, ?, ?, 'Review', ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(card)
            .bind(rating)
            .bind(elapsed)
            .bind(elapsed.max(1.0))
            .bind(ts)
            .execute(pool.inner())
            .await
            .unwrap();
        }

        let seqs = load_review_sequences(&pool).await.unwrap();
        assert_eq!(seqs.len(), 2, "two cards");
        let card_a = seqs.iter().find(|s| s.reviews.len() == 3).expect("cardA has 3 reviews");
        assert_eq!(card_a.reviews[0].0, 3);
        assert_eq!(card_a.reviews[2].0, 1);
    }
}
