//! Feature package abstraction for self-contained klyntbot features.

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
/// Each feature crate exports a struct implementing this trait. A feature owns
/// its migrations and health check; its tools are registered imperatively in the
/// matching `AppCorePlugin::init` (via `ctx.register_tool`), because most tools
/// need repos or agent-internal handles that aren't available on the bare
/// feature struct. (The former `tools()` method was vestigial — see ADR.)
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    /// Unique feature name (e.g., "todo", "finance").
    fn name(&self) -> &str;

    /// SQL migrations owned by this feature, in order.
    fn migrations(&self) -> Vec<FeatureMigration>;

    /// Health check (default: healthy).
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
