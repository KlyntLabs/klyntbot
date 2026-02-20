//! Runtime config persistence trait for feature packages.
//!
//! Allows features to read/write config sections without depending on
//! the config crate directly.

use async_trait::async_trait;
use common::Result;
use serde_json::Value;

/// Trait for persisting feature config sections at runtime.
///
/// Implementations live in the agent crate or CLI.
/// Features receive this as `Arc<dyn ConfigPersistence>`.
#[async_trait]
pub trait ConfigPersistence: Send + Sync {
    /// Load a config section by key. Returns the JSON value for the section.
    async fn load_section(&self, key: &str) -> Result<Value>;

    /// Save a config section by key.
    async fn save_section(&self, key: &str, value: Value) -> Result<()>;
}
