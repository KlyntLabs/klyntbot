use desktop_shared::commands::reforge::*;
use desktop_shared::{errors::ApiError, CommandResult};

use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn reforge_state() -> ReforgeStateResponse {
    state.reforge_state().await
}

#[klynt_command]
pub async fn reforge_skill_names() -> SkillListResponse {
    state.skill_names().await
}

#[klynt_command]
pub async fn reforge_skill_versions(
    skill_name: String,
) -> Vec<SkillVersionResponse> {
    state.skill_version_list(&skill_name).await
}

#[klynt_command]
pub async fn reforge_skill_version_detail(
    skill_name: String,
    version: i64,
) -> Vec<SkillVersionDetailResponse> {
    state.skill_version_detail(&skill_name, version).await
}

#[klynt_command]
pub async fn reforge_skill_reset(
    skill_name: String,
    file_path: String,
    version: i64,
) -> () {
    state
        .skill_version_reset(&skill_name, &file_path, version)
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};

    Some(match cmd {
        "reforge_state" => dev::val(core.reforge_state().await),
        "reforge_skill_names" => dev::val(core.skill_names().await),
        "reforge_skill_versions" => {
            let skill_name = try_field!(
                dev::get_str(body, "skill_name").or_else(|_| dev::get_str(body, "skillName"))
            );
            dev::val(core.skill_version_list(&skill_name).await)
        }
        "reforge_skill_version_detail" => {
            let skill_name = try_field!(
                dev::get_str(body, "skill_name").or_else(|_| dev::get_str(body, "skillName"))
            );
            let version: i64 = try_field!(dev::get(body, "version")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: version")));
            dev::val(core.skill_version_detail(&skill_name, version).await)
        }
        "reforge_skill_reset" => {
            let skill_name = try_field!(
                dev::get_str(body, "skill_name").or_else(|_| dev::get_str(body, "skillName"))
            );
            let file_path = try_field!(
                dev::get_str(body, "file_path").or_else(|_| dev::get_str(body, "filePath"))
            );
            let version: i64 = try_field!(dev::get(body, "version")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: version")));
            dev::val(
                core.skill_version_reset(&skill_name, &file_path, version)
                    .await,
            )
        }
        _ => return None,
    })
}
