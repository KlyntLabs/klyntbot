use app_core::coding::skills_handler::{SkillInfo, SkillListItem, SkillValidationResult};
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_skills_list() -> Vec<SkillListItem> {
    state
        .coding_skills_list()
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_info(name: String) -> SkillInfo {
    state
        .coding_skills_info(&name)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_install(source: String) -> SkillListItem {
    state
        .coding_skills_install(source)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_update(name: String) -> SkillListItem {
    state
        .coding_skills_update(&name)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_uninstall(name: String) -> () {
    state
        .coding_skills_uninstall(&name)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_toggle(name: String, enabled: bool) -> () {
    state
        .coding_skills_toggle(&name, enabled)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_validate(name: String) -> SkillValidationResult {
    state
        .coding_skills_validate(&name)
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_skills_reload() -> () {
    state
        .coding_skills_reload()
        .await
        .map_err(|e| ApiError::new("SKILL_ERROR", e.to_string()))
}
