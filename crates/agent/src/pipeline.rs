//! Agent pipeline — wires together the full Adaptive Orchestrator pipeline:
//! Orchestrator → ContextEngine → EngineDispatch → ResponseValidator → CostTracker
//!
//! This is the main entry point for processing user messages through the
//! adaptive orchestration system.

use std::sync::Arc;
use std::time::Instant;

use common::Result;
use context_engine::{ContextEngine, ContextRequest};
use providers::Message;
use tools::RoutingContext;
use tracing::{debug, info, warn};

use crate::events::AgentEvent;

use crate::execution::{EngineDispatch, ExecutionParams};
use crate::orchestrator::{ClassificationResult, Orchestrator};
use crate::output::cost_tracker::CostTracker;
use crate::output::validator::{ResponseValidator, ValidationResult};

/// Result of processing a message through the full pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    /// The final response content.
    pub content: String,
    /// Which strategy was used (may differ from classified due to escalation).
    pub strategy_used: String,
    /// Classification details.
    pub classification: ClassificationResult,
    /// Number of escalations during execution.
    pub escalations: u32,
    /// Validation warnings (if any).
    pub validation: ValidationResult,
}

/// Configuration for the pipeline.
pub struct PipelineConfig {
    /// Model name for execution (tool calls, responses).
    pub execution_model: String,
    /// System prompt to prepend.
    pub system_prompt: String,
    /// Context window size in tokens.
    pub context_window: usize,
    /// Maximum response tokens for validation.
    pub max_response_tokens: usize,
    /// Channel name for cost tracking.
    pub channel: String,
    /// Provider name for cost tracking.
    pub provider_name: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            execution_model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: "You are a helpful assistant.".to_string(),
            context_window: 128_000,
            max_response_tokens: 4096,
            channel: "unknown".to_string(),
            provider_name: "unknown".to_string(),
        }
    }
}

/// The main agent pipeline that wires together all orchestration components.
pub struct AgentPipeline {
    orchestrator: Arc<Orchestrator>,
    context_engine: ContextEngine,
    engine_dispatch: Arc<EngineDispatch>,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
}

impl AgentPipeline {
    pub fn new(
        orchestrator: Arc<Orchestrator>,
        engine_dispatch: Arc<EngineDispatch>,
        cost_tracker: Arc<CostTracker>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            orchestrator,
            context_engine: ContextEngine::new(),
            engine_dispatch,
            validator: ResponseValidator::new(config.max_response_tokens),
            cost_tracker,
            config,
        }
    }

    /// Process a user message through the full pipeline.
    ///
    /// 1. Classify intent (heuristics → LLM)
    /// 2. Assemble context with budget allocation
    /// 3. Execute with appropriate engine (with escalation)
    /// 4. Validate response
    /// 5. Record usage
    #[allow(clippy::too_many_arguments)]
    pub async fn process_message(
        &self,
        message: &str,
        history: Vec<Message>,
        tool_definitions: &[serde_json::Value],
        tool_names: &[&str],
        ctx: &RoutingContext,
        system_prompt: Option<&str>,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    ) -> Result<PipelineResult> {
        // Step 1: Classify (with timing)
        let classify_start = Instant::now();
        let classification = self.orchestrator.classify(message, tool_names).await;
        let classify_ms = classify_start.elapsed().as_millis() as u64;
        debug!(
            "Pipeline: classified as {:?} (source: {:?}, confidence: {:.2})",
            classification.strategy, classification.source, classification.confidence
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ClassificationComplete {
                    strategy: format!("{:?}", classification.strategy),
                    confidence: classification.confidence,
                    source: format!("{:?}", classification.source),
                    duration_ms: classify_ms,
                })
                .await;
        }

        // Use provided system_prompt or fall back to config default
        let prompt = system_prompt.unwrap_or(&self.config.system_prompt);

        // Step 2: Assemble context (with timing)
        let context_request = ContextRequest {
            message_text: message.to_string(),
            history,
            system_prompt: prompt.to_string(),
            strategy: classification.strategy.clone(),
            tool_definitions: tool_definitions.to_vec(),
            context_window: self.config.context_window,
        };
        let assemble_start = Instant::now();
        let assembled = self.context_engine.assemble(context_request).await;
        let assemble_ms = assemble_start.elapsed().as_millis() as u64;

        debug!(
            "Pipeline: assembled context with {} messages, {} tokens",
            assembled.messages.len(),
            assembled.token_count
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ContextAssembled {
                    total_tokens: assembled.token_count,
                    budget: self.config.context_window,
                    duration_ms: assemble_ms,
                })
                .await;
        }

        // Step 3: Execute
        if let Some(ref tx) = event_tx {
            let engine_name = format!("{:?}", classification.strategy);
            let max_iter = match &classification.strategy {
                context_engine::ExecutionStrategy::ToolAssisted { max_iterations } => {
                    *max_iterations as usize
                }
                context_engine::ExecutionStrategy::AutonomousTask { max_iterations } => {
                    *max_iterations as usize
                }
                _ => 1,
            };
            let _ = tx
                .send(AgentEvent::ExecutionStarted {
                    engine: engine_name,
                    max_iterations: max_iter,
                })
                .await;
        }

        let params = ExecutionParams::new(&self.config.execution_model);
        let dispatch_result = self
            .engine_dispatch
            .execute(
                classification.strategy.clone(),
                Arc::new(assembled.messages),
                tool_definitions,
                &params,
                ctx,
                event_tx,
            )
            .await?;

        if dispatch_result.escalation_count > 0 {
            info!(
                "Pipeline: escalated {} time(s), final strategy: {:?}",
                dispatch_result.escalation_count, dispatch_result.final_strategy
            );
        }

        // Step 4: Validate
        let validation = self.validator.validate(&dispatch_result.content);
        if !validation.is_valid {
            warn!(
                "Pipeline: response validation failed with {} warning(s)",
                validation.warnings.len()
            );
        }

        let final_content = validation.filtered_content.clone();
        let strategy_name = format!("{:?}", dispatch_result.final_strategy);

        // Step 5: Record usage (best-effort, don't fail the pipeline)
        let usage = &dispatch_result.usage;
        if let Err(e) = self
            .cost_tracker
            .record(
                usage,
                &self.config.execution_model,
                &self.config.provider_name,
                &strategy_name,
                ctx.channel.as_str(),
            )
            .await
        {
            warn!("Pipeline: failed to record usage: {}", e);
        }

        Ok(PipelineResult {
            content: final_content,
            strategy_used: strategy_name,
            classification,
            escalations: dispatch_result.escalation_count,
            validation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    use crate::execution::ExecutionCore;

    // ── Mock Provider ──

    struct MockPipelineProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl MockPipelineProvider {
        fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for MockPipelineProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(LlmResponse {
                    content: Some("fallback response".to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                })
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

    // ── Helpers ──

    fn text_response(text: &str) -> LlmResponse {
        LlmResponse {
            content: Some(text.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    async fn make_pipeline(provider: Arc<dyn LlmProvider>) -> AgentPipeline {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(provider.clone(), registry));
        let orchestrator = Arc::new(Orchestrator::new(provider.clone(), "mock"));
        let dispatch = Arc::new(EngineDispatch::new(core));

        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("in-memory SQLite for tests");
        let usage_repo = storage::UsageRepo::new(pool);
        let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));

        AgentPipeline::new(
            orchestrator,
            dispatch,
            cost_tracker,
            PipelineConfig::default(),
        )
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_direct_response_pipeline() {
        // "hello" → heuristic classifies as DirectResponse → single LLM call
        let provider = MockPipelineProvider::new(vec![text_response("Hi there!")]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        assert_eq!(result.content, "Hi there!");
        assert!(result.strategy_used.contains("DirectResponse"));
        assert_eq!(result.escalations, 0);
        assert!(result.validation.is_valid);
    }

    #[tokio::test]
    async fn test_tool_assisted_pipeline() {
        // "show my tasks" → heuristic classifies as ToolAssisted
        // Provider returns a text response (no actual tool calls)
        let provider = MockPipelineProvider::new(vec![text_response("Here are your tasks: ...")]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message(
                "show my tasks",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.content, "Here are your tasks: ...");
        assert!(result.strategy_used.contains("ToolAssisted"));
        assert_eq!(result.escalations, 0);
    }

    #[tokio::test]
    async fn test_pipeline_validates_response() {
        // Empty response triggers validation warning
        let provider = MockPipelineProvider::new(vec![LlmResponse {
            content: Some(String::new()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message("hi", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        // Empty response should trigger validation warnings
        assert!(!result.validation.is_valid);
    }

    #[tokio::test]
    async fn test_e2e_escalation_from_direct_to_tool_assisted() {
        // "hello" → DirectResponse, but LLM requests tool calls → escalation
        let provider = MockPipelineProvider::new(vec![
            // First call: tool call response (triggers escalation)
            LlmResponse {
                content: None,
                tool_calls: vec![providers::ToolCall {
                    id: "call_1".to_string(),
                    name: "web_search".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Second call: final text response after escalation
            text_response("I found the answer after escalation"),
        ]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        assert_eq!(result.escalations, 1);
        assert!(result.strategy_used.contains("ToolAssisted"));
        assert!(result.content.contains("found the answer"));
    }

    #[tokio::test]
    async fn test_e2e_autonomous_task_path() {
        // "write me a script to automate deployments" → AutonomousTask
        let provider = MockPipelineProvider::new(vec![text_response(
            "Here is the deployment script:\n```bash\necho deploy\n```",
        )]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message(
                "write me a script to automate deployments",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
            )
            .await
            .unwrap();

        assert!(result.strategy_used.contains("AutonomousTask"));
        assert!(result.content.contains("deployment script"));
        assert_eq!(result.escalations, 0);
    }

    #[tokio::test]
    async fn test_e2e_with_conversation_history() {
        // Pipeline works with conversation history
        let provider = MockPipelineProvider::new(vec![text_response(
            "Based on our earlier discussion, here's my answer.",
        )]);
        let pipeline = make_pipeline(provider).await;

        let history = vec![
            Message::user("Tell me about Rust"),
            Message::assistant("Rust is a systems programming language..."),
        ];

        let result = pipeline
            .process_message(
                "what is Rust?",
                history,
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
            )
            .await
            .unwrap();

        assert!(result.validation.is_valid);
        assert!(result.content.contains("earlier discussion"));
    }

    #[tokio::test]
    async fn test_e2e_classification_confidence_tracking() {
        // Verify classification metadata is preserved
        let provider = MockPipelineProvider::new(vec![text_response("Done")]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        // "hello" is classified by heuristic with confidence 1.0
        assert!((result.classification.confidence - 1.0f32).abs() < f32::EPSILON);
        assert_eq!(
            result.classification.source,
            crate::orchestrator::ClassificationSource::Heuristic
        );
    }

    #[tokio::test]
    async fn test_pipeline_emits_classification_event() {
        use crate::events::AgentEvent;
        use tokio::sync::mpsc;

        let provider = MockPipelineProvider::new(vec![text_response("Hi!")]);
        let pipeline = make_pipeline(provider).await;
        let (event_tx, mut event_rx) = mpsc::channel(64);

        let _result = pipeline
            .process_message(
                "hello",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                Some(event_tx),
            )
            .await
            .unwrap();

        // Should have received ClassificationComplete event
        let mut found_classification = false;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, AgentEvent::ClassificationComplete { .. }) {
                found_classification = true;
            }
        }
        assert!(
            found_classification,
            "Expected ClassificationComplete event"
        );
    }
}
