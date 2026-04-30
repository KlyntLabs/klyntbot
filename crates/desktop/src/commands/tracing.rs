//! Tauri adapters for the tracing module.

use desktop_macros::klynt_command;

#[klynt_command]
pub async fn tracing_list_providers() -> Vec<app_core::tracing::types::ProviderInfo> {
    state.tracing_list_providers().await
}

#[klynt_command]
pub async fn tracing_list_sessions(provider_id: String) -> Vec<app_core::tracing::types::SessionSummary> {
    state.tracing_list_sessions(provider_id).await
}

#[klynt_command]
pub async fn tracing_load_session(
    provider_id: String,
    session_id: String,
    scope: app_core::tracing::types::Scope,
) -> app_core::tracing::types::SessionDetail {
    state.tracing_load_session(provider_id, session_id, scope).await
}

#[klynt_command]
pub async fn tracing_load_context(
    provider_id: String,
    session_id: String,
    scope: app_core::tracing::types::Scope,
) -> Vec<app_core::tracing::types::ContextMessage> {
    state.tracing_load_context(provider_id, session_id, scope).await
}

#[klynt_command]
pub async fn tracing_load_state(
    provider_id: String,
    session_id: String,
) -> app_core::tracing::types::SessionState {
    state.tracing_load_state(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_list_subagents(
    provider_id: String,
    session_id: String,
) -> Vec<app_core::tracing::types::SubagentSummary> {
    state.tracing_list_subagents(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_import(provider_id: String, file_path: String) -> String {
    state.tracing_import(provider_id, file_path).await
}

#[klynt_command]
pub async fn tracing_get_dir(provider_id: String, session_id: String) -> std::path::PathBuf {
    state.tracing_get_dir(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_open_dir(provider_id: String, session_id: String) -> std::path::PathBuf {
    state.tracing_open_dir(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_stats(provider_id: String) -> app_core::tracing::types::StatsBundle {
    state.tracing_stats(provider_id).await
}
