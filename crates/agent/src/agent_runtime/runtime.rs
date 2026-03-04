//! AgentRuntime — replaces IntentPipeline with agent-first execution.
//!
//! Flow: AgentManager.match_agent → set active profile → IntentAnalyzer →
//! ContextEngine → tool filtering via AgentProfile → ExecutionRouter →
//! ResponseValidator → CostTracker → StrategyRepo

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use common::{utils::tool_def_name, Result};
use context_engine::{ContextEngine, ContextRequest, ExecutionStrategy};
use providers::Message;
use tokio::sync::RwLock;
use tools::RoutingContext;
use tracing::{debug, warn};

use crate::agent_profile::{AgentManager, AgentProfile};
use crate::events::AgentEvent;
use crate::execution::ExecutionParams;
use crate::intent_pipeline::analysis::IntentAnalyzer;
use crate::intent_pipeline::pipeline::PipelineConfig;
use crate::intent_pipeline::router::{ExecutionRouter, RouterResult};
use crate::intent_pipeline::types::IntentAnalysis;
use crate::output::cost_tracker::CostTracker;
use crate::output::validator::{ResponseValidator, ValidationResult};

const CLARIFICATION_MODE: &str = "clarification";

/// Result of processing a message through the agent runtime.
/// Same structure as `PipelineResult` for compatibility during migration.
#[derive(Debug)]
pub struct RuntimeResult {
    /// The final response content.
    pub content: String,
    /// Which execution mode actually produced the result.
    pub mode_used: String,
    /// Full intent analysis from the classifier.
    pub classification: IntentAnalysis,
    /// Validation warnings (if any).
    pub validation: ValidationResult,
    /// Name of the agent that handled this message.
    pub agent_name: String,
}

/// Agent-driven runtime that replaces IntentPipeline.
///
/// The key difference: agent selection happens first, and the agent profile
/// shapes everything downstream (system prompt, tool filtering, iteration budget).
pub struct AgentRuntime {
    agent_manager: Arc<AgentManager>,
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
    confidence_evaluator: Option<Arc<crate::confidence::ConfidenceEvaluator>>,
    /// Shared with AgentContextSource — written here, read during context assembly.
    active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>,
}

impl AgentRuntime {
    pub fn new(
        agent_manager: Arc<AgentManager>,
        analyzer: IntentAnalyzer,
        context_engine: Arc<ContextEngine>,
        router: ExecutionRouter,
        cost_tracker: Arc<CostTracker>,
        config: PipelineConfig,
        active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>,
    ) -> Self {
        Self {
            agent_manager,
            analyzer,
            context_engine,
            router,
            validator: ResponseValidator::new(config.max_response_tokens),
            cost_tracker,
            config,
            strategy_repo: None,
            confidence_evaluator: None,
            active_profile,
        }
    }

    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    pub fn with_confidence_evaluator(
        mut self,
        evaluator: Arc<crate::confidence::ConfidenceEvaluator>,
    ) -> Self {
        self.confidence_evaluator = Some(evaluator);
        self
    }

    /// Get the shared active profile handle (for AgentContextSource).
    pub fn active_profile_handle(&self) -> Arc<RwLock<Option<Arc<AgentProfile>>>> {
        Arc::clone(&self.active_profile)
    }

    /// Process a user message through the agent runtime.
    ///
    /// 1. Match message to agent profile
    /// 2. Set active profile (read by AgentContextSource during context assembly)
    /// 3. Classify intent (heuristics → LLM classifier)
    /// 4. Override max_iterations from agent profile
    /// 5. Confidence check
    /// 6. Assemble context (includes agent instructions via AgentContextSource)
    /// 7. Filter tools based on agent profile (not ToolGroup)
    /// 8. Execute via router
    /// 9. Validate response
    /// 10. Record usage + strategy
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
        cancel_token: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<RuntimeResult> {
        let pipeline_start = Instant::now();

        // Step 1: Match message to agent
        let profile = self.agent_manager.match_agent(message);
        let agent_name = profile.name.clone();
        debug!("AgentRuntime: matched agent '{}'", agent_name);

        // Step 2: Set active profile for AgentContextSource
        {
            let mut guard = self.active_profile.write().await;
            *guard = Some(Arc::clone(profile));
        }

        // Step 3: Classify intent
        let classify_start = Instant::now();
        let mut analysis = self.analyzer.analyze(message, tool_names).await;
        let classify_ms = classify_start.elapsed().as_millis() as u64;
        debug!(
            "AgentRuntime: classified as {:?} (source: {:?}, confidence: {:.2})",
            analysis.mode, analysis.source, analysis.confidence
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ClassificationComplete {
                    strategy: analysis.mode.to_string(),
                    confidence: analysis.confidence,
                    source: format!("{:?}", analysis.source),
                    duration_ms: classify_ms,
                })
                .await;
        }

        // Step 4: Override max_iterations from agent profile
        if let crate::intent_pipeline::types::ExecutionMode::Reactive {
            ref mut max_iterations,
        } = analysis.mode
        {
            *max_iterations = (*max_iterations).min(profile.max_iterations);
        }

        // Step 5: Confidence check
        if let Some(ref evaluator) = self.confidence_evaluator {
            let threshold = evaluator.threshold();
            if analysis.confidence < threshold {
                debug!(
                    "AgentRuntime: low confidence ({:.2} < {:.2}), suggesting clarification",
                    analysis.confidence, threshold
                );
                let content = format!(
                    "I'm not entirely sure what you'd like me to do. Could you clarify? \
                     I interpreted this as a {} request (confidence: {:.0}%).",
                    analysis.mode.short_name(),
                    analysis.confidence * 100.0
                );
                return Ok(RuntimeResult {
                    content,
                    mode_used: CLARIFICATION_MODE.to_string(),
                    classification: analysis,
                    validation: ValidationResult {
                        is_valid: true,
                        warnings: vec![],
                        filtered_content: String::new(),
                    },
                    agent_name,
                });
            }
        }

        // Step 6: Assemble context (AgentContextSource injects agent instructions + skills)
        let prompt = system_prompt.unwrap_or(&self.config.system_prompt);
        let strategy = ExecutionStrategy::from(&analysis.mode);

        let context_request = ContextRequest {
            message_text: message.to_string(),
            history,
            system_prompt: prompt.to_string(),
            strategy,
            tool_definitions: tool_definitions.to_vec(),
            context_window: self.config.context_window,
        };
        let assemble_start = Instant::now();
        let assembled = self.context_engine.assemble(context_request).await;
        let assemble_ms = assemble_start.elapsed().as_millis() as u64;

        debug!(
            "AgentRuntime: assembled context with {} messages, {} tokens",
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

        // Step 7: Filter tools based on agent profile (replaces ToolGroup filtering)
        let filtered_tools: Cow<'_, [serde_json::Value]> =
            if let Some(allowed) = profile.allowed_tool_names() {
                Cow::Owned(
                    tool_definitions
                        .iter()
                        .filter(|t| {
                            tool_def_name(t)
                                .map(|name| {
                                    // MCP tools bypass agent filtering
                                    name.starts_with("mcp_") || allowed.contains(name)
                                })
                                .unwrap_or(true)
                        })
                        .cloned()
                        .collect(),
                )
            } else {
                Cow::Borrowed(tool_definitions)
            };

        debug!(
            "AgentRuntime: filtered {} → {} tools (agent: {})",
            tool_definitions.len(),
            filtered_tools.len(),
            agent_name
        );

        // Step 8: Execute via router
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ExecutionStarted {
                    engine: analysis.mode.to_string(),
                    max_iterations: analysis.mode.max_iterations() as usize,
                })
                .await;
        }

        let mut params = ExecutionParams::new(&self.config.execution_model)
            .with_max_iterations(analysis.mode.max_iterations())
            .with_original_message(message.to_string());

        if let Some(token) = cancel_token {
            params = params.with_cancel_token(token);
        }

        let router_result = self
            .router
            .execute(
                analysis.mode.clone(),
                assembled.messages,
                &filtered_tools,
                &params,
                ctx,
                event_tx.clone(),
            )
            .await?;

        // Step 9: Validate
        let mut validation = self.validator.validate(&router_result.content);
        if !validation.is_valid {
            warn!(
                "AgentRuntime: response validation failed with {} warning(s)",
                validation.warnings.len()
            );
        }

        let mode_name = router_result.final_mode.clone();

        // Step 10: Record usage + strategy
        let pipeline_elapsed_ms = pipeline_start.elapsed().as_millis() as u64;
        self.record_usage(
            &router_result,
            &mode_name,
            ctx,
            &event_tx,
            pipeline_elapsed_ms,
        )
        .await;
        self.record_strategy(&analysis, &router_result, &validation, ctx, pipeline_start)
            .await;

        let final_content = std::mem::take(&mut validation.filtered_content);

        Ok(RuntimeResult {
            content: final_content,
            mode_used: mode_name,
            classification: analysis,
            validation,
            agent_name,
        })
    }

    async fn record_usage(
        &self,
        result: &RouterResult,
        mode_name: &str,
        ctx: &RoutingContext,
        event_tx: &Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        pipeline_elapsed_ms: u64,
    ) {
        let cost =
            crate::output::cost_tracker::estimate_cost(&result.usage, &self.config.execution_model);

        if let Err(e) = self
            .cost_tracker
            .record(
                &result.usage,
                &self.config.execution_model,
                &self.config.provider_name,
                mode_name,
                ctx.channel.as_str(),
            )
            .await
        {
            warn!("AgentRuntime: failed to record usage: {}", e);
        }

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::UsageReport {
                    prompt_tokens: result.usage.prompt_tokens,
                    completion_tokens: result.usage.completion_tokens,
                    cache_read_tokens: result.usage.cache_read_tokens,
                    cache_write_tokens: result.usage.cache_write_tokens,
                    estimated_cost_usd: cost,
                    model: self.config.execution_model.clone(),
                    response_time_ms: pipeline_elapsed_ms,
                })
                .await;
        }
    }

    async fn record_strategy(
        &self,
        analysis: &IntentAnalysis,
        result: &RouterResult,
        validation: &ValidationResult,
        ctx: &RoutingContext,
        start: Instant,
    ) {
        let Some(ref strategy_repo) = self.strategy_repo else {
            return;
        };

        let elapsed_ms = start.elapsed().as_millis() as i64;

        let record = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
            predicted_strategy: analysis.mode.to_string(),
            actual_strategy: result.final_mode.clone(),
            escalation_count: 0,
            iterations_used: result.iterations as i32,
            max_iterations: analysis.mode.max_iterations() as i32,
            success: validation.is_valid,
            user_satisfaction: None,
            response_time_ms: elapsed_ms,
            chat_id: Some(ctx.chat_id.to_string()),
            tool_name: result.tool_name.clone(),
            tool_success: result.tool_name.as_ref().map(|_| validation.is_valid),
            tool_duration_ms: result.tool_name.as_ref().map(|_| elapsed_ms),
            complexity_signals: serde_json::to_value(&analysis.signals).unwrap_or_default(),
            execution_mode: Some(result.final_mode.clone()),
        };

        if let Err(e) = strategy_repo.create(&record).await {
            warn!("AgentRuntime: failed to record strategy: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use config::OrchestratorConfig;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tools::registry::ToolRegistry;

    use crate::execution::ExecutionCore;
    use crate::intent_pipeline::engines::direct::DirectEngine;
    use crate::intent_pipeline::engines::reactive::ReactiveEngine;

    struct MockProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl MockProvider {
        fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
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
                Ok(LlmResponse {
                    content: Some("fallback".to_string()),
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

    fn make_agent_manager() -> Arc<AgentManager> {
        let mut mgr = AgentManager::new();
        mgr.load_builtin_agents().unwrap();
        Arc::new(mgr)
    }

    async fn make_runtime(provider: Arc<dyn LlmProvider>) -> AgentRuntime {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(provider.clone(), registry));

        let direct = DirectEngine::new(Arc::clone(&core));
        let reactive = ReactiveEngine::new(Arc::clone(&core), 10);
        let router = ExecutionRouter::new(direct, reactive);

        let analyzer =
            IntentAnalyzer::new(provider.clone(), "mock", &OrchestratorConfig::default());

        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("in-memory SQLite for tests");
        let usage_repo = storage::UsageRepo::new(pool);
        let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));

        let active_profile = Arc::new(RwLock::new(None));

        AgentRuntime::new(
            make_agent_manager(),
            analyzer,
            Arc::new(ContextEngine::new()),
            router,
            cost_tracker,
            PipelineConfig::default(),
            active_profile,
        )
    }

    #[tokio::test]
    async fn test_agent_runtime_selects_correct_agent() {
        let agent_manager = make_agent_manager();
        let selected = agent_manager.match_agent("create a task to review budget");
        assert_eq!(selected.name, "task");
    }

    #[tokio::test]
    async fn test_runtime_processes_greeting_via_general_agent() {
        let provider = MockProvider::new(vec![text_response("Hi there!")]);
        let runtime = make_runtime(provider).await;

        let result = runtime
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None, None)
            .await
            .unwrap();

        assert_eq!(result.agent_name, "general");
        assert_eq!(result.content, "Hi there!");
    }

    #[tokio::test]
    async fn test_runtime_routes_task_message_to_task_agent() {
        let provider = MockProvider::new(vec![text_response("Task created!")]);
        let runtime = make_runtime(provider).await;

        let result = runtime
            .process_message(
                "create a task for me",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.agent_name, "task");
        assert_eq!(result.content, "Task created!");
    }

    #[tokio::test]
    async fn test_runtime_sets_active_profile() {
        let provider = MockProvider::new(vec![text_response("Done")]);
        let runtime = make_runtime(provider).await;
        let handle = runtime.active_profile_handle();

        // Before processing, active profile is None
        assert!(handle.read().await.is_none());

        let _result = runtime
            .process_message(
                "check my budget",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // After processing, active profile should be set
        let guard = handle.read().await;
        assert!(guard.is_some());
        assert_eq!(guard.as_ref().unwrap().name, "finance");
    }

    #[tokio::test]
    async fn test_runtime_clarification_on_low_confidence() {
        let provider = MockProvider::new(vec![text_response("should not reach")]);
        let runtime = make_runtime(provider)
            .await
            .with_confidence_evaluator(Arc::new(crate::confidence::ConfidenceEvaluator::new(0.99)));

        let result = runtime
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None, None)
            .await
            .unwrap();

        assert_eq!(result.mode_used, CLARIFICATION_MODE);
        assert!(result.content.contains("clarify"));
    }
}
