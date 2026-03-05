//! Handler trait for AI-powered productivity features.
//! Implemented in the agent crate to avoid circular dependencies.

use async_trait::async_trait;

#[async_trait]
pub trait ProductivityHandler: Send + Sync {
    /// Generate a natural language summary of the day's productivity data.
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String>;
}
