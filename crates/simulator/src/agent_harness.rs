//! Agent execution harness — wraps AgentRuntime with real tools for
//! end-to-end simulation.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use agent::agent_runtime::AgentRuntime;
use agent::AgentEvent;
use agent::ExecutionCore;
use bus::DomainEventBus;
use common::{ChannelName, ChatId};
use providers::DynProvider;
use tools::registry::ToolRegistry;
use tools::RoutingContext;

use crate::agent_types::{AgentBreakpoint, AgentResult, BreakpointKind};
use crate::persona::types::AnnotatedMessage;
use crate::providers::SimulationProvider;

/// Create an LLM provider from scenario config using the provider registry.
/// Falls back to mock with a warning if the provider is unknown or the API key is missing.
fn create_provider(provider_name: &str, model: &str, seed: u64) -> DynProvider {
    if provider_name == "mock" {
        return Arc::new(SimulationProvider::new(seed));
    }

    let spec = match providers::ProviderRegistry::find_by_name(provider_name) {
        Some(s) => s,
        None => {
            warn!(
                provider = provider_name,
                "Unknown provider — falling back to mock"
            );
            return Arc::new(SimulationProvider::new(seed));
        }
    };

    let api_key = match std::env::var(spec.env_key) {
        Ok(key) if !key.is_empty() => key,
        _ => {
            warn!(
                env_var = spec.env_key,
                provider = provider_name,
                "API key not found — falling back to mock provider"
            );
            return Arc::new(SimulationProvider::new(seed));
        }
    };

    if provider_name == "anthropic" {
        Arc::new(providers::AnthropicNativeProvider::new(
            config::Secret::new(api_key),
            spec.default_api_base.to_string(),
            model.to_string(),
        ))
    } else {
        match providers::OpenAiCompatProvider::new(spec.default_api_base, api_key, model) {
            Ok(p) => Arc::new(p),
            Err(e) => {
                warn!(error = %e, "Failed to create provider — falling back to mock");
                Arc::new(SimulationProvider::new(seed))
            }
        }
    }
}

/// Wraps an `AgentRuntime` with real registered tools for simulation.
pub struct AgentHarness {
    runtime: Arc<AgentRuntime>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
}

impl AgentHarness {
    /// Construct the agent harness with real tools mirroring production registration.
    ///
    /// Uses the shared in-memory pool so tools execute real DB operations.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        pool: &storage::StoragePool,
        inner_pool: sqlx::SqlitePool,
        bus: Arc<DomainEventBus>,
        context_queue: Arc<bus::ContextUpdateQueue>,
        provider_name: &str,
        model: &str,
        provider_error_rate: f64,
        seed: u64,
    ) -> common::Result<Self> {
        let inner_provider = create_provider(provider_name, model, seed);

        // Wrap with adversarial error injection if configured
        let provider: DynProvider = if provider_error_rate > 0.0 {
            Arc::new(crate::providers::AdversarialProviderWrapper::new(
                inner_provider,
                provider_error_rate,
                seed,
            ))
        } else {
            inner_provider
        };

        // Build tool registry with real domain tools
        let mut tool_registry = ToolRegistry::new();
        Self::register_tools(&mut tool_registry, pool, &inner_pool, &bus);

        let tool_registry = Arc::new(RwLock::new(tool_registry));

        // Build execution core
        let core = Arc::new(
            ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry))
                .with_domain_bus(Arc::clone(&bus)),
        );

        // Build context engine with real sources matching production
        let context_sources: Vec<Box<dyn context_engine::source::ContextSource>> = vec![
            Box::new(agent::context_sources::IdentitySource::new(
                std::path::PathBuf::from("/tmp/klyntbot-sim"),
                "UTC".to_string(),
            )),
            Box::new(cognitive::CognitiveContextSource::new(
                cognitive::SemanticFactRepo::new(inner_pool.clone()),
                cognitive::ProceduralRuleRepo::new(inner_pool.clone()),
            )),
            Box::new(agent::context_sources::ProductivityContextSource::new(
                feature_productivity::repos::ProductivityRepos::new(inner_pool.clone()),
            )),
        ];
        let context_engine =
            Arc::new(context_engine::ContextEngine::new().with_sources(context_sources));

        // Build cost tracker
        let usage_repo = storage::UsageRepo::new(inner_pool);
        let cost_tracker = Arc::new(agent::output::cost_tracker::CostTracker::from_repo(
            usage_repo,
        ));

        // Build hot config
        let hot_config = Arc::new(RwLock::new(config::HotConfig::from(
            &config::Config::default(),
        )));

        // Assemble runtime — override execution_model with the scenario's
        // agent_model so the correct model name is sent to the LLM provider.
        let mut runtime = AgentRuntime::new(
            context_engine,
            core,
            cost_tracker,
            agent::RuntimeConfig {
                execution_model: model.to_string(),
                provider_name: provider_name.to_string(),
                context_window: 128_000,
                max_response_tokens: 8_192,
            },
            hot_config,
        );

        // Wire optional deps
        runtime = runtime
            .with_tool_registry(Arc::clone(&tool_registry))
            .with_context_update_queue(context_queue);

        let runtime = Arc::new(runtime);

        Ok(Self {
            runtime,
            tool_registry,
        })
    }

    /// Register real domain tools mirroring the production builder.
    fn register_tools(
        registry: &mut ToolRegistry,
        pool: &storage::StoragePool,
        inner_pool: &sqlx::SqlitePool,
        bus: &Arc<DomainEventBus>,
    ) {
        let repos = storage::Repos::from_pool(pool);

        // Tasks tool (wired directly, not via FeaturePackage)
        let task_tool = feature_tasks::TaskTool::new(
            repos.tasks.clone(),
            3,  // max_focus_slots
            24, // focus_deadline_hours
            "UTC".to_string(),
        )
        .with_area_repo(repos.areas.clone());
        registry.register(task_tool);

        // OKR tool
        registry.register(tools::OkrTool::new(
            repos.objectives.clone(),
            repos.key_results.clone(),
        ));

        // Area tool
        registry.register(tools::AreaTool::new(repos.areas.clone()));

        // Project tool
        registry.register(tools::domain::project_tool::ProjectTool::new(
            repos.projects.clone(),
            repos.tasks.clone(),
        ));

        // Annotate tool
        registry.register(tools::AnnotateTool::new(cognitive::AnnotationRepo::new(
            inner_pool.clone(),
        )));

        // Notes tool
        let note_repo = feature_notes::repo::NoteRepo::new(inner_pool.clone());
        registry.register(feature_notes::tool::NotesTool::new(note_repo));

        // Finance tool (simplified — no price service in simulation)
        let finance_storage = storage::FinanceStorage::from_pool(inner_pool);
        let finance_tool = feature_finance::FinanceTool::new(
            finance_storage,
            feature_finance::PriceService::new(60),
            "VND".to_string(),
        )
        .with_domain_bus(Arc::clone(bus));
        registry.register(finance_tool);

        // Work context tool
        registry.register(activity_log::WorkContextTool::new(pool.clone()));

        // Productivity tool
        let prod_repos = feature_productivity::repos::ProductivityRepos::new(inner_pool.clone());
        let focus_mgr = std::sync::Arc::new(
            feature_productivity::FocusManager::new(
                prod_repos.clone(),
                feature_productivity::config::FocusConfig::default(),
            )
            .with_domain_bus(Arc::clone(bus)),
        );
        let aggregator = std::sync::Arc::new(
            feature_productivity::DailyAggregator::new(prod_repos.clone())
                .with_domain_bus(Arc::clone(bus)),
        );
        registry.register(feature_productivity::ProductivityTool::new(
            prod_repos, focus_mgr, aggregator,
        ));

        // Learning tool (no handler — graceful no-op in simulation)
        registry.register(tools::LearningTool::new(None));

        // Mirror tool (read-only access to self-reflection layer)
        let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
        let mirror_facade = std::sync::Arc::new(cognitive::mirror::MirrorFacade::new(mirror_repo));
        registry.register(tools::MirrorTool::new(mirror_facade));

        // Cron tool (no handler — read-only listing in simulation)
        registry.register(tools::cron_tool::CronTool::new());

        debug!(
            tool_count = registry.tool_names().len(),
            "Agent harness tools registered"
        );
    }

    /// Process a single message through the agent pipeline.
    pub async fn process(
        &self,
        msg: &AnnotatedMessage,
        day: u32,
        history: &[providers::types::Message],
    ) -> AgentResult {
        let ctx = RoutingContext::new(
            ChannelName::new("simulation".to_string()),
            ChatId::new("sim-session".to_string()),
        );

        // Get tool definitions under lock
        let tool_defs = {
            let registry = self.tool_registry.read().await;
            registry.get_definitions()
        };

        // Collect agent events via channel — drain live for real-time visibility
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);

        let msg_preview: String = msg.content.chars().take(60).collect();
        let event_drain = tokio::spawn(async move {
            let mut tool_calls = Vec::<String>::new();
            let mut iterations = 0u32;
            while let Some(event) = event_rx.recv().await {
                match event {
                    AgentEvent::ToolStart { ref name, .. } => {
                        eprintln!("      → tool: {name}");
                        tool_calls.push(name.clone());
                    }
                    AgentEvent::IterationStart { iteration, max, .. } => {
                        eprintln!("      ↻ turn {iteration}/{max}");
                        iterations = iteration as u32;
                    }
                    AgentEvent::ContextAssembled {
                        total_tokens,
                        budget,
                        duration_ms,
                    } => {
                        eprintln!(
                            "      ◈ context: {total_tokens}/{budget} tokens ({duration_ms}ms)"
                        );
                    }
                    AgentEvent::UsageReport {
                        prompt_tokens,
                        completion_tokens,
                        estimated_cost_usd,
                        response_time_ms,
                        ..
                    } => {
                        eprintln!(
                            "      $ {prompt_tokens}+{completion_tokens} tokens, ${estimated_cost_usd:.4}, {response_time_ms}ms"
                        );
                    }
                    AgentEvent::Done { .. } => {
                        eprintln!("      ✓ done: {msg_preview}…");
                    }
                    AgentEvent::Error { message } => {
                        eprintln!("      ✗ error: {message}");
                    }
                    _ => {}
                }
            }
            (tool_calls, iterations)
        });

        let result = self
            .runtime
            .process_message(
                &msg.content,
                {
                    let mut h = history.to_vec();
                    h.push(providers::types::Message::user(&msg.content));
                    h
                },
                &tool_defs,
                &ctx,
                Some(event_tx),
                None, // no cancellation
                agent::execution::DepthMode::Normal,
            )
            .await;

        // Wait for event drain to finish and collect results
        let (tool_calls, iterations) = event_drain.await.unwrap_or_default();

        let mut breakpoints = Vec::new();
        let phase = msg.phase.to_string();

        match result {
            Ok(runtime_result) => {
                // Check for routing mismatch
                if let Some(ref gt) = msg.ground_truth {
                    if let Some(ref expected_skill) = gt.expected_skill {
                        if runtime_result.agent_name != *expected_skill {
                            breakpoints.push(AgentBreakpoint {
                                kind: BreakpointKind::RoutingMismatch,
                                message_content: msg.content.clone(),
                                details: format!(
                                    "expected skill '{}', got '{}'",
                                    expected_skill, runtime_result.agent_name
                                ),
                                day,
                                phase: phase.clone(),
                            });
                        }
                    }
                }

                // Check for empty response
                if runtime_result.content.trim().is_empty() {
                    breakpoints.push(AgentBreakpoint {
                        kind: BreakpointKind::ResponseEmpty,
                        message_content: msg.content.clone(),
                        details: "agent returned empty response".to_string(),
                        day,
                        phase: phase.clone(),
                    });
                }

                AgentResult {
                    selected_skill: runtime_result.agent_name,
                    mode_used: runtime_result.mode_used,
                    tool_calls,
                    iterations,
                    response: runtime_result.content,
                    error: None,
                    breakpoints,
                }
            }
            Err(e) => {
                let error_str = e.to_string();

                // Classify the error
                let kind = if error_str.contains("timeout") || error_str.contains("max_iterations")
                {
                    BreakpointKind::LoopTimeout
                } else if error_str.contains("fabricat") {
                    BreakpointKind::FabricationDetected
                } else {
                    BreakpointKind::ToolExecutionFailed
                };

                breakpoints.push(AgentBreakpoint {
                    kind,
                    message_content: msg.content.clone(),
                    details: error_str.clone(),
                    day,
                    phase,
                });

                AgentResult {
                    selected_skill: String::new(),
                    mode_used: "error".to_string(),
                    tool_calls,
                    iterations,
                    response: String::new(),
                    error: Some(error_str),
                    breakpoints,
                }
            }
        }
    }
}
