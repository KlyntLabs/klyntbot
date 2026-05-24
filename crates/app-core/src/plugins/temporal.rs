use async_trait::async_trait;
use std::sync::Arc;

use bus::DomainEvent;
use feature_tasks::recurrence_repo::{SqliteInstanceRepo, SqliteTemplateRepo};
use scheduling::temporal::cron_bridge::CronBridge;
use scheduling::temporal::fire_store::FireStore;
use scheduling::temporal::recurrence::RecurrenceEngine;
use scheduling::temporal::{SchedulerConfig, TemporalScheduler};
use tracing::{info, warn};

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Default number of instances to materialise ahead per recurrence spawn cycle.
const DEFAULT_MATERIALIZE_AHEAD: u32 = 3;

/// Result of temporal plugin initialization.
pub struct TemporalInitResult {
    pub scheduler: TemporalScheduler,
    pub cron_bridge: CronBridge,
    pub scheduler_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub wake_subscriber: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Plugin that owns the scheduling migration, starts the TemporalScheduler,
/// and registers the temporal (fact changelog) tool.
pub struct TemporalPlugin;

#[async_trait]
impl AppCorePlugin for TemporalPlugin {
    fn name(&self) -> &str {
        "temporal"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        vec![tools_core::FeatureMigration {
            feature_name: "scheduling".to_string(),
            version: 1,
            description: "Create scheduled_fires table".to_string(),
            sql: include_str!("../../../scheduling/migrations/001_scheduled_fires.sql").to_string(),
        }]
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let repos = &ctx.deps.repos;
        let domain_event_bus = ctx.deps.domain_event_bus.as_ref().ok_or_else(|| {
            common::KlyntbotError::Storage("no domain event bus".into())
        })?;
        let pool = ctx.deps.storage_pool.clone();

        let fire_store = FireStore::new(repos.scheduled_fires.clone());
        let bridge = CronBridge::new(repos.cron.clone(), fire_store.clone());

        bridge
            .reconcile_all()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("TemporalScheduler cron reconcile failed: {e}")))?;

        // Clone before the bridge is moved into the scheduler.
        let bridge_for_appcore = bridge.clone();

        // Build the RecurrenceEngine backed by SQLite repos.
        let fire_store_arc = Arc::new(fire_store.clone());
        let template_repo = Arc::new(SqliteTemplateRepo::new(pool.clone()));
        let instance_repo = Arc::new(SqliteInstanceRepo::new(pool, Arc::clone(&fire_store_arc)));
        let recurrence_engine = Arc::new(RecurrenceEngine::new(
            fire_store_arc,
            template_repo,
            instance_repo,
            DEFAULT_MATERIALIZE_AHEAD,
        ));

        let scheduler = TemporalScheduler::new(
            fire_store,
            Arc::clone(domain_event_bus),
            SchedulerConfig::default(),
        )
        .with_cron_bridge(bridge)
        .with_recurrence_engine(recurrence_engine);

        // Subscribe to SystemDidWake → immediate scheduler.wake() for sub-second
        // catch-up after laptop resume.
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
        info!("TemporalScheduler started (side-by-side with CronExecutor)");

        // Register temporal tool
        let temporal_service = ::cognitive::TemporalService::new(
            ::cognitive::SemanticFactRepo::new(ctx.deps.storage_pool.inner().clone()),
        )
        .with_changelog(::cognitive::FactChangelogRepo::new(
            ctx.deps.storage_pool.inner().clone(),
        ));
        ctx.register_tool(tools::TemporalTool::new(temporal_service));
        tracing::info!("Temporal tool registered");

        ctx.insert_handle(Arc::new(TemporalInitResult {
            scheduler,
            cron_bridge: bridge_for_appcore,
            scheduler_handle: std::sync::Mutex::new(Some(scheduler_handle)),
            wake_subscriber: std::sync::Mutex::new(Some(wake_subscriber)),
        }));

        Ok(())
    }

}
