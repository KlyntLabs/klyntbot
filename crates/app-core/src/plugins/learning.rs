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
        <feature_learning::LearningFeature as FeaturePackage>::migrations(
            &feature_learning::LearningFeature::default(),
        )
    }

    async fn init(&self, _ctx: &mut PluginContext) -> common::Result<()> {
        Ok(())
    }
}
