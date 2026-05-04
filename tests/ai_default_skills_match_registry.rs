//! Invariant test: every `DEFAULT_SKILLS` entry corresponds to a registered feature.
//!
//! `DEFAULT_SKILLS` is hand-edited, but every entry should correspond to a real
//! `AiFeature` (or be explicitly allowlisted as a non-feature skill).

use std::collections::HashSet;

#[test]
fn every_default_skill_filename_corresponds_to_registered_feature_skill() {
    let reg = klyntbot::app_core::init::ai_pipeline::build_feature_registry();
    let registered_skills: HashSet<&'static str> = reg.iter().map(|r| r.skill).collect();

    // Hardcoded DEFAULT_SKILLS filenames (must match crates/skill-system/src/store.rs).
    // The skill-name is the filename minus ".md".
    let default_skill_names: Vec<&str> = vec![
        "task-management",
        "finance-management",
        "automation",
        "notebook",
        "learning",
        "coding-orchestrator",
    ];

    // Skills that are pure orchestrators without a corresponding AiFeature crate.
    // These are valid DEFAULT_SKILLS entries that don't need feature registration.
    let non_feature_skills: HashSet<&str> = ["coding-orchestrator"].into_iter().collect();

    for skill in &default_skill_names {
        if non_feature_skills.contains(skill) {
            continue;
        }
        assert!(
            registered_skills.contains(skill),
            "DEFAULT_SKILLS entry {:?} has no corresponding AiFeature::SKILL in registry. \
             Either add an AiFeature with this skill or remove the skill from DEFAULT_SKILLS.",
            skill
        );
    }
}
