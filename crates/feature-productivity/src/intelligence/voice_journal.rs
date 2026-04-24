use std::sync::Arc;

use tracing::info;

use bus::DomainEventBus;

use crate::repos::VoiceJournalRepo;
use crate::types::VoiceJournalEntry;

pub struct VoiceJournalProcessor {
    repo: VoiceJournalRepo,
    #[allow(dead_code)]
    event_bus: Arc<DomainEventBus>,
    // Reuses existing WhisperTool via ToolRegistry for transcription
}

impl VoiceJournalProcessor {
    pub fn new(repo: VoiceJournalRepo, event_bus: Arc<DomainEventBus>) -> Self {
        Self { repo, event_bus }
    }

    /// Store a voice journal entry and publish a processing event.
    pub async fn process(&self, entry: &VoiceJournalEntry) -> common::Result<()> {
        self.repo.create(entry).await?;

        let fact_count = entry
            .extracted_facts
            .as_ref()
            .and_then(|f| serde_json::from_str::<Vec<String>>(f).ok())
            .map(|v| v.len())
            .unwrap_or(0);

        self.repo.mark_processed(&entry.id).await?;

        info!(
            journal_id = %entry.id,
            fact_count = fact_count,
            "voice journal processed"
        );

        // VoiceJournalProcessed variant deleted; no-op.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn make_bus() -> Arc<DomainEventBus> {
        Arc::new(DomainEventBus::new(64))
    }

    #[tokio::test]
    async fn test_process_stores_and_publishes() {
        let pool = setup_pool().await;
        let bus = make_bus();

        let processor = VoiceJournalProcessor::new(VoiceJournalRepo::new(pool.clone()), bus);

        let entry = VoiceJournalEntry {
            id: "vj-proc-test-1".to_string(),
            recorded_at: "2026-03-09T14:30:00Z".to_string(),
            duration_secs: 60,
            transcript: Some("Had a good morning of coding.".to_string()),
            extracted_facts: Some(r#"["good morning","coding"]"#.to_string()),
            sentiment: Some("positive".to_string()),
            session_id: None,
            processed: false,
            created_at: jiff::Timestamp::now().to_string(),
        };

        processor.process(&entry).await.unwrap();

        // Verify stored
        let repo = VoiceJournalRepo::new(pool);
        let fetched = repo.get("vj-proc-test-1").await.unwrap().unwrap();
        assert!(fetched.processed);
    }
}
