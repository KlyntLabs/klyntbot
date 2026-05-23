use async_trait::async_trait;
use ai_core::AiEventMeta;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of all coaching initialization results for FeatureHost storage.
pub struct CoachingInitResult {
    pub intervention_rx:
        std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<feature_coaching::router::DeliveredIntervention>>>,
    pub signal_accumulator: Option<Arc<tokio::sync::Mutex<feature_coaching::SignalAccumulator>>>,
    pub pattern_detector: Option<Arc<tokio::sync::Mutex<feature_coaching::PatternDetector>>>,
    pub intervention_router: Option<Arc<tokio::sync::Mutex<feature_coaching::InterventionRouter>>>,
    pub feedback_tracker: Option<Arc<tokio::sync::Mutex<feature_coaching::FeedbackTracker>>>,
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
    pub intervention_tx: tokio::sync::mpsc::Sender<feature_coaching::router::DeliveredIntervention>,
}

/// Plugin that initializes the coaching feature.
pub struct CoachingPlugin;

#[async_trait]
impl AppCorePlugin for CoachingPlugin {
    fn name(&self) -> &str {
        "coaching"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(|reg| feature_coaching::CoachingFeature::register(reg));
        ctx.register_metrics(|reg| {
            reg.register_all(feature_coaching::events::CoachingEvent::FEATURE_METRICS)
        });
        ctx.add_feature_translator(
            feature_coaching::events::try_from_domain_event,
            ai_core::RecallDomain::Coaching,
        );

        let prod_repos = ctx
            .host
            .get::<feature_productivity::repos::ProductivityRepos>();

        let result = {
            let config = ctx.deps.config.read().await;
            crate::init::coaching::init_coaching(
                ctx.deps.mode,
                &config,
                &ctx.deps.storage_pool,
                &ctx.deps.repos,
                prod_repos.as_ref().map(|arc| arc.as_ref()),
                ctx.deps
                    .user_situation
                    .as_ref()
                    .ok_or_else(|| common::KlyntbotError::Storage("no user situation".into()))?,
                ctx.deps
                    .domain_event_bus
                    .as_ref()
                    .ok_or_else(|| common::KlyntbotError::Storage("no domain event bus".into()))?,
                &ctx.deps.cognitive_provider,
                &ctx.deps.shutdown_token,
            )
            .await
        };

        let bundle = CoachingInitResult {
            intervention_rx: std::sync::Mutex::new(Some(result.intervention_rx)),
            signal_accumulator: result.signal_accumulator,
            pattern_detector: result.pattern_detector,
            intervention_router: result.intervention_router,
            feedback_tracker: result.feedback_tracker,
            coaching_intervention_log_repo: result.coaching_intervention_log_repo,
            intervention_tx: result.intervention_tx,
        };

        if let Some(ref acc) = bundle.signal_accumulator {
            ctx.insert_handle(Arc::clone(acc));
        }
        if let Some(ref det) = bundle.pattern_detector {
            ctx.insert_handle(Arc::clone(det));
        }
        if let Some(ref router) = bundle.intervention_router {
            ctx.insert_handle(Arc::clone(router));
        }
        if let Some(ref tracker) = bundle.feedback_tracker {
            ctx.insert_handle(Arc::clone(tracker));
        }
        if let Some(ref repo) = bundle.coaching_intervention_log_repo {
            ctx.insert_handle(Arc::new(repo.clone()));
        }
        ctx.insert_handle(Arc::new(bundle));

        tracing::info!("coaching plugin initialized");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        crate::init::coaching::spawn_situation_recompute(
            app.productivity_repos.clone(),
            app.repos.clone(),
            app.intervention_router.clone(),
            app.user_situation.clone(),
            &app.shutdown_token,
        );
        Ok(())
    }
}
