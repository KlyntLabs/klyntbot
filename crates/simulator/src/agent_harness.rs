//! Agent execution harness — wraps AgentRuntime with real tools for
//! end-to-end simulation.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

use agent::agent_runtime::AgentRuntime;
use agent::intent_pipeline::analysis::IntentAnalyzer;
use agent::intent_pipeline::engines::direct::DirectEngine;
use agent::intent_pipeline::engines::reactive::ReactiveEngine;
use agent::intent_pipeline::router::ExecutionRouter;
use agent::intent_pipeline::types::PipelineConfig;
use agent::AgentEvent;
use agent::ExecutionCore;
use bus::DomainEventBus;
use common::{ChannelName, ChatId};
use config::OrchestratorConfig;
use providers::DynProvider;
use skill_system::router::SkillRouter;
use skill_system::types::SkillCatalog;
use tools::registry::ToolRegistry;
use tools::RoutingContext;

use crate::agent_types::{AgentBreakpoint, AgentResult, BreakpointKind};
use crate::persona::types::AnnotatedMessage;
use crate::providers::SimulationProvider;

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
        skill_catalog: Arc<RwLock<SkillCatalog>>,
        skill_router: Arc<RwLock<SkillRouter>>,
        embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
        max_iterations: u32,
        seed: u64,
    ) -> common::Result<Self> {
        let provider: DynProvider = Arc::new(SimulationProvider::new(seed));

        // Build tool registry with real domain tools
        let mut tool_registry = ToolRegistry::new();
        Self::register_tools(&mut tool_registry, pool, &inner_pool, &bus);

        let tool_registry = Arc::new(RwLock::new(tool_registry));

        // Build execution core -> engines -> router
        let core = Arc::new(
            ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry))
                .with_domain_bus(Arc::clone(&bus)),
        );
        let direct = DirectEngine::new(Arc::clone(&core));
        let reactive = ReactiveEngine::new(Arc::clone(&core), max_iterations);
        let exec_router = ExecutionRouter::new(direct, reactive);

        // Build IntentAnalyzer (heuristic-only, no LLM classifier calls)
        let orch_config = OrchestratorConfig::default();
        let analyzer = IntentAnalyzer::new(provider.clone(), "simulation-agent", &orch_config)
            .with_shadow_mode();

        // Build context engine (minimal — no context sources for simulation)
        let context_engine = Arc::new(context_engine::ContextEngine::new());

        // Build cost tracker
        let usage_repo = storage::UsageRepo::new(inner_pool);
        let cost_tracker = Arc::new(agent::output::cost_tracker::CostTracker::from_repo(
            usage_repo,
        ));

        // Build hot config
        let hot_config = Arc::new(RwLock::new(config::HotConfig::from(
            &config::Config::default(),
        )));
        let active_profile = Arc::new(RwLock::new(None));

        // Assemble runtime
        let mut runtime = AgentRuntime::new(
            skill_catalog,
            skill_router,
            analyzer,
            context_engine,
            exec_router,
            cost_tracker,
            PipelineConfig::default(),
            active_profile,
            hot_config,
        );

        // Wire optional deps
        runtime = runtime
            .with_tool_registry(Arc::clone(&tool_registry))
            .with_domain_bus(Arc::clone(&bus))
            .with_context_update_queue(context_queue);

        if let Some(engine) = embedding_engine {
            runtime = runtime.with_embedding_engine(engine);
        }

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

        // Get tool definitions and names under a single lock
        let (tool_defs, tool_names) = {
            let registry = self.tool_registry.read().await;
            (registry.get_definitions(), registry.tool_names())
        };
        let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        // Collect agent events via channel
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);

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
                &tool_name_refs,
                &ctx,
                None, // no system prompt override
                Some(event_tx),
                None, // no cancellation
                None, // no correction context
            )
            .await;

        // Drain events to count tool calls and iterations
        let mut tool_calls = Vec::new();
        let mut iterations = 0u32;
        while let Ok(event) = event_rx.try_recv() {
            match event {
                AgentEvent::ToolStart { name, .. } => {
                    tool_calls.push(name);
                }
                AgentEvent::IterationStart { iteration, .. } => {
                    iterations = iteration as u32;
                }
                _ => {}
            }
        }

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

                // Check for low confidence
                if runtime_result.classification.confidence < 0.5 {
                    breakpoints.push(AgentBreakpoint {
                        kind: BreakpointKind::ClassificationLowConfidence,
                        message_content: msg.content.clone(),
                        details: format!(
                            "confidence {:.2} below threshold",
                            runtime_result.classification.confidence
                        ),
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
