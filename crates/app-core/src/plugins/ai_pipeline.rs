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

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let config = ctx.deps.config.read().await;
        let mirror_consumers = ctx
            .host
            .get::<crate::plugins::mirror::MirrorInitResult>()
            .map(|r| r.consumers.clone())
            .unwrap_or_default();

        let observation_repo =
            ::cognitive::repos::AccumulatedObservationRepo::new(ctx.deps.storage_pool.inner().clone());
        let entity_repo = ::cognitive::repos::EntityRepo::new(ctx.deps.storage_pool.inner().clone());
        let episodic_repo =
            ::cognitive::EpisodicMemoryRepo::new(ctx.deps.storage_pool.inner().clone());
        let extraction_handler: Option<Arc<dyn ::cognitive::ExtractionHandler>> = ctx
            .deps
            .cognitive_provider
            .as_ref()
            .map(|cp| {
                let params = providers::cognitive_chat_params(&config, 4096);
                Arc::new(::agent::cognitive_handlers::LlmExtractionHandler::new(cp.clone(), params))
                    as Arc<dyn ::cognitive::ExtractionHandler>
            });
        let audd_resolver: Option<Arc<dyn ::cognitive::services::extraction::ConflictResolver>> =
            ctx.deps.cognitive_provider.clone().map(|cp| {
                let params = providers::cognitive_chat_params(&config, 1024);
                Arc::new(::agent::cognitive_handlers::LlmConflictResolver::new(cp, params))
                    as Arc<dyn ::cognitive::services::extraction::ConflictResolver>
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

        // 5 cognitive collectors
        let (cognitive_tx, cognitive_rx): (
            ::cognitive::pipeline::SignalSender,
            ::cognitive::pipeline::SignalReceiver,
        ) = ::cognitive::pipeline::signal_queue(128);
        let chat_turn: Arc<dyn ai_core::SignalConsumer> =
            Arc::new(::cognitive::pipeline::ChatTurnCollector::new(cognitive_tx.clone()));
        let recall: Arc<dyn ai_core::SignalConsumer> =
            Arc::new(::cognitive::pipeline::RecallCollector::new(cognitive_tx.clone()));
        let session: Arc<dyn ai_core::SignalConsumer> = Arc::new(
            ::cognitive::pipeline::SessionCollector::new(
                cognitive_tx.clone(),
                ctx.deps.repos.session_memory.clone(),
            ),
        );
        let atom: Arc<dyn ai_core::SignalConsumer> =
            Arc::new(::cognitive::pipeline::AtomCollector::new(cognitive_tx.clone()));
        let coaching_collector: Arc<dyn ai_core::SignalConsumer> =
            Arc::new(::cognitive::pipeline::CoachingCollector::new(cognitive_tx.clone()));

        // Coaching signal consumer
        let (coaching_signal_tx, coaching_signal_rx) = tokio::sync::mpsc::channel(256);
        let coaching_consumer: Arc<dyn ai_core::SignalConsumer> = Arc::new(
            feature_coaching::CoachingSignalConsumer::new(coaching_signal_tx),
        );

        // Metric harvest consumer
        let metric_repo = ::cognitive::MetricRepo::new(ctx.deps.storage_pool.inner().clone());
        let metric_harvest: Arc<dyn ai_core::SignalConsumer> = Arc::new(
            ::cognitive::consumers::MetricHarvestConsumer::new(metric_repo),
        );

        // Activity-log normalizer consumer
        let activity_normalizer: Arc<dyn ai_core::SignalConsumer> = Arc::new(
            activity_log::NormalizerSignalConsumer::new(Arc::clone(
                ctx.deps.activity_svc.as_ref().expect("activity svc available"),
            )),
        );

        // Retrieval indexer
        let signal_index_repo =
            ::cognitive::AiSignalIndexRepo::new(ctx.deps.storage_pool.inner().clone());
        let retrieval_indexer: Arc<dyn ai_core::SignalConsumer> = Arc::new(
            ::cognitive::consumers::RetrievalIndexer::new(signal_index_repo),
        );

        // Build consumer list: 10 base + mirror consumers
        let mut consumers: Vec<Arc<dyn ai_core::SignalConsumer>> = vec![
            ingestion,
            chat_turn,
            recall,
            session,
            atom,
            coaching_collector,
            coaching_consumer,
            metric_harvest,
            activity_normalizer,
            retrieval_indexer,
        ];
        consumers.extend(mirror_consumers.iter().cloned());

        // Add system event translator (must be last so feature translators get priority).
        ctx.add_event_translator(crate::init::ai_pipeline::translate_system_event);

        // Build composite translate from all plugin-registered translators.
        let translators = ctx.event_translators.clone();
        let translate = move |event: &bus::DomainEvent| -> Option<ai_core::AiSignal> {
            PluginContext::run_translators(&translators, event)
        };

        let domain_event_bus =
            Arc::clone(ctx.deps.domain_event_bus.as_ref().expect("domain event bus available"));
        let router = ai_core::SignalRouter::start(domain_event_bus, consumers, translate);
        tracing::info!(
            "AI pipeline SignalRouter started with {} consumers (10 base + {} mirror)",
            10 + mirror_consumers.len(),
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
                            &ops, &repo, &rule_repo, &episodic, None, Some(&pipeline_tx),
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
        let _coaching_service = if let (
            Some(acc),
            Some(det),
            Some(router),
            Some(fb),
            Some(log_repo),
        ) = (
            ctx.host.get::<tokio::sync::Mutex<feature_coaching::SignalAccumulator>>(),
            ctx.host.get::<tokio::sync::Mutex<feature_coaching::PatternDetector>>(),
            ctx.host.get::<tokio::sync::Mutex<feature_coaching::InterventionRouter>>(),
            ctx.host.get::<tokio::sync::Mutex<feature_coaching::FeedbackTracker>>(),
            ctx.host.get::<::storage::CoachingInterventionLogRepo>(),
        ) {
            let log_repo = (*log_repo).clone();
            let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
                if let Some(ref cp) = ctx.deps.cognitive_provider {
                    let params = providers::cognitive_chat_params(&config, 1024);
                    Arc::new(::agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
                        cp.clone(), params,
                    ))
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
                tracing::info!("coaching service started (reading AiSignals via CoachingSignalConsumer)");
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
        // Post-load fill: if the user hasn't overridden exposed_tools, derive it
        // from the registry so new features are auto-exposed without config edits.
        let mut config = app.config.write().await;
        if config.mcp.server.exposed_tools.is_empty() {
            let mut tools: Vec<String> = app
                .feature_registry
                .tool_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            tools.extend(
                config::schema::EXPLICIT_TOOL_ALLOWLIST
                    .iter()
                    .map(|s| s.to_string()),
            );
            config.mcp.server.exposed_tools = tools;
            config.mcp.server.exposed_tools_auto_filled = true;
            tracing::info!(
                tools = ?config.mcp.server.exposed_tools,
                "mcp exposed_tools auto-filled from AiFeatureRegistry"
            );
        }
        Ok(())
    }
}
