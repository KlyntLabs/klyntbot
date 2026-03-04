//! Interaction recorder — writes to `interaction_log` table after each message.
//!
//! Lightweight, no LLM call. Best-effort recording (errors are logged, not propagated).

use tracing::warn;

/// Records user interactions for behavioral pattern analysis.
#[derive(Clone)]
pub struct InteractionRecorder {
    repo: storage::InteractionLogRepo,
}

impl InteractionRecorder {
    pub fn new(repo: storage::InteractionLogRepo) -> Self {
        Self { repo }
    }

    /// Record an interaction (best-effort, never propagates errors).
    pub async fn record(
        &self,
        agent_name: &str,
        tools_used: &[&str],
        channel: &str,
        duration_ms: u64,
    ) {
        if let Err(e) = self
            .repo
            .create(agent_name, tools_used, channel, Some(duration_ms as i64))
            .await
        {
            warn!("Failed to record interaction: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_interaction() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);
        let recorder = InteractionRecorder::new(repos.interaction_log.clone());

        recorder
            .record("task", &["task", "memory"], "telegram", 150)
            .await;

        let count = repos.interaction_log.count().await.unwrap();
        assert_eq!(count, 1);

        let recent = repos.interaction_log.list_recent(1).await.unwrap();
        assert_eq!(recent[0].agent_name, "task");
        assert_eq!(recent[0].channel, "telegram");
        assert_eq!(recent[0].duration_ms, Some(150));
    }

    #[tokio::test]
    async fn test_record_multiple_interactions() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);
        let recorder = InteractionRecorder::new(repos.interaction_log.clone());

        recorder.record("task", &["task"], "telegram", 100).await;
        recorder
            .record("finance", &["finance"], "discord", 200)
            .await;
        recorder.record("general", &[], "telegram", 50).await;

        let count = repos.interaction_log.count().await.unwrap();
        assert_eq!(count, 3);
    }
}
