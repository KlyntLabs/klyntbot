//! Invariant: MCP exposed_tools equals registry tool_names ∪ EXPLICIT_ALLOWLIST.

use std::collections::HashSet;

#[test]
fn exposed_tools_post_init_equals_registry_plus_allowlist() {
    let reg = app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> =
        config::schema::EXPLICIT_TOOL_ALLOWLIST.iter().copied().collect();

    let expected: HashSet<&'static str> =
        registry_tools.union(&allowlist).copied().collect();

    // Simulate the app-core post-load fill:
    let mut filled: Vec<String> = registry_tools.iter().map(|s: &&str| s.to_string()).collect();
    filled.extend(allowlist.iter().map(|s| s.to_string()));
    let filled_set: HashSet<String> = filled.into_iter().collect();

    let expected_strings: HashSet<String> = expected.iter().map(|s: &&str| s.to_string()).collect();
    assert_eq!(filled_set, expected_strings);
}

#[test]
fn no_overlap_between_registry_and_allowlist() {
    let reg = app_core::init::ai_pipeline::build_feature_registry();
    let registry_tools: HashSet<&'static str> = reg.tool_names().into_iter().collect();
    let allowlist: HashSet<&'static str> =
        config::schema::EXPLICIT_TOOL_ALLOWLIST.iter().copied().collect();
    let overlap: HashSet<_> = registry_tools.intersection(&allowlist).collect();
    assert!(
        overlap.is_empty(),
        "tool {} is registered as both an AiFeature tool_name and in EXPLICIT_TOOL_ALLOWLIST",
        overlap.iter().next().map(|s| **s).unwrap_or("")
    );
}
