use std::sync::Arc;

use app_core::handlers::skills::{AdaptPreviewResponse, SkillBrowseRow};
use desktop_shared::errors::ApiError;
use tauri::State;

use skills_installer::{InstallPlan, UninstallMode, UpgradePlan};
use skills_marketplace::InstalledSkill;
use skills_registry::{AvailableVersion, GitRef};

use crate::app_core::AppCore;

#[tauri::command]
pub async fn skill_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<InstalledSkill>, ApiError> {
    state.skill_list().await
}

#[tauri::command]
pub async fn skill_browse(
    state: State<'_, Arc<AppCore>>,
    query: Option<String>,
) -> Result<Vec<SkillBrowseRow>, ApiError> {
    state.skill_browse(query).await
}

#[tauri::command]
pub async fn skill_install_preview(
    state: State<'_, Arc<AppCore>>,
    shorthand: String,
    version: Option<GitRef>,
) -> Result<InstallPlan, ApiError> {
    state.skill_install_preview(shorthand, version).await
}

#[tauri::command]
pub async fn skill_install_apply(
    state: State<'_, Arc<AppCore>>,
    plan: InstallPlan,
) -> Result<InstalledSkill, ApiError> {
    state.skill_install_apply(plan).await
}

#[tauri::command]
pub async fn skill_check_updates(
    state: State<'_, Arc<AppCore>>,
    name: String,
) -> Result<Vec<AvailableVersion>, ApiError> {
    state.skill_check_updates(name).await
}

#[tauri::command]
pub async fn skill_upgrade_preview(
    state: State<'_, Arc<AppCore>>,
    name: String,
    target_sha: String,
) -> Result<UpgradePlan, ApiError> {
    state.skill_upgrade_preview(name, target_sha).await
}

#[tauri::command]
pub async fn skill_upgrade_apply(
    state: State<'_, Arc<AppCore>>,
    plan: UpgradePlan,
) -> Result<InstalledSkill, ApiError> {
    state.skill_upgrade_apply(plan).await
}

#[tauri::command]
pub async fn skill_uninstall(
    state: State<'_, Arc<AppCore>>,
    name: String,
    mode: UninstallMode,
) -> Result<(), ApiError> {
    state.skill_uninstall(name, mode).await
}

#[tauri::command]
pub async fn skill_toggle_enabled(
    state: State<'_, Arc<AppCore>>,
    name: String,
    enabled: bool,
) -> Result<(), ApiError> {
    state.skill_toggle_enabled(name, enabled).await
}

#[tauri::command]
pub async fn skill_adapt_preview(
    state: State<'_, Arc<AppCore>>,
    shorthand: String,
) -> Result<AdaptPreviewResponse, ApiError> {
    state.skill_adapt_preview(shorthand).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "skill_list",
    "skill_browse",
    "skill_install_preview",
    "skill_install_apply",
    "skill_check_updates",
    "skill_upgrade_preview",
    "skill_upgrade_apply",
    "skill_uninstall",
    "skill_toggle_enabled",
    "skill_adapt_preview",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "skill_list" => dev::val(core.skill_list().await),
        "skill_browse" => {
            let q: Option<String> = dev::get(body, "query");
            dev::val(core.skill_browse(q).await)
        }
        "skill_install_preview" => {
            let sh = try_field!(dev::get_str(body, "shorthand"));
            let version: Option<GitRef> = dev::get(body, "version");
            dev::val(core.skill_install_preview(sh, version).await)
        }
        "skill_install_apply" => {
            let plan: InstallPlan = try_field!(dev::require_de(body, "plan"));
            dev::val(core.skill_install_apply(plan).await)
        }
        "skill_check_updates" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(core.skill_check_updates(name).await)
        }
        "skill_upgrade_preview" => {
            let name = try_field!(dev::get_str(body, "name"));
            let target = try_field!(dev::get_str(body, "targetSha"));
            dev::val(core.skill_upgrade_preview(name, target).await)
        }
        "skill_upgrade_apply" => {
            let plan: UpgradePlan = try_field!(dev::require_de(body, "plan"));
            dev::val(core.skill_upgrade_apply(plan).await)
        }
        "skill_uninstall" => {
            let name = try_field!(dev::get_str(body, "name"));
            let mode: UninstallMode = try_field!(dev::require_de(body, "mode"));
            dev::val(core.skill_uninstall(name, mode).await)
        }
        "skill_toggle_enabled" => {
            let name = try_field!(dev::get_str(body, "name"));
            let enabled: bool = try_field!(dev::require(body, "enabled"));
            dev::val(core.skill_toggle_enabled(name, enabled).await)
        }
        "skill_adapt_preview" => {
            let sh = try_field!(dev::get_str(body, "shorthand"));
            dev::val(core.skill_adapt_preview(sh).await)
        }
        _ => return None,
    })
}
