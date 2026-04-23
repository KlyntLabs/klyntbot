use std::collections::HashMap;
use ai_core::RecallDomain;

#[test]
fn tasks_override_beats_global() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Tasks, n);
    }

    let global = 5usize;
    let effective_tasks =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::Tasks,
            &overrides,
            global,
        );
    assert_eq!(effective_tasks, 3);

    let effective_general =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::General,
            &HashMap::new(),
            global,
        );
    assert_eq!(effective_general, global);
}

#[test]
fn finance_override_is_lower_still() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_finance::FinanceFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Finance, n);
    }
    let effective =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::Finance,
            &overrides,
            5,
        );
    assert_eq!(effective, 2);
}
