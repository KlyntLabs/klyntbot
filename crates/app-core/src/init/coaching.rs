use std::sync::Arc;

use chrono::{Duration, Timelike, Utc};
use cognitive::situation::{compute_situation, SituationInputs, UserSituation};
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_productivity::repos::ProductivityRepos;
use storage::Repos;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Results from the coaching initialization phase.
pub(super) struct CoachingResult {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    pub pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    pub intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    pub feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    pub coaching_service: Option<feature_coaching::CoachingService>,
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
}

/// Initialize coaching pipeline (desktop mode only).
#[allow(clippy::too_many_arguments)]
pub(super) async fn init_coaching(
    mode: common::AppMode,
    config: &config::Config,
    storage_pool: &storage::StoragePool,
    repos: &Repos,
    productivity_repos: Option<&ProductivityRepos>,
    user_situation: &Arc<Mutex<UserSituation>>,
    domain_event_bus: &Arc<bus::DomainEventBus>,
    cognitive_provider: &Option<providers::DynProvider>,
    shutdown_token: &CancellationToken,
) -> CoachingResult {
    // Always create the intervention channel pair (EventChannels requires it).
    let (intervention_tx, intervention_rx) =
        mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);

    let (
        signal_accumulator,
        pattern_detector,
        intervention_router,
        feedback_tracker,
        coaching_service,
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

        // Start CoachingService — processes domain events through coaching pipeline.
        let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
            if let Some(ref cp) = cognitive_provider {
                let params = providers::cognitive_chat_params(config, 1024);
                Arc::new(agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
                    cp.clone(),
                    params,
                ))
            } else {
                Arc::new(agent::cognitive_handlers::HeuristicCoachingReasonerHandler)
            };

        let coaching_cancel = shutdown_token.child_token();
        let coaching_service = feature_coaching::CoachingService::start(
            domain_event_bus.subscribe(),
            signal_accumulator.clone(),
            pattern_detector.clone(),
            intervention_router.clone(),
            feedback_tracker.clone(),
            user_situation.clone(),
            coaching_reasoner,
            intervention_tx.clone(),
            Some(intervention_log_repo.clone()),
            coaching_cancel,
        );
        info!("coaching service started");

        (
            Some(signal_accumulator),
            Some(pattern_detector),
            Some(intervention_router),
            Some(feedback_tracker),
            Some(coaching_service),
            Some(intervention_log_repo),
        )
    } else {
        // Server mode: drop intervention_tx so intervention_rx.recv() returns None immediately.
        drop(intervention_tx);
        info!("coaching service skipped (server mode)");
        (None, None, None, None, None, None)
    };

    CoachingResult {
        intervention_rx,
        signal_accumulator,
        pattern_detector,
        intervention_router,
        feedback_tracker,
        coaching_service,
        coaching_intervention_log_repo,
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
    let now = Utc::now();
    let hour_of_day = now.hour();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc())
        .unwrap_or(now);
    let thirty_min_ago = now - Duration::minutes(30);

    let mut inputs = SituationInputs {
        hour_of_day,
        coaching_intensity: 0.7,
        ..Default::default()
    };

    // Query productivity data if available.
    if let Some(pr) = prod_repos {
        // Hours active today
        if let Ok(active_secs) = pr.events.total_active_secs(&today_start, &now).await {
            inputs.hours_active_today = active_secs as f64 / 3600.0;
        }

        // Context switches in last 30 min
        if let Ok(switches) = pr
            .events
            .count_context_switches(&thirty_min_ago, &now)
            .await
        {
            inputs.context_switches_last_30min = switches as i32;
        }

        // Historical average context switches (from last 7 days of daily summaries)
        let week_ago_date = (now - Duration::days(7)).format("%Y-%m-%d").to_string();
        let today_date = now.format("%Y-%m-%d").to_string();
        if let Ok(summaries) = pr.summaries.list_range(&week_ago_date, &today_date).await {
            if !summaries.is_empty() {
                let total_switches: i64 = summaries.iter().map(|s| s.context_switches).sum();
                inputs.historical_avg_context_switches =
                    total_switches as f64 / summaries.len() as f64;
            }
        }

        // Productive ratio today (productive_secs / total_active_secs)
        if let Ok((productive, neutral, distracting)) =
            pr.events.aggregate_by_type(&today_start, &now).await
        {
            let total_work = productive + neutral + distracting;
            if total_work > 0 {
                inputs.productive_ratio_today = productive as f64 / total_work as f64;
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
            let focus_mins = (now - session.started_at).num_minutes() as f64;
            inputs.mins_since_break = focus_mins;
        } else {
            // mins_since_break: time since last idle event
            if let Ok(idle_secs) = pr.events.total_idle_secs(&today_start, &now).await {
                if idle_secs > 0 {
                    // Check most recent events to find last idle gap
                    if let Ok(recent) = pr.events.list_recent(50).await {
                        let last_idle = recent.iter().find(|e| e.is_idle);
                        if let Some(idle_event) = last_idle {
                            let idle_end = idle_event.ended_at.unwrap_or(now);
                            inputs.mins_since_break = (now - idle_end).num_minutes() as f64;
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
    let tomorrow = now + Duration::hours(24);
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
pub(super) fn spawn_situation_recompute(
    prod_repos: Option<ProductivityRepos>,
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
                        prod_repos.as_ref(),
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
