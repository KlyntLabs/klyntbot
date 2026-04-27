use std::sync::Arc;

use desktop_shared::commands::reforge::*;
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn reforge_state(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<ReforgeStateResponse> {
    state.reforge_state().await
}

#[tauri::command]
#[specta::specta]
pub async fn reforge_skill_names(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<SkillListResponse> {
    state.skill_names().await
}

#[tauri::command]
#[specta::specta]
pub async fn reforge_skill_versions(
    skill_name: String,
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<SkillVersionResponse>> {
    state.skill_version_list(&skill_name).await
}

#[tauri::command]
#[specta::specta]
pub async fn reforge_skill_version_detail(
    skill_name: String,
    version: i64,
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<SkillVersionDetailResponse>> {
    state.skill_version_detail(&skill_name, version).await
}

#[tauri::command]
#[specta::specta]
pub async fn reforge_skill_reset(
    skill_name: String,
    file_path: String,
    version: i64,
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<()> {
    state
        .skill_version_reset(&skill_name, &file_path, version)
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "reforge_state",
    "reforge_skill_names",
    "reforge_skill_versions",
    "reforge_skill_version_detail",
    "reforge_skill_reset",
];

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
