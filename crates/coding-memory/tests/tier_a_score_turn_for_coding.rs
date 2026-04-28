//! Tier A activation: score_turn must classify coding-verb turns as high-value.

use cognitive::services::value_density::score_turn;

#[test]
fn deployed_keyword_scores_high() {
    // Capitalized entity names boost the score above medium threshold.
    let s = score_turn("deployed the Authentication Service to Staging", Some(&[]));
    assert!(s.total >= 0.4, "expected >=0.4, got {}", s.total);
}

#[test]
fn refactored_keyword_scores_high() {
    let s = score_turn(
        "refactored the User Repo to use Storage Abstraction",
        Some(&[]),
    );
    assert!(s.total >= 0.4, "expected >=0.4, got {}", s.total);
}

#[test]
fn idle_chitchat_scores_low() {
    let s = score_turn("how was your day?", Some(&[]));
    assert!(s.total < 0.4);
}
