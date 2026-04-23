use ai_core::RecallDomain;
use std::collections::HashMap;

#[test]
fn tasks_override_beats_global() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Tasks, n);
    }

    let global = 5usize;
    let effective_tasks = effective_threshold(&RecallDomain::Tasks, &overrides, global);
    assert_eq!(effective_tasks, 3);

    let effective_general = effective_threshold(&RecallDomain::General, &HashMap::new(), global);
    assert_eq!(effective_general, global);
}

#[test]
fn finance_override_is_lower_still() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_finance::FinanceFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Finance, n);
    }
    let effective = effective_threshold(&RecallDomain::Finance, &overrides, 5);
    assert_eq!(effective, 2);
}

fn effective_threshold(
    domain: &RecallDomain,
    overrides: &HashMap<RecallDomain, usize>,
    global: usize,
) -> usize {
    overrides.get(domain).copied().unwrap_or(global)
}
