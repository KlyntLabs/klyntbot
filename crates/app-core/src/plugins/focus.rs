use async_trait::async_trait;
use std::sync::Arc;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

// ── Non-macOS stub ───────────────────────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
struct NoopFocusBridge;

#[cfg(not(target_os = "macos"))]
#[async_trait]
impl feature_focus::FocusBridge for NoopFocusBridge {
    async fn turn_on(&self, _mode: feature_focus::FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_on (DND only supported on macOS)");
        Ok(())
    }
    async fn turn_off(&self, _mode: feature_focus::FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_off (DND only supported on macOS)");
        Ok(())
    }
    async fn is_ready(&self) -> common::Result<bool> {
        Ok(false)
    }
}

// ── Plugin ───────────────────────────────────────────────────────────────────

/// Plugin wrapper for the `feature-focus` crate.
/// Initializes the DND manager and wires the focus-end subscriber.
pub struct FocusPlugin;

#[async_trait]
impl AppCorePlugin for FocusPlugin {
    fn name(&self) -> &str {
        "focus"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        <feature_focus::FocusFeature as FeaturePackage>::migrations(&feature_focus::FocusFeature)
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let fire_store = scheduling::temporal::fire_store::FireStore::new(
            ctx.deps.repos.scheduled_fires.clone(),
        );
        let alarm_bridge = Arc::new(feature_focus::alarm_bridge::TemporalAlarmBridge::new(
            fire_store,
        ));

        #[cfg(target_os = "macos")]
        let focus_bridge: Arc<dyn feature_focus::FocusBridge> =
            Arc::new(feature_focus::bridge::macos::MacosFocusBridge);

        #[cfg(not(target_os = "macos"))]
        let focus_bridge: Arc<dyn feature_focus::FocusBridge> = Arc::new(NoopFocusBridge);

        let repo = feature_focus::FocusSessionRepo::new(ctx.deps.storage_pool.inner().clone());
        let manager = Arc::new(feature_focus::DndManager::new(repo, alarm_bridge, focus_bridge));

        if let Some(ref bus) = ctx.deps.domain_event_bus {
            crate::focus::end_subscriber::spawn_focus_end_subscriber(
                Arc::clone(&manager),
                Arc::clone(bus),
                ctx.deps.shutdown_token.clone(),
            );
        }

        ctx.insert_handle(manager);
        tracing::info!("DndManager initialized");
        Ok(())
    }
}
