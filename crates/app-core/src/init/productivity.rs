use std::sync::Arc;

use bus::DomainEventBus;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::tracker::categorizer::Categorizer;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use storage::StoragePool;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Results from the productivity initialization phase.
pub(super) struct ProductivityResult {
    pub dashboard_poll_interval_secs: u64,
    pub productivity_repos: Option<ProductivityRepos>,
    pub focus_manager: Option<Arc<FocusManager>>,
    pub productivity_engine: Option<Arc<Mutex<ProductivityEngine>>>,
    pub aggregator: Option<Arc<DailyAggregator>>,
    pub nudge_service: Option<Arc<Mutex<NudgeService>>>,
    pub distraction_interceptor:
        Option<Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusEvent>>,
    pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub distraction_alert_rx:
        Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>>,
    pub dashboard_tick_rx:
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
}

/// Initialize productivity feature (optional — requires enabled config).
pub(super) async fn init_productivity(
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
        auto_focus_rx,
        nudge_rx,
        dashboard_tick_rx,
    ) = if config.productivity.enabled {
        let pool = storage_pool.inner().clone();
        // Run feature migrations before creating repos.
        if let Err(e) = StoragePool::run_feature_migrations(
            &pool,
            &feature_productivity::ProductivityFeature::migrations_static(),
        )
        .await
        {
            error!("productivity migration failed — feature disabled: {e}");
            (None, None, None, None, None, None, None, None, None, None)
        } else {
            let prod_repos = ProductivityRepos::new(pool);
            let prod_config = &config.productivity;
            let mgr = Arc::new(FocusManager::new(
                prod_repos.clone(),
                prod_config.focus.clone(),
            ));

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

            // Take auto-focus receiver — caller wires to transport.
            let auto_focus_rx = engine.take_auto_focus_rx();

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
                auto_focus_rx,
                Some(nudge_rx),
                dashboard_tick_rx,
            )
        }
    } else {
        (None, None, None, None, None, None, None, None, None, None)
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
        auto_focus_rx,
        nudge_rx,
        dashboard_tick_rx,
    }
}
