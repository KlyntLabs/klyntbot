use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[test]
fn heuristic_count_is_chars_over_four() {
    let b = HeuristicBudgeter;
    let n = b.count("abcdefghij"); // 10 chars
    assert!((2..=3).contains(&n), "got {n}");
}

#[test]
fn truncate_at_budget_keeps_under_cap() {
    let b = HeuristicBudgeter;
    let long = "x".repeat(10_000);
    let out = b.truncate_to(&long, 100);
    assert!(b.count(&out) <= 100);
}

#[test]
fn truncate_preserves_short_input() {
    let b = HeuristicBudgeter;
    let s = "hello world";
    assert_eq!(b.truncate_to(s, 100), s);
}
