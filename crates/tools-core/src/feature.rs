//! Feature package abstraction for self-contained klyntbot features.

use crate::DynTool;
use async_trait::async_trait;
use common::Result;
/// A SQL migration owned by a feature.
#[derive(Debug, Clone)]
pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub description: String,
    pub sql: String,
}

/// Health status for a feature.
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Trait that all feature packages must implement.
///
/// Each feature crate exports a struct implementing this trait.
/// The agent discovers features and registers their tools automatically.
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    /// Unique feature name (e.g., "todo", "finance").
    fn name(&self) -> &str;

    /// The tool(s) this feature provides.
    fn tools(&self) -> Vec<DynTool>;

    /// SQL migrations owned by this feature, in order.
    fn migrations(&self) -> Vec<FeatureMigration>;

    /// Health check (default: healthy).
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
