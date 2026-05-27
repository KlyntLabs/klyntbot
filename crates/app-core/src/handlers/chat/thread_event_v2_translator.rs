//! Translator: `agent::AgentEvent` → `desktop_shared::thread_event_v2::ThreadEvent`.
//!
//! Used by the relay to emit v2 events alongside legacy v1 events during
//! the migration window (PR4–PR5).

use desktop_shared::thread_event_v2::{Generation, TerminalKind, ThreadEvent};

/// Convert an `AgentEvent` into a `ThreadEvent` v2.
///
/// Returns `None` for events that have no v2 equivalent (e.g. internal
/// telemetry events like `ProviderRequest`, `SubagentProgress`, etc.).
pub fn agent_event_to_thread_event(
    event: agent::AgentEvent,
    session_key: String,
    generation: Generation,
) -> Option<ThreadEvent> {
    Some(match event {
        agent::AgentEvent::ContentChunk { data } => ThreadEvent::ContentChunk {
            generation,
            session_key,
            data,
        },
        agent::AgentEvent::ToolStart {
            name,
            agent: tool_agent,
            ..
        } => ThreadEvent::ToolStart {
            generation,
            session_key,
            name,
            action: None, // action is in args, not extracted here for v2
            agent: tool_agent,
        },
        agent::AgentEvent::ToolEnd {
            name,
            success,
            duration_ms,
            result,
            agent: tool_agent,
            ..
        } => ThreadEvent::ToolEnd {
            generation,
            session_key,
            name,
            action: None,
            success,
            duration_ms,
            result: result.clone(),
            estimated_tokens: result
                .as_ref()
                .map(|r| (r.len() as u32).saturating_add(3) / 4),
            agent: tool_agent,
        },
        agent::AgentEvent::EntityCreated(card) => ThreadEvent::EntityCreated {
            generation,
            session_key,
            entity_type: card.entity_type,
            entity_id: card.entity_id,
        },
        agent::AgentEvent::PipelineStarted => ThreadEvent::PipelineStarted {
            generation,
            session_key,
        },
        agent::AgentEvent::ExecutionStarted {
            engine,
            max_iterations,
        } => ThreadEvent::ExecutionStarted {
            generation,
            session_key,
            engine,
            max_iterations,
        },
        agent::AgentEvent::ContextAssembled {
            total_tokens,
            duration_ms,
            ..
        } => ThreadEvent::ContextAssembled {
            generation,
            session_key,
            total_tokens,
            duration_ms,
        },
        agent::AgentEvent::RetrievalEnhanced {
            stages,
            total_latency_ms,
            total_llm_calls,
        } => {
            let stages = stages
                .into_iter()
                .map(|s| {
                    let (status, status_detail) = s.status.to_parts();
                    desktop_shared::events::EnhancementStagePayload {
                        name: s.name.to_string(),
                        status: status.to_string(),
                        status_detail: status_detail.map(String::from),
                        latency_ms: s.latency_ms,
                        llm_calls: s.llm_calls,
                        output_summary: s.output_summary,
                    }
                })
                .collect();
            ThreadEvent::RetrievalEnhanced {
                generation,
                session_key,
                stages,
                total_latency_ms,
                total_llm_calls,
            }
        }
        agent::AgentEvent::IterationStart { iteration, max } => ThreadEvent::IterationStart {
            generation,
            session_key,
            iteration,
            max_iterations: max,
        },
        agent::AgentEvent::ConfidenceAssessed { score, action } => {
            ThreadEvent::ConfidenceAssessed {
                generation,
                session_key,
                score,
                action,
            }
        }
        agent::AgentEvent::UsageReport {
            prompt_tokens,
            completion_tokens,
            cache_read_tokens,
            cache_write_tokens,
            estimated_cost_usd,
            model,
            response_time_ms,
            ..
        } => ThreadEvent::UsageReport {
            generation,
            session_key,
            prompt_tokens,
            completion_tokens,
            cache_read_tokens,
            cache_write_tokens,
            estimated_cost_usd,
            model,
            response_time_ms,
        },
        agent::AgentEvent::MemoryAccess {
            action,
            query,
            results_count,
        } => ThreadEvent::MemoryAccess {
            generation,
            session_key,
            action,
            query,
            results_count,
        },
        agent::AgentEvent::SkillLoaded {
            name,
            trigger,
            agent: skill_agent,
        } => ThreadEvent::SkillLoaded {
            generation,
            session_key,
            name,
            trigger,
            agent: skill_agent,
        },
        agent::AgentEvent::LearningEvent { event_type, detail } => ThreadEvent::LearningEvent {
            generation,
            session_key,
            event_type,
            detail,
        },
        agent::AgentEvent::AgentSelected { name, description } => ThreadEvent::AgentSelected {
            generation,
            session_key,
            name,
            description,
        },
        agent::AgentEvent::SubagentSpawned {
            agent_id: _,
            label,
            profile,
            ..
        } => ThreadEvent::SubagentSpawned {
            generation,
            session_key,
            label,
            profile,
        },
        agent::AgentEvent::DelegationStarted {
            from_agent,
            to_agent,
            query,
            depth,
        } => ThreadEvent::DelegationStarted {
            generation,
            session_key,
            from_agent,
            to_agent,
            query,
            depth,
        },
        agent::AgentEvent::DelegationCompleted {
            from_agent,
            to_agent,
            success,
            duration_ms,
        } => ThreadEvent::DelegationCompleted {
            generation,
            session_key,
            from_agent,
            to_agent,
            success,
            duration_ms,
        },
        agent::AgentEvent::PlanGenerated { steps, raw_plan } => ThreadEvent::PlanGenerated {
            generation,
            session_key,
            steps,
            raw_plan,
        },
        agent::AgentEvent::PlanStepCompleted {
            step_index,
            description,
            tool_name,
        } => ThreadEvent::PlanStepCompleted {
            generation,
            session_key,
            step_index,
            description,
            tool_name,
        },
        agent::AgentEvent::BudgetWarning {
            monthly_spend_usd,
            monthly_budget_usd,
            usage_percent,
        } => ThreadEvent::BudgetWarning {
            generation,
            session_key,
            monthly_spend_usd,
            monthly_budget_usd,
            usage_percent,
        },
        agent::AgentEvent::MemoryPromoted {
            fact_id,
            from_scope,
            to_scope,
            subject,
            predicate,
        } => ThreadEvent::MemoryPromoted {
            generation,
            session_key,
            fact_id,
            from_scope,
            to_scope,
            subject,
            predicate,
        },
        // Terminal events — these map to the `Terminal` variant.
        agent::AgentEvent::Done {
            content,
            message_id,
        } => ThreadEvent::Terminal {
            generation,
            session_key,
            kind: TerminalKind::Done {
                content,
                message_id,
            },
            transparency: None, // populated by relay before emit if available
        },
        agent::AgentEvent::Error { message } => ThreadEvent::Terminal {
            generation,
            session_key,
            kind: TerminalKind::Error { message },
            transparency: None,
        },
        agent::AgentEvent::Cancelled {
            partial_content,
            partial_reasoning,
        } => ThreadEvent::Terminal {
            generation,
            session_key,
            kind: TerminalKind::Cancelled {
                partial_content,
                partial_reasoning,
            },
            transparency: None,
        },

        // Telemetry / internal events with no v2 equivalent.
        agent::AgentEvent::ReasoningChunk { .. }
        | agent::AgentEvent::SubagentProgress { .. }
        | agent::AgentEvent::SubagentCompleted { .. }
        | agent::AgentEvent::SubagentCancelled { .. }
        | agent::AgentEvent::SkillActivationConsidered { .. }
        | agent::AgentEvent::SkillActivated { .. }
        | agent::AgentEvent::SkillReferenceLoaded { .. }
        | agent::AgentEvent::ContextEngineDecision { .. }
        | agent::AgentEvent::ToolCallStreamChunk { .. }
        | agent::AgentEvent::MCPSubcallTrace { .. }
        | agent::AgentEvent::ProviderRequest { .. }
        | agent::AgentEvent::ProviderResponse { .. }
        | agent::AgentEvent::MidLoopCompressionTriggered { .. }
        | agent::AgentEvent::TestRunDetailed { .. }
        | agent::AgentEvent::PowerModeToggled { .. }
        | agent::AgentEvent::TurnInterrupted { .. } => {
            tracing::debug!(?event, "agent event has no v2 equivalent");
            return None;
        }
        // Safety net for future AgentEvent variants.
        _ => {
            tracing::warn!(?event, "unknown agent event variant has no v2 equivalent");
            return None;
        }
    })
}
