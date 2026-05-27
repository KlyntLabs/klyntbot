use ai_core::AiEventMeta;
use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of all productivity initialization results for FeatureHost storage.
pub struct ProductivityInitResult {
    pub dashboard_poll_interval_secs: u64,
    pub productivity_repos: Option<Arc<feature_productivity::repos::ProductivityRepos>>,
    pub focus_manager: Option<Arc<feature_productivity::FocusManager>>,
    pub productivity_engine:
        Option<Arc<tokio::sync::Mutex<feature_productivity::ProductivityEngine>>>,
    pub aggregator: Option<Arc<feature_productivity::DailyAggregator>>,
    pub nudge_service: Option<Arc<tokio::sync::Mutex<feature_productivity::NudgeService>>>,
    pub distraction_interceptor:
        Option<Arc<tokio::sync::Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
    pub distraction_alert_rx: std::sync::Mutex<
        Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>>,
    >,
    pub nudge_rx: std::sync::Mutex<
        Option<tokio::sync::mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    >,
    pub dashboard_tick_rx: std::sync::Mutex<
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    >,
}

/// Plugin that initializes the productivity feature.
pub struct ProductivityPlugin;

#[async_trait]
impl AppCorePlugin for ProductivityPlugin {
    fn name(&self) -> &str {
        "productivity"
    }

    fn dependencies(&self) -> &[&str] {
        // activity_log: init() calls ctx.require_activity_svc()
        &["activity_log"]
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_productivity::productivity_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(|reg| feature_productivity::ProductivityFeature::register(reg));
        ctx.register_metrics(|reg| {
            reg.register_all(feature_productivity::events::ProductivityEvent::FEATURE_METRICS)
        });
        ctx.add_feature_translator(
            feature_productivity::events::try_from_domain_event,
            ai_core::RecallDomain::Productivity,
        );

        let config = ctx.deps.config.read().await.clone();
        let result = self::init::init_productivity(
            &config,
            &ctx.deps.storage_pool,
            &ctx.require_domain_bus()?,
            &ctx.require_activity_svc()?,
            &ctx.deps.cognitive_provider,
            &ctx.deps.shutdown_token,
        )
        .await;

        let bundle = ProductivityInitResult {
            dashboard_poll_interval_secs: result.dashboard_poll_interval_secs,
            productivity_repos: result.productivity_repos.map(Arc::new),
            focus_manager: result.focus_manager.clone(),
            productivity_engine: result.productivity_engine.clone(),
            aggregator: result.aggregator.clone(),
            nudge_service: result.nudge_service.clone(),
            distraction_interceptor: result.distraction_interceptor.clone(),
            distraction_alert_rx: std::sync::Mutex::new(result.distraction_alert_rx),
            nudge_rx: std::sync::Mutex::new(result.nudge_rx),
            dashboard_tick_rx: std::sync::Mutex::new(result.dashboard_tick_rx),
        };

        ctx.insert_handle_opt(&bundle.productivity_repos);
        ctx.insert_handle_opt(&bundle.focus_manager);
        ctx.insert_handle_opt(&bundle.productivity_engine);
        ctx.insert_handle_opt(&bundle.aggregator);
        ctx.insert_handle_opt(&bundle.nudge_service);
        ctx.insert_handle_opt(&bundle.distraction_interceptor);

        // Register productivity context source
        if let Some(ref prod_repos) = bundle.productivity_repos {
            ctx.add_context_source(Box::new(
                ::agent::context_sources::ProductivityContextSource::new((**prod_repos).clone()),
            ));
        }

        ctx.insert_handle(Arc::new(bundle));
        tracing::info!("productivity plugin initialized");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        if let Ok(engine) = app.launcher_engine() {
            let config = app.config.read().await;
            if config.launcher.sources.calendar.enabled {
                if let Some(prod_repos) = app
                    .host
                    .get::<feature_productivity::repos::ProductivityRepos>()
                {
                    let fetcher = Arc::new(
                        crate::handlers::launcher::calendar_fetcher_impl::AppCalendarFetcher::new(
                            prod_repos,
                        ),
                    );
                    engine
                        .registry
                        .register(Arc::new(feature_launcher::CalendarSource::new(
                            fetcher,
                            config.launcher.sources.calendar.lookback_days,
                            config.launcher.sources.calendar.lookahead_days,
                        )));
                    tracing::info!("calendar source registered");
                }
            }
        }
        Ok(())
    }
}

mod init {
    use std::sync::Arc;

    use bus::DomainEventBus;
    use feature_productivity::repos::ProductivityRepos;
    use feature_productivity::tracker::categorizer::Categorizer;
    use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
    use storage::StoragePool;
    use tokio::sync::{mpsc, Mutex};
    use tokio_util::sync::CancellationToken;
    use tracing::{info, warn};

    /// Results from the productivity initialization phase.
    pub struct ProductivityResult {
        pub dashboard_poll_interval_secs: u64,
        pub productivity_repos: Option<ProductivityRepos>,
        pub focus_manager: Option<Arc<FocusManager>>,
        pub productivity_engine: Option<Arc<Mutex<ProductivityEngine>>>,
        pub aggregator: Option<Arc<DailyAggregator>>,
        pub nudge_service: Option<Arc<Mutex<NudgeService>>>,
        pub distraction_interceptor:
            Option<Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
        pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
        pub distraction_alert_rx: Option<
            tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>,
        >,
        pub dashboard_tick_rx:
            Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    }

    /// Initialize productivity feature (optional — requires enabled config).
    pub async fn init_productivity(
        config: &config::Config,
        storage_pool: &StoragePool,
        domain_event_bus: &Arc<DomainEventBus>,
        activity_svc: &Arc<activity_log::ActivityIngestionService>,
        cognitive_provider: &Option<providers::DynProvider>,
        shutdown_token: &CancellationToken,
    ) -> ProductivityResult {
        let dashboard_poll_interval_secs = config.productivity.tracking.poll_interval_secs;
        let (
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            distraction_alert_rx,
            nudge_rx,
            dashboard_tick_rx,
        ) = if config.productivity.enabled {
            let pool = storage_pool.inner().clone();
            // Migrations already ran in FeatureHost Phase 1 (declared via
            // ProductivityPlugin::migrations) — do not re-run them here.
            {
                let prod_repos = ProductivityRepos::new(pool);
                let prod_config = &config.productivity;
                let mgr = Arc::new(
                    FocusManager::new(prod_repos.clone(), prod_config.focus.clone())
                        .with_domain_bus(Arc::clone(domain_event_bus)),
                );

                let interceptor = Arc::new(Mutex::new(
                    feature_productivity::distraction::DistractionInterceptor::new(
                        prod_config.focus.clone(),
                        prod_repos.learned_rules.clone(),
                    ),
                ));

                // Daily aggregator for live summaries.
                let quality_scorer = feature_productivity::intelligence::QualityScorer::new(
                    prod_repos.intelligence_sessions.clone(),
                    prod_repos.quality_scores.clone(),
                    Arc::clone(domain_event_bus),
                );
                let agg = Arc::new(
                    DailyAggregator::new(prod_repos.clone()).with_quality_scorer(quality_scorer),
                );

                // Build and start the productivity engine (tracker + all subscribers).
                let categories = prod_repos.categories.list_all().await.unwrap_or_default();
                let categorizer = Categorizer::new(categories);
                let mut engine = ProductivityEngine::new_full(
                    prod_config.clone(),
                    prod_repos.clone(),
                    categorizer,
                    Some(Arc::clone(domain_event_bus)),
                    Some(Arc::clone(activity_svc)),
                );

                // Subscribe to dashboard ticks — caller wires to DashboardEmitter.
                let dashboard_tick_rx = Some(engine.subscribe());

                engine.start();

                // Start distraction monitor — watches for distracting apps during focus sessions.
                let distraction_alert_rx = {
                    let monitor_rx = engine.subscribe();
                    let monitor = feature_productivity::distraction::DistractionMonitor::new(
                        monitor_rx,
                        Arc::clone(&mgr),
                        Arc::clone(&interceptor),
                        prod_config.focus.clone(),
                        shutdown_token.child_token(),
                    );
                    Some(monitor.start())
                };

                // Wire ProductivityIntelligenceLayer — subscribes to tick broadcast
                // for classification, session aggregation, quality scoring, and interventions.
                {
                    let prod_handler: Option<Arc<dyn feature_productivity::ProductivityHandler>> =
                        cognitive_provider.as_ref().map(|cp| {
                            let model = config.agents.defaults.model.clone();
                            Arc::new(agent::ProductivityHandlerImpl::new(cp.clone(), model))
                                as Arc<dyn feature_productivity::ProductivityHandler>
                        });

                    match feature_productivity::intelligence::ProductivityIntelligenceLayer::new(
                        engine.tick_sender(),
                        Arc::clone(domain_event_bus),
                        prod_repos.clone(),
                        prod_handler,
                        shutdown_token.child_token(),
                    )
                    .await
                    {
                        Ok(layer) => {
                            layer.start();
                            info!("productivity intelligence layer started");
                        }
                        Err(e) => {
                            warn!("Failed to start intelligence layer: {e}");
                        }
                    }
                }

                let engine = Arc::new(Mutex::new(engine));

                // Nudge service — break reminders + burnout alerts.
                let (nudge_tx, nudge_rx) =
                    mpsc::channel::<feature_productivity::types::NudgeRecord>(32);
                let mut nudge_svc = NudgeService::new(
                    prod_repos.clone(),
                    config.productivity.nudges.clone(),
                    config.productivity.focus.clone(),
                    nudge_tx,
                );
                nudge_svc.start();
                let nudge_svc = Arc::new(Mutex::new(nudge_svc));

                (
                    Some(prod_repos),
                    Some(mgr),
                    Some(engine),
                    Some(agg),
                    Some(nudge_svc),
                    Some(interceptor),
                    distraction_alert_rx,
                    Some(nudge_rx),
                    dashboard_tick_rx,
                )
            }
        } else {
            (None, None, None, None, None, None, None, None, None)
        };

        ProductivityResult {
            dashboard_poll_interval_secs,
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            distraction_alert_rx,
            nudge_rx,
            dashboard_tick_rx,
        }
    }
}
