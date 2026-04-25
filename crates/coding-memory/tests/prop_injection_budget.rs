use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn truncate_to_never_exceeds_budget(
        s in "\\PC{0,5000}",
        budget in 1usize..3000,
    ) {
        let b = HeuristicBudgeter;
        let out = b.truncate_to(&s, budget);
        prop_assert!(b.count(&out) <= budget + 1, "got {} for budget {}", b.count(&out), budget);
    }
}
