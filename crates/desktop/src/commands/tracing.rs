//! Tauri adapters for the tracing module.

use desktop_macros::klynt_command;

#[klynt_command]
pub async fn tracing_list_providers() -> Vec<app_core::tracing::types::ProviderInfo> {
    state.tracing_list_providers().await
}

#[klynt_command]
pub async fn tracing_list_sessions(
    provider_id: String,
) -> Vec<app_core::tracing::types::SessionSummary> {
    state.tracing_list_sessions(provider_id).await
}

#[klynt_command]
pub async fn tracing_load_session(
    provider_id: String,
    session_id: String,
    scope: app_core::tracing::types::Scope,
) -> app_core::tracing::types::SessionDetail {
    state
        .tracing_load_session(provider_id, session_id, scope)
        .await
}

#[klynt_command]
pub async fn tracing_load_context(
    provider_id: String,
    session_id: String,
    scope: app_core::tracing::types::Scope,
) -> Vec<app_core::tracing::types::ContextMessage> {
    state
        .tracing_load_context(provider_id, session_id, scope)
        .await
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

#[derive(serde::Serialize, specta::Type)]
pub struct ImportResponse {
    pub session_id: String,
    pub work_dir_hash: String,
}

#[klynt_command]
pub async fn tracing_import(
    provider_id: String,
    bytes: Vec<u8>,
    file_name: String,
) -> ImportResponse {
    let temp_dir = std::env::temp_dir().join("klyntbot_imports");
    let _ = tokio::fs::create_dir_all(&temp_dir).await;
    let temp_path = temp_dir.join(&file_name);
    tokio::fs::write(&temp_path, &bytes)
        .await
        .expect("write temp import file");
    let session_id = state
        .tracing_import(provider_id, temp_path.to_string_lossy().to_string())
        .await?;
    Ok(ImportResponse {
        session_id,
        work_dir_hash: "".to_string(),
    })
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

#[klynt_command]
pub async fn tracing_session_summary(
    provider_id: String,
    session_id: String,
) -> app_core::tracing::types::SessionSummary {
    state.tracing_session_summary(provider_id, session_id).await
}

#[klynt_command]
pub async fn tracing_load_subagent_session(
    provider_id: String,
    session_id: String,
    agent_id: String,
) -> app_core::tracing::types::SessionDetail {
    state
        .tracing_load_subagent_session(provider_id, session_id, agent_id)
        .await
}

#[klynt_command]
pub async fn tracing_load_subagent_context(
    provider_id: String,
    session_id: String,
    agent_id: String,
) -> Vec<app_core::tracing::types::ContextMessage> {
    state
        .tracing_load_subagent_context(provider_id, session_id, agent_id)
        .await
}

#[klynt_command]
pub async fn tracing_supported_tabs(
    provider_id: String,
) -> Vec<app_core::tracing::types::SessionTab> {
    state.tracing_supported_tabs(provider_id).await
}

#[klynt_command]
pub async fn tracing_header_layout(
    provider_id: String,
) -> Vec<app_core::tracing::types::HeaderChip> {
    state.tracing_header_layout(provider_id).await
}
