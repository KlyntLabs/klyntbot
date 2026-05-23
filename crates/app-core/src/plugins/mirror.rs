use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of mirror initialization results for FeatureHost storage.
pub struct MirrorInitResult {
    pub facade: Arc<cognitive::mirror::MirrorFacade>,
    pub consumers: Vec<Arc<dyn ai_core::SignalConsumer>>,
    pub flush_handles: std::sync::Mutex<Option<Vec<tokio::task::JoinHandle<()>>>>,
    pub shutdown: tokio_util::sync::CancellationToken,
}

/// Plugin wrapper for the mirror self-reflection tool.
pub struct MirrorPlugin;

#[async_trait]
impl AppCorePlugin for MirrorPlugin {
    fn name(&self) -> &str {
        "mirror"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let mirror_model = {
            let config = ctx.deps.config.read().await;
            config
                .cognitive
                .model
                .as_deref()
                .unwrap_or(&config.agents.defaults.model)
                .to_string()
        };
        let mirror_repo =
            ::cognitive::mirror::MirrorRepo::new(ctx.deps.storage_pool.clone());
        let narrative_handler: Option<Arc<dyn ::cognitive::mirror::NarrativeHandler>> = ctx
            .deps
            .cognitive_provider
            .as_ref()
            .map(|cp| {
                Arc::new(::agent::mirror_handlers::LlmNarrativeHandler::new(
                    cp.clone(),
                    mirror_model.clone(),
                )) as Arc<dyn ::cognitive::mirror::NarrativeHandler>
            });
        let autotuner_bridge: Option<Arc<dyn ::cognitive::mirror::AutotunerBridge>> = ctx
            .deps
            .autotuner
            .as_ref()
            .map(|orch| {
                Arc::new(crate::adapters::autotuner_bridge::AppAutotunerBridge::new(
                    Arc::clone(orch),
                )) as Arc<dyn ::cognitive::mirror::AutotunerBridge>
            });
        let episodic_repo = Some(::cognitive::EpisodicMemoryRepo::new(
            ctx.deps.storage_pool.inner().clone(),
        ));
        let rule_repo = Some(::cognitive::ProceduralRuleRepo::new(
            ctx.deps.storage_pool.inner().clone(),
        ));
        let trial_evaluator: Option<Arc<dyn ::cognitive::mirror::EarlyTrialEvaluator>> = Some(
            Arc::new(crate::adapters::trial_evaluator::AppTrialEvaluator::new(
                ::storage::StrategyRepo::new(ctx.deps.storage_pool.inner().clone()),
            )),
        );
        let started = ::cognitive::mirror::MirrorEngine::start(
            mirror_repo.clone(),
            narrative_handler,
            autotuner_bridge,
            episodic_repo,
            rule_repo,
            trial_evaluator,
            None,
        );

        // Bootstrap brain version 1 on first run
        let bootstrap_archiver =
            ::cognitive::mirror::sources::ConfigArchiverSource::new(mirror_repo.clone(), None);
        tokio::spawn(async move {
            let _ = bootstrap_archiver.bootstrap(serde_json::json!({})).await;
        });

        // Spawn retention sweep
        let retention_cancel = ctx.deps.shutdown_token.child_token();
        let retention_handle = ::cognitive::mirror::MirrorRetentionService::spawn(
            Arc::new(mirror_repo),
            ::cognitive::mirror::MirrorRetentionConfig::default(),
            retention_cancel.clone(),
        );

        let facade = {
            let embedder = ctx
                .deps
                .embedding_engine
                .as_ref()
                .expect("embedding engine initialized above");
            let text_embedder: Arc<dyn ::cognitive::TextEmbedder> =
                Arc::new(::agent::TextEmbedderImpl::new(Arc::clone(embedder)));
            started.facade.with_text_embedder(text_embedder)
        };

        tracing::info!(
            consumer_count = started.consumers.len(),
            "mirror self-reflection engine started"
        );

        let mut all_handles = started.flush_handles;
        all_handles.push(retention_handle);

        let facade = Arc::new(facade);

        ctx.insert_handle(Arc::new(MirrorInitResult {
            facade: Arc::clone(&facade),
            consumers: started.consumers,
            flush_handles: std::sync::Mutex::new(Some(all_handles)),
            shutdown: started.shutdown,
        }));
        ctx.insert_handle(facade);

        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        if let Some(ref facade) = app.mirror_facade {
            let reg = app.agent.tool_registry();
            let mut registry = reg.write().await;
            registry.register(tools::MirrorTool::new(Arc::clone(facade)));
            tracing::info!("Mirror tool registered");

            // Wire the Mirror facade as the approval gate's suggester via bridge adapter.
            let suggester = Arc::new(
                crate::adapters::approval_suggester::MirrorApprovalSuggester::new(Arc::clone(facade)),
            );
            app.agent.set_approval_suggester(suggester);
        }
        Ok(())
    }
}
