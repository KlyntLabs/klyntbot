//! AgentRuntime — flat 3-phase execution pipeline.
//!
//! 3-phase flow: Prepare → Execute (budget-bounded) → Record
//! All tools available to every message. No routing, no classification, no delegation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Result;
use context_engine::{ContextEngine, ContextRequest, ExecutionStrategy};
use providers::Message;
use tokio::sync::RwLock;
use tools::RoutingContext;
use tracing::warn;

use crate::autotuner::hooks::AutoTunerHook;
use crate::events::AgentEvent;
use crate::execution::{execute_loop, DepthMode, ExecutionBudget, ExecutionParams};
use crate::output::cost_tracker::CostTracker;
use crate::output::validator::{ResponseValidator, ValidationResult};

/// LLM endpoint configuration for the runtime.
pub struct RuntimeConfig {
    pub execution_model: String,
    pub provider_name: String,
    pub context_window: usize,
    pub max_response_tokens: usize,
}

/// Result of processing a message through the agent runtime.
#[derive(Debug)]
pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,
    pub validation: ValidationResult,
    pub agent_name: String,
    pub turns: u32,
    pub budget_exhausted: bool,
    pub tool_calls: Vec<String>,
}

/// Flat agent runtime — all tools available to every message.
///
/// Replaces the routed/delegated pipeline with a simple 3-phase flow:
/// 1. Prepare (context assembly)
/// 2. Execute (budget-bounded loop)
/// 3. Record (usage + interaction)
pub struct AgentRuntime {
    context_engine: Arc<ContextEngine>,
    core: Arc<crate::execution::ExecutionCore>,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    execution_model: String,
    provider_name: String,
    context_window: usize,
    interaction_recorder: Option<crate::learning::InteractionRecorder>,
    procedural_rule_repo: Option<cognitive::ProceduralRuleRepo>,
    tool_registry: Option<Arc<RwLock<tools::registry::ToolRegistry>>>,
    autotuner_hook: Option<Arc<dyn AutoTunerHook>>,
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    task_repo: Option<storage::TaskRepo>,
    active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
    hot_config: Arc<RwLock<config::HotConfig>>,
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
}

impl AgentRuntime {
    pub fn new(
        context_engine: Arc<ContextEngine>,
        core: Arc<crate::execution::ExecutionCore>,
        cost_tracker: Arc<CostTracker>,
        cfg: RuntimeConfig,
        hot_config: Arc<RwLock<config::HotConfig>>,
    ) -> Self {
        Self {
            context_engine,
            core,
            validator: ResponseValidator::new(cfg.max_response_tokens),
            cost_tracker,
            execution_model: cfg.execution_model,
            provider_name: cfg.provider_name,
            context_window: cfg.context_window,
            interaction_recorder: None,
            procedural_rule_repo: None,
            tool_registry: None,
            autotuner_hook: None,
            user_situation: None,
            task_repo: None,
            active_view: None,
            hot_config,
            context_update_queue: None,
        }
    }

    // ── Builder methods ─────────────────────────────────────────

    pub fn with_interaction_recorder(
        mut self,
        recorder: crate::learning::InteractionRecorder,
    ) -> Self {
        self.interaction_recorder = Some(recorder);
        self
    }

    pub fn with_procedural_rule_repo(mut self, repo: cognitive::ProceduralRuleRepo) -> Self {
        self.procedural_rule_repo = Some(repo);
        self
    }

    pub fn with_tool_registry(
        mut self,
        registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    ) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn with_autotuner_hook(mut self, hook: Arc<dyn AutoTunerHook>) -> Self {
        self.autotuner_hook = Some(hook);
        self
    }

    pub fn with_user_situation(
        mut self,
        sit: Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>,
    ) -> Self {
        self.user_situation = Some(sit);
        self
    }

    pub fn with_task_repo(mut self, repo: storage::TaskRepo) -> Self {
        self.task_repo = Some(repo);
        self
    }

    pub fn with_active_view(
        mut self,
        view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>,
    ) -> Self {
        self.active_view = Some(view);
        self
    }

    pub fn with_context_update_queue(mut self, queue: Arc<bus::ContextUpdateQueue>) -> Self {
        self.context_update_queue = Some(queue);
        self
    }

    // ── Accessors ───────────────────────────────────────────────

    pub fn tool_registry(&self) -> Option<&Arc<RwLock<tools::registry::ToolRegistry>>> {
        self.tool_registry.as_ref()
    }

    // ── Main pipeline ───────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn process_message(
        &self,
        message: &str,
        history: Vec<Message>,
        tool_definitions: &[serde_json::Value], // ALL tools, unfiltered
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        cancel_token: Option<tokio_util::sync::CancellationToken>,
        depth: DepthMode,
    ) -> Result<RuntimeResult> {
        let pipeline_start = Instant::now();
        let hot = self.hot_config.read().await;
        let safety_timeout_secs = hot.safety_timeout_secs;
        drop(hot);

        // Emit pipeline start
        if let Some(ref tx) = event_tx {
            let _ = tx.send(AgentEvent::PipelineStarted).await;
        }

        // ── Phase 1: Prepare ─────────────────────────────────────
        let retrieval_context = self.build_retrieval_context(&history).await;

        let context_request = ContextRequest {
            message_text: message.to_string(),
            history,
            system_prompt: String::new(), // Built by ContextSources (SoulContextSource + SkillListingSource)
            strategy: ExecutionStrategy::ToolAssisted { max_iterations: 30 },
            tool_definitions: tool_definitions.to_vec(),
            context_window: self.context_window,
            session_key: Some(common::SessionKey::new(&ctx.channel, &ctx.chat_id).to_string()),
            retrieval_context,
        };

        let assemble_start = Instant::now();
        let assembled = self.context_engine.assemble(context_request).await;
        let assemble_ms = assemble_start.elapsed().as_millis() as u64;

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::ContextAssembled {
                    total_tokens: assembled.token_count,
                    budget: self.context_window,
                    duration_ms: assemble_ms,
                })
                .await;
        }

        // Emit learning context summaries
        if let Some(ref tx) = event_tx {
            self.emit_learning_summary(tx).await;
        }

        // Create budget
        let mut budget = ExecutionBudget::new(depth, "general");

        // Build execution params
        let mut params = ExecutionParams::new(&self.execution_model)
            .with_max_iterations(budget.max_turns())
            .with_original_message(message.to_string())
            .with_context_window(self.context_window);

        if let Some(token) = cancel_token {
            params = params.with_cancel_token(token);
        }
        if let Some(ref queue) = self.context_update_queue {
            params = params.with_context_update_queue(Arc::clone(queue));
        }

        // ── Phase 2: Execute ─────────────────────────────────────
        let safety_timeout = Duration::from_secs(safety_timeout_secs.max(1));

        let loop_result = tokio::time::timeout(
            safety_timeout,
            execute_loop(
                &self.core,
                assembled.messages,
                tool_definitions, // ALL tools, flat
                &params,
                &mut budget,
                ctx,
                event_tx.clone(),
            ),
        )
        .await
        .map_err(|_| {
            common::KlyntbotError::Timeout(format!(
                "Safety timeout ({safety_timeout_secs}s) — this is a bug, please report it"
            ))
        })??;

        // ── Phase 3: Record ──────────────────────────────────────
        let mode_name = depth.to_string();
        let mut validation = self.validator.validate(&loop_result.content);
        let pipeline_elapsed_ms = pipeline_start.elapsed().as_millis() as u64;

        // Record usage
        self.record_usage(
            &loop_result.usage,
            &mode_name,
            ctx,
            &event_tx,
            pipeline_elapsed_ms,
        )
        .await;

        // Record interaction
        if let Some(ref recorder) = self.interaction_recorder {
            let tools_used: Vec<&str> = loop_result.tool_calls.iter().map(|s| s.as_str()).collect();
            recorder
                .record(
                    "flat",
                    &tools_used,
                    ctx.channel.as_str(),
                    pipeline_elapsed_ms,
                )
                .await;
        }

        // AutoTuner hook
        if let Some(ref hook) = self.autotuner_hook {
            let tokens = loop_result.usage.prompt_tokens + loop_result.usage.completion_tokens;
            hook.on_message_completed(
                ctx.chat_id.as_str(),
                "flat",
                &mode_name,
                tokens,
                pipeline_elapsed_ms,
            )
            .await;
        }

        let final_content = std::mem::take(&mut validation.filtered_content);

        Ok(RuntimeResult {
            content: final_content,
            mode_used: mode_name,
            validation,
            agent_name: "klyntbot".to_string(),
            turns: loop_result.turns,
            budget_exhausted: loop_result.budget_exhausted,
            tool_calls: loop_result.tool_calls,
        })
    }

    // ── Helpers ──────────────────────────────────────────────────

    async fn build_retrieval_context(
        &self,
        history: &[Message],
    ) -> Option<context_engine::RetrievalContext> {
        let recent_user_messages: Vec<String> = history
            .iter()
            .rev()
            .filter(|m| m.role() == common::MessageRole::User)
            .take(2)
            .map(|m| match m {
                Message::User {
                    content: providers::UserContent::Text(t),
                } => t.chars().take(200).collect(),
                _ => String::new(),
            })
            .collect();

        let situation = if let Some(ref sit) = self.user_situation {
            let s = sit.lock().await;
            Some(context_engine::UserSituationSnapshot {
                energy_level: s.energy_level,
                focus_state: s.focus_state,
                deadline_pressure: s.deadline_pressure,
                distraction_risk: s.distraction_risk,
            })
        } else {
            None
        };

        let active_task = if let Some(ref repo) = self.task_repo {
            match repo.list_focused().await {
                Ok(tasks) => tasks
                    .into_iter()
                    .next()
                    .map(|t| context_engine::ActiveTaskContext {
                        title: t.title,
                        project_name: t.project_id,
                        domain: None,
                    }),
                Err(e) => {
                    warn!("Failed to query focused task for retrieval context: {e}");
                    None
                }
            }
        } else {
            None
        };

        Some(context_engine::RetrievalContext {
            active_skill: None,
            active_task,
            recent_user_messages,
            situation,
            active_view: if let Some(ref view_lock) = self.active_view {
                view_lock.read().await.clone()
            } else {
                None
            },
            recent_correction: None,
            hierarchical_intent: None,
        })
    }

    async fn emit_learning_summary(&self, tx: &tokio::sync::mpsc::Sender<AgentEvent>) {
        if let Some(ref rule_repo) = self.procedural_rule_repo {
            if let Ok(rules) = rule_repo.list_all_active().await {
                if !rules.is_empty() {
                    let previews: Vec<&str> =
                        rules.iter().take(3).map(|r| r.rule_text.as_str()).collect();
                    let _ = tx
                        .send(AgentEvent::LearningEvent {
                            event_type: "patterns".into(),
                            detail: format!(
                                "{} learned rules ({})",
                                rules.len(),
                                previews.join(", ")
                            ),
                        })
                        .await;
                }
            }
        }
    }

    async fn record_usage(
        &self,
        usage: &providers::Usage,
        mode_name: &str,
        ctx: &RoutingContext,
        event_tx: &Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        pipeline_elapsed_ms: u64,
    ) {
        let cost = crate::output::cost_tracker::estimate_cost(usage, &self.execution_model);

        if let Err(e) = self
            .cost_tracker
            .record(
                usage,
                &self.execution_model,
                &self.provider_name,
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
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                    estimated_cost_usd: cost,
                    model: self.execution_model.clone(),
                    response_time_ms: pipeline_elapsed_ms,
                })
                .await;

            if let Some(alert) = self.cost_tracker.check_budget().await {
                let _ = tx
                    .send(AgentEvent::BudgetWarning {
                        monthly_spend_usd: alert.monthly_spend_usd,
                        monthly_budget_usd: alert.monthly_budget_usd,
                        usage_percent: alert.usage_percent,
                    })
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::{ChatParams, DynProvider, LlmResponse, Usage};
    use std::sync::Arc;
    use tools::registry::ToolRegistry;

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new(
            common::ChannelName::new("test".to_string()),
            common::ChatId::new("test-chat".to_string()),
        )
    }

    fn text_response(text: &str) -> LlmResponse {
        LlmResponse {
            content: Some(text.to_string()),
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            tool_calls: vec![],
            finish_reason: "end_turn".to_string(),
            reasoning_content: None,
        }
    }

    struct MockProvider {
        response: String,
    }

    #[async_trait::async_trait]
    impl providers::LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[providers::Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> std::result::Result<LlmResponse, common::KlyntbotError> {
            Ok(text_response(&self.response))
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }

        fn context_window(&self) -> usize {
            128_000
        }
    }

    async fn make_runtime(response: &str) -> (AgentRuntime, Arc<RwLock<ToolRegistry>>) {
        let provider: DynProvider = Arc::new(MockProvider {
            response: response.to_string(),
        });
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(crate::execution::ExecutionCore::new(
            provider,
            Arc::clone(&registry),
        ));

        let context_engine = Arc::new(context_engine::ContextEngine::new());
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let usage_repo = storage::UsageRepo::new(pool.inner().clone());
        let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));
        let hot_config = Arc::new(RwLock::new(config::HotConfig::from(
            &config::Config::default(),
        )));

        let runtime = AgentRuntime::new(
            context_engine,
            core,
            cost_tracker,
            RuntimeConfig {
                execution_model: "mock-model".to_string(),
                provider_name: "mock".to_string(),
                context_window: 128_000,
                max_response_tokens: 8192,
            },
            hot_config,
        )
        .with_tool_registry(Arc::clone(&registry));

        (runtime, registry)
    }

    #[tokio::test]
    async fn test_runtime_processes_greeting() {
        let (runtime, _registry) = make_runtime("Hello! How can I help you?").await;
        let ctx = routing_ctx();

        let result = runtime
            .process_message(
                "hi",
                vec![providers::Message::user("hi")],
                &[],
                &ctx,
                None,
                None,
                DepthMode::Normal,
            )
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.content.contains("Hello"));
        assert_eq!(result.agent_name, "klyntbot");
        assert_eq!(result.mode_used, "normal");
    }

    #[tokio::test]
    async fn test_runtime_result_has_no_classification() {
        let (runtime, _registry) = make_runtime("Sure!").await;
        let ctx = routing_ctx();

        let result = runtime
            .process_message(
                "test",
                vec![providers::Message::user("test")],
                &[],
                &ctx,
                None,
                None,
                DepthMode::Normal,
            )
            .await
            .unwrap();

        // RuntimeResult has no classification field — flat pipeline
        assert!(!result.content.is_empty());
        assert_eq!(result.turns, 1);
    }
}
