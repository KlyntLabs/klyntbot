use desktop_macros::klynt_command;
use desktop_shared::commands::{AiToolInfo, AiToolInstallResult, AiToolsInstallParams};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn ai_tools_detect() -> Vec<AiToolInfo> {
    state.ai_tools_detect().await
}

#[klynt_command]
pub async fn ai_tools_install(params: AiToolsInstallParams) -> Vec<AiToolInstallResult> {
    state.ai_tools_install(params).await
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
        "ai_tools_detect" => dev::val(core.ai_tools_detect().await),
        "ai_tools_install" => dev::val(
            core.ai_tools_install(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
