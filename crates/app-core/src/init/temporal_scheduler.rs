//! Phase 2 unified TemporalScheduler initialization.
//!
//! Runs SIDE-BY-SIDE with the legacy `CronService` in `init/cron.rs`.
//! Phase 3 will migrate callback consumers to bus subscribers; at that point
//! `CronService` can be removed.

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use feature_tasks::recurrence_repo::{SqliteInstanceRepo, SqliteTemplateRepo};
use scheduling::temporal::cron_bridge::CronBridge;
use scheduling::temporal::fire_store::FireStore;
use scheduling::temporal::recurrence::RecurrenceEngine;
use scheduling::temporal::{SchedulerConfig, TemporalScheduler};
use storage::Repos;
use tracing::{info, warn};

/// Results from temporal scheduler initialization.
pub(super) struct TemporalSchedulerResult {
    pub scheduler: TemporalScheduler,
    pub scheduler_handle: tokio::task::JoinHandle<()>,
    pub wake_subscriber: tokio::task::JoinHandle<()>,
}

/// Spawn the unified `TemporalScheduler` with a `CronBridge` seeded from
/// existing `cron_jobs`. Subscribes to `SystemDidWake` for immediate catch-up
/// after macOS sleep/resume.
///
/// Runs alongside `CronService` for Phase 2 — both schedulers will process
/// cron_jobs independently. Phase 3 migrates callback consumers to bus
/// subscribers and removes `CronService`.
pub(super) async fn init_temporal_scheduler(
    repos: &Repos,
    domain_event_bus: Arc<DomainEventBus>,
    pool: storage::StoragePool,
) -> Result<TemporalSchedulerResult, String> {
    let fire_store = FireStore::new(repos.scheduled_fires.clone());
    let bridge = CronBridge::new(repos.cron.clone(), fire_store.clone());

    bridge
        .reconcile_all()
        .await
        .map_err(|e| format!("TemporalScheduler cron reconcile failed: {e}"))?;

    // Build the RecurrenceEngine backed by SQLite repos.
    let template_repo = Arc::new(SqliteTemplateRepo::new(pool.clone()));
    let instance_repo = Arc::new(SqliteInstanceRepo::new(pool));
    let recurrence_engine = Arc::new(RecurrenceEngine::new(
        Arc::new(fire_store.clone()),
        template_repo,
        instance_repo,
        3, // default_materialize_ahead
    ));

    let scheduler = TemporalScheduler::new(
        fire_store,
        Arc::clone(&domain_event_bus),
        SchedulerConfig::default(),
    )
    .with_cron_bridge(bridge)
    .with_recurrence_engine(recurrence_engine);

    // Subscribe to SystemDidWake → immediate scheduler.wake() for sub-second
    // catch-up after laptop resume (tokio sleeps pause during system sleep,
    // so this complements the 30-second sleep cap).
    let wake_scheduler = scheduler.clone();
    let mut wake_rx = domain_event_bus.subscribe();
    let wake_subscriber = tokio::spawn(async move {
        loop {
            match wake_rx.recv().await {
                Ok(DomainEvent::SystemDidWake { .. }) => {
                    wake_scheduler.wake();
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(lagged = n, "TemporalScheduler wake subscriber lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("TemporalScheduler wake subscriber exiting (bus closed)");
                    break;
                }
            }
        }
    });

    let scheduler_handle = scheduler.clone().start_background();
    info!("TemporalScheduler started (side-by-side with CronService)");

    Ok(TemporalSchedulerResult {
        scheduler,
        scheduler_handle,
        wake_subscriber,
    })
}
