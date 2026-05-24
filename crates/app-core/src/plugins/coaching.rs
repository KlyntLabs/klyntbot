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

        let config = ctx.deps.config.read().await.clone();
        let result = self::init::init_coaching(
            ctx.deps.mode,
            &config,
            &ctx.deps.storage_pool,
            &ctx.deps.repos,
            prod_repos.as_ref().map(|arc| arc.as_ref()),
            &ctx.require_user_situation()?,
            &ctx.require_domain_bus()?,
            &ctx.deps.cognitive_provider,
            &ctx.deps.shutdown_token,
        )
        .await;

        let bundle = CoachingInitResult {
            intervention_rx: std::sync::Mutex::new(Some(result.intervention_rx)),
            signal_accumulator: result.signal_accumulator,
            pattern_detector: result.pattern_detector,
            intervention_router: result.intervention_router,
            feedback_tracker: result.feedback_tracker,
            coaching_intervention_log_repo: result.coaching_intervention_log_repo,
            intervention_tx: result.intervention_tx,
        };

        ctx.insert_handle_opt(&bundle.signal_accumulator);
        ctx.insert_handle_opt(&bundle.pattern_detector);
        ctx.insert_handle_opt(&bundle.intervention_router);
        ctx.insert_handle_opt(&bundle.feedback_tracker);
        if let Some(ref repo) = bundle.coaching_intervention_log_repo {
            ctx.insert_handle(Arc::new(repo.clone()));
        }
        ctx.insert_handle(Arc::new(bundle));

        tracing::info!("coaching plugin initialized");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        self::init::spawn_situation_recompute(
            app.productivity_repos().ok(),
            app.repos.clone(),
            app.intervention_router().ok(),
            app.user_situation.clone(),
            &app.shutdown_token,
        );
        Ok(())
    }
}

mod init {
use std::sync::Arc;

use cognitive::situation::{compute_situation, SituationInputs, UserSituation};
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_productivity::repos::ProductivityRepos;
use storage::Repos;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Results from the coaching initialization phase.
pub struct CoachingResult {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    pub pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    pub intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    pub feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
    pub intervention_tx: mpsc::Sender<feature_coaching::router::DeliveredIntervention>,
}

/// Initialize coaching pipeline components (desktop mode only). The
/// `CoachingService` itself is started after the `SignalRouter` exists, since
/// it consumes `AiSignal`s from `CoachingSignalConsumer`.
#[allow(clippy::too_many_arguments)]
pub async fn init_coaching(
    mode: common::AppMode,
    _config: &config::Config,
    storage_pool: &storage::StoragePool,
    repos: &Repos,
    productivity_repos: Option<&ProductivityRepos>,
    user_situation: &Arc<Mutex<UserSituation>>,
    _domain_event_bus: &Arc<bus::DomainEventBus>,
    _cognitive_provider: &Option<providers::DynProvider>,
    _shutdown_token: &CancellationToken,
) -> CoachingResult {
    // Always create the intervention channel pair (EventChannels requires it).
    let (intervention_tx, intervention_rx) =
        mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);

    let (
        signal_accumulator,
        pattern_detector,
        intervention_router,
        feedback_tracker,
        coaching_intervention_log_repo,
    ) = if mode == common::AppMode::Desktop {
        // Initialize coaching engine state.
        let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
        let intervention_router = Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
        let coaching_repo = storage::CoachingStrategyRepo::new(storage_pool.inner().clone());
        let intervention_log_repo =
            storage::CoachingInterventionLogRepo::new(storage_pool.inner().clone());
        let mut tracker = FeedbackTracker::new().with_repo(coaching_repo);
        tracker.load_from_db().await;
        let feedback_tracker = Arc::new(Mutex::new(tracker));

        // Compute real user situation now that productivity_repos is available.
        {
            let real_situation = build_situation_inputs(
                productivity_repos,
                repos,
                None, // router just created, no dismissals yet
            )
            .await;
            *user_situation.lock().await = real_situation;
        }

        info!("coaching components initialized (service starts after SignalRouter)");

        (
            Some(signal_accumulator),
            Some(pattern_detector),
            Some(intervention_router),
            Some(feedback_tracker),
            Some(intervention_log_repo),
        )
    } else {
        info!("coaching components skipped (server mode)");
        (None, None, None, None, None)
    };

    CoachingResult {
        intervention_rx,
        signal_accumulator,
        pattern_detector,
        intervention_router,
        feedback_tracker,
        coaching_intervention_log_repo,
        intervention_tx,
    }
}

/// Build a `UserSituation` from real productivity + task data.
///
/// Called at startup and periodically to keep the situation accurate.
pub(super) async fn build_situation_inputs(
    prod_repos: Option<&ProductivityRepos>,
    repos: &Repos,
    router: Option<&Arc<Mutex<InterventionRouter>>>,
) -> UserSituation {
    let now = jiff::Timestamp::now();
    let now_zoned = now.to_zoned(jiff::tz::TimeZone::UTC);
    let hour_of_day = now_zoned.hour() as u32;
    let today_start = now_zoned
        .date()
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map(|z| z.timestamp())
        .unwrap_or(now);
    let thirty_min_ago = now
        .checked_sub(jiff::SignedDuration::from_secs(30 * 60))
        .unwrap_or(now);

    let mut inputs = SituationInputs {
        hour_of_day,
        coaching_intensity: 0.7,
        ..Default::default()
    };

    // Query productivity data if available.
    if let Some(pr) = prod_repos {
        // Fire independent queries concurrently.
        let (active_secs, switches, aggregates) = tokio::join!(
            pr.events.total_active_secs(&today_start, &now),
            pr.events.count_context_switches(&thirty_min_ago, &now),
            pr.events.aggregate_by_type(&today_start, &now),
        );

        if let Ok(active_secs) = active_secs {
            inputs.hours_active_today = active_secs as f64 / 3600.0;
        }
        if let Ok(switches) = switches {
            inputs.context_switches_last_30min = switches as i32;
        }
        if let Ok((productive, neutral, distracting)) = aggregates {
            let total_work = productive + neutral + distracting;
            if total_work > 0 {
                inputs.productive_ratio_today = productive as f64 / total_work as f64;
            }
        }

        // Historical average context switches (from last 7 days of daily summaries)
        let week_ago_date = now
            .checked_sub(jiff::SignedDuration::from_secs(7 * 86400))
            .unwrap_or(now)
            .strftime("%Y-%m-%d")
            .to_string();
        let today_date = now.strftime("%Y-%m-%d").to_string();
        if let Ok(summaries) = pr.summaries.list_range(&week_ago_date, &today_date).await {
            if !summaries.is_empty() {
                let total_switches: i64 = summaries.iter().map(|s| s.context_switches).sum();
                inputs.historical_avg_context_switches =
                    total_switches as f64 / summaries.len() as f64;
            }
        }

        // Distraction count in last 30 min
        if let Ok(patterns) = pr
            .distraction_patterns
            .list_range(&today_date, &today_date)
            .await
        {
            let recent_count = patterns
                .iter()
                .filter(|p| p.created_at >= thirty_min_ago)
                .count();
            inputs.distraction_count_last_30min = recent_count as i32;
        }

        // Focus session status
        if let Ok(Some(session)) = pr.sessions.get_active().await {
            inputs.is_in_focus_session = true;
            inputs.focus_quality = session.quality_score;

            // mins_since_break = time since focus session started (no break during focus)
            let focus_secs = now.duration_since(session.started_at).as_secs_f64();
            inputs.mins_since_break = focus_secs / 60.0;
        } else {
            // mins_since_break: time since last idle event
            if let Ok(idle_secs) = pr.events.total_idle_secs(&today_start, &now).await {
                if idle_secs > 0 {
                    // Check most recent events to find last idle gap
                    if let Ok(recent) = pr.events.list_recent(50).await {
                        let last_idle = recent.iter().find(|e| e.is_idle);
                        if let Some(idle_event) = last_idle {
                            let idle_end = idle_event.ended_at.unwrap_or(now);
                            let secs = now.duration_since(idle_end).as_secs_f64();
                            inputs.mins_since_break = secs / 60.0;
                        } else {
                            // No idle events found, use hours active as proxy
                            inputs.mins_since_break = inputs.hours_active_today * 60.0;
                        }
                    }
                } else {
                    // No idle time at all today
                    inputs.mins_since_break = inputs.hours_active_today * 60.0;
                }
            }
        }
    }

    // Task pressure: overdue tasks
    if let Ok(overdue) = repos.tasks.overdue().await {
        inputs.overdue_task_count = overdue.len() as i32;
    }

    // Tasks due within 24h
    let tomorrow = now
        .checked_add(jiff::SignedDuration::from_secs(24 * 3600))
        .unwrap_or(now);
    let filter = storage::TaskFilter {
        due_after: Some(now),
        due_before: Some(tomorrow),
        ..Default::default()
    };
    if let Ok(upcoming) = repos.tasks.list(&filter).await {
        inputs.tasks_due_within_24h = upcoming.len() as i32;
    }

    // Dismissals from intervention router
    if let Some(router) = router {
        let r = router.lock().await;
        inputs.recent_dismissals = r.total_dismissals();
    }

    // Peak hour match — default to common productive hours (9-12, 14-17)
    // A future enhancement could read this from semantic facts.
    let is_peak = matches!(hour_of_day, 9..=12 | 14..=17);
    inputs.peak_hour_match = is_peak;

    compute_situation(&inputs)
}

/// Spawn a periodic task that recomputes UserSituation from real data every 2 minutes.
pub fn spawn_situation_recompute(
    prod_repos: Option<Arc<feature_productivity::repos::ProductivityRepos>>,
    repos: Repos,
    router: Option<Arc<Mutex<InterventionRouter>>>,
    situation: Option<Arc<Mutex<UserSituation>>>,
    shutdown: &CancellationToken,
) {
    let situation = match situation {
        Some(s) => s,
        None => return,
    };
    let token = shutdown.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the immediate first tick (initial situation already computed).
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let new_sit = build_situation_inputs(
                        prod_repos.as_ref().map(|arc| arc.as_ref()),
                        &repos,
                        router.as_ref(),
                    ).await;
                    debug!(
                        energy = format!("{:.2}", new_sit.energy_level),
                        focus = format!("{:.2}", new_sit.focus_state),
                        hours_active = format!("{:.1}", new_sit.hours_active_today),
                        "situation recomputed"
                    );
                    *situation.lock().await = new_sit;
                }
                _ = token.cancelled() => break,
            }
        }
    });
}
}
