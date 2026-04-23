//! CoachingFeature — pipeline-registration-only adapter.
//! The actual CoachingService lives in this crate's service.rs and is wired
//! through app-core::init::coaching. This struct exists so coaching can
//! participate in AiFeatureRegistry for skill discovery and metric harvesting.

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Coaching",
    skill = "automation",
    event = "crate::events::CoachingEvent"
)]
pub struct CoachingFeature;

impl CoachingFeature {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FeaturePackage for CoachingFeature {
    fn name(&self) -> &str {
        "coaching"
    }

    fn tools(&self) -> Vec<DynTool> {
        Vec::new()
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Vec::new()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
