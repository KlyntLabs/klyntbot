use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin that initializes the activity-log feature.
pub struct ActivityLogPlugin;

#[async_trait]
impl AppCorePlugin for ActivityLogPlugin {
    fn name(&self) -> &str {
        "activity_log"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        activity_log::activity_log_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let activity_svc = Arc::new(activity_log::ActivityIngestionService::new(
            ctx.deps.storage_pool.clone(),
            activity_log::PrivacyFilter::default(),
        ));
        ctx.insert_handle(activity_svc);
        tracing::info!("activity_log plugin initialized");
        Ok(())
    }
}
