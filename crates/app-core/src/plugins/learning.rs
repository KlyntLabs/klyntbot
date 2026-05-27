use ai_core::AiEventMeta;
use async_trait::async_trait;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin wrapper for the `feature-learning` crate.
pub struct LearningPlugin;

#[async_trait]
impl AppCorePlugin for LearningPlugin {
    fn name(&self) -> &str {
        "learning"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_learning::LearningFeature::default().migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(|reg| feature_learning::LearningFeature::register(reg));
        ctx.register_metrics(|reg| {
            reg.register_all(feature_learning::LearningEvent::FEATURE_METRICS)
        });
        ctx.add_feature_translator(
            feature_learning::try_from_domain_event,
            ai_core::RecallDomain::Learning,
        );
        Ok(())
    }
}
