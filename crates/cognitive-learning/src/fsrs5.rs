//! FSRS-5 algorithm for spaced-repetition flashcard scheduling.
//!
//! Implements the Free Spaced Repetition Scheduler v5 formulas for computing
//! stability, difficulty, retrievability, and optimal review intervals.
//! This is separate from the cognitive memory decay system in [`super::decay`].

/// Default FSRS-5 weights (19 parameters).
pub const DEFAULT_WEIGHTS: [f64; 19] = [
    0.40255,  // w0:  initial stability for rating 1 (Again)
    1.18385,  // w1:  initial stability for rating 2 (Hard)
    3.173,    // w2:  initial stability for rating 3 (Good)
    15.69105, // w3:  initial stability for rating 4 (Easy)
    7.1949,   // w4:  initial difficulty offset
    0.5345,   // w5:  initial difficulty scaling
    1.4604,   // w6:  difficulty delta per rating deviation
    0.0046,   // w7:  mean reversion weight
    1.54575,  // w8:  success stability: base multiplier
    0.1192,   // w9:  success stability: stability decay exponent
    1.01925,  // w10: success stability: retrievability bonus exponent
    1.9395,   // w11: failure stability: base multiplier
    0.11,     // w12: failure stability: difficulty exponent
    0.29605,  // w13: failure stability: stability exponent
    2.2698,   // w14: failure stability: retrievability exponent
    0.2315,   // w15: hard penalty
    2.9898,   // w16: easy bonus
    0.51655,  // w17: (reserved)
    0.6621,   // w18: (reserved)
];

/// FSRS-5 retrievability: probability of successful recall after `elapsed_days`.
///
/// R = (1 + elapsed_days / (9 * S))^(-1)
///
/// At t=0, R=1.0. At t=S, R≈0.9. Monotonically decreasing.
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    1.0 / (1.0 + elapsed_days / (9.0 * stability))
}

/// Initial stability for a given first-review rating (1..=4).
///
/// S₀(G) = w[G-1], clamped to a minimum of 0.01.
pub fn initial_stability(rating: u8, w: &[f64; 19]) -> f64 {
    let idx = (rating.clamp(1, 4) - 1) as usize;
    w[idx].max(0.01)
}

/// Initial difficulty for a given first-review rating (1..=4).
///
/// D₀(G) = w₄ - exp(w₅ * (G - 1)) + 1, clamped to [1.0, 10.0].
pub fn initial_difficulty(rating: u8, w: &[f64; 19]) -> f64 {
    let g = rating as f64;
    let d = w[4] - (w[5] * (g - 1.0)).exp() + 1.0;
    d.clamp(1.0, 10.0)
}

/// Mean reversion helper: blends `init` toward `current` using weight w₇.
fn mean_revert(init: f64, current: f64, w: &[f64; 19]) -> f64 {
    w[7] * init + (1.0 - w[7]) * current
}

/// Next difficulty after a review with the given rating.
///
/// D' = w₇·D₀(4) + (1-w₇)·(D - w₆·(G-3)), clamped to [1.0, 10.0].
pub fn next_difficulty(current_d: f64, rating: u8, w: &[f64; 19]) -> f64 {
    let g = rating as f64;
    let delta = current_d - w[6] * (g - 3.0);
    let d0_easy = initial_difficulty(4, w);
    mean_revert(d0_easy, delta, w).clamp(1.0, 10.0)
}

/// Next stability after a successful review (rating ∈ {2, 3, 4}).
///
/// S'_r = S · (e^(w₈ · (11-D) · S^(-w₉) · (e^(w₁₀·(1-R)) - 1)) · hard_penalty · easy_bonus + 1)
///
/// Hard penalty = w₁₅ when rating==2, easy bonus = w₁₆ when rating==4.
/// Result is clamped to a minimum of 0.01.
pub fn next_stability_success(s: f64, d: f64, r: f64, rating: u8, w: &[f64; 19]) -> f64 {
    let hard_penalty = if rating == 2 { w[15] } else { 1.0 };
    let easy_bonus = if rating == 4 { w[16] } else { 1.0 };

    let exponent = w[8] * (11.0 - d) * s.powf(-w[9]) * ((w[10] * (1.0 - r)).exp() - 1.0);
    let new_s = s * (exponent.exp() * hard_penalty * easy_bonus + 1.0);
    if new_s.is_infinite() || new_s.is_nan() {
        return s;
    }
    new_s.max(0.01)
}

/// Next stability after a failed review (rating == 1, i.e. "Again").
///
/// S'_f = w₁₁ · D^(-w₁₂) · ((S+1)^w₁₃ - 1) · e^(w₁₄·(1-R))
///
/// Result is clamped to [0.01, s] — failure never increases stability.
pub fn next_stability_failure(s: f64, d: f64, r: f64, w: &[f64; 19]) -> f64 {
    let new_s = w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0) * (w[14] * (1.0 - r)).exp();
    new_s.clamp(0.01, s)
}

/// Compute the next review interval in days for a given stability and desired retention.
///
/// I = S · 9 · (1/R - 1)
///
/// Desired retention is clamped to [0.7, 0.99]. Result is at least 1.0 day, rounded.
pub fn next_interval(stability: f64, desired_retention: f64) -> f64 {
    let r = desired_retention.clamp(0.7, 0.99);
    let interval = stability * 9.0 * (1.0 / r - 1.0);
    interval.max(1.0).round()
}

/// Full review scheduling: computes new stability, new difficulty, and interval.
///
/// Returns `(new_stability, new_difficulty, interval_days)`.
pub fn schedule_review(
    stability: f64,
    difficulty: f64,
    elapsed_days: f64,
    rating: u8,
    desired_retention: f64,
    w: &[f64; 19],
) -> (f64, f64, f64) {
    let r = retrievability(elapsed_days, stability);
    let new_d = next_difficulty(difficulty, rating, w);
    let new_s = if rating == 1 {
        next_stability_failure(stability, difficulty, r, w)
    } else {
        next_stability_success(stability, difficulty, r, rating, w)
    };
    let interval = next_interval(new_s, desired_retention);
    (new_s, new_d, interval)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: [f64; 19] = DEFAULT_WEIGHTS;

    // ── Retrievability ──────────────────────────────────────────────

    #[test]
    fn fsrs5_retrievability_at_zero_is_one() {
        let r = retrievability(0.0, 5.0);
        assert!((r - 1.0).abs() < 1e-9, "R(0, S) should be 1.0, got {r}");
    }

    #[test]
    fn fsrs5_retrievability_at_stability_about_point_nine() {
        // At t = S: R = (1 + S/(9S))^-1 = (1 + 1/9)^-1 = (10/9)^-1 = 0.9
        let r = retrievability(5.0, 5.0);
        assert!(
            (r - 0.9).abs() < 1e-9,
            "R(S, S) should be exactly 0.9, got {r}"
        );
    }

    #[test]
    fn fsrs5_retrievability_decays_over_time() {
        let r1 = retrievability(1.0, 5.0);
        let r10 = retrievability(10.0, 5.0);
        let r30 = retrievability(30.0, 5.0);
        assert!(r1 > r10, "R should decrease: r1={r1}, r10={r10}");
        assert!(r10 > r30, "R should decrease: r10={r10}, r30={r30}");
    }

    #[test]
    fn fsrs5_retrievability_zero_stability() {
        assert_eq!(retrievability(1.0, 0.0), 0.0);
    }

    #[test]
    fn fsrs5_retrievability_negative_stability() {
        assert_eq!(retrievability(1.0, -1.0), 0.0);
    }

    // ── Initial stability ───────────────────────────────────────────

    #[test]
    fn fsrs5_initial_stability_by_rating_ordering() {
        let s1 = initial_stability(1, &W);
        let s2 = initial_stability(2, &W);
        let s3 = initial_stability(3, &W);
        let s4 = initial_stability(4, &W);
        assert!(
            s1 < s2 && s2 < s3 && s3 < s4,
            "S₀ should increase with rating: {s1} < {s2} < {s3} < {s4}"
        );
    }

    #[test]
    fn fsrs5_initial_stability_minimum() {
        // Even with a zero weight, result should be at least 0.01
        let mut w_zero = W;
        w_zero[0] = 0.0;
        assert!((initial_stability(1, &w_zero) - 0.01).abs() < 1e-9);
    }

    // ── Initial difficulty ──────────────────────────────────────────

    #[test]
    fn fsrs5_initial_difficulty_easy_is_lowest() {
        let d1 = initial_difficulty(1, &W);
        let d4 = initial_difficulty(4, &W);
        assert!(
            d4 < d1,
            "Easy (4) should yield lower difficulty than Again (1): d4={d4}, d1={d1}"
        );
    }

    #[test]
    fn fsrs5_initial_difficulty_ordering() {
        let d1 = initial_difficulty(1, &W);
        let d2 = initial_difficulty(2, &W);
        let d3 = initial_difficulty(3, &W);
        let d4 = initial_difficulty(4, &W);
        assert!(
            d1 >= d2 && d2 >= d3 && d3 >= d4,
            "Difficulty should decrease as rating increases: {d1}, {d2}, {d3}, {d4}"
        );
    }

    #[test]
    fn fsrs5_initial_difficulty_clamped() {
        // With extreme weights, difficulty should stay in [1.0, 10.0]
        let mut w_high = W;
        w_high[4] = 100.0; // push difficulty very high
        assert!(initial_difficulty(1, &w_high) <= 10.0);

        let mut w_low = W;
        w_low[4] = -100.0; // push difficulty very low
        assert!(initial_difficulty(4, &w_low) >= 1.0);
    }

    // ── Next difficulty ─────────────────────────────────────────────

    #[test]
    fn fsrs5_next_difficulty_again_increases() {
        let d = 5.0;
        let d_next = next_difficulty(d, 1, &W);
        assert!(
            d_next > d,
            "Again (1) should increase difficulty: {d} -> {d_next}"
        );
    }

    #[test]
    fn fsrs5_next_difficulty_easy_decreases() {
        let d = 5.0;
        let d_next = next_difficulty(d, 4, &W);
        assert!(
            d_next < d,
            "Easy (4) should decrease difficulty: {d} -> {d_next}"
        );
    }

    #[test]
    fn fsrs5_next_difficulty_clamped() {
        // Very low difficulty + easy rating should not go below 1.0
        assert!(next_difficulty(1.0, 4, &W) >= 1.0);
        // Very high difficulty + again rating should not exceed 10.0
        assert!(next_difficulty(10.0, 1, &W) <= 10.0);
    }

    // ── Stability success ───────────────────────────────────────────

    #[test]
    fn fsrs5_stability_success_good_increases() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let new_s = next_stability_success(s, d, r, 3, &W);
        assert!(
            new_s > s,
            "Good review should increase stability: {s} -> {new_s}"
        );
    }

    #[test]
    fn fsrs5_stability_success_hard_penalty() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_hard = next_stability_success(s, d, r, 2, &W);
        let s_good = next_stability_success(s, d, r, 3, &W);
        assert!(
            s_hard < s_good,
            "Hard should yield less stability than Good: hard={s_hard}, good={s_good}"
        );
    }

    #[test]
    fn fsrs5_stability_success_easy_bonus() {
        let s = 5.0;
        let d = 5.0;
        let r = retrievability(5.0, s);
        let s_good = next_stability_success(s, d, r, 3, &W);
        let s_easy = next_stability_success(s, d, r, 4, &W);
        assert!(
            s_easy > s_good,
            "Easy should yield more stability than Good: easy={s_easy}, good={s_good}"
        );
    }

    #[test]
    fn fsrs5_stability_success_minimum() {
        // Even with tiny inputs, result should be at least 0.01
        let new_s = next_stability_success(0.01, 10.0, 1.0, 3, &W);
        assert!(new_s >= 0.01);
    }

    // ── Stability failure ───────────────────────────────────────────

    #[test]
    fn fsrs5_stability_failure_decreases() {
        let s = 10.0;
        let d = 5.0;
        let r = retrievability(10.0, s);
        let new_s = next_stability_failure(s, d, r, &W);
        assert!(
            new_s < s,
            "Failure should decrease stability: {s} -> {new_s}"
        );
    }

    #[test]
    fn fsrs5_stability_failure_stays_positive() {
        let new_s = next_stability_failure(0.5, 10.0, 0.1, &W);
        assert!(
            new_s >= 0.01,
            "Failed stability should be at least 0.01, got {new_s}"
        );
    }

    #[test]
    fn fsrs5_stability_failure_clamped_to_current() {
        let s = 1.0;
        let d = 5.0;
        let r = retrievability(0.0, s); // R=1.0 at t=0
        let new_s = next_stability_failure(s, d, r, &W);
        assert!(
            new_s <= s,
            "Failed stability should not exceed current: {new_s} > {s}"
        );
    }

    // ── Interval ────────────────────────────────────────────────────

    #[test]
    fn fsrs5_interval_at_90_percent_retention() {
        // At R=0.9: I = S * 9 * (1/0.9 - 1) = S * 9 * (1/9) = S
        let interval = next_interval(10.0, 0.9);
        assert!(
            (interval - 10.0).abs() < 1e-9,
            "At 90% retention, interval should equal stability, got {interval}"
        );
    }

    #[test]
    fn fsrs5_interval_always_positive() {
        assert!(next_interval(0.001, 0.99) >= 1.0);
        assert!(next_interval(0.001, 0.7) >= 1.0);
    }

    #[test]
    fn fsrs5_interval_retention_clamped() {
        // Retention below 0.7 should be treated as 0.7
        let i_low = next_interval(10.0, 0.5);
        let i_70 = next_interval(10.0, 0.7);
        assert!(
            (i_low - i_70).abs() < 1e-9,
            "Retention below 0.7 should be clamped: i_low={i_low}, i_70={i_70}"
        );

        // Retention above 0.99 should be treated as 0.99
        let i_high = next_interval(10.0, 1.0);
        let i_99 = next_interval(10.0, 0.99);
        assert!(
            (i_high - i_99).abs() < 1e-9,
            "Retention above 0.99 should be clamped: i_high={i_high}, i_99={i_99}"
        );
    }

    // ── Schedule review (integration) ───────────────────────────────

    #[test]
    fn fsrs5_schedule_review_integration() {
        let s = 5.0;
        let d = 5.0;
        let elapsed = 5.0;
        let desired_r = 0.9;

        // Good review
        let (new_s, new_d, interval) = schedule_review(s, d, elapsed, 3, desired_r, &W);
        assert!(new_s > s, "Good review should increase stability");
        assert!(interval >= 1.0, "Interval should be at least 1 day");
        assert!(
            new_d > 0.0 && new_d <= 10.0,
            "Difficulty should be in range"
        );

        // Again review
        let (new_s_fail, new_d_fail, interval_fail) =
            schedule_review(s, d, elapsed, 1, desired_r, &W);
        assert!(
            new_s_fail < s,
            "Again should decrease stability: {new_s_fail} vs {s}"
        );
        assert!(
            interval_fail < interval,
            "Again interval should be shorter: {interval_fail} vs {interval}"
        );
        assert!(new_d_fail > new_d, "Again should increase difficulty");
    }

    #[test]
    fn fsrs5_schedule_review_rating_ordering() {
        let s = 5.0;
        let d = 5.0;
        let elapsed = 5.0;
        let desired_r = 0.9;

        let (s1, _, i1) = schedule_review(s, d, elapsed, 1, desired_r, &W);
        let (s2, _, i2) = schedule_review(s, d, elapsed, 2, desired_r, &W);
        let (s3, _, i3) = schedule_review(s, d, elapsed, 3, desired_r, &W);
        let (s4, _, i4) = schedule_review(s, d, elapsed, 4, desired_r, &W);

        assert!(
            s1 < s2 && s2 < s3 && s3 < s4,
            "Stability should increase with rating: {s1}, {s2}, {s3}, {s4}"
        );
        assert!(
            i1 <= i2 && i2 <= i3 && i3 <= i4,
            "Interval should increase with rating: {i1}, {i2}, {i3}, {i4}"
        );
    }
}
