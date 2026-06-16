use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::specta_helpers::JsonValueWrapper;
#[klynt_command]
pub async fn mcp_get_config() -> McpConfigResponse {
    state.mcp_get_config().await
}

#[klynt_command]
pub async fn mcp_add_server(params: McpAddServerParams) -> McpConfigResponse {
    state.mcp_add_server(params).await
}

#[klynt_command]
pub async fn mcp_remove_server(params: McpRemoveParams) -> McpConfigResponse {
    state.mcp_remove_server(params).await
}

#[klynt_command]
pub async fn mcp_toggle_server(params: McpToggleParams) -> McpConfigResponse {
    state.mcp_toggle_server(params).await
}

#[klynt_command]
pub async fn mcp_update_server(params: McpUpdateServerParams) -> McpConfigResponse {
    state.mcp_update_server(params).await
}

#[klynt_command]
pub async fn app_info() -> AppInfoResponse {
    state.app_info().await
}

#[klynt_command]
pub async fn config_get_section(section: String) -> JsonValueWrapper {
    state
        .config_get_section(section)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_update_section(section: String, patch: JsonValueWrapper) -> JsonValueWrapper {
    state
        .config_update_section(section, patch.0)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_mark_setup_completed() -> () {
    state.config_mark_setup_completed().await
}

/// Returns the full app settings object. Currently returns a default object;
/// persistence will be wired to the config crate in a follow-up.
#[klynt_command]
pub async fn get_app_settings() -> serde_json::Value {
    Ok(default_app_settings())
}

/// Accepts app settings updates. Currently a no-op that echoes the input back;
/// persistence will be wired to the config crate in a follow-up.
#[klynt_command]
pub async fn update_app_settings(settings: serde_json::Value) -> serde_json::Value {
    Ok(settings)
}

#[klynt_command]
pub async fn app_build_type() -> String {
    Ok(if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "release".to_string()
    })
}

#[klynt_command]
pub async fn is_mobile_runtime() -> bool {
    Ok(false)
}

fn default_app_settings() -> serde_json::Value {
    serde_json::json!({
        "codexBin": null,
        "codexArgs": null,
        "backendMode": "local",
        "remoteBackendProvider": "tcp",
        "remoteBackendHost": "",
        "remoteBackendToken": null,
        "remoteBackends": [],
        "activeRemoteBackendId": null,
        "keepDaemonRunningAfterAppClose": false,
        "defaultAccessMode": "full-access",
        "reviewDeliveryMode": "inline",
        "composerModelShortcut": null,
        "composerAccessShortcut": null,
        "composerReasoningShortcut": null,
        "composerCollaborationShortcut": null,
        "interruptShortcut": null,
        "newAgentShortcut": null,
        "newWorktreeAgentShortcut": null,
        "newCloneAgentShortcut": null,
        "archiveThreadShortcut": null,
        "toggleProjectsSidebarShortcut": null,
        "toggleGitSidebarShortcut": null,
        "branchSwitcherShortcut": null,
        "toggleDebugPanelShortcut": null,
        "toggleTerminalShortcut": null,
        "cycleAgentNextShortcut": null,
        "cycleAgentPrevShortcut": null,
        "cycleWorkspaceNextShortcut": null,
        "cycleWorkspacePrevShortcut": null,
        "lastComposerModelId": null,
        "lastComposerReasoningEffort": null,
        "uiScale": 1.0,
        "theme": "system",
        "showMessageFilePath": false,
        "chatHistoryScrollbackItems": 100,
        "threadTitleAutogenerationEnabled": true,
        "automaticAppUpdateChecksEnabled": true,
        "uiFontFamily": "system-ui",
        "codeFontFamily": "monospace",
        "codeFontSize": 14,
        "notificationSoundsEnabled": true,
        "systemNotificationsEnabled": true,
        "subagentSystemNotificationsEnabled": true,
        "splitChatDiffView": false,
        "preloadGitDiffs": true,
        "gitDiffIgnoreWhitespaceChanges": false,
        "commitMessagePrompt": "",
        "commitMessageModelId": null,
        "collaborationModesEnabled": false,
        "steerEnabled": true,
        "followUpMessageBehavior": "queue",
        "composerFollowUpHintEnabled": true,
        "pauseQueuedMessagesWhenResponseRequired": false,
        "unifiedExecEnabled": false,
        "experimentalAppsEnabled": false,
        "personality": "pragmatic",
        "dictationEnabled": false,
        "dictationModelId": "",
        "dictationPreferredLanguage": null,
        "dictationHoldKey": null,
        "composerEditorPreset": "default",
        "composerFenceExpandOnSpace": true,
        "composerFenceExpandOnEnter": true,
        "composerFenceLanguageTags": true,
        "composerFenceWrapSelection": true,
        "composerFenceAutoWrapPasteMultiline": true,
        "composerFenceAutoWrapPasteCodeLike": true,
        "composerListContinuation": true,
        "composerCodeBlockCopyUseModifier": true,
        "workspaceGroups": [],
        "globalWorktreesFolder": null,
        "openAppTargets": [],
        "selectedOpenAppId": ""
    })
}
