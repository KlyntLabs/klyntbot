//! Typing indicators and step progress reporting.
//!
//! Provides a `ProgressReporter` that logs typing/progress events via tracing.
//! Real bus integration (sending typing indicators to channels) comes in Phase 9.

use common::{ChannelName, ChatId};

/// Reports typing indicators and step progress for a chat session.
pub struct ProgressReporter {
    channel: ChannelName,
    chat_id: ChatId,
    is_typing: bool,
}

impl ProgressReporter {
    pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
        Self {
            channel,
            chat_id,
            is_typing: false,
        }
    }

    /// Signal that the agent has started processing (typing indicator).
    pub async fn start_typing(&mut self) {
        self.is_typing = true;
        tracing::debug!(
            channel = %self.channel,
            chat_id = %self.chat_id,
            "typing indicator: started"
        );
    }

    /// Report progress on a specific step.
    pub async fn report_step(&self, step: &str) {
        tracing::debug!(
            channel = %self.channel,
            chat_id = %self.chat_id,
            step = step,
            "progress: step update"
        );
    }

    /// Signal that the agent has finished processing.
    pub async fn stop_typing(&mut self) {
        self.is_typing = false;
        tracing::debug!(
            channel = %self.channel,
            chat_id = %self.chat_id,
            "typing indicator: stopped"
        );
    }

    /// Whether the agent is currently in typing state.
    pub fn is_typing(&self) -> bool {
        self.is_typing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typing_lifecycle() {
        let mut reporter =
            ProgressReporter::new(ChannelName::new("telegram"), ChatId::new("123"));

        assert!(!reporter.is_typing());

        reporter.start_typing().await;
        assert!(reporter.is_typing());

        reporter.report_step("Analyzing code").await;
        assert!(reporter.is_typing());

        reporter.stop_typing().await;
        assert!(!reporter.is_typing());
    }

    #[tokio::test]
    async fn test_progress_reporter_creation() {
        let reporter =
            ProgressReporter::new(ChannelName::new("discord"), ChatId::new("456"));
        assert!(!reporter.is_typing());
    }
}
