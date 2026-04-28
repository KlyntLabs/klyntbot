use desktop_shared::cognitive_commands::*;
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

use desktop_macros::klynt_command;

// ── Memory Reads ────────────────────────────────────────────────────────

#[klynt_command]
pub async fn cognitive_user_model() -> UserModelSummaryResponse {
    state.cognitive_user_model().await
}

#[klynt_command]
pub async fn cognitive_facts_list(domain: Option<String>) -> Vec<SemanticFactResponse> {
    state.cognitive_facts_list(domain).await
}

#[klynt_command]
pub async fn cognitive_episodic_list(
    domain: Option<String>,
    limit: Option<i64>,
) -> Vec<EpisodicMemoryResponse> {
    state.cognitive_episodic_list(domain, limit).await
}

#[klynt_command]
pub async fn cognitive_rules_list(domain: Option<String>) -> Vec<ProceduralRuleResponse> {
    state.cognitive_rules_list(domain).await
}

#[klynt_command]
pub async fn cognitive_memory_stats() -> MemoryStatsResponse {
    state.cognitive_memory_stats().await
}

#[klynt_command]
pub async fn memory_health() -> MemoryHealthResponse {
    state.memory_health().await
}

// ── Coaching Reads ──────────────────────────────────────────────────────

#[klynt_command]
pub async fn coaching_situation() -> UserSituationResponse {
    state.coaching_situation().await
}

#[klynt_command]
pub async fn coaching_signals() -> SignalWindowResponse {
    state.coaching_signals().await
}

#[klynt_command]
pub async fn coaching_patterns() -> Vec<DetectedPatternResponse> {
    state.coaching_patterns().await
}

#[klynt_command]
pub async fn coaching_feedback_stats() -> Vec<StrategyFeedbackResponse> {
    state.coaching_feedback_stats().await
}

#[klynt_command]
pub async fn coaching_router_status() -> RouterStatusResponse {
    state.coaching_router_status().await
}

#[klynt_command]
pub async fn coaching_pending_interventions(
) -> Vec<desktop_shared::cognitive_commands::DeliveredInterventionResponse> {
    state.coaching_pending_interventions().await
}

// ── Memory Reference Detail ─────────────────────────────────────────────

#[klynt_command]
pub async fn memory_reference_detail(ref_type: String, ref_id: String) -> MemoryReferenceDetail {
    state.memory_reference_detail(&ref_type, &ref_id).await
}

// ── System Status ───────────────────────────────────────────────────────

#[klynt_command]
pub async fn cognitive_system_status() -> SystemStatusResponse {
    state.cognitive_system_status().await
}

// ── Mutations ───────────────────────────────────────────────────────────

#[klynt_command]
pub async fn cognitive_fact_create(params: FactCreateParams) -> SemanticFactResponse {
    state.cognitive_fact_create(params).await
}

#[klynt_command]
pub async fn cognitive_fact_update(id: String, params: FactUpdateParams) -> SemanticFactResponse {
    state.cognitive_fact_update(id, params).await
}

#[klynt_command]
pub async fn cognitive_fact_delete(id: String) -> bool {
    state.cognitive_fact_delete(id).await
}

#[klynt_command]
pub async fn cognitive_rule_create(params: RuleCreateParams) -> ProceduralRuleResponse {
    state.cognitive_rule_create(params).await
}

#[klynt_command]
pub async fn cognitive_rule_deactivate(id: String) -> bool {
    state.cognitive_rule_deactivate(id).await
}

#[klynt_command]
pub async fn cognitive_run_compaction() -> CompactionResultResponse {
    state.cognitive_run_compaction().await
}

// ── Coaching Mutations ──────────────────────────────────────────────────

#[klynt_command]
pub async fn coaching_reset_dismissals(trigger_name: Option<String>) -> bool {
    state.coaching_reset_dismissals(trigger_name).await
}

#[klynt_command]
pub async fn coaching_clear_signals() -> bool {
    state.coaching_clear_signals().await
}

#[klynt_command]
pub async fn coaching_submit_feedback(intervention_id: String, response: String) -> bool {
    state
        .coaching_submit_feedback(intervention_id, response)
        .await
}

#[klynt_command]
pub async fn coaching_report_ignored(intervention_id: String) -> bool {
    state.coaching_report_ignored(intervention_id).await
}

#[klynt_command]
pub async fn coaching_intervention_log(
    limit: Option<i64>,
) -> Vec<desktop_shared::cognitive_commands::InterventionLogResponse> {
    state.coaching_intervention_log(limit).await
}

#[klynt_command]
pub async fn coaching_seed_patterns() -> bool {
    state.coaching_seed_patterns().await
}

#[klynt_command]
pub async fn cognitive_inject_event(
    event_type: String,
    payload: desktop_shared::specta_helpers::JsonValueWrapper,
) -> bool {
    state.cognitive_inject_event(event_type, payload.0).await
}

#[klynt_command]
pub async fn cognitive_event_log(limit: Option<i64>) -> Vec<cognitive::DomainEventRow> {
    state.cognitive_event_log(limit).await
}

#[klynt_command]
pub async fn cognitive_pipeline_log(limit: Option<i64>) -> Vec<cognitive::PipelineEventRow> {
    state.cognitive_pipeline_log(limit).await
}

// ── Cognitive Graph ─────────────────────────────────────────────────

#[klynt_command]
pub async fn cognitive_graph_data() -> desktop_shared::commands::cognitive_graph::CognitiveGraphData
{
    state.cognitive_graph_data().await
}

#[klynt_command]
pub async fn cognitive_graph_expand_topic(
    params: desktop_shared::commands::cognitive_graph::TopicExpandParams,
) -> desktop_shared::commands::cognitive_graph::TopicDetail {
    state.cognitive_graph_expand_topic(params).await
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
            core.coaching_intervention_log(dev::get(body, "limit"))
                .await,
        ),
        "memory_health" => dev::val(core.memory_health().await),
        "memory_reference_detail" => dev::val(
            core.memory_reference_detail(
                &dev::get::<String>(body, "ref_type").unwrap_or_default(),
                &dev::get::<String>(body, "ref_id").unwrap_or_default(),
            )
            .await,
        ),
        // Cognitive Graph
        "cognitive_graph_data" => dev::val(core.cognitive_graph_data().await),
        "cognitive_graph_expand_topic" => dev::val(
            core.cognitive_graph_expand_topic(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
