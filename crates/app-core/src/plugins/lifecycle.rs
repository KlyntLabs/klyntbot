use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;
use tracing::info;

/// Result of lifecycle plugin initialization.
pub struct LifecycleInitResult {
    pub config_watcher_token: Option<tokio_util::sync::CancellationToken>,
    pub lifecycle_monitor: std::sync::Mutex<Option<platform_macos::lifecycle::LifecycleMonitor>>,
    pub wake_orchestrator_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Plugin that starts the config file watcher, macOS lifecycle monitor,
/// and wake orchestrator.
pub struct LifecyclePlugin;

#[async_trait]
impl AppCorePlugin for LifecyclePlugin {
    fn name(&self) -> &str {
        "lifecycle"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let config_watcher_token = crate::infrastructure::config_watcher::start_config_watcher(
            Arc::clone(&ctx.deps.config),
            Arc::clone(&ctx.deps.hot_config),
            ctx.deps.shutdown_token.clone(),
        );

        // Read lifecycle config once for both wake orchestrator and monitor.
        let lifecycle_config = {
            let cfg = ctx.deps.config.read().await;
            cfg.lifecycle.clone()
        };

        // Start wake orchestrator.
        let wake_orchestrator_handle = if let Some(ref bus) = ctx.deps.domain_event_bus {
            let orchestrator =
                crate::wake_orchestrator::WakeOrchestrator::new(bus.clone(), lifecycle_config.wake_delivery.clone());
            Some(orchestrator.start())
        } else {
            None
        };

        // Start lifecycle monitor (macOS only).
        #[cfg(target_os = "macos")]
        let lifecycle_monitor = {
            if let Some(ref bus) = ctx.deps.domain_event_bus {
                let bus_clone = bus.clone();

                let monitor_config = platform_macos::lifecycle::MonitorConfig {
                    idle_threshold_secs: lifecycle_config.idle_threshold_secs,
                    presence_threshold_secs: lifecycle_config.presence_threshold_secs,
                    wake_grace_period_secs: lifecycle_config.wake_grace_period_secs,
                    active_poll_interval_secs: lifecycle_config.active_poll_interval_secs,
                    idle_poll_interval_secs: lifecycle_config.idle_poll_interval_secs,
                };

                let monitor = platform_macos::lifecycle::LifecycleMonitor::start(
                    monitor_config,
                    move |event| {
                        use platform_macos::lifecycle::{LifecycleEvent as LE, WakeType as LWT};
                        let bus_wt = |wt: LWT| match wt {
                            LWT::FromSleep => bus::domain_events::WakeType::FromSleep,
                            LWT::FromIdle => bus::domain_events::WakeType::FromIdle,
                        };
                        match event {
                            LE::SystemWillSleep => {
                                bus_clone.publish(bus::DomainEvent::SystemWillSleep);
                            }
                            LE::SystemDidWake {
                                away_duration,
                                wake_type,
                            } => {
                                let away_secs = away_duration.as_secs();
                                bus_clone.publish(bus::DomainEvent::SystemDidWake {
                                    away_secs,
                                    wake_type: bus_wt(wake_type),
                                });
                            }
                            LE::UserBecameIdle { idle_secs } => {
                                bus_clone.publish(bus::DomainEvent::UserBecameIdle { idle_secs });
                            }
                            LE::UserReturned {
                                absence_duration,
                                wake_type,
                            } => {
                                bus_clone.publish(bus::DomainEvent::UserReturned {
                                    absence_secs: absence_duration.as_secs(),
                                    wake_type: bus_wt(wake_type),
                                });
                            }
                        }
                    },
                );
                info!("lifecycle monitor started");
                Some(monitor)
            } else {
                None
            }
        };
        #[cfg(not(target_os = "macos"))]
        let lifecycle_monitor = None;

        ctx.insert_handle(Arc::new(LifecycleInitResult {
            config_watcher_token: Some(config_watcher_token),
            lifecycle_monitor: std::sync::Mutex::new(lifecycle_monitor),
            wake_orchestrator_handle: std::sync::Mutex::new(wake_orchestrator_handle),
        }));

        Ok(())
    }

    async fn post_init(&self, _app: &AppCore) -> common::Result<()> {
        Ok(())
    }
}
