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

#[tauri::command]
pub async fn memory_health(
    state: State<'_, Arc<AppCore>>,
) -> Result<MemoryHealthResponse, ApiError> {
    state.memory_health().await
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

#[tauri::command]
pub async fn coaching_pending_interventions(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<desktop_shared::cognitive_commands::DeliveredInterventionResponse>, ApiError> {
    state.coaching_pending_interventions().await
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

#[tauri::command]
pub async fn cognitive_run_reflection(
    state: State<'_, Arc<AppCore>>,
) -> Result<ReflectionResultResponse, ApiError> {
    state.cognitive_run_reflection().await
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
pub async fn coaching_submit_feedback(
    state: State<'_, Arc<AppCore>>,
    intervention_id: String,
    response: String,
) -> Result<bool, ApiError> {
    state
        .coaching_submit_feedback(intervention_id, response)
        .await
}

#[tauri::command]
pub async fn coaching_report_ignored(
    state: State<'_, Arc<AppCore>>,
    intervention_id: String,
) -> Result<bool, ApiError> {
    state.coaching_report_ignored(intervention_id).await
}

#[tauri::command]
pub async fn coaching_intervention_log(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<desktop_shared::cognitive_commands::InterventionLogResponse>, ApiError> {
    state.coaching_intervention_log(limit).await
}

#[tauri::command]
pub async fn coaching_seed_patterns(
    state: State<'_, Arc<AppCore>>,
) -> Result<bool, ApiError> {
    state.coaching_seed_patterns().await
}

#[tauri::command]
pub async fn cognitive_inject_event(
    state: State<'_, Arc<AppCore>>,
    event_type: String,
    payload: serde_json::Value,
) -> Result<bool, ApiError> {
    state.cognitive_inject_event(event_type, payload).await
}

#[tauri::command]
pub async fn cognitive_event_log(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<cognitive::DomainEventRow>, ApiError> {
    state.cognitive_event_log(limit).await
}

#[tauri::command]
pub async fn cognitive_pipeline_log(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<cognitive::PipelineEventRow>, ApiError> {
    state.cognitive_pipeline_log(limit).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "cognitive_user_model",
    "cognitive_facts_list",
    "cognitive_episodic_list",
    "cognitive_rules_list",
    "cognitive_memory_stats",
    "cognitive_system_status",
    "cognitive_fact_create",
    "cognitive_fact_update",
    "cognitive_fact_delete",
    "cognitive_rule_create",
    "cognitive_rule_deactivate",
    "cognitive_run_compaction",
    "cognitive_run_reflection",
    "cognitive_inject_event",
    "cognitive_event_log",
    "cognitive_pipeline_log",
    "coaching_situation",
    "coaching_signals",
    "coaching_patterns",
    "coaching_feedback_stats",
    "coaching_intervention_log",
    "coaching_router_status",
    "coaching_seed_patterns",
    "coaching_pending_interventions",
    "coaching_reset_dismissals",
    "coaching_clear_signals",
    "coaching_submit_feedback",
    "coaching_report_ignored",
    "memory_health",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "cognitive_user_model" => dev::val(core.cognitive_user_model().await),
        "cognitive_facts_list" => {
            dev::val(core.cognitive_facts_list(dev::get(body, "domain")).await)
        }
        "cognitive_episodic_list" => dev::val(
            core.cognitive_episodic_list(dev::get(body, "domain"), dev::get(body, "limit"))
                .await,
        ),
        "cognitive_rules_list" => {
            dev::val(core.cognitive_rules_list(dev::get(body, "domain")).await)
        }
        "cognitive_memory_stats" => dev::val(core.cognitive_memory_stats().await),
        "cognitive_system_status" => dev::val(core.cognitive_system_status().await),
        "cognitive_fact_create" => dev::val(
            core.cognitive_fact_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "cognitive_fact_update" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(
                core.cognitive_fact_update(id, try_field!(dev::parse_params(body)))
                    .await,
            )
        }
        "cognitive_fact_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cognitive_fact_delete(id).await)
        }
        "cognitive_rule_create" => dev::val(
            core.cognitive_rule_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "cognitive_rule_deactivate" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cognitive_rule_deactivate(id).await)
        }
        "cognitive_run_compaction" => dev::val(core.cognitive_run_compaction().await),
        "cognitive_run_reflection" => dev::val(core.cognitive_run_reflection().await),
        "cognitive_inject_event" => {
            let event_type = try_field!(dev::get_str(body, "event_type"));
            let payload = body.get("payload").cloned().unwrap_or_default();
            dev::val(core.cognitive_inject_event(event_type, payload).await)
        }
        "cognitive_event_log" => dev::val(core.cognitive_event_log(dev::get(body, "limit")).await),
        "cognitive_pipeline_log" => {
            dev::val(core.cognitive_pipeline_log(dev::get(body, "limit")).await)
        }
        // Coaching
        "coaching_situation" => dev::val(core.coaching_situation().await),
        "coaching_signals" => dev::val(core.coaching_signals().await),
        "coaching_patterns" => dev::val(core.coaching_patterns().await),
        "coaching_feedback_stats" => dev::val(core.coaching_feedback_stats().await),
        "coaching_router_status" => dev::val(core.coaching_router_status().await),
        "coaching_pending_interventions" => dev::val(core.coaching_pending_interventions().await),
        "coaching_reset_dismissals" => dev::val(
            core.coaching_reset_dismissals(dev::get(body, "trigger_name"))
                .await,
        ),
        "coaching_seed_patterns" => dev::val(core.coaching_seed_patterns().await),
        "coaching_clear_signals" => dev::val(core.coaching_clear_signals().await),
        "coaching_submit_feedback" => dev::val(
            core.coaching_submit_feedback(
                dev::get(body, "intervention_id").unwrap_or_default(),
                dev::get(body, "response").unwrap_or_default(),
            )
            .await,
        ),
        "coaching_report_ignored" => dev::val(
            core.coaching_report_ignored(dev::get(body, "intervention_id").unwrap_or_default())
                .await,
        ),
        "coaching_intervention_log" => dev::val(
            core.coaching_intervention_log(dev::get(body, "limit")).await,
        ),
        "memory_health" => dev::val(core.memory_health().await),
        _ => return None,
    })
}
