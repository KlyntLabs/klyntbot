use std::collections::{HashMap, VecDeque};

use agent::AgentEvent;
use desktop_shared::events::{self, *};

/// One UI emission produced by the translator. The relay shell forwards each
/// to `AppEventEmitter::emit_event`.
pub struct UiEmission {
    pub event: &'static str,
    pub payload: serde_json::Value,
}

/// Terminal result of a chat turn. The relay shell hands this to `TurnFinalizer`
/// (Done persists; Error/Cancelled only emit + clean up).
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TurnOutcome {
    Done {
        content: String,
        message_id: Option<String>,
        segments: Vec<events::MessageSegment>,
        transparency: TransparencyData,
    },
    Error {
        message: String,
    },
    Cancelled {
        partial_content: String,
        partial_reasoning: String,
    },
}

/// Accumulated turn state. Public for assertions; mutated only by the translator.
#[derive(Default)]
pub struct RelayState {
    pub segments: Vec<events::MessageSegment>,
    pub transparency: TransparencyData,
    pub current_text: String,
    pub tool_names: Vec<String>,
    pub entity_cards: Vec<common::EntityCard>,
    pub tool_token_sum: u32,
    pub(crate) pending_actions: HashMap<String, VecDeque<String>>,
    pub(crate) pending_approvals: HashMap<String, (String, Option<String>)>,
}

/// Pure translation of the agent's `AgentEvent` stream into UI emissions.
/// No I/O, no emitter, no clock. The relay shell drives it.
pub struct ChatEventTranslator {
    session_key: String,
    generation: u32,
    state: RelayState,
    terminal: Option<TurnOutcome>,
}

impl ChatEventTranslator {
    pub fn new(session_key: String, generation: u32) -> Self {
        Self {
            session_key,
            generation,
            state: RelayState::default(),
            terminal: None,
        }
    }

    pub fn state(&self) -> &RelayState {
        &self.state
    }

    pub fn take_terminal(&mut self) -> Option<TurnOutcome> {
        self.terminal.take()
    }

    fn flush_text(&mut self) {
        if !self.state.current_text.is_empty() {
            self.state.segments.push(events::MessageSegment::Text {
                content: std::mem::take(&mut self.state.current_text),
            });
        }
    }

    /// Translate one event: mutate state, return UI emissions. Terminal events
    /// also stash a `TurnOutcome` (retrieve via `take_terminal`).
    pub fn handle(&mut self, event: AgentEvent) -> Vec<UiEmission> {
        let mut out: Vec<UiEmission> = Vec::new();
        let sk = self.session_key.clone();

        // v2 thread:event (pure) — preserved from the old loop head.
        if let Some(te) = super::thread_event_v2_translator::agent_event_to_thread_event(
            event.clone(),
            sk.clone(),
            self.generation,
        ) {
            if let Ok(val) = serde_json::to_value(&te) {
                out.push(UiEmission {
                    event: "thread:event",
                    payload: val,
                });
            }
        }

        // Redefined `emit!`: push a UiEmission instead of calling an emitter.
        // This lets the v1 arms below move VERBATIM from `relay_chat_stream`.
        macro_rules! emit {
            ($event:expr, $payload:expr) => {
                if let Ok(val) = serde_json::to_value(&$payload) {
                    out.push(UiEmission {
                        event: $event,
                        payload: val,
                    });
                }
            };
        }

        match event {
            AgentEvent::ContentChunk { data } => {
                self.state.current_text.push_str(&data);
                emit!(
                    AGENT_CONTENT_CHUNK,
                    ContentChunkPayload {
                        session_key: sk.clone(),
                        data,
                    }
                );
            }
            AgentEvent::ToolStart {
                name, args, agent, ..
            } => {
                self.flush_text();
                self.state.tool_names.push(name.clone());
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                if let Some(ref a) = action {
                    self.state
                        .pending_actions
                        .entry(name.clone())
                        .or_default()
                        .push_back(a.clone());
                }
                emit!(
                    AGENT_TOOL_START,
                    ToolStartPayload {
                        session_key: sk.clone(),
                        name,
                        action,
                        agent,
                    }
                );
            }
            AgentEvent::ToolEnd {
                name,
                success,
                duration_ms,
                result,
                agent,
                ..
            } => {
                // Pop the stashed action from ToolStart (FIFO per tool name)
                let action = match self.state.pending_actions.entry(name.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        let a = e.get_mut().pop_front();
                        if e.get().is_empty() {
                            e.remove();
                        }
                        a
                    }
                    std::collections::hash_map::Entry::Vacant(_) => None,
                };
                // Estimate tokens from result length (~4 chars per token)
                let estimated_tokens = result
                    .as_ref()
                    .map(|r| (r.len() as u32).saturating_add(3) / 4);
                if let Some(t) = estimated_tokens {
                    self.state.tool_token_sum += t;
                }
                self.state.segments.push(events::MessageSegment::Tool {
                    name: name.clone(),
                    action: action.clone(),
                    success,
                    duration_ms,
                    result: result.clone(),
                    estimated_tokens,
                    agent: agent.clone(),
                });
                self.state.transparency.tools.push(TransparencyTool {
                    name: name.clone(),
                    action: action.clone(),
                    success,
                    duration_ms,
                    estimated_tokens,
                    agent: agent.clone(),
                });
                // Emit entity:updated so the UI refreshes affected lists
                if success {
                    for intent in
                        crate::entity_update_intent::project_entity_update(&name, action.as_deref())
                    {
                        let payload = serde_json::json!({
                            "entityKind": intent.kind,
                            "id": intent.id,
                        });
                        out.push(UiEmission {
                            event: ENTITY_UPDATED,
                            payload,
                        });
                    }
                }
                emit!(
                    AGENT_TOOL_END,
                    ToolEndPayload {
                        session_key: sk.clone(),
                        name,
                        action,
                        success,
                        duration_ms,
                        result,
                        estimated_tokens,
                        agent,
                    }
                );
            }
            AgentEvent::EntityCreated(card) => {
                emit!(
                    AGENT_ENTITY_CREATED,
                    EntityCreatedPayload {
                        session_key: sk.clone(),
                        entity_type: card.entity_type.clone(),
                        entity_id: card.entity_id.clone(),
                    }
                );
                if let Some(kind) = desktop_shared::types::EntityKind::parse(&card.entity_type) {
                    let payload = serde_json::json!({
                        "entityKind": kind,
                        "id": card.entity_id
                    });
                    out.push(UiEmission {
                        event: ENTITY_UPDATED,
                        payload,
                    });
                }
                self.state.entity_cards.push(card);
            }
            AgentEvent::Done {
                content,
                message_id,
            } => {
                self.flush_text();
                if self.state.tool_token_sum > 0 {
                    self.state.transparency.tool_tokens_total = Some(self.state.tool_token_sum);
                }
                emit!(
                    AGENT_DONE,
                    DonePayload {
                        session_key: sk.clone(),
                        content: content.clone()
                    }
                );
                emit!(
                    CHAT_MESSAGE_ADDED,
                    ChatMessagePayload {
                        session_key: sk.clone(),
                        source: "chat".to_string(),
                    }
                );
                self.terminal = Some(TurnOutcome::Done {
                    content,
                    message_id,
                    segments: std::mem::take(&mut self.state.segments),
                    transparency: std::mem::take(&mut self.state.transparency),
                });
            }
            AgentEvent::Error { message } => {
                emit!(
                    AGENT_ERROR,
                    AgentErrorPayload {
                        session_key: sk.clone(),
                        message: message.clone()
                    }
                );
                emit!(
                    CHAT_MESSAGE_ADDED,
                    ChatMessagePayload {
                        session_key: sk.clone(),
                        source: "agent_error".to_string(),
                    }
                );
                self.terminal = Some(TurnOutcome::Error { message });
            }
            AgentEvent::Cancelled {
                partial_content,
                partial_reasoning,
            } => {
                emit!(
                    AGENT_CANCELLED,
                    CancelledPayload {
                        session_key: sk.clone(),
                        partial_content: partial_content.clone(),
                        partial_reasoning: partial_reasoning.clone(),
                    }
                );
                self.terminal = Some(TurnOutcome::Cancelled {
                    partial_content,
                    partial_reasoning,
                });
            }
            AgentEvent::ExecutionStarted {
                engine,
                max_iterations,
            } => {
                self.state.transparency.execution = Some(TransparencyExecution {
                    engine: engine.clone(),
                    iterations: 0,
                    max_iterations: max_iterations as u32,
                    escalations: 0,
                });
                emit!(
                    AGENT_EXECUTION_STARTED,
                    ExecutionStartedPayload {
                        session_key: sk.clone(),
                        engine,
                        max_iterations,
                    }
                );
            }
            AgentEvent::PipelineStarted => {
                emit!(
                    AGENT_PIPELINE_STARTED,
                    PipelineStartedPayload {
                        session_key: sk.clone(),
                    }
                );
            }
            AgentEvent::ContextAssembled {
                total_tokens,
                budget: _,
                duration_ms,
            } => {
                self.state
                    .transparency
                    .timing
                    .get_or_insert_with(Default::default)
                    .context_assembly_ms = Some(duration_ms);
                emit!(
                    AGENT_CONTEXT_ASSEMBLED,
                    ContextAssembledPayload {
                        session_key: sk.clone(),
                        total_tokens,
                        duration_ms,
                    }
                );
            }
            AgentEvent::RetrievalEnhanced {
                stages,
                total_latency_ms,
                total_llm_calls,
            } => {
                let stage_payloads: Vec<events::EnhancementStagePayload> = stages
                    .iter()
                    .map(|s| {
                        let (status, detail) = s.status.to_parts();
                        events::EnhancementStagePayload {
                            name: s.name.to_string(),
                            status: status.to_string(),
                            status_detail: detail.map(String::from),
                            latency_ms: s.latency_ms,
                            llm_calls: s.llm_calls,
                            output_summary: s.output_summary.clone(),
                        }
                    })
                    .collect();
                self.state.transparency.enhancement = Some(events::TransparencyEnhancement {
                    stages: stage_payloads.clone(),
                    total_latency_ms,
                    total_llm_calls,
                });
                emit!(
                    events::AGENT_RETRIEVAL_ENHANCED,
                    events::RetrievalEnhancedPayload {
                        session_key: sk.clone(),
                        stages: stage_payloads,
                        total_latency_ms,
                        total_llm_calls,
                    }
                );
            }
            AgentEvent::IterationStart { iteration, max } => {
                if let Some(ref mut exec) = self.state.transparency.execution {
                    exec.iterations = iteration as u32;
                }
                emit!(
                    events::AGENT_ITERATION_START,
                    events::IterationStartPayload {
                        session_key: sk.clone(),
                        iteration,
                        max_iterations: max,
                    }
                );
            }
            AgentEvent::ConfidenceAssessed { score, action } => {
                emit!(
                    events::AGENT_CONFIDENCE_ASSESSED,
                    events::ConfidenceAssessedPayload {
                        session_key: sk.clone(),
                        score,
                        action,
                    }
                );
            }
            AgentEvent::UsageReport {
                prompt_tokens,
                completion_tokens,
                cache_read_tokens,
                cache_write_tokens,
                estimated_cost_usd,
                model,
                response_time_ms,
                ..
            } => {
                self.state.transparency.usage = Some(TransparencyUsage {
                    prompt_tokens,
                    completion_tokens,
                    cache_read_tokens,
                    cache_write_tokens,
                });
                self.state.transparency.cost = Some(TransparencyCost {
                    estimated_usd: estimated_cost_usd,
                    model: model.clone(),
                });
                self.state
                    .transparency
                    .timing
                    .get_or_insert_with(Default::default)
                    .total_ms = response_time_ms;
                emit!(
                    events::AGENT_USAGE_REPORT,
                    events::UsageReportPayload {
                        session_key: sk.clone(),
                        prompt_tokens,
                        completion_tokens,
                        cache_read_tokens,
                        cache_write_tokens,
                        estimated_cost_usd,
                        model,
                        response_time_ms,
                    }
                );
            }
            AgentEvent::MemoryAccess {
                action,
                query,
                results_count,
            } => {
                self.state
                    .transparency
                    .memory_accesses
                    .push(TransparencyMemoryAccess {
                        action: action.clone(),
                        query: query.clone(),
                        results_count,
                    });
                emit!(
                    events::AGENT_MEMORY_ACCESS,
                    events::MemoryAccessPayload {
                        session_key: sk.clone(),
                        action,
                        query,
                        results_count,
                    }
                );
            }
            AgentEvent::SkillLoaded {
                name,
                trigger,
                agent,
            } => {
                self.state.transparency.skills.push(TransparencySkill {
                    name: name.clone(),
                    trigger: trigger.clone(),
                    agent: agent.clone(),
                });
                emit!(
                    events::AGENT_SKILL_LOADED,
                    events::SkillLoadedPayload {
                        session_key: sk.clone(),
                        name,
                        trigger,
                        agent,
                    }
                );
            }
            AgentEvent::LearningEvent { event_type, detail } => {
                self.state.transparency.learning.push(TransparencyLearning {
                    event_type: event_type.clone(),
                    detail: detail.clone(),
                });
                emit!(
                    events::AGENT_LEARNING_EVENT,
                    events::LearningEventPayload {
                        session_key: sk.clone(),
                        event_type,
                        detail,
                    }
                );
            }
            AgentEvent::AgentSelected { name, description } => {
                self.state.transparency.agent_selected = Some(TransparencyAgentSelected {
                    name: name.clone(),
                    description: description.clone(),
                });
                emit!(
                    events::AGENT_SELECTED,
                    events::AgentSelectedPayload {
                        session_key: sk.clone(),
                        name,
                        description,
                    }
                );
            }
            AgentEvent::SubagentSpawned { label, profile, .. } => {
                self.state
                    .transparency
                    .subagents
                    .push(TransparencySubagent {
                        label: label.clone(),
                        profile: profile.clone(),
                    });
                emit!(
                    events::AGENT_SUBAGENT_SPAWNED,
                    events::SubagentSpawnedPayload {
                        session_key: sk.clone(),
                        label,
                        profile,
                    }
                );
            }
            AgentEvent::DelegationStarted {
                from_agent,
                to_agent,
                query,
                depth,
            } => {
                self.state
                    .transparency
                    .delegations
                    .push(TransparencyDelegation {
                        from_agent: from_agent.clone(),
                        to_agent: to_agent.clone(),
                        query: query.clone(),
                        depth,
                        status: "active".to_string(),
                        duration_ms: None,
                    });
                emit!(
                    events::AGENT_DELEGATION_STARTED,
                    events::DelegationStartedPayload {
                        session_key: sk.clone(),
                        from_agent,
                        to_agent,
                        query,
                        depth,
                    }
                );
            }
            AgentEvent::DelegationCompleted {
                from_agent,
                to_agent,
                success,
                duration_ms,
            } => {
                // Update the matching delegation entry
                if let Some(d) = self
                    .state
                    .transparency
                    .delegations
                    .iter_mut()
                    .find(|d| d.to_agent == to_agent && d.status == "active")
                {
                    d.status = if success {
                        "completed".to_string()
                    } else {
                        "failed".to_string()
                    };
                    d.duration_ms = Some(duration_ms);
                }
                emit!(
                    events::AGENT_DELEGATION_COMPLETED,
                    events::DelegationCompletedPayload {
                        session_key: sk.clone(),
                        from_agent,
                        to_agent,
                        success,
                        duration_ms,
                    }
                );
            }
            AgentEvent::McpServerStatus {
                server_name,
                status,
                tool_count,
                error,
            } => {
                emit!(
                    events::MCP_SERVER_STATUS,
                    events::McpServerStatusPayload {
                        server_name,
                        status,
                        tool_count,
                        error,
                    }
                );
            }
            AgentEvent::McpStartupComplete {
                ready,
                failed,
                skipped,
            } => {
                emit!(
                    events::MCP_STARTUP_COMPLETE,
                    events::McpStartupCompletePayload {
                        ready,
                        failed,
                        skipped,
                    }
                );
            }
            AgentEvent::PlanningStarted { .. } => {}
            AgentEvent::PlanGenerated { steps, raw_plan } => {
                self.state.transparency.plan = Some(events::TransparencyPlan {
                    steps: steps.clone(),
                    completed_steps: Vec::new(),
                });
                emit!(
                    events::AGENT_PLAN_GENERATED,
                    events::PlanGeneratedPayload {
                        session_key: sk.to_string(),
                        steps,
                        raw_plan,
                    }
                );
            }
            AgentEvent::PlanStepCompleted {
                step_index,
                description,
                tool_name,
            } => {
                if let Some(ref mut plan) = self.state.transparency.plan {
                    plan.completed_steps.push(step_index);
                }
                emit!(
                    events::AGENT_PLAN_STEP_COMPLETED,
                    events::PlanStepCompletedPayload {
                        session_key: sk.to_string(),
                        step_index,
                        description,
                        tool_name,
                    }
                );
            }
            AgentEvent::BudgetWarning {
                monthly_spend_usd,
                monthly_budget_usd,
                usage_percent,
            } => {
                emit!(
                    events::AGENT_BUDGET_WARNING,
                    events::BudgetWarningPayload {
                        session_key: sk.to_string(),
                        monthly_spend_usd,
                        monthly_budget_usd,
                        usage_percent,
                    }
                );
            }

            AgentEvent::MemoryPromoted {
                fact_id,
                from_scope,
                to_scope,
                subject,
                predicate,
            } => {
                emit!(
                    events::AGENT_MEMORY_PROMOTED,
                    events::MemoryPromotedPayload {
                        session_key: sk.to_string(),
                        fact_id,
                        from_scope,
                        to_scope,
                        subject,
                        predicate,
                    }
                );
            }
            // AutoTuner events — forwarded to the UI for toast notifications and panel updates.
            AgentEvent::AutoTunerReport(report) => {
                emit!(events::AUTOTUNER_REPORT, report);
            }
            AgentEvent::AutoTunerPromotion(promotion) => {
                emit!(events::AUTOTUNER_PROMOTION, promotion);
            }
            AgentEvent::AutoTunerRollback(rollback) => {
                emit!(events::AUTOTUNER_ROLLBACK, rollback);
            }
            AgentEvent::ContextCompressed {
                before_tokens,
                after_tokens,
                iteration,
            } => {
                tracing::info!(
                    before_tokens,
                    after_tokens,
                    iteration,
                    "mid-loop context compression applied"
                );
            }
            AgentEvent::ContextTieredCompressed {
                tier0_kept,
                tier1_tokens,
                tier2_tokens,
                cognitive_scoring_used,
                delta_only,
            } => {
                tracing::info!(
                    tier0_kept,
                    tier1_tokens,
                    tier2_tokens,
                    cognitive_scoring_used,
                    delta_only,
                    "tiered history compression applied"
                );
            }
            AgentEvent::LoopDetected {
                iteration,
                tools_summary,
                suggestion,
            } => {
                tracing::info!(
                    iteration,
                    tools_summary = %tools_summary,
                    suggestion = %suggestion,
                    "loop detected: repeating tool pattern"
                );
            }
            AgentEvent::LoopHardStop {
                iteration,
                tools_summary,
            } => {
                tracing::warn!(
                    iteration,
                    tools_summary = %tools_summary,
                    "loop hard-stop: forcing synthesis"
                );
            }
            AgentEvent::ContextReassembled {
                updates,
                tokens_added,
            } => {
                tracing::info!(
                    updates_count = updates.len(),
                    tokens_added,
                    "live context reassembled during execution"
                );
            }
            // Budget-bounded execution events — logged for now, UI integration later.
            AgentEvent::BudgetUpdate { .. }
            | AgentEvent::DepthSuggestion { .. }
            | AgentEvent::EnrichmentStarted { .. }
            | AgentEvent::EnrichmentComplete { .. }
            | AgentEvent::TurnComplete { .. } => {}
            AgentEvent::ApprovalRequested {
                requires_user_input,
                ..
            } if !requires_user_input => {
                // Auto-allow / auto-deny / privacy: telemetry only — UI doesn't need them.
            }
            AgentEvent::ApprovalRequested {
                ref request_id,
                ref tool,
                ref args_hash,
                ref layer,
                ref rule_matched,
                ref mirror_history,
                ref sandbox_summary,
                requires_user_input,
                ref args,
                ref cwd,
                ref layer_reason,
            } => {
                let path = args.as_ref().and_then(approval::extract_path_str_from_args);
                self.state
                    .pending_approvals
                    .insert(request_id.clone(), (tool.clone(), path));
                let payload = serde_json::json!({
                    "request_id": request_id,
                    "tool": tool,
                    "args_hash": args_hash,
                    "layer": layer,
                    "rule_matched": rule_matched,
                    "mirror_history": mirror_history,
                    "sandbox_summary": sandbox_summary,
                    "requires_user_input": requires_user_input,
                    "args": args,
                    "cwd": cwd,
                    "layer_reason": layer_reason,
                });
                out.push(UiEmission {
                    event: "agent:approval_requested",
                    payload,
                });
            }
            AgentEvent::ApprovalResolved {
                ref request_id,
                ref decision,
                ref decision_reason,
                ref latency_ms,
                ref persisted_rule,
                ref decided_by,
            } => {
                let (_tool_name, _path) = self
                    .state
                    .pending_approvals
                    .remove(request_id)
                    .unwrap_or_default();
                let payload = serde_json::json!({
                    "request_id": request_id,
                    "decision": decision,
                    "decision_reason": decision_reason,
                    "latency_ms": latency_ms,
                    "persisted_rule": persisted_rule,
                    "decided_by": decided_by,
                });
                out.push(UiEmission {
                    event: "agent:approval_resolved",
                    payload,
                });
            }
            AgentEvent::SandboxPolicyApplied {
                ref tool,
                ref policy_summary,
                ref policy_hash,
                fallback_unsandboxed,
                ref fs_constraints,
                ref network_constraints,
            } => {
                let payload = serde_json::json!({
                    "tool": tool,
                    "policy_summary": policy_summary,
                    "policy_hash": policy_hash,
                    "fallback_unsandboxed": fallback_unsandboxed,
                    "fs_constraints": fs_constraints,
                    "network_constraints": network_constraints,
                });
                out.push(UiEmission {
                    event: "agent:sandbox_policy_applied",
                    payload,
                });
            }
            AgentEvent::FileEditWithSymbols {
                ref path,
                ref op,
                bytes,
                ref diff_full,
                ..
            } => {
                let payload = serde_json::json!({
                    "path": path,
                    "op": op,
                    "bytes": bytes,
                    "diff": diff_full,
                });
                out.push(UiEmission {
                    event: "agent:file_edit_with_symbols",
                    payload,
                });
            }
            AgentEvent::RecallInjected {
                ref memory_ids,
                coverage_score,
                ref escalation_chain,
                dead_end_warning,
                budget_used_tokens,
                budget_limit_tokens,
            } => {
                let payload = serde_json::json!({
                    "session_key": sk.clone(),
                    "memory_ids": memory_ids,
                    "coverage_score": coverage_score,
                    "escalation_chain": escalation_chain,
                    "dead_end_warning": dead_end_warning,
                    "budget_used_tokens": budget_used_tokens,
                    "budget_limit_tokens": budget_limit_tokens,
                });
                out.push(UiEmission {
                    event: "agent:recall_injected",
                    payload,
                });
            }
            AgentEvent::DeadEndWarningSurfaced {
                ref approach_summary,
                ref prior_attempt_id,
                confidence,
            } => {
                let payload = serde_json::json!({
                    "session_key": sk.clone(),
                    "approach_summary": approach_summary,
                    "prior_attempt_id": prior_attempt_id,
                    "confidence": confidence,
                });
                out.push(UiEmission {
                    event: "agent:dead_end_warning_surfaced",
                    payload,
                });
            }
            AgentEvent::PlanModeChanged {
                ref session_key,
                active,
                ref requested_by,
            } => {
                let payload = serde_json::json!({
                    "session_key": session_key, "active": active, "requested_by": requested_by,
                });
                out.push(UiEmission {
                    event: "agent:plan_mode_changed",
                    payload,
                });
            }
            // Telemetry / internal events — intentionally not relayed to FE.
            AgentEvent::ReasoningChunk { .. }
            | AgentEvent::SubagentProgress { .. }
            | AgentEvent::SubagentCompleted { .. }
            | AgentEvent::SubagentCancelled { .. }
            | AgentEvent::SkillActivationConsidered { .. }
            | AgentEvent::SkillActivated { .. }
            | AgentEvent::SkillReferenceLoaded { .. }
            | AgentEvent::ContextEngineDecision { .. }
            | AgentEvent::ToolCallStreamChunk { .. }
            | AgentEvent::MCPSubcallTrace { .. }
            | AgentEvent::ProviderRequest { .. }
            | AgentEvent::ProviderResponse { .. }
            | AgentEvent::MidLoopCompressionTriggered { .. }
            | AgentEvent::TestRunDetailed { .. }
            | AgentEvent::PowerModeToggled { .. }
            | AgentEvent::TurnInterrupted { .. } => {
                tracing::debug!(event_type = ?event, "chat relay: dropped event (no v1 FE relay)");
            }
            // Safety net for future AgentEvent variants added without updating this match.
            _ => {
                tracing::warn!(event_type = ?event, "chat relay: unknown event variant dropped");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::events::AgentEvent;

    #[test]
    fn content_chunk_accumulates_text_and_emits() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        let emits = tr.handle(AgentEvent::ContentChunk {
            data: "hello ".to_string(),
        });
        // v2 thread:event (if mapped) + v1 AGENT_CONTENT_CHUNK both pushed.
        assert!(
            emits
                .iter()
                .any(|e| e.event == "agent:content_chunk" || e.event.contains("content")),
            "expected a content emission, got {:?}",
            emits.iter().map(|e| e.event).collect::<Vec<_>>()
        );
        // Text accumulates in state, not yet flushed into a segment.
        assert_eq!(tr.state().current_text, "hello ");
        assert!(tr.state().segments.is_empty());
        assert!(tr.take_terminal().is_none());
    }

    #[test]
    fn tool_end_pushes_a_tool_segment() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ToolStart {
            name: "tasks".to_string(),
            args: serde_json::json!({"action": "list"}),
            agent: None,
            call_id: None,
        });
        tr.handle(AgentEvent::ToolEnd {
            name: "tasks".to_string(),
            success: true,
            duration_ms: 12,
            result: Some("ok".to_string()),
            agent: None,
            call_id: None,
        });
        assert_eq!(tr.state().segments.len(), 1);
        assert_eq!(tr.state().tool_names, vec!["tasks".to_string()]);
    }

    #[test]
    fn tool_end_success_mutating_emits_wildcard_entity_updated() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ToolStart {
            name: "tasks".to_string(),
            args: serde_json::json!({"action": "create"}),
            agent: None,
            call_id: None,
        });
        let emits = tr.handle(AgentEvent::ToolEnd {
            name: "tasks".to_string(),
            success: true,
            duration_ms: 5,
            result: Some("ok".to_string()),
            agent: None,
            call_id: None,
        });
        let entity = emits
            .iter()
            .find(|e| e.event == ENTITY_UPDATED)
            .expect("entity:updated");
        assert_eq!(entity.payload["id"], "*");
        assert_eq!(entity.payload["entityKind"], "task");
    }

    #[test]
    fn tool_end_success_read_only_skips_entity_updated() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ToolStart {
            name: "tasks".to_string(),
            args: serde_json::json!({"action": "list"}),
            agent: None,
            call_id: None,
        });
        let emits = tr.handle(AgentEvent::ToolEnd {
            name: "tasks".to_string(),
            success: true,
            duration_ms: 5,
            result: Some("ok".to_string()),
            agent: None,
            call_id: None,
        });
        assert!(emits.iter().all(|e| e.event != ENTITY_UPDATED));
    }

    #[test]
    fn tool_end_failure_skips_entity_updated() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ToolStart {
            name: "tasks".to_string(),
            args: serde_json::json!({"action": "create"}),
            agent: None,
            call_id: None,
        });
        let emits = tr.handle(AgentEvent::ToolEnd {
            name: "tasks".to_string(),
            success: false,
            duration_ms: 5,
            result: Some("err".to_string()),
            agent: None,
            call_id: None,
        });
        assert!(emits.iter().all(|e| e.event != ENTITY_UPDATED));
    }

    #[test]
    fn tool_end_parity_cases_match_projection() {
        use crate::entity_update_intent::PARITY_CASES;
        use desktop_shared::types::EntityKind;

        for (tool, action, expected) in PARITY_CASES {
            let mut tr = ChatEventTranslator::new("sess-parity".to_string(), 0);
            let action_val = action.unwrap_or("");
            tr.handle(AgentEvent::ToolStart {
                name: (*tool).to_string(),
                args: serde_json::json!({ "action": action_val }),
                agent: None,
                call_id: None,
            });
            let emits = tr.handle(AgentEvent::ToolEnd {
                name: (*tool).to_string(),
                success: true,
                duration_ms: 1,
                result: Some("ok".to_string()),
                agent: None,
                call_id: None,
            });
            let kinds: Vec<EntityKind> = emits
                .iter()
                .filter(|e| e.event == ENTITY_UPDATED)
                .filter_map(|e| {
                    e.payload
                        .get("entityKind")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                })
                .collect();
            assert_eq!(
                kinds.as_slice(),
                *expected,
                "chat parity {tool:?} {action:?}"
            );
            for e in emits.iter().filter(|e| e.event == ENTITY_UPDATED) {
                assert_eq!(e.payload["id"], "*", "chat wildcard {tool:?}");
            }
        }
    }

    #[test]
    fn entity_created_still_emits_concrete_id() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        let card = common::EntityCard {
            entity_type: "task".to_string(),
            entity_id: "task-concrete-99".to_string(),
            title: "T".to_string(),
            subtitle: None,
            route: None,
            icon_hint: "task".to_string(),
            metadata: Default::default(),
        };
        let emits = tr.handle(AgentEvent::EntityCreated(card));
        let entity = emits
            .iter()
            .find(|e| e.event == ENTITY_UPDATED)
            .expect("entity:updated from EntityCreated");
        assert_eq!(entity.payload["id"], "task-concrete-99");
        assert_ne!(entity.payload["id"], "*");
    }

    #[test]
    fn done_yields_terminal_outcome_without_io() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ContentChunk {
            data: "answer".to_string(),
        });
        let _ = tr.handle(AgentEvent::Done {
            content: "answer".to_string(),
            message_id: Some("m1".to_string()),
        });
        match tr.take_terminal() {
            Some(TurnOutcome::Done {
                content,
                message_id,
                segments,
                ..
            }) => {
                assert_eq!(content, "answer");
                assert_eq!(message_id.as_deref(), Some("m1"));
                // The flushed text became a segment carried out in the outcome.
                assert!(!segments.is_empty());
            }
            other => panic!("expected Done outcome, got {:?}", other),
        }
    }
}
