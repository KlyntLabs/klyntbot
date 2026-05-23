use async_trait::async_trait;
use std::sync::Arc;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Plugin wrapper for the `feature-launcher` crate.
/// Initializes the launcher search engine and registers launcher tools post-init.
pub struct LauncherPlugin;

#[async_trait]
impl AppCorePlugin for LauncherPlugin {
    fn name(&self) -> &str {
        "launcher"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_launcher::launcher_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let result = {
            let config = ctx.deps.config.read().await;
            crate::init::launcher::init_launcher(
                &config,
                &ctx.deps.storage_pool,
                &ctx.deps.shutdown_token,
            )
            .await
        };

        if let Some(engine) = result.launcher_engine {
            ctx.insert_handle(engine);
        }

        // Seed brand-new installs only — established users wait for the nightly cron.
        let pool = ctx.deps.storage_pool.inner().clone();
        tokio::spawn(async move {
            let already_populated: bool = sqlx::query_scalar::<_, i64>(
                "SELECT EXISTS(SELECT 1 FROM entity_attention LIMIT 1)",
            )
            .fetch_one(&pool)
            .await
            .map(|n| n != 0)
            .unwrap_or(false);
            if already_populated {
                return;
            }
            let aggregator = feature_launcher::AttentionAggregator::new(pool);
            match aggregator.rebuild_from_activity(90).await {
                Ok(n) => tracing::info!(rows = n, "Initial attention rebuild complete"),
                Err(e) => {
                    tracing::warn!(error = %e, "Initial attention rebuild failed — will retry via cron")
                }
            }
        });

        tracing::info!("launcher plugin initialized");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        if let Some(ref engine) = app.launcher_engine {
            let reg = app.agent.tool_registry();
            let mut registry = reg.write().await;
            let launcher = feature_launcher::LauncherFeature::with_tool_deps(
                feature_launcher::LauncherToolDeps {
                    registry: Arc::clone(&engine.registry),
                    frequency: Arc::clone(&engine.frequency_repo),
                    pins: Arc::clone(&engine.pins_repo),
                },
            );
            for tool in launcher.tools() {
                registry.register_dyn(tool);
            }
            tracing::info!("Launcher tools registered in MCP registry");
        }
        Ok(())
    }
}
