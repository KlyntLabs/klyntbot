//! ReactiveEngine — ReAct loop with mid-execution escalation.
//!
//! Ported from `execution/react_plus.rs` with these changes:
//! - Returns `EngineResult` instead of `ReactOutcome`
//! - Tracks `completed_work` for escalation context preservation
//! - Implements `ExecutionEngine` trait

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tracing::debug;

use common::{utils::tool_def_name, Result};
use providers::Message;
use tools::RoutingContext;

use super::{EngineResult, ExecutionEngine};
use crate::execution::scratchpad::{ReasoningTrace, Scratchpad};
use crate::execution::types::{accumulate_usage, CycleOutcome, ExecutionParams};
use crate::execution::ExecutionCore;
use crate::intent_pipeline::router::{CompletedStep, EscalationContext};

/// ReactiveEngine — ReAct loop that escalates when complexity exceeds capacity.
pub struct ReactiveEngine {
    core: Arc<ExecutionCore>,
    max_iterations: u32,
}

impl ReactiveEngine {
    pub fn new(core: Arc<ExecutionCore>, max_iterations: u32) -> Self {
        Self {
            core,
            max_iterations,
        }
    }
}

#[async_trait]
impl ExecutionEngine for ReactiveEngine {
    async fn execute(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        let mut messages = messages;
        let mut scratchpad = Scratchpad::new();
        // Use per-request max_iterations from params, fall back to engine default
        let max_iterations = if params.max_iterations > 0 {
            params.max_iterations
        } else {
            self.max_iterations
        };
        let escalation_threshold = (max_iterations as f32 * 0.8).ceil() as u32;
        let mut accumulated_usage = providers::Usage::default();
        let mut fabrication_retries = 0u32;
        let max_fabrication_retries = params.max_fabrication_retries;
        let mut seen_tool_calls: HashSet<String> = HashSet::new();
        let mut last_tool_name: Option<String> = None;
        let mut completed_work: Vec<CompletedStep> = Vec::new();

        for iteration in 1..=max_iterations {
            // Check cancellation
            if let Some(ref token) = params.cancel_token {
                if token.is_cancelled() {
                    return Ok(EngineResult::Complete {
                        content: String::new(),
                        usage: accumulated_usage,
                        iterations: iteration - 1,
                        traces: scratchpad.traces().to_vec(),
                        tool_name: last_tool_name,
                    });
                }
            }

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(crate::events::AgentEvent::IterationStart {
                        iteration: iteration as usize,
                        max: max_iterations as usize,
                    })
                    .await;
            }

            let (outcome, cycle_usage) = self
                .core
                .run_cycle(
                    &mut messages,
                    tools,
                    params,
                    ctx,
                    event_tx.as_ref(),
                    Some(&mut seen_tool_calls),
                )
                .await?;
            accumulate_usage(&mut accumulated_usage, &cycle_usage);

            match outcome {
                CycleOutcome::FinalResponse { content } => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received final response".to_string(),
                        planned_actions: vec![],
                        actual_action: "final_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });

                    return Ok(EngineResult::Complete {
                        content,
                        usage: accumulated_usage,
                        iterations: iteration,
                        traces: scratchpad.traces().to_vec(),
                        tool_name: last_tool_name,
                    });
                }

                CycleOutcome::FabricatedResponse { content } => {
                    fabrication_retries += 1;
                    if fabrication_retries > max_fabrication_retries {
                        debug!(
                            "ReactiveEngine: fabrication retries exhausted ({}), returning as-is",
                            max_fabrication_retries
                        );
                        return Ok(EngineResult::Complete {
                            content,
                            usage: accumulated_usage,
                            iterations: iteration,
                            traces: scratchpad.traces().to_vec(),
                            tool_name: last_tool_name,
                        });
                    }
                    let tool_list: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();

                    messages.push(Message::user(format!(
                        "You returned a text response instead of calling a tool. \
                         You have these tools available: [{}]. \
                         You MUST call the appropriate tool to complete the user's request. \
                         Do NOT respond with text describing what you would do — actually call the tool.",
                        tool_list.join(", ")
                    )));

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Detected fabricated tool response — forcing retry".to_string(),
                        planned_actions: tool_list.iter().map(|s| s.to_string()).collect(),
                        actual_action: "fabrication_retry".to_string(),
                        reflection: Some(
                            "LLM returned text instead of tool call, injecting force prompt"
                                .to_string(),
                        ),
                        timestamp: Utc::now(),
                    });
                }

                CycleOutcome::ToolsExecuted { results } => {
                    let tool_names: Vec<String> =
                        results.iter().map(|r| r.tool_name.clone()).collect();
                    if let Some(name) = tool_names.last() {
                        last_tool_name = Some(name.clone());
                    }

                    // Track completed work for potential escalation
                    for r in &results {
                        if r.success {
                            completed_work.push(CompletedStep {
                                description: format!("Called {} tool", r.tool_name),
                                tool_name: r.tool_name.clone(),
                                result: r.result.clone(),
                            });
                        }
                    }

                    let had_failure = results.iter().any(|r| !r.success);
                    let failure_details: Vec<String> = results
                        .iter()
                        .filter(|r| !r.success)
                        .map(|r| format!("{}: {}", r.tool_name, r.result))
                        .collect();

                    // Duplicate handling
                    let all_skipped_duplicates = !results.is_empty()
                        && results
                            .iter()
                            .all(|r| !r.success && r.result.starts_with("Skipped:"));

                    let mut reflection = None;

                    if all_skipped_duplicates {
                        debug!(
                            "ReactiveEngine: blocked duplicate tool calls on iteration {}: {:?}",
                            iteration, tool_names
                        );
                        let dup_prompt = format!(
                            "You just attempted to call the same tool(s) with the same arguments as a previous iteration: [{}]. \
                             The calls were blocked. Do NOT repeat these calls. Either proceed with a final response or take a DIFFERENT action.",
                            tool_names.join(", ")
                        );
                        messages.push(Message::user(&dup_prompt));
                        reflection = Some(dup_prompt);
                    }

                    // Reflection on failure
                    if reflection.is_none() && had_failure {
                        let reflection_prompt = format!(
                            "Reflection: Tool failures occurred: {}. What went wrong and how should I adjust my approach?",
                            failure_details.join("; ")
                        );
                        messages.push(Message::user(&reflection_prompt));
                        reflection = Some(reflection_prompt);
                    }

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: format!("Executed {} tool(s)", tool_names.len()),
                        planned_actions: tool_names,
                        actual_action: "tools_executed".to_string(),
                        reflection,
                        timestamp: Utc::now(),
                    });

                    // Check escalation threshold
                    if iteration >= escalation_threshold {
                        return Ok(EngineResult::Escalate {
                            reason: format!(
                                "Used {}% of max iterations ({}/{}), task may need planning",
                                (iteration * 100) / max_iterations,
                                iteration,
                                max_iterations
                            ),
                            carried_context: EscalationContext {
                                messages,
                                completed_work,
                                original_message: params.original_message.clone(),
                            },
                            usage: accumulated_usage,
                        });
                    }
                }

                CycleOutcome::EmptyResponse => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received empty response from LLM".to_string(),
                        planned_actions: vec![],
                        actual_action: "empty_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        // Max iterations reached without final response — complete with empty
        Ok(EngineResult::Complete {
            content: String::new(),
            usage: accumulated_usage,
            iterations: max_iterations,
            traces: scratchpad.traces().to_vec(),
            tool_name: last_tool_name,
        })
    }

    fn mode(&self) -> &str {
        "reactive"
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;
    use tokio::sync::RwLock;
    use tools::{registry::ToolRegistry, Tool};

    struct OkTool;

    #[async_trait]
    impl Tool for OkTool {
        fn name(&self) -> &str {
            "ok_tool"
        }
        fn description(&self) -> &str {
            "Always succeeds"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("success".to_string())
        }
    }

    fn registry_with_ok_tool() -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(OkTool);
        Arc::new(RwLock::new(reg))
    }

    #[tokio::test]
    async fn reactive_completes_simple_tool_use() {
        let provider =
            MockSequenceProvider::new(vec![tool_call_response("ok_tool"), text_response("Done!")]);
        let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
        let engine = ReactiveEngine::new(core, 10);

        let result = engine
            .execute(
                vec![Message::user("do something")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete {
                content,
                iterations,
                ..
            } => {
                assert!(content.contains("Done"));
                assert_eq!(iterations, 2);
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[tokio::test]
    async fn reactive_escalates_on_complexity() {
        // With max_iterations=5, escalation at ceil(4.0) = 4
        let responses: Vec<_> = (0..10).map(|_| tool_call_response("ok_tool")).collect();
        let provider = MockSequenceProvider::new(responses);
        let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
        let engine = ReactiveEngine::new(core, 5);

        let result = engine
            .execute(
                vec![Message::user("complex task")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Escalate {
                reason,
                carried_context,
                ..
            } => {
                assert!(reason.contains("80%"));
                assert!(!carried_context.completed_work.is_empty());
            }
            EngineResult::Complete { .. } => panic!("Expected Escalate, got Complete"),
        }
    }

    #[tokio::test]
    async fn completed_work_tracks_successful_tools() {
        let provider = MockSequenceProvider::new(vec![
            tool_call_response("ok_tool"),
            tool_call_response("ok_tool"),
            text_response("All done"),
        ]);
        let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
        let engine = ReactiveEngine::new(core, 10);

        let result = engine
            .execute(
                vec![Message::user("multi-step")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete { traces, .. } => {
                assert_eq!(traces.len(), 3); // 2 tools + final
                assert_eq!(traces[0].actual_action, "tools_executed");
                assert_eq!(traces[1].actual_action, "tools_executed");
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete"),
        }
    }

    #[tokio::test]
    async fn respects_per_request_max_iterations() {
        // max_iterations=3 in params, engine default is 10
        let responses: Vec<_> = (0..10).map(|_| tool_call_response("ok_tool")).collect();
        let provider = MockSequenceProvider::new(responses);
        let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
        let engine = ReactiveEngine::new(core, 10); // default 10

        let params = default_params().with_max_iterations(3);
        let result = engine
            .execute(
                vec![Message::user("short task")],
                &[],
                &params,
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Escalate { reason, .. } => {
                assert!(
                    reason.contains("3"),
                    "reason should reference 3 iterations: {}",
                    reason
                );
            }
            EngineResult::Complete { iterations, .. } => {
                assert!(iterations <= 3, "should not exceed 3 iterations");
            }
        }
    }

    #[tokio::test]
    async fn checks_cancel_token() {
        let token = tokio_util::sync::CancellationToken::new();
        token.cancel(); // pre-cancel

        let provider = MockSequenceProvider::new(vec![tool_call_response("ok_tool")]);
        let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
        let engine = ReactiveEngine::new(core, 10);

        let params = default_params().with_cancel_token(token);
        let result = engine
            .execute(
                vec![Message::user("test")],
                &[],
                &params,
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        // Should return early due to cancellation
        if let EngineResult::Complete {
            content,
            iterations,
            ..
        } = result
        {
            assert!(
                content.is_empty() || content.contains("cancelled"),
                "content should be empty or mention cancelled"
            );
            assert_eq!(iterations, 0, "should not have completed any iterations");
        }
    }

    #[test]
    fn mode_returns_reactive() {
        let engine = ReactiveEngine::new(make_core(MockSequenceProvider::new(vec![])), 10);
        assert_eq!(engine.mode(), "reactive");
    }
}
