//! Agent-runtime assembly for the agent loop.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use bus::DomainEventBus;
use config::Config;
use providers::DynProvider;

/// Everything needed to build the [`AgentRuntime`].
pub(crate) struct RuntimeBuildInput {
    pub config: Config,
    pub provider: DynProvider,
    pub context_engine: Arc<context_engine::ContextEngine>,
    pub tool_registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    pub token_counter: Arc<dyn context_engine::TokenCounter>,
    pub outcome_recorder: Option<Arc<crate::learning::recorder::OutcomeRecorder>>,
    pub domain_event_bus: Option<Arc<DomainEventBus>>,
    pub pool: Option<sqlx::SqlitePool>,
    pub approval_channel: Option<Arc<dyn approval::ApprovalChannel>>,
    pub approval_suggester: Option<Arc<dyn approval::ApprovalSuggester>>,
    pub hot_config: Arc<RwLock<config::HotConfig>>,
    pub autotuner: Option<Arc<crate::autotuner::AutoTunerOrchestrator>>,
    pub predictive_cache: Option<Arc<cognitive::services::predictive_cache::PredictiveCache>>,
    pub cognitive_provider: Option<DynProvider>,
    pub user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    pub task_repo: storage::TaskRepo,
    pub interaction_log_repo: storage::InteractionLogRepo,
    pub active_view: Option<Arc<RwLock<Option<context_engine::ActiveView>>>>,
    pub memory_service_for_shadow: Option<Arc<cognitive::UnifiedMemoryService>>,
    pub context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
    pub injector_registry: Option<bus::InjectorRegistry>,
    pub storage_pool: storage::StoragePool,
}

/// Assemble the [`AgentRuntime`] with all optional extensions wired.
pub(crate) fn build_runtime(input: RuntimeBuildInput) -> crate::agent_runtime::AgentRuntime {
    let RuntimeBuildInput {
        config,
        provider,
        context_engine,
        tool_registry,
        token_counter,
        outcome_recorder,
        domain_event_bus,
        pool,
        approval_channel,
        approval_suggester,
        hot_config,
        autotuner,
        predictive_cache,
        cognitive_provider,
        user_situation,
        task_repo,
        interaction_log_repo,
        active_view,
        memory_service_for_shadow,
        context_update_queue,
        injector_registry,
        storage_pool,
    } = input;

    let mut execution_core =
        crate::execution::ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry))
            .with_token_counter(Arc::clone(&token_counter));
    if let Some(ref recorder) = outcome_recorder {
        execution_core = execution_core.with_outcome_recorder(Arc::clone(recorder));
    }
    if let Some(ref bus) = domain_event_bus {
        execution_core = execution_core.with_domain_bus(Arc::clone(bus));
    }

    // Approval gate: built whenever a storage pool is available so that
    // every tool call passes through ApprovalGate::check before execute.
    if let Some(ref p) = pool {
        let grants_repo = approval::ApprovalGrantsRepo::new(
            storage::StoragePool::from_existing(p.clone()),
        );
        let channel: Arc<dyn approval::ApprovalChannel> = approval_channel
            .clone()
            .unwrap_or_else(|| Arc::new(approval::BlockingFallbackChannel::desktop_prompt()));
        let mut gate = approval::ApprovalGate::new(grants_repo, channel);
        if let Some(suggester) = approval_suggester {
            gate = gate.with_suggester(suggester);
        }
        execution_core = execution_core.with_approval_gate(Arc::new(gate));
        info!("approval gate wired into ExecutionCore");
    } else {
        warn!("approval gate NOT wired (no storage pool) — tool calls will not be gated");
    }

    let execution_core = Arc::new(execution_core);

    let cost_tracker = Arc::new(
        crate::output::CostTracker::from_repo(storage::UsageRepo::new(
            storage_pool.inner().clone(),
        ))
        .with_monthly_budget(config.agents.monthly_budget_usd),
    );

    // ── Interaction recorder ──────────────────────────────────────────
    let interaction_recorder = if config.learning.enabled {
        Some(crate::learning::InteractionRecorder::new(interaction_log_repo))
    } else {
        None
    };

    let mut runtime = crate::agent_runtime::AgentRuntime::new(
        Arc::clone(&context_engine),
        execution_core,
        cost_tracker,
        crate::agent_runtime::RuntimeConfig {
            execution_model: config.agents.defaults.model.clone(),
            provider_name: provider.name().to_string(),
            context_window: provider.context_window(),
            max_response_tokens: config.agents.defaults.max_tokens as usize,
            cache_enabled: config.providers.cache.enabled,
        },
        Arc::clone(&hot_config),
    )
    .with_tool_registry(Arc::clone(&tool_registry))
    .with_enhancement_budget_overrides(
        config.cognitive.query_enhancement.budget_overrides.clone(),
    );

    if let Some(ref bus) = domain_event_bus {
        runtime = runtime.with_domain_event_bus(Arc::clone(bus));
    }

    if let Some(ref orchestrator) = autotuner {
        if let Some(sink) = orchestrator.memory_param_sink() {
            runtime = runtime.with_enhancement_param_sink(sink);
        }
    }

    if let Some(recorder) = interaction_recorder {
        runtime = runtime.with_interaction_recorder(recorder);
    }

    // Inject procedural rule repo for transparency (L5 cognitive rules)
    let rule_repo = cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone());
    runtime = runtime.with_procedural_rule_repo(rule_repo);

    // KCA Track 4: micro-Reforge turn counter.
    let micro_reforge_svc = Arc::new(
        cognitive::services::micro_reforge::MicroReforgeService::new(
            storage_pool.clone(),
            config.cognitive.micro_reforge.clone(),
        ),
    );
    runtime = runtime.with_micro_reforge(micro_reforge_svc);

    // KCA Track 7: wire predictive cache + query predictor into runtime.
    if let Some(ref pc) = predictive_cache {
        runtime = runtime
            .with_predictive_cache(pc.clone())
            .with_predictions_per_turn(config.cognitive.predictive_cache.predictions_per_turn);
    }
    if let Some(ref cp) = cognitive_provider {
        if config.cognitive.predictive_cache.enabled {
            let model = config
                .cognitive
                .predictive_cache
                .model
                .clone()
                .unwrap_or_else(|| {
                    config
                        .cognitive
                        .model
                        .clone()
                        .unwrap_or_else(|| config.agents.defaults.model.clone())
                });
            let predictor = Arc::new(
                crate::adapters::cognitive_handlers::LlmQueryPredictorHandler::new(
                    cp.clone(),
                    providers::ChatParams::new(&model)
                        .with_max_tokens(256)
                        .with_temperature(0.5)
                        .with_response_format(providers::ResponseFormat::JsonObject),
                ),
            );
            runtime = runtime.with_query_predictor(predictor);
        }
    }

    // Inject user situation for RetrievalContext
    if let Some(ref sit) = user_situation {
        runtime = runtime.with_user_situation(Arc::clone(sit));
    }

    // Wire task repo for active task context in query rewriting
    runtime = runtime.with_task_repo(task_repo);

    // Inject active view for RetrievalContext
    if let Some(ref view) = active_view {
        runtime = runtime.with_active_view(Arc::clone(view));
    }

    // Inject autotuner shadow hook
    if let Some(ref orchestrator) = autotuner {
        // Build the concrete AutoTunerHook for shadow classification
        if let Some(ref p) = pool {
            let trial_repo = storage::TrialRepo::new(p.clone());
            let mut hook = crate::autotuner::hooks::AutoTunerHookImpl::new(
                Arc::clone(orchestrator),
                trial_repo,
            );

            // Wire Phase 2 shadow retriever if memory service is available
            if let Some(ref mem_svc) = memory_service_for_shadow {
                let config_defaults = [
                    config.cognitive.relevance_weight_semantic,
                    config.cognitive.relevance_weight_retrievability,
                    config.cognitive.relevance_weight_importance,
                    config.cognitive.relevance_weight_frequency,
                    config.cognitive.relevance_weight_situation,
                    config.cognitive.relevance_weight_temporal,
                    0.10_f64, // relevance_weight_hierarchy
                    0.05_f64, // relevance_weight_path_coherence
                    0.15_f64, // relevance_weight_community
                    0.10_f64, // relevance_weight_cross_note
                    config.cognitive.relevance_weight_recall_support, // recall_support
                    config.cognitive.relevance_weight_graph_path_boost, // graph_path_boost
                ];
                let shadow_retriever = Arc::new(
                    crate::autotuner::shadow_retriever::AgentShadowRetriever::new(
                        Arc::clone(mem_svc),
                        config_defaults,
                    ),
                );
                hook = hook.with_shadow_retriever(
                    shadow_retriever as Arc<dyn autotuner::ShadowRetriever>,
                );
            }

            runtime = runtime.with_autotuner_hook(Arc::new(hook));
        }
    }

    // Inject context update queue for live context refresher
    if let Some(ref queue) = context_update_queue {
        runtime = runtime.with_context_update_queue(Arc::clone(queue));
    }
    if let Some(ref registry) = injector_registry {
        runtime = runtime.with_injector_registry(registry.clone());
    }

    // Wire retrieval feedback recording
    if let Some(ref mem_svc) = memory_service_for_shadow {
        runtime = runtime.with_memory_service(Arc::clone(mem_svc));
    }
    if let Some(ref p) = pool {
        runtime = runtime.with_feedback_repo(storage::RetrievalFeedbackRepo::new(p.clone()));
        runtime = runtime.with_strategy_repo(storage::StrategyRepo::new(p.clone()));
        runtime = runtime.with_warning_repo(storage::ResponseWarningRepo::new(p.clone()));
    }

    info!("Agent runtime initialized");

    runtime
}
