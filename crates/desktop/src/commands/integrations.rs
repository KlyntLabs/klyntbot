use desktop_macros::klynt_command;
use desktop_shared::commands::{AiToolInfo, AiToolInstallResult, AiToolsInstallParams};

#[klynt_command]
pub async fn ai_tools_detect() -> Vec<AiToolInfo> {
    state.ai_tools_detect().await
}

#[klynt_command]
pub async fn ai_tools_install(params: AiToolsInstallParams) -> Vec<AiToolInstallResult> {
    state.ai_tools_install(params).await
}
