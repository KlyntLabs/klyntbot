use async_trait::async_trait;
use ai_core::AiEventMeta;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of all productivity initialization results for FeatureHost storage.
pub struct ProductivityInitResult {
    pub dashboard_poll_interval_secs: u64,
    pub productivity_repos: Option<Arc<feature_productivity::repos::ProductivityRepos>>,
    pub focus_manager: Option<Arc<feature_productivity::FocusManager>>,
    pub productivity_engine: Option<Arc<tokio::sync::Mutex<feature_productivity::ProductivityEngine>>>,
    pub aggregator: Option<Arc<feature_productivity::DailyAggregator>>,
    pub nudge_service: Option<Arc<tokio::sync::Mutex<feature_productivity::NudgeService>>>,
    pub distraction_interceptor:
        Option<Arc<tokio::sync::Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
    pub distraction_alert_rx:
        std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>>>,
    pub nudge_rx: std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<feature_productivity::types::NudgeRecord>>>,
    pub dashboard_tick_rx:
        std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>>,
}

/// Plugin that initializes the productivity feature.
pub struct ProductivityPlugin;

#[async_trait]
impl AppCorePlugin for ProductivityPlugin {
    fn name(&self) -> &str {
        "productivity"
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

        let result = {
            let config = ctx.deps.config.read().await;
            crate::init::productivity::init_productivity(
                &config,
                &ctx.deps.storage_pool,
                ctx.deps
                    .domain_event_bus
                    .as_ref()
                    .ok_or_else(|| common::KlyntbotError::Storage("no domain event bus".into()))?,
                ctx.deps
                    .activity_svc
                    .as_ref()
                    .ok_or_else(|| common::KlyntbotError::Storage("no activity svc".into()))?,
                &ctx.deps.cognitive_provider,
                &ctx.deps.shutdown_token,
            )
            .await
        };

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

        if let Some(ref repos) = bundle.productivity_repos {
            ctx.insert_handle(Arc::clone(repos));
        }
        if let Some(ref mgr) = bundle.focus_manager {
            ctx.insert_handle(Arc::clone(mgr));
        }
        if let Some(ref engine) = bundle.productivity_engine {
            ctx.insert_handle(Arc::clone(engine));
        }
        if let Some(ref agg) = bundle.aggregator {
            ctx.insert_handle(Arc::clone(agg));
        }
        if let Some(ref svc) = bundle.nudge_service {
            ctx.insert_handle(Arc::clone(svc));
        }
        if let Some(ref interceptor) = bundle.distraction_interceptor {
            ctx.insert_handle(Arc::clone(interceptor));
        }
        ctx.insert_handle(Arc::new(bundle));
        tracing::info!("productivity plugin initialized");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        if let Some(ref engine) = app.launcher_engine {
            let config = app.config.read().await;
            if config.launcher.sources.calendar.enabled {
                if let Some(prod_repos) = app.host.get::<feature_productivity::repos::ProductivityRepos>() {
                    let fetcher = Arc::new(
                        crate::handlers::launcher::calendar_fetcher_impl::AppCalendarFetcher::new(
                            prod_repos,
                        ),
                    );
                    engine.registry.register(Arc::new(feature_launcher::CalendarSource::new(
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
