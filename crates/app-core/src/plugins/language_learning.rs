use async_trait::async_trait;
use ai_core::AiEventMeta;

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

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(|reg| {
            feature_language_learning::LanguageLearningFeature::register(reg)
        });
        ctx.register_metrics(|reg| {
            reg.register_all(feature_language_learning::LanguageLearningEvent::FEATURE_METRICS)
        });
        ctx.add_feature_translator(
            feature_language_learning::try_from_domain_event,
            ai_core::RecallDomain::LanguageLearning,
        );

        let config = ctx.deps.config.read().await;
        if config.language_learning.enabled {
            ctx.register_tool(
                feature_language_learning::practice_tool::LanguagePracticeTool::new(),
            );
            tracing::info!("Language practice tool registered");
        }
        Ok(())
    }

    async fn post_init(&self, _app: &AppCore) -> common::Result<()> {
        Ok(())
    }
}
