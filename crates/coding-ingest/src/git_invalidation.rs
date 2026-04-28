//! `GitInvalidationHandler` — trait the daemon dispatches `GitCommit` events through.

use crate::event::AgentEvent;
use async_trait::async_trait;

/// Trait the daemon dispatches `GitCommit` events through.
#[async_trait]
pub trait GitInvalidationHandler: Send + Sync + std::fmt::Debug {
    /// Handle a single git-commit event. Best-effort — errors logged.
    async fn handle(&self, event: &AgentEvent) -> common::Result<()>;
}
