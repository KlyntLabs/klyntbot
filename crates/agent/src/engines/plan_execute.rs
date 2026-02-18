//! PlanExecute engine — drives multi-step plan execution with rich context.
//!
//! Key improvements over legacy plan_executor.rs:
//! 1. Real parameter generation via step description in LLM prompt
//! 2. Rich step context (accumulated results from prior steps)
//! 3. Reflection checkpoints every N steps
//! 4. Structured outcome types

use common::Result;
use providers::Message;
use tools::RoutingContext;

use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams, ReasoningTrace, Scratchpad};

/// Outcome of plan execution.
#[derive(Debug)]
pub enum PlanExecuteOutcome {
    /// All steps completed successfully.
    Completed {
        plan_id: String,
        steps_executed: u32,
        final_summary: String,
    },
    /// A step failed after retries.
    Failed {
        plan_id: String,
        step: u32,
        reason: String,
    },
    /// Need more information to continue.
    NeedsMoreInfo { plan_id: String, question: String },
}

/// A step in a plan to execute.
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub description: String,
    pub expected_tool: Option<String>,
}

/// Drives multi-step plan execution with context accumulation and reflection.
pub struct PlanExecuteEngine {
    core: ExecutionCore,
    reflection_interval: u32,
    max_retries_per_step: u32,
}

impl PlanExecuteEngine {
    pub fn new(core: ExecutionCore) -> Self {
        Self {
            core,
            reflection_interval: 5,
            max_retries_per_step: 3,
        }
    }

    pub fn with_reflection_interval(mut self, interval: u32) -> Self {
        self.reflection_interval = interval;
        self
    }

    /// Execute a plan: run each step with accumulated context.
    pub async fn execute(
        &self,
        plan_id: &str,
        steps: &[PlanStep],
        base_messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<PlanExecuteOutcome> {
        let mut scratchpad = Scratchpad::new();
        let mut accumulated_results: Vec<String> = Vec::new();
        let mut steps_executed = 0u32;

        for (i, step) in steps.iter().enumerate() {
            let step_num = i as u32 + 1;

            // Build step-specific messages with accumulated context
            let mut messages = base_messages.clone();
            messages.push(Message::system(Self::build_step_prompt(
                step,
                step_num,
                steps.len() as u32,
                &accumulated_results,
                steps.get(i + 1..),
            )));
            messages.push(Message::user(format!(
                "Execute step {}: {}",
                step_num, step.description
            )));

            // Attempt step with retries
            let mut succeeded = false;
            let mut last_error = String::new();

            for attempt in 0..self.max_retries_per_step {
                let outcome = self
                    .core
                    .run_cycle(&mut messages, tools, params, ctx)
                    .await?;

                match outcome {
                    CycleOutcome::FinalResponse { content } => {
                        accumulated_results.push(format!(
                            "Step {}: {} → {}",
                            step_num, step.description, content
                        ));
                        scratchpad.add(ReasoningTrace {
                            cycle: step_num,
                            thought: format!("Executing step: {}", step.description),
                            planned_actions: vec![step.description.clone()],
                            actual_action: "completed".to_string(),
                            reflection: Some(content),
                            timestamp: chrono::Utc::now(),
                        });
                        succeeded = true;
                        break;
                    }
                    CycleOutcome::ToolsExecuted { results } => {
                        let summary: String = results
                            .iter()
                            .map(|r| {
                                format!(
                                    "{}({}): {}",
                                    r.tool_name,
                                    if r.success { "ok" } else { "err" },
                                    &r.result[..r.result.len().min(200)]
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        accumulated_results.push(format!(
                            "Step {}: {} → tools: {}",
                            step_num, step.description, summary
                        ));
                        scratchpad.add(ReasoningTrace {
                            cycle: step_num,
                            thought: format!("Executing step: {}", step.description),
                            planned_actions: vec![step.description.clone()],
                            actual_action: format!("tool calls (attempt {})", attempt + 1),
                            reflection: Some(summary),
                            timestamp: chrono::Utc::now(),
                        });
                        succeeded = true;
                        break;
                    }
                    CycleOutcome::EmptyResponse => {
                        last_error = "Empty response from LLM".to_string();
                        // Retry
                    }
                }
            }

            if !succeeded {
                return Ok(PlanExecuteOutcome::Failed {
                    plan_id: plan_id.to_string(),
                    step: step_num,
                    reason: last_error,
                });
            }

            steps_executed += 1;

            // Reflection checkpoint
            if self.reflection_interval > 0
                && steps_executed % self.reflection_interval == 0
                && (i + 1) < steps.len()
            {
                // Insert reflection as a trace entry
                scratchpad.add(ReasoningTrace {
                    cycle: step_num,
                    thought: "Reflection checkpoint".to_string(),
                    planned_actions: vec![],
                    actual_action: "reflect".to_string(),
                    reflection: Some(format!(
                        "Completed {}/{} steps. Remaining: {}",
                        steps_executed,
                        steps.len(),
                        steps.len() - i - 1
                    )),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        let summary = scratchpad.summarize();
        Ok(PlanExecuteOutcome::Completed {
            plan_id: plan_id.to_string(),
            steps_executed,
            final_summary: summary,
        })
    }

    fn build_step_prompt(
        step: &PlanStep,
        step_num: u32,
        total_steps: u32,
        accumulated_results: &[String],
        remaining: Option<&[PlanStep]>,
    ) -> String {
        let mut prompt = format!(
            "You are executing step {}/{} of a plan.\n\nCurrent step: {}\n",
            step_num, total_steps, step.description
        );

        if let Some(tool) = &step.expected_tool {
            prompt.push_str(&format!("Expected tool: {}\n", tool));
        }

        if !accumulated_results.is_empty() {
            prompt.push_str("\nPrevious results:\n");
            for result in accumulated_results {
                prompt.push_str(&format!("- {}\n", result));
            }
        }

        if let Some(remaining) = remaining {
            if !remaining.is_empty() {
                prompt.push_str("\nUpcoming steps:\n");
                for (j, s) in remaining.iter().enumerate().take(3) {
                    prompt.push_str(&format!(
                        "- Step {}: {}\n",
                        step_num + j as u32 + 1,
                        s.description
                    ));
                }
            }
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, DynProvider, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    // ── Mock provider ────────────────────────────────────────────

    struct MockProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl MockProvider {
        fn with_responses(responses: Vec<LlmResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
        }

        fn text_response(text: &str) -> LlmResponse {
            LlmResponse {
                content: Some(text.to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            }
        }

        fn empty_response() -> LlmResponse {
            LlmResponse {
                content: None,
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(MockProvider::text_response("default"))
            } else {
                Ok(responses.remove(0))
            }
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    fn make_engine(provider: Arc<MockProvider>) -> PlanExecuteEngine {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = ExecutionCore::new(provider as DynProvider, registry);
        PlanExecuteEngine::new(core)
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    #[tokio::test]
    async fn test_plan_executes_all_steps() {
        let provider = MockProvider::with_responses(vec![
            MockProvider::text_response("Step 1 done"),
            MockProvider::text_response("Step 2 done"),
            MockProvider::text_response("Step 3 done"),
        ]);
        let engine = make_engine(provider);

        let steps = vec![
            PlanStep {
                description: "Initialize project".to_string(),
                expected_tool: None,
            },
            PlanStep {
                description: "Create files".to_string(),
                expected_tool: Some("write_file".to_string()),
            },
            PlanStep {
                description: "Run tests".to_string(),
                expected_tool: Some("shell".to_string()),
            },
        ];

        let result = engine
            .execute(
                "plan-1",
                &steps,
                vec![Message::system("You are a coding assistant.")],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
            )
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Completed {
                plan_id,
                steps_executed,
                ..
            } => {
                assert_eq!(plan_id, "plan-1");
                assert_eq!(steps_executed, 3);
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_failed_step_returns_failed_outcome() {
        // First step succeeds, second step returns empty (fails after retries)
        let provider = MockProvider::with_responses(vec![
            MockProvider::text_response("Step 1 ok"),
            MockProvider::empty_response(),
            MockProvider::empty_response(),
            MockProvider::empty_response(), // 3 retries exhausted
        ]);
        let engine = make_engine(provider);

        let steps = vec![
            PlanStep {
                description: "Step A".to_string(),
                expected_tool: None,
            },
            PlanStep {
                description: "Step B".to_string(),
                expected_tool: None,
            },
        ];

        let result = engine
            .execute(
                "plan-2",
                &steps,
                vec![],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
            )
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Failed {
                plan_id,
                step,
                reason,
            } => {
                assert_eq!(plan_id, "plan-2");
                assert_eq!(step, 2);
                assert!(reason.contains("Empty response"));
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_step_prompt_contains_description_and_context() {
        let prompt = PlanExecuteEngine::build_step_prompt(
            &PlanStep {
                description: "Create user model".to_string(),
                expected_tool: Some("write_file".to_string()),
            },
            2,
            5,
            &["Step 1: Setup database → Schema created".to_string()],
            Some(&[PlanStep {
                description: "Add validation".to_string(),
                expected_tool: None,
            }]),
        );

        assert!(prompt.contains("step 2/5"), "Should contain step number");
        assert!(
            prompt.contains("Create user model"),
            "Should contain step description"
        );
        assert!(
            prompt.contains("write_file"),
            "Should contain expected tool"
        );
        assert!(
            prompt.contains("Schema created"),
            "Should contain previous results"
        );
        assert!(
            prompt.contains("Add validation"),
            "Should contain upcoming steps"
        );
    }

    #[tokio::test]
    async fn test_empty_plan_completes_immediately() {
        let provider = MockProvider::with_responses(vec![]);
        let engine = make_engine(provider);

        let result = engine
            .execute(
                "plan-empty",
                &[],
                vec![],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
            )
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Completed { steps_executed, .. } => {
                assert_eq!(steps_executed, 0);
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }
}
