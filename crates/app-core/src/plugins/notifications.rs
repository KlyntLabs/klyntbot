use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of notification initialization results for FeatureHost storage.
pub struct NotificationInitResult {
    pub dispatcher_handle: std::sync::Mutex<Option<notifications::NotificationDispatcherHandle>>,
}

/// Plugin wrapper for the notification dispatcher.
pub struct NotificationPlugin;

#[async_trait]
impl AppCorePlugin for NotificationPlugin {
    fn name(&self) -> &str {
        "notifications"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        vec![notifications::migration()]
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let (notif_cfg, quiet_hours) = {
            let config = ctx.deps.config.read().await;
            let notif_cfg = config.notifications.clone();
            let quiet_hours = if notif_cfg.quiet_hours.enabled {
                let tz = config.timezone.as_str();
                match notifications::quiet_hours::QuietHoursPolicy::new(
                    notif_cfg.quiet_hours.clone(),
                    tz,
                ) {
                    Ok(qh) => Some(qh),
                    Err(e) => {
                        tracing::warn!("quiet hours policy init failed ({e}), disabling");
                        None
                    }
                }
            } else {
                None
            };
            (notif_cfg, quiet_hours)
        };

        let last_active: std::sync::Arc<
            tokio::sync::RwLock<Option<(common::ChannelName, common::ChatId)>>,
        > = std::sync::Arc::new(tokio::sync::RwLock::new(None));

        let mut registry = notifications::channel::ChannelRegistry::new();

        // OS-native channel: delegate to provided sender or fallback.
        let os_sender: std::sync::Arc<dyn common::NotificationSender> =
            match &ctx.deps.notification_sender {
                Some(s) => std::sync::Arc::clone(s),
                None => std::sync::Arc::new(common::notify::OsNotificationSender),
            };
        registry.register(std::sync::Arc::new(
            notifications::channel::os_native::OsNativeChannel::new(os_sender),
        ));
        registry.register(std::sync::Arc::new(
            notifications::channel::tray::TrayChannel::new(Arc::clone(
                ctx.deps.domain_event_bus.as_ref().expect("domain event bus available"),
            )),
        ));
        // Outbound channels (telegram, discord, slack, email)
        for ch_name in ["telegram", "discord", "slack", "email"] {
            registry.register(std::sync::Arc::new(
                notifications::channel::outbound::OutboundChannel::new(
                    ch_name,
                    ctx.deps.bus.clone(),
                    Arc::clone(&last_active),
                ),
            ));
        }

        let sf_repo =
            ::storage::repos::ScheduledFiresRepo::new(ctx.deps.storage_pool.inner().clone());
        let fire_store = scheduling::FireStore::new(sf_repo);

        let dispatcher = notifications::NotificationDispatcher::new(
            Arc::clone(ctx.deps.domain_event_bus.as_ref().expect("domain event bus available")),
            registry,
            notif_cfg.default_channels.clone(),
            quiet_hours,
            ::storage::repos::NotificationLogRepo::new(ctx.deps.storage_pool.inner().clone()),
            ::storage::repos::HeldNotificationsRepo::new(ctx.deps.storage_pool.inner().clone()),
            notifications::held::HeldReleaseService::new(
                ::storage::repos::HeldNotificationsRepo::new(ctx.deps.storage_pool.inner().clone()),
                fire_store,
            ),
            notifications::retry::RetryPolicy::from_config(&notif_cfg.retry),
        );
        let handle = dispatcher.start();
        tracing::info!("NotificationDispatcher started (Phase 3)");
        ctx.insert_handle(Arc::new(NotificationInitResult {
            dispatcher_handle: std::sync::Mutex::new(Some(handle)),
        }));

        Ok(())
    }

    async fn post_init(&self, _app: &AppCore) -> common::Result<()> {
        Ok(())
    }
}
