//! Phase 5 — skill evolver drafts SKILL.md via `ProviderRole::ReforgeRules`.

#[test]
fn skills_uses_reforge_rules_role() {
    let src = include_str!("../src/skills.rs");
    assert!(
        src.contains("ProviderRole::ReforgeRules"),
        "skills.rs should call provider with ProviderRole::ReforgeRules"
    );
    assert!(
        src.contains("draft_skill_md"),
        "skills.rs should expose draft_skill_md"
    );
}
