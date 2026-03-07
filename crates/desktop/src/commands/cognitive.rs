use std::sync::Arc;

use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

// ── Memory Reads ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_user_model(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserModelSummaryResponse, ApiError> {
    state.cognitive_user_model().await
}

#[tauri::command]
pub async fn cognitive_facts_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    state.cognitive_facts_list(domain).await
}

#[tauri::command]
pub async fn cognitive_episodic_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EpisodicMemoryResponse>, ApiError> {
    state.cognitive_episodic_list(domain, limit).await
}

#[tauri::command]
pub async fn cognitive_rules_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<ProceduralRuleResponse>, ApiError> {
    state.cognitive_rules_list(domain).await
}

#[tauri::command]
pub async fn cognitive_memory_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<MemoryStatsResponse, ApiError> {
    state.cognitive_memory_stats().await
}

// ── Coaching Reads ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_situation(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserSituationResponse, ApiError> {
    state.coaching_situation().await
}

#[tauri::command]
pub async fn coaching_signals(
    state: State<'_, Arc<AppCore>>,
) -> Result<SignalWindowResponse, ApiError> {
    state.coaching_signals().await
}

#[tauri::command]
pub async fn coaching_patterns(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<DetectedPatternResponse>, ApiError> {
    state.coaching_patterns().await
}

#[tauri::command]
pub async fn coaching_feedback_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StrategyFeedbackResponse>, ApiError> {
    state.coaching_feedback_stats().await
}

#[tauri::command]
pub async fn coaching_router_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<RouterStatusResponse, ApiError> {
    state.coaching_router_status().await
}

// ── System Status ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_system_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<SystemStatusResponse, ApiError> {
    state.cognitive_system_status().await
}

// ── Mutations ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_fact_create(
    state: State<'_, Arc<AppCore>>,
    params: FactCreateParams,
) -> Result<SemanticFactResponse, ApiError> {
    state.cognitive_fact_create(params).await
}

#[tauri::command]
pub async fn cognitive_fact_update(
    state: State<'_, Arc<AppCore>>,
    id: String,
    params: FactUpdateParams,
) -> Result<SemanticFactResponse, ApiError> {
    state.cognitive_fact_update(id, params).await
}

#[tauri::command]
pub async fn cognitive_fact_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.cognitive_fact_delete(id).await
}

#[tauri::command]
pub async fn cognitive_rule_create(
    state: State<'_, Arc<AppCore>>,
    params: RuleCreateParams,
) -> Result<ProceduralRuleResponse, ApiError> {
    state.cognitive_rule_create(params).await
}

#[tauri::command]
pub async fn cognitive_rule_deactivate(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.cognitive_rule_deactivate(id).await
}

#[tauri::command]
pub async fn cognitive_run_compaction(
    state: State<'_, Arc<AppCore>>,
) -> Result<CompactionResultResponse, ApiError> {
    state.cognitive_run_compaction().await
}

// ── Coaching Mutations ──────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_reset_dismissals(
    state: State<'_, Arc<AppCore>>,
    trigger_name: Option<String>,
) -> Result<bool, ApiError> {
    state.coaching_reset_dismissals(trigger_name).await
}

#[tauri::command]
pub async fn coaching_clear_signals(state: State<'_, Arc<AppCore>>) -> Result<bool, ApiError> {
    state.coaching_clear_signals().await
}

#[tauri::command]
pub async fn cognitive_inject_event(
    state: State<'_, Arc<AppCore>>,
    event_type: String,
    payload: serde_json::Value,
) -> Result<bool, ApiError> {
    state.cognitive_inject_event(event_type, payload).await
}
