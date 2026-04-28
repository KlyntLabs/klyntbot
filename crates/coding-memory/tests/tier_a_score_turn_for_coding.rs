//! Tier A activation: score_turn must classify coding-verb turns as high-value.

use cognitive::services::value_density::score_turn;

#[test]
fn deployed_keyword_scores_high() {
    let s = score_turn("deployed the authentication service to staging", Some(&[]));
    assert!(s.total >= 0.7, "expected >=0.7, got {}", s.total);
}

#[test]
fn refactored_keyword_scores_high() {
    let s = score_turn("refactored the user repo to use storage abstraction", Some(&[]));
    assert!(s.total >= 0.7);
}

#[test]
fn idle_chitchat_scores_low() {
    let s = score_turn("how was your day?", Some(&[]));
    assert!(s.total < 0.4);
}
