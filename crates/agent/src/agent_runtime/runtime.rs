//! AgentRuntime — agent-first execution pipeline.
//!
//! Flow: SkillRouter.select_orchestrator → set active profile → IntentAnalyzer →
//! ContextEngine → tool filtering via SkillPackage → ExecutionRouter →
//! ResponseValidator → CostTracker → StrategyRepo

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{helpers::tool_def_name, Result};
use context_engine::{ContextEngine, ContextRequest, ExecutionStrategy};
use providers::Message;
use skill_system::types::{SkillCatalog, SkillPackage};
use tokio::sync::RwLock;
use tools::RoutingContext;
use tracing::{debug, warn};

use super::scenario;
use crate::autotuner::hooks::AutoTunerHook;
use crate::events::AgentEvent;

use crate::execution::ExecutionParams;
use crate::intent_pipeline::analysis::IntentAnalyzer;
use crate::intent_pipeline::router::{ExecutionRouter, RouterResult};
use crate::intent_pipeline::types::IntentAnalysis;
use crate::intent_pipeline::types::PipelineConfig;
use crate::output::cost_tracker::CostTracker;
use crate::output::validator::{ResponseValidator, ValidationResult};

/// Default maximum delegation depth to prevent infinite loops.
const MAX_DELEGATION_DEPTH: u32 = 2;

/// The agent used as orchestrator for multi-agent requests.
const ORCHESTRATOR_AGENT: &str = "general";

/// Tools allowed for the orchestrator (in addition to `delegate`, which is injected separately).
const ORCHESTRATOR_ALLOWED_TOOLS: &[&str] = &[tools::ask_user::ASK_USER_TOOL_NAME, "memory"];

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
    /// Multi-voice formatted output (all personas, markdown). Only set in squad mode.
    pub multi_voice: Option<String>,
    /// Raw per-persona responses for structured display. Only set in squad mode.
    pub persona_responses: Option<Vec<(String, String)>>,
}

/// Agent-driven runtime that replaces IntentPipeline.
///
/// The key difference: agent selection happens first, and the agent profile
/// shapes everything downstream (system prompt, tool filtering, iteration budget).
/// Bundled dependencies for squad chat mode.
pub(crate) struct SquadDeps {
    pub repo: cognitive::SquadRepo,
    pub provider: providers::DynProvider,
    pub chat_params: providers::ChatParams,
    pub blackboard_repo: Option<cognitive::BlackboardRepo>,
}

pub struct AgentRuntime {
    skill_catalog: Arc<RwLock<SkillCatalog>>,
    skill_router: Arc<RwLock<skill_system::router::SkillRouter>>,
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
    confidence_evaluator: Option<Arc<crate::confidence::ConfidenceEvaluator>>,
    /// Shared with SkillContextSource — written here, read during context assembly.
    active_profile: Arc<RwLock<Option<Arc<SkillPackage>>>>,
    /// Records interactions for behavioral pattern analysis.
    interaction_recorder: Option<crate::learning::InteractionRecorder>,
    /// Procedural rules repo for transparency (L5 cognitive rules).
    procedural_rule_repo: Option<cognitive::ProceduralRuleRepo>,
    /// Tool registry for looking up tool definitions during delegation.
    tool_registry: Option<Arc<RwLock<tools::registry::ToolRegistry>>>,
    /// Self-reference for delegation handler (set after Arc construction via OnceLock).
    delegation_self_ref: std::sync::OnceLock<Arc<dyn tools::DelegationHandler>>,
    /// Event sender for transparency events during delegation (set per-message).
    current_event_tx: RwLock<Option<tokio::sync::mpsc::Sender<AgentEvent>>>,
    /// Bundled dependencies for squad chat mode — always set together.
    squad_deps: Option<SquadDeps>,
    /// AutoTuner hook — runs shadow classification and records ground truth.
    autotuner_hook: Option<Arc<dyn AutoTunerHook>>,
    /// Shared user situation for building RetrievalContext.
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    /// Task repo for querying the focused task (active_task in RetrievalContext).
    task_repo: Option<storage::TaskRepo>,
    /// Shared active desktop view for query rewriting context.
    active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
    /// Shared activated skills — written per-message, read by SkillContextSource.
    activated_skills: Option<Arc<tokio::sync::RwLock<Vec<Arc<SkillPackage>>>>>,
}

impl AgentRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        skill_catalog: Arc<RwLock<SkillCatalog>>,
        skill_router: Arc<RwLock<skill_system::router::SkillRouter>>,
        analyzer: IntentAnalyzer,
        context_engine: Arc<ContextEngine>,
        router: ExecutionRouter,
        cost_tracker: Arc<CostTracker>,
        config: PipelineConfig,
        active_profile: Arc<RwLock<Option<Arc<SkillPackage>>>>,
    ) -> Self {
        Self {
            skill_catalog,
            skill_router,
            analyzer,
            context_engine,
            router,
            validator: ResponseValidator::new(config.max_response_tokens),
            cost_tracker,
            config,
            strategy_repo: None,
            confidence_evaluator: None,
            active_profile,
            interaction_recorder: None,
            procedural_rule_repo: None,
            tool_registry: None,
            delegation_self_ref: std::sync::OnceLock::new(),
            current_event_tx: RwLock::new(None),
            squad_deps: None,
            autotuner_hook: None,
            user_situation: None,
            task_repo: None,
            active_view: None,
            activated_skills: None,
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

    /// Set the tool registry for delegation support.
    pub fn with_tool_registry(
        mut self,
        registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    ) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Set squad execution dependencies for multi-persona squad mode.
    pub fn with_squad_deps(
        mut self,
        repo: cognitive::SquadRepo,
        provider: providers::DynProvider,
        chat_params: providers::ChatParams,
        blackboard_repo: Option<cognitive::BlackboardRepo>,
    ) -> Self {
        self.squad_deps = Some(SquadDeps {
            repo,
            provider,
            chat_params,
            blackboard_repo,
        });
        self
    }

    /// Set the autotuner hook for shadow classification callbacks.
    pub fn with_autotuner_hook(mut self, hook: Arc<dyn AutoTunerHook>) -> Self {
        self.autotuner_hook = Some(hook);
        self
    }

    /// Set the shared user situation for building RetrievalContext.
    pub fn with_user_situation(
        mut self,
        sit: Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>,
    ) -> Self {
        self.user_situation = Some(sit);
        self
    }

    /// Set the task repo for querying the focused task during query rewriting.
    pub fn with_task_repo(mut self, repo: storage::TaskRepo) -> Self {
        self.task_repo = Some(repo);
        self
    }

    /// Set the shared active desktop view for RetrievalContext.
    pub fn with_active_view(
        mut self,
        view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>,
    ) -> Self {
        self.active_view = Some(view);
        self
    }

    /// Set the shared activated skills for per-message skill activation.
    pub fn with_activated_skills(
        mut self,
        skills: Arc<tokio::sync::RwLock<Vec<Arc<SkillPackage>>>>,
    ) -> Self {
        self.activated_skills = Some(skills);
        self
    }

    /// Set the self-reference for delegation support (called after Arc wrapping).
    pub fn set_delegation_self_ref(&self, handler: Arc<dyn tools::DelegationHandler>) {
        let _ = self.delegation_self_ref.set(handler);
    }

    /// Get the shared active profile handle (for SkillContextSource).
    pub fn active_profile_handle(&self) -> Arc<RwLock<Option<Arc<SkillPackage>>>> {
        Arc::clone(&self.active_profile)
    }

    /// Get the shared skill catalog handle (for reload, delegation lookups, etc.).
    pub fn skill_catalog_handle(&self) -> Arc<RwLock<SkillCatalog>> {
        Arc::clone(&self.skill_catalog)
    }

    /// Get the shared skill router handle.
    pub fn skill_router_handle(&self) -> Arc<RwLock<skill_system::router::SkillRouter>> {
        Arc::clone(&self.skill_router)
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
        correction: Option<context_engine::CorrectionContext>,
    ) -> Result<RuntimeResult> {
        let pipeline_start = Instant::now();

        // Step 0a: AutoTuner hook — shadow classification before live processing.
        if let Some(ref hook) = self.autotuner_hook {
            hook.on_message_received(message, ctx.chat_id.as_str())
                .await;
        }

        // Step 1: Match message to orchestrator skill via SkillRouter
        let mut profile = {
            let catalog = self.skill_catalog.read().await;
            let router = self.skill_router.read().await;
            let champion_params = self
                .autotuner_hook
                .as_ref()
                .and_then(|h| h.current_champion_params());
            Arc::clone(router.select_orchestrator(message, &catalog, champion_params.as_ref()))
        };
        let mut agent_name = profile.name.clone();
        debug!("AgentRuntime: matched skill '{}'", agent_name);

        // Emit agent selection event for transparency
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::AgentSelected {
                    name: profile.name.clone(),
                    description: profile.description.clone(),
                })
                .await;

            // Emit always-loaded skill references
            for skill_name in profile.always_skills() {
                let _ = tx
                    .send(AgentEvent::SkillLoaded {
                        name: skill_name.clone(),
                        trigger: "always".to_string(),
                        agent: Some(profile.name.clone()),
                    })
                    .await;
            }
        }

        // Step 2: Set active profile for AgentContextSource
        {
            let mut guard = self.active_profile.write().await;
            *guard = Some(Arc::clone(&profile));
        }

        // Step 2a: Activate per-message skills (keyword-only for now)
        if let Some(ref activated_skills) = self.activated_skills {
            let catalog = self.skill_catalog.read().await;
            let router = self.skill_router.read().await;
            let activated = router.activate_skills(message, &[], &catalog, None);
            if !activated.is_empty() {
                let mut lock = activated_skills.write().await;
                lock.clear();
                for skill in activated {
                    lock.push(Arc::clone(skill));
                }
            }
        }

        // Step 2b: Squad detection — if a squad_id is set, fan out to personas
        if let (Some(ref squad_id), Some(ref deps)) = (&ctx.squad_id, &self.squad_deps) {
            let result = self
                .run_squad_execution(
                    message,
                    history,
                    system_prompt,
                    event_tx,
                    squad_id,
                    &deps.repo,
                    &deps.provider,
                    &deps.chat_params,
                    ctx.squad_mode.as_deref(),
                    cancel_token.as_ref(),
                )
                .await;

            // Write ground truth for shadow classification (Step 0a wrote the shadow log).
            if let Some(ref hook) = self.autotuner_hook {
                let elapsed = pipeline_start.elapsed().as_millis() as u64;
                hook.on_message_completed(ctx.chat_id.as_str(), "squad", "reactive", 0, elapsed)
                    .await;
            }

            return result;
        }

        // Step 3: Filter MCP tool names to those the matched agent can access
        let filtered_tool_names: Vec<&str> = tool_names
            .iter()
            .filter(|name| {
                match mcp::sanitize::extract_server_name(name) {
                    Some(server) => profile.allows_mcp_server(server),
                    None => true, // Native tools pass through (filtered separately by profile.tools)
                }
            })
            .copied()
            .collect();

        // Step 4: Classify intent
        let classify_start = Instant::now();
        let mut analysis = self.analyzer.analyze(message, &filtered_tool_names).await;
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

        // Step 3b: Orchestration override — route multi-agent intents to orchestrator
        if analysis.needs_orchestration {
            let general = {
                let catalog = self.skill_catalog.read().await;
                catalog.get(ORCHESTRATOR_AGENT).cloned()
            };
            if let Some(general) = general {
                debug!(
                    "Orchestration override: routing '{}' → {} agent",
                    agent_name, ORCHESTRATOR_AGENT
                );
                profile = general;
                agent_name = ORCHESTRATOR_AGENT.to_string();

                // Update active profile
                {
                    let mut guard = self.active_profile.write().await;
                    *guard = Some(Arc::clone(&profile));
                }

                // Emit updated agent selection so the UI reflects the orchestrator
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(AgentEvent::AgentSelected {
                            name: profile.name.clone(),
                            description: profile.description.clone(),
                        })
                        .await;
                }

                // Increase iteration budget for orchestration (multiple delegations)
                if let crate::intent_pipeline::types::ExecutionMode::Reactive {
                    ref mut max_iterations,
                } = analysis.mode
                {
                    *max_iterations = (*max_iterations)
                        .max(crate::intent_pipeline::analysis::ORCHESTRATION_MIN_ITERATIONS);
                }
            }
        }

        // Step 4: Cap max_iterations from agent profile (skip for orchestrator —
        // the orchestration boost in step 3b must not be clipped by the profile cap)
        if !analysis.needs_orchestration {
            if let crate::intent_pipeline::types::ExecutionMode::Reactive {
                ref mut max_iterations,
            } = analysis.mode
            {
                *max_iterations = (*max_iterations).min(profile.max_iterations());
            }
        }

        // Step 5: Confidence check — downgrade to Direct mode for low-confidence
        // instead of blocking the user with a technical clarification message.
        if let Some(ref evaluator) = self.confidence_evaluator {
            let threshold = evaluator.threshold();
            if analysis.confidence < threshold {
                debug!(
                    "AgentRuntime: low confidence ({:.2} < {:.2}), downgrading to Direct mode",
                    analysis.confidence, threshold
                );
                analysis.mode = crate::intent_pipeline::types::ExecutionMode::Direct;
            }
        }

        // Step 5.5: Build retrieval context for query rewriting
        let retrieval_context =
            {
                let active_skill = Some(profile.name.clone());

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

                let active_task =
                    if let Some(ref repo) = self.task_repo {
                        match repo.list_focused().await {
                            Ok(tasks) => tasks.into_iter().next().map(|t| {
                                context_engine::ActiveTaskContext {
                                    title: t.title,
                                    project_name: t.project_id,
                                    domain: active_skill
                                        .as_deref()
                                        .map(|s| s.replace("-management", "").replace('-', " ")),
                                }
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
                    active_skill,
                    active_task,
                    recent_user_messages,
                    situation,
                    active_view: if let Some(ref view_lock) = self.active_view {
                        view_lock.read().await.clone()
                    } else {
                        None
                    },
                    recent_correction: correction,
                })
            };

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
            session_key: Some(common::SessionKey::new(&ctx.channel, &ctx.chat_id).to_string()),
            retrieval_context,
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

        // Emit learning context summaries for transparency
        if let Some(ref tx) = event_tx {
            self.emit_learning_summary(tx).await;
        }

        // Step 7: Filter tools based on agent profile (replaces ToolGroup filtering)
        // When orchestrating, restrict to coordination-only tools (delegate is added in 7b).
        let mut filtered_tools = if analysis.needs_orchestration {
            tool_definitions
                .iter()
                .filter(|t| {
                    tool_def_name(t)
                        .map(|n| ORCHESTRATOR_ALLOWED_TOOLS.contains(&n))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            filter_tools_for_profile(tool_definitions, &profile)
        };

        // Step 7b: Add DelegationTool if agent can delegate and depth allows
        inject_delegation_tool(
            &profile,
            ctx.delegation_depth,
            &self.delegation_self_ref,
            &self.tool_registry,
            &mut filtered_tools,
        )
        .await;

        debug!(
            "AgentRuntime: filtered {} → {} tools (agent: {})",
            tool_definitions.len(),
            filtered_tools.len(),
            agent_name
        );

        // Store event_tx for delegation transparency events
        *self.current_event_tx.write().await = event_tx.clone();

        // Step 7c: Chain-of-thought planning for complex tasks
        // Threshold 4: triggers for multi-step requests (3+ tools + sequential deps)
        const COT_COMPLEXITY_THRESHOLD: u8 = 4;
        let complexity_score = analysis.signals.complexity_score();

        let planning_prompt = match analysis.mode {
            crate::intent_pipeline::types::ExecutionMode::Reactive { .. }
                if analysis.signals.has_hypothetical =>
            {
                // Scenario reasoning — use specialized prompt
                Some(scenario::build_scenario_prompt(
                    message,
                    &filtered_tools,
                    self.config.scenario_max_graph_depth,
                ))
            }
            crate::intent_pipeline::types::ExecutionMode::Reactive { .. }
                if complexity_score >= COT_COMPLEXITY_THRESHOLD =>
            {
                Some(build_planning_prompt(message, &filtered_tools))
            }
            _ => None,
        };

        if planning_prompt.is_some() {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(AgentEvent::PlanningStarted { complexity_score })
                    .await;
            }
        }

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
        if let Some(prompt) = planning_prompt {
            params = params.with_planning_prompt(prompt);
        }

        let timeout_secs = self.config.pipeline_timeout_secs;
        if timeout_secs > 0 {
            params = params.with_pipeline_timeout(Duration::from_secs(timeout_secs));
        }

        let retrieved_memory_count = assembled.retrieved_memory_count;
        let rewrite_triggered = assembled.rewrite_triggered;
        let rewrite_source = assembled.rewrite_source.clone();

        let pipeline_future = self.router.execute(
            analysis.mode.clone(),
            assembled.messages,
            &filtered_tools,
            &params,
            ctx,
            event_tx.clone(),
        );

        let router_result = if let Some(timeout_dur) = params.pipeline_timeout {
            match tokio::time::timeout(timeout_dur, pipeline_future).await {
                Ok(result) => result?,
                Err(_) => {
                    warn!("Pipeline execution timed out after {:?}", timeout_dur);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(AgentEvent::Error {
                                message: format!(
                                    "Execution timed out after {}s",
                                    timeout_dur.as_secs()
                                ),
                            })
                            .await;
                    }
                    return Err(common::KlyntbotError::Timeout(format!(
                        "Pipeline execution exceeded {}s limit",
                        timeout_dur.as_secs()
                    )));
                }
            }
        } else {
            pipeline_future.await?
        };

        // Clear event_tx to prevent stale state across calls
        *self.current_event_tx.write().await = None;

        // Step 9: Validate
        let mut validation = self.validator.validate(&router_result.content);
        if !validation.is_valid {
            warn!(
                "AgentRuntime: response validation failed with {} warning(s)",
                validation.warnings.len()
            );
        }

        let mode_name = router_result.final_mode.clone();

        // Step 10: Record usage + strategy + interaction in parallel (independent DB writes)
        let pipeline_elapsed_ms = pipeline_start.elapsed().as_millis() as u64;
        let usage_fut = self.record_usage(
            &router_result,
            &mode_name,
            ctx,
            &event_tx,
            pipeline_elapsed_ms,
        );
        let strategy_fut = self.record_strategy(
            &analysis,
            &router_result,
            &validation,
            ctx,
            pipeline_start,
            retrieved_memory_count,
            rewrite_triggered,
            rewrite_source,
        );
        let interaction_fut = async {
            if let Some(ref recorder) = self.interaction_recorder {
                let tools_used: Vec<&str> =
                    router_result.tool_name.as_deref().into_iter().collect();
                recorder
                    .record(
                        &agent_name,
                        &tools_used,
                        ctx.channel.as_str(),
                        pipeline_elapsed_ms,
                    )
                    .await;
            }
        };
        tokio::join!(usage_fut, strategy_fut, interaction_fut);

        // Step 11: AutoTuner hook — record ground truth after response delivery.
        if let Some(ref hook) = self.autotuner_hook {
            let tokens = router_result.usage.prompt_tokens + router_result.usage.completion_tokens;
            hook.on_message_completed(
                ctx.chat_id.as_str(),
                &agent_name,
                analysis.mode.short_name(),
                tokens,
                pipeline_elapsed_ms,
            )
            .await;
        }

        let final_content = std::mem::take(&mut validation.filtered_content);

        Ok(RuntimeResult {
            content: final_content,
            mode_used: mode_name,
            classification: analysis,
            validation,
            agent_name,
            multi_voice: None,
            persona_responses: None,
        })
    }

    /// Emit learning context summary events for transparency panel.
    async fn emit_learning_summary(&self, tx: &tokio::sync::mpsc::Sender<AgentEvent>) {
        // Learned procedural rules (from L5 cognitive pipeline)
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

        // Confidence threshold (in-memory read, no DB hit)
        if let Some(ref evaluator) = self.confidence_evaluator {
            let threshold = evaluator.threshold();
            let _ = tx
                .send(AgentEvent::LearningEvent {
                    event_type: "confidence".into(),
                    detail: format!("threshold: {:.0}%", threshold * 100.0),
                })
                .await;
        }
    }

    /// Execute via multi-persona squad fan-out instead of normal pipeline.
    #[allow(clippy::too_many_arguments)]
    async fn run_squad_execution(
        &self,
        message: &str,
        history: Vec<Message>,
        system_prompt: Option<&str>,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        squad_id: &str,
        squad_repo: &cognitive::SquadRepo,
        provider: &providers::DynProvider,
        params: &providers::ChatParams,
        _squad_mode_str: Option<&str>,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<RuntimeResult> {
        use crate::intent_pipeline::engines;
        use crate::intent_pipeline::engines::squad;

        // 1. Resolve squad
        let resolved = squad_repo
            .resolve_squad(squad_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("squad resolve: {e}")))?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("squad '{squad_id}' not found"))
            })?;

        // 2. Build orchestrator context from system prompt + conversation history
        let mut context_parts = vec![system_prompt
            .unwrap_or("You are a helpful AI assistant.")
            .to_string()];
        for msg in &history {
            match msg {
                Message::User {
                    content: providers::UserContent::Text(t),
                } => {
                    context_parts.push(format!("User: {t}"));
                }
                Message::Assistant {
                    content: Some(c), ..
                } => {
                    context_parts.push(format!("Assistant: {c}"));
                }
                _ => {}
            }
        }
        let orchestrator_context = context_parts.join("\n\n");

        // 3. Persona fan-out — always use room debate when blackboard is available
        let blackboard_repo = self
            .squad_deps
            .as_ref()
            .and_then(|d| d.blackboard_repo.as_ref());
        let use_debate = blackboard_repo.is_some();

        let persona_responses = if use_debate {
            let blackboard_repo = blackboard_repo.unwrap();
            let debate_session_key = format!("debate:{}:{}", squad_id, uuid::Uuid::new_v4());
            let debate_fut = engines::debate::run_room_debate(
                provider,
                &orchestrator_context,
                message,
                &resolved.personas,
                params,
                blackboard_repo,
                &debate_session_key,
                squad_id,
                event_tx.as_ref(),
                cancel_token,
            );
            let debate_results =
                match tokio::time::timeout(std::time::Duration::from_secs(120), debate_fut).await {
                    Ok(results) => results,
                    Err(_) => {
                        tracing::warn!("Squad debate timed out after 120s");
                        Vec::new()
                    }
                };

            // Collect responses from the final round for synthesis
            debate_results
                .last()
                .map(|(_, responses, _)| responses.clone())
                .unwrap_or_default()
        } else {
            squad::fan_out_personas(
                provider,
                &orchestrator_context,
                message,
                &resolved.personas,
                params,
                event_tx.as_ref(),
            )
            .await
        };

        // 4. Synthesis
        let synthesis_prompt = squad::build_squad_synthesis_prompt(message, &persona_responses);
        let synthesis_messages = vec![
            Message::System {
                content: synthesis_prompt,
            },
            Message::User {
                content: providers::UserContent::Text("Synthesize now.".to_string()),
            },
        ];
        let synthesis = provider.chat(&synthesis_messages, None, params).await?;

        let multi_voice = squad::format_multi_voice(&persona_responses);
        let content = synthesis.content.unwrap_or_else(|| multi_voice.clone());

        // Build a minimal classification for the result
        let classification = IntentAnalysis {
            mode: crate::intent_pipeline::types::ExecutionMode::Direct,
            signals: crate::intent_pipeline::types::ComplexitySignals {
                estimated_tool_calls: 0,
                has_sequential_deps: false,
                failure_risk: crate::intent_pipeline::types::FailureRisk::Low,
                requires_state_tracking: false,
                requires_retries: false,
                has_hypothetical: false,
            },
            confidence: 1.0,
            source: crate::intent_pipeline::types::AnalysisSource::Heuristic,
            reasoning: "Squad multi-persona execution".to_string(),
            needs_orchestration: false,
        };

        Ok(RuntimeResult {
            content: content.clone(),
            mode_used: "squad".to_string(),
            classification,
            validation: self.validator.validate(&content),
            agent_name: format!("squad:{}", resolved.squad.name),
            multi_voice: Some(multi_voice),
            persona_responses: Some(persona_responses),
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

    async fn record_strategy(
        &self,
        analysis: &IntentAnalysis,
        result: &RouterResult,
        validation: &ValidationResult,
        ctx: &RoutingContext,
        start: Instant,
        retrieved_memory_count: usize,
        rewrite_triggered: bool,
        rewrite_source: Option<String>,
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
            escalation_count: result.escalated as i32,
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
            retrieved_memory_count: Some(retrieved_memory_count as i32),
            rewrite_triggered: rewrite_triggered as i32,
            rewrite_source,
        };

        if let Err(e) = strategy_repo.create(&record).await {
            warn!("AgentRuntime: failed to record strategy: {}", e);
        }
    }
}

/// Create a filtered event sender for delegation.
///
/// Spawns a tokio task that reads from a new channel, suppresses sub-agent
/// reasoning (ContentChunk, IterationStart), and forwards tool/skill events
/// with agent attribution injected.
fn delegation_event_filter(
    parent_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    agent_name: String,
) -> tokio::sync::mpsc::Sender<AgentEvent> {
    let (filtered_tx, mut filtered_rx) = tokio::sync::mpsc::channel::<AgentEvent>(64);

    tokio::spawn(async move {
        while let Some(event) = filtered_rx.recv().await {
            let forwarded = match event {
                // Suppress sub-agent reasoning from reaching the parent stream
                AgentEvent::ContentChunk { .. } | AgentEvent::IterationStart { .. } => continue,

                // Forward tool events with agent attribution
                AgentEvent::ToolStart { name, args, .. } => AgentEvent::ToolStart {
                    name,
                    args,
                    agent: Some(agent_name.clone()),
                },
                AgentEvent::ToolEnd {
                    name,
                    success,
                    duration_ms,
                    result,
                    ..
                } => AgentEvent::ToolEnd {
                    name,
                    success,
                    duration_ms,
                    result,
                    agent: Some(agent_name.clone()),
                },
                AgentEvent::SkillLoaded { name, trigger, .. } => AgentEvent::SkillLoaded {
                    name,
                    trigger,
                    agent: Some(agent_name.clone()),
                },

                // Pass through all other events unchanged
                other => other,
            };

            if parent_tx.send(forwarded).await.is_err() {
                break;
            }
        }
    });

    filtered_tx
}

/// Filter tool definitions to only those allowed by the skill package.
/// Native tools are filtered by `pkg.tools` (None = all allowed, Some([]) = deny-all).
/// MCP tools are filtered by `pkg.mcp_tools` (empty = none allowed, `["*"]` = all).
fn filter_tools_for_profile(
    tool_defs: &[serde_json::Value],
    profile: &SkillPackage,
) -> Vec<serde_json::Value> {
    let native_allowlist = profile.allowed_tool_names();

    tool_defs
        .iter()
        .filter(|t| {
            let Some(name) = tool_def_name(t) else {
                return true;
            };

            if let Some(server_name) = mcp::sanitize::extract_server_name(name) {
                return profile.allows_mcp_server(server_name);
            }

            match &native_allowlist {
                Some(allowed) => allowed.contains(name),
                None => true,
            }
        })
        .cloned()
        .collect()
}

/// Inject a DelegationTool into the tool list and registry if the agent can delegate
/// and we haven't reached max depth. Returns true if the tool was injected.
async fn inject_delegation_tool(
    profile: &SkillPackage,
    depth: u32,
    delegation_self_ref: &std::sync::OnceLock<Arc<dyn tools::DelegationHandler>>,
    tool_registry: &Option<Arc<RwLock<tools::registry::ToolRegistry>>>,
    filtered_tools: &mut Vec<serde_json::Value>,
) {
    if profile.can_delegate_to().is_empty() || depth >= MAX_DELEGATION_DEPTH {
        return;
    }

    let Some(handler) = delegation_self_ref.get() else {
        return;
    };

    let delegation_tool = tools::DelegationTool::with_handler(handler.clone())
        .with_allowed_agents(profile.can_delegate_to().to_vec())
        .with_depth(depth, MAX_DELEGATION_DEPTH);

    filtered_tools.push(tools::Tool::to_schema(&delegation_tool));

    if let Some(ref registry) = tool_registry {
        let mut reg = registry.write().await;
        reg.register(delegation_tool);
    }
}

#[async_trait::async_trait]
impl tools::DelegationHandler for AgentRuntime {
    async fn delegate(
        &self,
        agent_name: &str,
        query: &str,
        ctx: &RoutingContext,
        depth: u32,
    ) -> Result<String> {
        let start = Instant::now();

        // 1. Look up the delegated skill package
        let profile = {
            let catalog = self.skill_catalog.read().await;
            catalog.get(agent_name).cloned()
        }
        .ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Unknown agent for delegation: '{agent_name}'"
            )))
        })?;

        // Read the current caller agent name for transparency events
        let caller_name = {
            let guard = self.active_profile.read().await;
            guard
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "unknown".to_string())
        };

        debug!(
            "Delegation: {} → {} for query '{}' (depth {})",
            caller_name, agent_name, query, depth
        );

        // Emit DelegationStarted event and sub-agent skills
        if let Some(tx) = self.current_event_tx.read().await.as_ref() {
            let _ = tx
                .send(AgentEvent::DelegationStarted {
                    from_agent: caller_name.clone(),
                    to_agent: agent_name.to_string(),
                    query: query.to_string(),
                    depth,
                })
                .await;

            // Emit SkillLoaded for the delegated skill's always-loaded references
            for skill_name in profile.always_skills() {
                let _ = tx
                    .send(AgentEvent::SkillLoaded {
                        name: skill_name.clone(),
                        trigger: "always".to_string(),
                        agent: Some(agent_name.to_string()),
                    })
                    .await;
            }
        }

        // 2. Set the delegated agent as active profile (for AgentContextSource)
        {
            let mut guard = self.active_profile.write().await;
            *guard = Some(Arc::clone(&profile));
        }

        // 3. Build context with the delegated agent's instructions
        let messages = vec![Message::user(query)];
        let strategy = ExecutionStrategy::ToolAssisted {
            max_iterations: profile.max_iterations().min(8),
        };
        let context_request = ContextRequest {
            message_text: query.to_string(),
            history: messages,
            system_prompt: self.config.system_prompt.clone(),
            strategy,
            tool_definitions: vec![],
            context_window: self.config.context_window,
            session_key: None, // delegation — no session tracking
            retrieval_context: None,
        };
        let assembled = self.context_engine.assemble(context_request).await;

        // 4. Filter tools to delegated agent's allowed set
        let tool_defs: Vec<serde_json::Value> = if let Some(ref registry) = self.tool_registry {
            let reg = registry.read().await;
            reg.get_definitions().to_vec()
        } else {
            vec![]
        };

        let mut filtered_tools = filter_tools_for_profile(&tool_defs, &profile);

        // 5. Optionally add DelegationTool for chained delegation
        inject_delegation_tool(
            &profile,
            depth,
            &self.delegation_self_ref,
            &self.tool_registry,
            &mut filtered_tools,
        )
        .await;

        // 6. Execute via router with reduced budget
        let max_iters = profile.max_iterations().min(8);
        let mode = crate::intent_pipeline::types::ExecutionMode::Reactive {
            max_iterations: max_iters,
        };
        let params = ExecutionParams::new(&self.config.execution_model)
            .with_max_iterations(max_iters)
            .with_original_message(query.to_string());

        // Build delegated routing context with incremented depth
        let mut delegated_ctx = ctx.clone();
        delegated_ctx.delegation_depth = depth + 1;

        // Use a filtered event sender that suppresses sub-agent reasoning
        // and injects agent attribution into tool/skill events.
        let delegation_tx = {
            let parent_tx = self.current_event_tx.read().await.clone();
            parent_tx.map(|tx| delegation_event_filter(tx, agent_name.to_string()))
        };

        let result = self
            .router
            .execute(
                mode,
                assembled.messages,
                &filtered_tools,
                &params,
                &delegated_ctx,
                delegation_tx,
            )
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = result.is_ok();

        // Emit DelegationCompleted event
        if let Some(tx) = self.current_event_tx.read().await.as_ref() {
            let _ = tx
                .send(AgentEvent::DelegationCompleted {
                    from_agent: caller_name.clone(),
                    to_agent: agent_name.to_string(),
                    success,
                    duration_ms,
                })
                .await;
        }

        // Restore caller's active profile
        // (The caller will re-set it if needed in its own process_message flow)

        debug!(
            "Delegation {} → {} completed in {}ms (success: {})",
            caller_name, agent_name, duration_ms, success
        );

        result.map(|r| r.content)
    }
}

/// Build a chain-of-thought planning prompt for complex tasks.
fn build_planning_prompt(user_message: &str, tools: &[serde_json::Value]) -> String {
    let tool_names: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();
    format!(
        "This is a complex request. Before executing:\n\
         1. Briefly consider the optimistic, skeptical, and practical angles.\n\
         2. Synthesize into a balanced approach.\n\
         3. Then create a step-by-step plan.\n\
         \n\
         User request: {user_message}\n\
         Available tools: [{}]\n\
         \n\
         Format each step as:\n\
         1. <description> [tool: <tool_name>]\n\
         2. <description> [tool: <tool_name>]\n\
         ...\n\
         \n\
         Keep the plan concise (3-7 steps). Then execute step 1.",
        tool_names.join(", ")
    )
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
    use tools::DelegationHandler;

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

    fn make_skill_catalog_and_router() -> (
        Arc<RwLock<SkillCatalog>>,
        Arc<RwLock<skill_system::router::SkillRouter>>,
    ) {
        let source = skill_system::discovery::SkillSource::BuiltIn(
            skill_system::discovery::BUILTIN_SKILLS
                .iter()
                .map(|(n, c)| (n.to_string(), c.to_string()))
                .collect(),
        );
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let router = skill_system::router::SkillRouter::new(&catalog);
        (
            Arc::new(RwLock::new(catalog)),
            Arc::new(RwLock::new(router)),
        )
    }

    async fn make_runtime(provider: Arc<dyn LlmProvider>) -> AgentRuntime {
        let (runtime, _registry) = make_runtime_with_registry(provider).await;
        runtime
    }

    /// Shared builder: returns the runtime and its tool registry for delegation wiring.
    async fn make_runtime_with_registry(
        provider: Arc<dyn LlmProvider>,
    ) -> (AgentRuntime, Arc<RwLock<ToolRegistry>>) {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(provider.clone(), Arc::clone(&registry)));

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
        let (skill_catalog, skill_router) = make_skill_catalog_and_router();

        let runtime = AgentRuntime::new(
            skill_catalog,
            skill_router,
            analyzer,
            Arc::new(ContextEngine::new()),
            router,
            cost_tracker,
            PipelineConfig::default(),
            active_profile,
        );
        (runtime, registry)
    }

    #[tokio::test]
    async fn test_agent_runtime_selects_correct_skill() {
        let (catalog, router) = make_skill_catalog_and_router();
        let catalog = catalog.read().await;
        let router = router.read().await;
        let selected = router.select_orchestrator("create a task to review budget", &catalog, None);
        assert_eq!(selected.name, "task-management");
    }

    #[tokio::test]
    async fn test_runtime_processes_greeting_via_general_agent() {
        let provider = MockProvider::new(vec![text_response("Hi there!")]);
        let runtime = make_runtime(provider).await;

        let result = runtime
            .process_message(
                "hello",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
                None,
                None,
            )
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
                "create a task for my project planning",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.agent_name, "task-management");
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
                None,
            )
            .await
            .unwrap();

        // After processing, active profile should be set
        let guard = handle.read().await;
        assert!(guard.is_some());
        assert_eq!(guard.as_ref().unwrap().name, "finance-management");
    }

    #[tokio::test]
    async fn test_runtime_downgrades_to_direct_on_low_confidence() {
        let provider = MockProvider::new(vec![text_response("Hello! How can I help?")]);
        let runtime = make_runtime(provider)
            .await
            .with_confidence_evaluator(Arc::new(crate::confidence::ConfidenceEvaluator::new(0.99)));

        let result = runtime
            .process_message(
                "hello",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Low confidence should downgrade to Direct mode, not block with clarification
        assert_eq!(result.mode_used, "direct");
        assert_eq!(result.content, "Hello! How can I help?");
    }

    /// Helper: build runtime with tool_registry and delegation_self_ref wired up.
    async fn make_delegation_runtime(provider: Arc<dyn LlmProvider>) -> Arc<AgentRuntime> {
        let (runtime, registry) = make_runtime_with_registry(provider).await;
        let runtime = Arc::new(runtime.with_tool_registry(registry));
        runtime.set_delegation_self_ref(Arc::clone(&runtime) as Arc<dyn tools::DelegationHandler>);
        runtime
    }

    #[tokio::test]
    async fn test_multi_agent_heuristic_defers_to_llm() {
        // A message with both finance and task triggers + sequential language
        // should cause the heuristic to return None (defer to LLM classifier).
        use crate::intent_pipeline::analysis::analyze_heuristic;

        let result = analyze_heuristic(
            "first check my transactions then create a task for the missing ones",
        );
        assert!(
            result.is_none(),
            "Expected heuristic to defer multi-agent message to LLM, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_delegation_tool_injected_for_general_agent() {
        // The general agent has can_delegate_to: [task, finance, automation, communication]
        // When delegation_self_ref is set and depth < max, the delegate tool should be added.
        let provider = MockProvider::new(vec![text_response("Hello!")]);
        let runtime = make_delegation_runtime(provider).await;

        // Use an event channel to capture events
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);

        let result = runtime
            .process_message(
                "hello",
                vec![],
                &[],
                &[],
                &routing_ctx(),
                None,
                Some(tx),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.agent_name, "general");

        // Verify the delegation tool was registered in the tool registry
        if let Some(ref registry) = runtime.tool_registry {
            let reg = registry.read().await;
            let defs = reg.get_definitions();
            let has_delegate = defs.iter().any(|d| {
                d.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("delegate")
            });
            assert!(
                has_delegate,
                "Expected 'delegate' tool in registry for general agent"
            );
        }

        // Drain events (we don't assert specific events here, just ensure no panic)
        rx.close();
        while rx.recv().await.is_some() {}
    }

    #[tokio::test]
    async fn test_delegation_tool_not_injected_at_max_depth() {
        // When delegation_depth >= MAX_DELEGATION_DEPTH, the delegate tool should NOT be added.
        let provider = MockProvider::new(vec![text_response("Hello!")]);
        let runtime = make_delegation_runtime(provider).await;

        let mut ctx = routing_ctx();
        ctx.delegation_depth = MAX_DELEGATION_DEPTH; // At max depth

        let result = runtime
            .process_message("hello", vec![], &[], &[], &ctx, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(result.agent_name, "general");

        // Verify the delegation tool was NOT registered
        if let Some(ref registry) = runtime.tool_registry {
            let reg = registry.read().await;
            let defs = reg.get_definitions();
            let has_delegate = defs.iter().any(|d| {
                d.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("delegate")
            });
            assert!(
                !has_delegate,
                "Expected NO 'delegate' tool at max delegation depth"
            );
        }
    }

    #[tokio::test]
    async fn test_delegation_events_emitted() {
        // Verify DelegationStarted and DelegationCompleted events are emitted during delegation.
        let provider = MockProvider::new(vec![
            // First call: the general agent's LLM response (delegate tool won't be called since
            // the mock just returns text). We test the DelegationHandler directly instead.
            text_response("Delegated response"),
        ]);
        let runtime = make_delegation_runtime(provider).await;

        let (tx, mut rx) = tokio::sync::mpsc::channel(32);

        // Set up event channel and active profile for the delegation
        *runtime.current_event_tx.write().await = Some(tx);
        {
            let catalog = runtime.skill_catalog.read().await;
            let general = catalog.get("general").unwrap();
            let mut guard = runtime.active_profile.write().await;
            *guard = Some(Arc::clone(general));
        }

        // Call delegate directly
        let ctx = routing_ctx();
        let result: common::Result<String> = runtime
            .delegate("task-management", "list my tasks", &ctx, 1)
            .await;

        assert!(result.is_ok(), "Delegation should succeed: {:?}", result);

        // Collect emitted events
        rx.close();
        let mut events = vec![];
        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        // Should have DelegationStarted and DelegationCompleted
        let started = events.iter().any(
            |e| matches!(e, AgentEvent::DelegationStarted { to_agent, .. } if to_agent == "task-management"),
        );
        let completed = events.iter().any(|e| {
            matches!(e, AgentEvent::DelegationCompleted { to_agent, success, .. } if to_agent == "task-management" && *success)
        });

        assert!(
            started,
            "Expected DelegationStarted event for 'task-management'"
        );
        assert!(
            completed,
            "Expected DelegationCompleted event for 'task-management'"
        );
    }

    mod filter_tests {
        use super::*;
        use serde_json::json;
        use skill_system::types::{KlyntbotMeta, SkillMetadata, SkillScope, SkillType};
        use std::path::PathBuf;
        use std::time::SystemTime;

        fn make_tool_def(name: &str) -> serde_json::Value {
            json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": "test tool",
                    "parameters": { "type": "object", "properties": {} }
                }
            })
        }

        fn make_test_pkg(
            name: &str,
            tools: Option<Vec<String>>,
            mcp_tools: Vec<String>,
        ) -> SkillPackage {
            SkillPackage {
                name: name.into(),
                description: "test".into(),
                skill_type: SkillType::Skill,
                scope: SkillScope::BuiltIn,
                location: PathBuf::new(),
                body: String::new(),
                metadata: SkillMetadata {
                    klyntbot: Some(KlyntbotMeta {
                        tools,
                        mcp_tools,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                resources: Vec::new(),
                loaded_at: SystemTime::now(),
                trusted: true,
            }
        }

        #[test]
        fn test_filter_blocks_mcp_tools_for_restricted_agent() {
            let profile = make_test_pkg("task", Some(vec!["task".into(), "area".into()]), vec![]);

            let tool_defs = vec![
                make_tool_def("task"),
                make_tool_def("area"),
                make_tool_def("mcp_linear_create_issue"),
                make_tool_def("mcp_linear_list_issues"),
                make_tool_def("finance"),
            ];

            let filtered = filter_tools_for_profile(&tool_defs, &profile);
            let names: Vec<&str> = filtered.iter().filter_map(|t| tool_def_name(t)).collect();

            // ask_user is always added, so task and area pass, plus ask_user if present
            assert!(names.contains(&"task"));
            assert!(names.contains(&"area"));
            assert!(!names.contains(&"mcp_linear_create_issue"));
            assert!(!names.contains(&"mcp_linear_list_issues"));
            assert!(!names.contains(&"finance"));
        }

        #[test]
        fn test_filter_allows_mcp_tools_for_wildcard_agent() {
            // tools: None means all native tools allowed
            let profile = make_test_pkg("general", None, vec!["*".into()]);

            let tool_defs = vec![
                make_tool_def("task"),
                make_tool_def("mcp_linear_create_issue"),
                make_tool_def("mcp_github_list_repos"),
            ];

            let filtered = filter_tools_for_profile(&tool_defs, &profile);
            let names: Vec<&str> = filtered.iter().filter_map(|t| tool_def_name(t)).collect();

            assert!(names.contains(&"task"));
            assert!(names.contains(&"mcp_linear_create_issue"));
            assert!(names.contains(&"mcp_github_list_repos"));
        }

        #[test]
        fn test_filter_allows_specific_mcp_server() {
            let profile =
                make_test_pkg("comms", Some(vec!["message".into()]), vec!["linear".into()]);

            let tool_defs = vec![
                make_tool_def("message"),
                make_tool_def("mcp_linear_create_issue"),
                make_tool_def("mcp_github_list_repos"),
                make_tool_def("task"),
            ];

            let filtered = filter_tools_for_profile(&tool_defs, &profile);
            let names: Vec<&str> = filtered.iter().filter_map(|t| tool_def_name(t)).collect();

            assert!(names.contains(&"message"));
            assert!(names.contains(&"mcp_linear_create_issue"));
            assert!(!names.contains(&"mcp_github_list_repos"));
            assert!(!names.contains(&"task"));
        }
    }
}
