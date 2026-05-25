use desktop_shared::cognitive_commands::*;

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
