//! IntentPipeline — full pipeline replacement.
//!
//! Replaces `AgentPipeline` (Orchestrator + EngineDispatch) with a unified flow:
//! IntentAnalyzer → ContextEngine → ExecutionRouter → ResponseValidator → CostTracker

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use common::{utils::tool_def_name, Result};
use context_engine::{ContextEngine, ContextRequest};
use providers::Message;
use tools::RoutingContext;
use tracing::{debug, info, warn};

use super::analysis::IntentAnalyzer;
use super::router::{ExecutionRouter, RouterResult};
use super::types::IntentAnalysis;
use crate::events::AgentEvent;
use crate::execution::ExecutionParams;
use crate::output::cost_tracker::CostTracker;
use crate::output::validator::{ResponseValidator, ValidationResult};

/// Result of processing a message through the intent pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    /// The final response content.
    pub content: String,
    /// Which execution mode actually produced the result.
    pub mode_used: String,
    /// Full intent analysis from the classifier.
    pub classification: IntentAnalysis,
    /// Number of escalations during execution.
    pub escalations: u32,
    /// Validation warnings (if any).
    pub validation: ValidationResult,
}

/// Configuration for the intent pipeline.
pub struct PipelineConfig {
    /// Model name for execution.
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

/// The intent pipeline — wires IntentAnalyzer + ContextEngine + ExecutionRouter
/// + ResponseValidator + CostTracker into a single `process_message()` call.
pub struct IntentPipeline {
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
}

impl IntentPipeline {
    pub fn new(
        analyzer: IntentAnalyzer,
        context_engine: Arc<ContextEngine>,
        router: ExecutionRouter,
        cost_tracker: Arc<CostTracker>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            analyzer,
            context_engine,
            router,
            validator: ResponseValidator::new(config.max_response_tokens),
            cost_tracker,
            config,
            strategy_repo: None,
        }
    }

    /// Attach a strategy repository for recording execution outcomes.
    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    /// Process a user message through the full intent pipeline.
    ///
    /// 1. Classify intent (heuristics → LLM classifier)
    /// 2. Assemble context with budget allocation
    /// 3. Filter tools based on intent classification
    /// 4. Execute with appropriate engine (with automatic escalation)
    /// 5. Validate response
    /// 6. Record usage
    /// 7. Record strategy outcome
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
        let pipeline_start = Instant::now();

        // Step 1: Classify intent
        let classify_start = Instant::now();
        let analysis = self.analyzer.analyze(message, tool_names).await;
        let classify_ms = classify_start.elapsed().as_millis() as u64;
        debug!(
            "IntentPipeline: classified as {:?} (source: {:?}, confidence: {:.2})",
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

        // Step 2: Assemble context
        let prompt = system_prompt.unwrap_or(&self.config.system_prompt);
        let strategy = context_engine::ExecutionStrategy::from(&analysis.mode);

        let context_request = ContextRequest {
            message_text: message.to_string(),
            history,
            system_prompt: prompt.to_string(),
            strategy: strategy.clone(),
            tool_definitions: tool_definitions.to_vec(),
            context_window: self.config.context_window,
        };
        let assemble_start = Instant::now();
        let assembled = self.context_engine.assemble(context_request).await;
        let assemble_ms = assemble_start.elapsed().as_millis() as u64;

        debug!(
            "IntentPipeline: assembled context with {} messages, {} tokens",
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

        // Step 3: Filter tools based on intent classification
        let filtered_tools: Cow<'_, [serde_json::Value]> =
            if let Some(allowed) = analysis.allowed_tool_names() {
                Cow::Owned(
                    tool_definitions
                        .iter()
                        .filter(|t| {
                            tool_def_name(t)
                                .map(|name| allowed.contains(name))
                                .unwrap_or(true) // Keep tools with unknown format
                        })
                        .cloned()
                        .collect(),
                )
            } else {
                Cow::Borrowed(tool_definitions)
            };

        debug!(
            "IntentPipeline: filtered {} → {} tools",
            tool_definitions.len(),
            filtered_tools.len()
        );

        // Step 4: Execute via router (with automatic escalation)
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ExecutionStarted {
                    engine: analysis.mode.to_string(),
                    max_iterations: analysis.mode.max_iterations() as usize,
                })
                .await;
        }

        let params = ExecutionParams::new(&self.config.execution_model);

        // On escalation the router may need all tools, so keep the full set available
        let router_result = self
            .router
            .execute(
                analysis.mode.clone(),
                assembled.messages,
                &filtered_tools,
                &params,
                ctx,
                event_tx,
            )
            .await?;

        if router_result.escalation_count > 0 {
            info!(
                "IntentPipeline: escalated {} time(s), final mode: {}",
                router_result.escalation_count, router_result.final_mode
            );
        }

        // Step 5: Validate
        let validation = self.validator.validate(&router_result.content);
        if !validation.is_valid {
            warn!(
                "IntentPipeline: response validation failed with {} warning(s)",
                validation.warnings.len()
            );
        }

        let final_content = validation.filtered_content.clone();
        let mode_name = router_result.final_mode.clone();

        // Step 6: Record usage (best-effort)
        self.record_usage(&router_result, &mode_name, ctx).await;

        // Step 7: Record strategy outcome (best-effort)
        self.record_strategy(&analysis, &router_result, &validation, ctx, pipeline_start)
            .await;

        Ok(PipelineResult {
            content: final_content,
            mode_used: mode_name,
            classification: analysis,
            escalations: router_result.escalation_count,
            validation,
        })
    }

    async fn record_usage(&self, result: &RouterResult, mode_name: &str, ctx: &RoutingContext) {
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
            warn!("IntentPipeline: failed to record usage: {}", e);
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
            escalation_count: result.escalation_count as i32,
            iterations_used: result.iterations as i32,
            max_iterations: analysis.mode.max_iterations() as i32,
            success: validation.is_valid,
            user_satisfaction: None,
            response_time_ms: elapsed_ms,
            chat_id: Some(ctx.chat_id.to_string()),
            tool_name: result.tool_name.clone(),
            tool_success: result.tool_name.as_ref().map(|_| validation.is_valid),
            tool_duration_ms: result.tool_name.as_ref().map(|_| elapsed_ms),
            complexity_signals: serde_json::Value::Null,
            execution_mode: Some(result.final_mode.clone()),
        };

        if let Err(e) = strategy_repo.create(&record).await {
            warn!("IntentPipeline: failed to record strategy: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_pipeline::AnalysisSource;
    use async_trait::async_trait;
    use config::OrchestratorConfig;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    use crate::execution::ExecutionCore;
    use crate::intent_pipeline::engines::direct::DirectEngine;
    use crate::intent_pipeline::engines::reactive::ReactiveEngine;

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

    async fn make_pipeline(provider: Arc<dyn LlmProvider>) -> IntentPipeline {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(provider.clone(), registry));

        let direct = DirectEngine::new(Arc::clone(&core));
        let reactive = ReactiveEngine::new(Arc::clone(&core), 10);
        let router = ExecutionRouter::new(direct, reactive, 3);

        let analyzer =
            IntentAnalyzer::new(provider.clone(), "mock", &OrchestratorConfig::default());

        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("in-memory SQLite for tests");
        let usage_repo = storage::UsageRepo::new(pool);
        let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));

        IntentPipeline::new(
            analyzer,
            Arc::new(ContextEngine::new()),
            router,
            cost_tracker,
            PipelineConfig::default(),
        )
    }

    // ── Tests ──

    #[tokio::test]
    async fn pipeline_processes_greeting() {
        let provider = MockPipelineProvider::new(vec![text_response("Hi there!")]);
        let pipeline = make_pipeline(provider).await;

        let result = pipeline
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        assert_eq!(result.classification.source, AnalysisSource::Heuristic);
        assert!(!result.content.is_empty());
        assert_eq!(result.content, "Hi there!");
        assert_eq!(result.escalations, 0);
    }

    #[tokio::test]
    async fn pipeline_processes_tool_task() {
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

        assert!(result.mode_used.contains("reactive"));
        assert_eq!(result.content, "Here are your tasks: ...");
    }

    #[tokio::test]
    async fn pipeline_validates_empty_response() {
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

        assert!(!result.validation.is_valid);
    }

    #[tokio::test]
    async fn pipeline_writes_strategy_record() {
        let storage_pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("in-memory StoragePool");
        let repos = storage::Repos::from_pool(&storage_pool);

        let provider = MockPipelineProvider::new(vec![text_response("Hi there!")]);
        let provider: Arc<dyn LlmProvider> = provider;

        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(provider.clone(), registry));

        let direct = DirectEngine::new(Arc::clone(&core));
        let reactive = ReactiveEngine::new(Arc::clone(&core), 10);
        let router = ExecutionRouter::new(direct, reactive, 3);

        let analyzer =
            IntentAnalyzer::new(provider.clone(), "mock", &OrchestratorConfig::default());
        let cost_tracker = Arc::new(CostTracker::from_repo(repos.usage.clone()));

        let pipeline = IntentPipeline::new(
            analyzer,
            Arc::new(ContextEngine::new()),
            router,
            cost_tracker,
            PipelineConfig::default(),
        )
        .with_strategy_repo(repos.strategies.clone());

        let result = pipeline
            .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
            .await
            .unwrap();

        assert_eq!(result.content, "Hi there!");

        let since = chrono::Utc::now() - chrono::Duration::minutes(1);
        let records = repos
            .strategies
            .list_by_date_range(since, chrono::Utc::now())
            .await
            .expect("should list strategy records");

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert!(record.predicted_strategy.contains("Direct"));
        assert_eq!(record.escalation_count, 0);
        assert!(record.success);
    }

    #[tokio::test]
    async fn pipeline_emits_classification_event() {
        let provider = MockPipelineProvider::new(vec![text_response("Hi!")]);
        let pipeline = make_pipeline(provider).await;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);

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
