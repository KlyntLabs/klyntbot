use desktop_shared::commands::{AiToolInfo, AiToolInstallResult, AiToolsInstallParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn ai_tools_detect(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<AiToolInfo>, ApiError> {
    state.ai_tools_detect().await
}

#[tauri::command]
pub async fn ai_tools_install(
    state: State<'_, Arc<AppCore>>,
    params: AiToolsInstallParams,
) -> Result<Vec<AiToolInstallResult>, ApiError> {
    state.ai_tools_install(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["ai_tools_detect", "ai_tools_install"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "ai_tools_detect" => dev::val(core.ai_tools_detect().await),
        "ai_tools_install" => {
            dev::val(core.ai_tools_install(try_field!(dev::parse_params(body))).await)
        }
        _ => return None,
    })
}
