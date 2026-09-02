use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of AI pipeline initialization results for FeatureHost storage.
pub struct AiPipelineInitResult {
    pub router: Arc<ai_core::SignalRouter>,
    pub _cognitive_handle: tokio::task::JoinHandle<()>,
    pub _coaching_service: Option<feature_coaching::CoachingService>,
}

/// Plugin that initializes the AI pipeline (SignalRouter + all consumers).
pub struct AiPipelinePlugin;

#[async_trait]
impl AppCorePlugin for AiPipelinePlugin {
    fn name(&self) -> &str {
        "ai-pipeline"
    }

    fn dependencies(&self) -> &[&str] {
        // activity_log: init() calls ctx.require_activity_svc()
        &["mirror", "coaching", "cognitive", "activity_log"]
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let config = ctx.deps.config.read().await;
        let mirror_consumers = ctx
            .host
            .get::<crate::plugins::mirror::MirrorInitResult>()
            .map(|r| r.consumers.clone())
            .unwrap_or_default();

        let observation_repo = ::cognitive::repos::AccumulatedObservationRepo::new(
            ctx.deps.storage_pool.inner().clone(),
        );
        let entity_repo =
            ::cognitive::repos::EntityRepo::new(ctx.deps.storage_pool.inner().clone());
        let episodic_repo =
            ::cognitive::EpisodicMemoryRepo::new(ctx.deps.storage_pool.inner().clone());
        let extraction_handler: Option<Arc<dyn ::cognitive::ExtractionHandler>> =
            ctx.deps.cognitive_provider.as_ref().map(|cp| {
                let params = providers::cognitive_chat_params(&config, 4096);
                Arc::new(::agent::cognitive_handlers::LlmExtractionHandler::new(
                    cp.clone(),
                    params,
                )) as Arc<dyn ::cognitive::ExtractionHandler>
            });
        let audd_resolver: Option<Arc<dyn ::cognitive::services::extraction::ConflictResolver>> =
            ctx.deps.cognitive_provider.clone().map(|cp| {
                let params = providers::cognitive_chat_params(&config, 1024);
                Arc::new(::agent::cognitive_handlers::LlmConflictResolver::new(
                    cp, params,
                )) as Arc<dyn ::cognitive::services::extraction::ConflictResolver>
            });
        let mut ingestion_inner = ::cognitive::consumers::IngestionConsumer::new(
            observation_repo,
            entity_repo,
            episodic_repo,
            extraction_handler,
        )
        .with_episodic_threshold(config.cognitive.episodic_importance_threshold)
        .with_fact_repo(::cognitive::SemanticFactRepo::new(
            ctx.deps.storage_pool.inner().clone(),
        ));
        let cognitive_fact_embedder = ctx
            .host
            .get::<crate::plugins::cognitive::CognitiveInitResult>()
            .and_then(|r| r.cognitive_fact_embedder.clone());
        if let Some(ref emb) = cognitive_fact_embedder {
            ingestion_inner = ingestion_inner.with_embedder(Arc::clone(emb));
        }
        if let Some(resolver) = audd_resolver {
            ingestion_inner = ingestion_inner.with_conflict_resolver(resolver);
        }
        let ingestion: Arc<dyn ai_core::SignalConsumer> = Arc::new(ingestion_inner);
        ctx.add_signal_consumer(ingestion);

        // 5 cognitive collectors
        let (cognitive_tx, cognitive_rx): (
            ::cognitive::pipeline::SignalSender,
            ::cognitive::pipeline::SignalReceiver,
        ) = ::cognitive::pipeline::signal_queue(128);
        ctx.add_signal_consumer(Arc::new(::cognitive::pipeline::ChatTurnCollector::new(
            cognitive_tx.clone(),
        )));
        ctx.add_signal_consumer(Arc::new(::cognitive::pipeline::RecallCollector::new(
            cognitive_tx.clone(),
        )));
        ctx.add_signal_consumer(Arc::new(::cognitive::pipeline::SessionCollector::new(
            cognitive_tx.clone(),
            ctx.deps.repos.session_memory.clone(),
        )));
        ctx.add_signal_consumer(Arc::new(::cognitive::pipeline::AtomCollector::new(
            cognitive_tx.clone(),
        )));
        ctx.add_signal_consumer(Arc::new(::cognitive::pipeline::CoachingCollector::new(
            cognitive_tx.clone(),
        )));

        // Coaching signal consumer
        let (coaching_signal_tx, coaching_signal_rx) = tokio::sync::mpsc::channel(256);
        ctx.add_signal_consumer(Arc::new(feature_coaching::CoachingSignalConsumer::new(
            coaching_signal_tx,
        )));

        // Metric harvest consumer
        let metric_repo = ::cognitive::MetricRepo::new(ctx.deps.storage_pool.inner().clone());
        ctx.add_signal_consumer(Arc::new(
            ::cognitive::consumers::MetricHarvestConsumer::new(metric_repo),
        ));

        // Activity-log normalizer consumer
        let activity_svc = ctx.require_activity_svc()?;
        ctx.add_signal_consumer(Arc::new(activity_log::NormalizerSignalConsumer::new(
            Arc::clone(&activity_svc),
        )));

        // Retrieval indexer
        let signal_index_repo =
            ::cognitive::AiSignalIndexRepo::new(ctx.deps.storage_pool.inner().clone());
        ctx.add_signal_consumer(Arc::new(::cognitive::consumers::RetrievalIndexer::new(
            signal_index_repo,
        )));

        // Build consumer list: plugin-registered + mirror consumers
        let mut consumers = ctx.signal_consumers.clone();
        consumers.extend(mirror_consumers.iter().cloned());

        // Add system event translator (must be last so feature translators get priority).
        ctx.add_event_translator(crate::init::ai_pipeline::translate_system_event);

        // Build composite translate from all plugin-registered translators.
        let translators = ctx.event_translators.clone();
        let translate = move |event: &bus::DomainEvent| -> Option<ai_core::AiSignal> {
            PluginContext::run_translators(&translators, event)
        };

        let domain_event_bus = Arc::clone(
            ctx.deps
                .domain_event_bus
                .as_ref()
                .expect("domain event bus available"),
        );
        let base_count = ctx.signal_consumers.len();
        let router = ai_core::SignalRouter::start(domain_event_bus, consumers, translate);
        tracing::info!(
            "AI pipeline SignalRouter started with {} consumers ({} base + {} mirror)",
            base_count + mirror_consumers.len(),
            base_count,
            mirror_consumers.len()
        );

        // Launch the cognitive consolidator task (reads cognitive_rx)
        let pipeline_broadcast_tx = ctx
            .deps
            .pipeline_broadcast
            .as_ref()
            .expect("pipeline broadcast initialized above")
            .clone();
        let _cognitive_handle = {
            let repo = ::cognitive::SemanticFactRepo::new(ctx.deps.storage_pool.inner().clone());
            let rule_repo =
                ::cognitive::ProceduralRuleRepo::new(ctx.deps.storage_pool.inner().clone());
            let episodic_repo =
                ::cognitive::EpisodicMemoryRepo::new(ctx.deps.storage_pool.inner().clone());
            let pipeline_tx = pipeline_broadcast_tx.clone();
            tokio::spawn(async move {
                let mut rx: ::cognitive::pipeline::SignalReceiver = cognitive_rx;
                let episodic = Some(episodic_repo);
                while let Some(signal) = rx.recv().await {
                    let clusters = ::cognitive::pipeline::group_signals(vec![signal]);
                    let ops = ::cognitive::pipeline::heuristic_promote(&clusters);
                    if !ops.is_empty() {
                        ::cognitive::pipeline::execute_promotions(
                            &ops,
                            &repo,
                            &rule_repo,
                            &episodic,
                            None,
                            Some(&pipeline_tx),
                        )
                        .await;
                    }
                }
            })
        };

        // CoachingService now reads AiSignals instead of DomainEvents
        let intervention_tx = ctx
            .host
            .get::<crate::plugins::coaching::CoachingInitResult>()
            .map(|b| b.intervention_tx.clone());
        let _coaching_service =
            if let (Some(acc), Some(det), Some(router), Some(fb), Some(log_repo)) = (
                ctx.host
                    .get::<tokio::sync::Mutex<feature_coaching::SignalAccumulator>>(),
                ctx.host
                    .get::<tokio::sync::Mutex<feature_coaching::PatternDetector>>(),
                ctx.host
                    .get::<tokio::sync::Mutex<feature_coaching::InterventionRouter>>(),
                ctx.host
                    .get::<tokio::sync::Mutex<feature_coaching::FeedbackTracker>>(),
                ctx.host.get::<::storage::CoachingInterventionLogRepo>(),
            ) {
                let log_repo = (*log_repo).clone();
                let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
                    if let Some(ref cp) = ctx.deps.cognitive_provider {
                        let params = providers::cognitive_chat_params(&config, 1024);
                        Arc::new(
                            ::agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
                                cp.clone(),
                                params,
                            ),
                        )
                    } else {
                        Arc::new(::agent::cognitive_handlers::HeuristicCoachingReasonerHandler)
                    };

                let coaching_cancel = ctx.deps.shutdown_token.child_token();
                if let Some(tx) = intervention_tx {
                    let service = feature_coaching::CoachingService::start(
                        coaching_signal_rx,
                        Arc::clone(&acc),
                        Arc::clone(&det),
                        Arc::clone(&router),
                        Arc::clone(&fb),
                        ctx.deps
                            .user_situation
                            .clone()
                            .expect("user situation available"),
                        coaching_reasoner,
                        tx,
                        Some(log_repo),
                        coaching_cancel,
                    );
                    tracing::info!(
                        "coaching service started (reading AiSignals via CoachingSignalConsumer)"
                    );
                    Some(service)
                } else {
                    None
                }
            } else {
                None
            };

        let router = Arc::new(router);
        ctx.insert_handle(Arc::new(AiPipelineInitResult {
            router: Arc::clone(&router),
            _cognitive_handle,
            _coaching_service,
        }));
        ctx.insert_handle(router);

        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        // Derive MCP defaults from live agent ToolRegistry policy (not AiFeature∪allowlist).
        let registry = app.agent.tool_registry();
        let registry_guard = registry.read().await;

        // `exposed_tools_auto_filled` is skip_serializing (always false on load);
        // cutover sets the flag when it fills an empty override.
        let (server_enabled, mut exposed_tools) = {
            let config = app.config.read().await;
            (
                config.mcp.server.enabled,
                config.mcp.server.exposed_tools.clone(),
            )
        };
        let mut auto_filled = false;

        let validation = apply_mcp_exposure_cutover(
            &*registry_guard,
            server_enabled,
            &mut exposed_tools,
            &mut auto_filled,
        );
        drop(registry_guard);

        {
            let mut config = app.config.write().await;
            if auto_filled {
                config.mcp.server.exposed_tools = exposed_tools;
                config.mcp.server.exposed_tools_auto_filled = true;
            }
        }

        match validation.runtime_state {
            mcp::server::exposure::RuntimeState::Ready => {
                tracing::info!(
                    tools = ?validation.effective_registry_tools,
                    builtins = ?validation
                        .effective_builtins
                        .iter()
                        .map(|b| b.as_str())
                        .collect::<Vec<_>>(),
                    "mcp exposure Ready (policy-derived from ToolRegistry)"
                );
            }
            mcp::server::exposure::RuntimeState::Disabled => {
                tracing::info!("mcp exposure Disabled (server.enabled = false)");
            }
            mcp::server::exposure::RuntimeState::Invalid => {
                // Keep the desktop process up; only the MCP subsystem is invalid.
                tracing::warn!(
                    rejected = ?validation.rejected,
                    "mcp exposure Invalid; leaving exposed_tools override untouched"
                );
            }
        }

        if app.mcp_exposure.set(validation).is_err() {
            tracing::warn!("mcp_exposure status already set; ignoring duplicate post_init");
        }
        Ok(())
    }
}

/// Validate MCP exposure and auto-fill empty overrides when Ready.
///
/// Always returns the full [`ExposureValidation`]. When the override list is
/// empty and the result is Ready, writes `effective_registry_tools` into
/// `exposed_tools` and sets `auto_filled`. Invalid overrides are left untouched.
pub fn apply_mcp_exposure_cutover(
    registry: &tools_core::ToolRegistry,
    server_enabled: bool,
    exposed_tools: &mut Vec<String>,
    auto_filled: &mut bool,
) -> mcp::server::exposure::ExposureValidation {
    use mcp::server::exposure::{validate_mcp_exposure, ExposureInput, RuntimeState};

    let override_empty = exposed_tools.is_empty();
    let validation = validate_mcp_exposure(ExposureInput {
        registry,
        server_enabled,
        override_tools: if override_empty {
            None
        } else {
            Some(exposed_tools.as_slice())
        },
    });

    if override_empty && validation.runtime_state == RuntimeState::Ready {
        *exposed_tools = validation.effective_registry_tools.clone();
        *auto_filled = true;
    }

    validation
}

#[cfg(test)]
mod cutover_tests {
    use super::apply_mcp_exposure_cutover;
    use async_trait::async_trait;
    use mcp::server::exposure::{RejectionReason, RuntimeState};
    use serde_json::{json, Value};
    use tools_core::{ExposurePolicy, McpExposure, RoutingContext, Tool, ToolRegistry};

    struct NamedTool {
        name: &'static str,
        mcp: McpExposure,
    }

    #[async_trait]
    impl Tool for NamedTool {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "test"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            Ok("ok".into())
        }
        fn exposure_policy(&self) -> ExposurePolicy {
            ExposurePolicy {
                mcp: self.mcp,
                ..Default::default()
            }
        }
    }

    fn registry_with(tools: &[(&'static str, McpExposure)]) -> ToolRegistry {
        let mut reg = ToolRegistry::new();
        for (name, mcp) in tools {
            reg.register(NamedTool {
                name,
                mcp: *mcp,
            });
        }
        reg
    }

    #[test]
    fn empty_override_ready_fills_defaults_only() {
        let reg = registry_with(&[
            ("tasks", McpExposure::Default),
            ("shell", McpExposure::Forbidden),
        ]);
        let mut exposed = Vec::new();
        let mut auto = false;
        let v = apply_mcp_exposure_cutover(&reg, true, &mut exposed, &mut auto);
        assert_eq!(v.runtime_state, RuntimeState::Ready);
        assert!(auto);
        assert_eq!(exposed, vec!["tasks".to_string()]);
        assert!(!exposed.iter().any(|n| n == "shell" || n == "agent"));
    }

    #[test]
    fn empty_override_disabled_does_not_fill() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);
        let mut exposed = Vec::new();
        let mut auto = false;
        let v = apply_mcp_exposure_cutover(&reg, false, &mut exposed, &mut auto);
        assert_eq!(v.runtime_state, RuntimeState::Disabled);
        assert!(!auto);
        assert!(exposed.is_empty());
        assert_eq!(v.effective_registry_tools, vec!["tasks".to_string()]);
    }

    #[test]
    fn invalid_override_left_untouched_and_does_not_exit() {
        let reg = registry_with(&[
            ("tasks", McpExposure::Default),
            ("shell", McpExposure::Forbidden),
        ]);
        let original = vec!["tasks".into(), "shell".into(), "ghost".into()];
        let mut exposed = original.clone();
        let mut auto = false;
        let v = apply_mcp_exposure_cutover(&reg, true, &mut exposed, &mut auto);
        assert_eq!(v.runtime_state, RuntimeState::Invalid);
        assert!(!auto);
        assert_eq!(exposed, original);
        assert!(v
            .rejected
            .iter()
            .any(|r| r.name == "shell" && r.reason == RejectionReason::Forbidden));
        assert!(v
            .rejected
            .iter()
            .any(|r| r.name == "ghost" && r.reason == RejectionReason::Unknown));
    }

    #[test]
    fn non_empty_valid_override_preserved() {
        let reg = registry_with(&[("tasks", McpExposure::Default)]);
        let mut exposed = vec!["tasks".into()];
        let mut auto = false;
        let v = apply_mcp_exposure_cutover(&reg, true, &mut exposed, &mut auto);
        assert_eq!(v.runtime_state, RuntimeState::Ready);
        assert!(!auto);
        assert_eq!(exposed, vec!["tasks".to_string()]);
    }
}
