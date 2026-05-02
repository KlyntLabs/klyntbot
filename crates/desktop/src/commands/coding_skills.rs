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

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_skills_list" => dev::val(
            core.coding_skills_list()
                .await
                .map_err(desktop_shared::errors::ApiError::from),
        ),
        "coding_skills_info" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(
                core.coding_skills_info(&name)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_install" => {
            let source = try_field!(dev::get_str(body, "source"));
            dev::val(
                core.coding_skills_install(source)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_update" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(
                core.coding_skills_update(&name)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_uninstall" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(
                core.coding_skills_uninstall(&name)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_toggle" => {
            let name = try_field!(dev::get_str(body, "name"));
            let enabled: bool = try_field!(dev::require(body, "enabled"));
            dev::val(
                core.coding_skills_toggle(&name, enabled)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_validate" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(
                core.coding_skills_validate(&name)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        "coding_skills_reload" => dev::val(
            core.coding_skills_reload()
                .await
                .map_err(desktop_shared::errors::ApiError::from),
        ),
        _ => return None,
    })
}
