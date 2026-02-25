//! GET /api/status — agent status overview.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub version: &'static str,
    pub model: String,
    pub provider: Option<String>,
    pub permission_level: String,
    /// Provider names that have a non-empty API key configured.
    pub configured_providers: Vec<String>,
    pub uptime_seconds: u64,
    pub storage: StorageStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub task_count: i64,
    pub session_count: i64,
}

pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, ApiError> {
    let task_count = state
        .repos
        .todos
        .summary()
        .await
        .map(|s| s.total)
        .unwrap_or(0);

    let session_count = state
        .repos
        .sessions
        .list_sessions()
        .await
        .map(|s| s.len() as i64)
        .unwrap_or(0);

    let config = state.config.read().map_err(|e| ApiError::internal(e.to_string()))?;

    let provider = config.agents.defaults.provider.clone();
    let model = config.agents.defaults.model.clone();
    let permission_level = config
        .tools
        .permissions
        .as_ref()
        .map(|p| p.default_level.clone())
        .unwrap_or_else(|| "full".to_string());

    // Collect providers that have a non-empty API key
    let p = &config.providers;
    let mut configured_providers = Vec::new();
    let providers: &[(&str, &config::schema::ProviderConfig)] = &[
        ("anthropic", &p.anthropic),
        ("openai", &p.openai),
        ("openrouter", &p.openrouter),
        ("deepseek", &p.deepseek),
        ("gemini", &p.gemini),
        ("groq", &p.groq),
        ("vllm", &p.vllm),
        ("zhipu", &p.zhipu),
        ("dashscope", &p.dashscope),
        ("moonshot", &p.moonshot),
        ("minimax", &p.minimax),
        ("aihubmix", &p.aihubmix),
    ];
    for (name, cfg) in providers {
        if !cfg.api_key.expose().is_empty() {
            configured_providers.push((*name).to_string());
        }
    }

    Ok(Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        model,
        provider,
        permission_level,
        configured_providers,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        storage: StorageStats { task_count, session_count },
    }))
}
