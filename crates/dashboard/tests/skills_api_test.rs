//! Integration tests for /api/skills endpoints.
//!
//! Covers AC-13.x: List skills, toggle enabled/disabled.

mod common;

#[tokio::test]
async fn get_skills_returns_list() {
    // AC-13.1: GET /api/skills returns Vec<Skill>
    // In test context, expects at least the built-in skills to be present.
    // TODO: implement
}

#[tokio::test]
async fn get_skills_includes_name_and_description() {
    // AC-13.1: Each skill entry has at minimum a "name" and "description" field
    // TODO: implement
}

#[tokio::test]
async fn patch_skill_toggle_enabled() {
    // AC-13.2: PATCH /api/skills/{name} { "enabled": true } → 200
    // Skill appears as enabled in subsequent GET /api/skills
    // TODO: implement
}

#[tokio::test]
async fn patch_skill_toggle_disabled() {
    // AC-13.2: PATCH /api/skills/{name} { "enabled": false } → 200
    // Skill appears as disabled in subsequent GET /api/skills
    // TODO: implement
}

#[tokio::test]
async fn patch_skill_not_found_returns_404() {
    // AC-13.2: PATCH /api/skills/nonexistent → 404
    // TODO: implement
}
