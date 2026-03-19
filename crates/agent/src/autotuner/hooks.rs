//! Hook trait for wiring autotuner shadow scoring into the agent runtime.

use async_trait::async_trait;
use common::TrialParams;

/// Hook into the agent runtime for autotuner shadow scoring.
///
/// Implementations collect signals during normal message processing so the
/// autotuner can evaluate trial candidates without affecting live traffic.
#[async_trait]
pub trait AutoTunerHook: Send + Sync {
    /// Called when a new user message arrives (before classification).
    async fn on_message_received(&self, message: &str, chat_id: &str);

    /// Called after the agent finishes processing a message.
    async fn on_message_completed(
        &self,
        chat_id: &str,
        user_corrected: bool,
        tokens_used: u32,
        response_time_ms: u64,
    );

    /// Return the current champion trial params, if any.
    fn current_champion_params(&self) -> Option<TrialParams>;
}
