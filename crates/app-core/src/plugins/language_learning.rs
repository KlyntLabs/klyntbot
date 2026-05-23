use async_trait::async_trait;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Plugin wrapper for the `feature-language-learning` crate.
pub struct LanguageLearningPlugin;

#[async_trait]
impl AppCorePlugin for LanguageLearningPlugin {
    fn name(&self) -> &str {
        "language-learning"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_language_learning::language_learning_migrations()
    }

    async fn init(&self, _ctx: &mut PluginContext) -> common::Result<()> {
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        let config = app.config.read().await;
        if config.language_learning.enabled {
            let reg = app.agent.tool_registry();
            let mut registry = reg.write().await;
            registry.register(
                feature_language_learning::practice_tool::LanguagePracticeTool::new(),
            );
            tracing::info!("Language practice tool registered");
        }
        Ok(())
    }
}
