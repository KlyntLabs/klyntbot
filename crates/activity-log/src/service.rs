use std::sync::Arc;

use storage::StoragePool;
use tracing::{debug, warn};

use crate::normalizers::ActivityNormalizer;
use crate::privacy::PrivacyFilter;
use crate::repo::ActivityLogRepo;
use crate::types::ActivityLogEntry;

/// Ingestion service: validate → privacy filter → dedup → insert.
pub struct ActivityIngestionService {
    pool: StoragePool,
    privacy_filter: PrivacyFilter,
}

impl ActivityIngestionService {
    pub fn new(pool: StoragePool, privacy_filter: PrivacyFilter) -> Self {
        Self {
            pool,
            privacy_filter,
        }
    }

    /// Ingest a single entry. Returns the entry ID on success, or None if excluded/deduped.
    pub async fn ingest(&self, mut entry: ActivityLogEntry) -> common::Result<Option<String>> {
        // Truncate preview
        entry.truncate_preview();

        // Privacy exclusion
        if self.privacy_filter.should_exclude(&entry) {
            debug!("Activity excluded by privacy filter: {}", entry.id);
            return Ok(None);
        }

        // Flag sensitive
        entry = self.privacy_filter.flag_sensitive(entry);

        // Dedup check
        if let Some(ref hash) = entry.content_hash {
            if ActivityLogRepo::exists_by_hash(&self.pool, hash).await? {
                debug!("Activity deduped by content_hash: {}", entry.id);
                return Ok(None);
            }
        }

        // Insert
        let id = entry.id.clone();
        ActivityLogRepo::insert(&self.pool, &entry).await?;
        Ok(Some(id))
    }

    /// Ingest a batch. Returns count of successfully inserted entries.
    pub async fn ingest_batch(&self, entries: Vec<ActivityLogEntry>) -> common::Result<usize> {
        let mut to_insert = Vec::with_capacity(entries.len());

        for mut entry in entries {
            entry.truncate_preview();

            if self.privacy_filter.should_exclude(&entry) {
                continue;
            }

            entry = self.privacy_filter.flag_sensitive(entry);

            if let Some(ref hash) = entry.content_hash {
                if ActivityLogRepo::exists_by_hash(&self.pool, hash).await? {
                    continue;
                }
            }

            to_insert.push(entry);
        }

        if to_insert.is_empty() {
            return Ok(0);
        }

        ActivityLogRepo::insert_batch(&self.pool, &to_insert).await
    }

    /// Normalize an input using the given normalizer, then ingest.
    pub async fn normalize_and_ingest(
        &self,
        normalizer: &dyn ActivityNormalizer,
        input: &dyn std::any::Any,
    ) -> common::Result<Option<String>> {
        match normalizer.normalize(input) {
            Some(entry) => self.ingest(entry).await,
            None => Ok(None),
        }
    }

    /// Non-blocking ingest via tokio::spawn. Logs errors but doesn't propagate.
    pub fn ingest_fire_and_forget(self: &std::sync::Arc<Self>, entry: ActivityLogEntry) {
        let svc = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = svc.ingest(entry).await {
                warn!("Activity ingestion failed: {e}");
            }
        });
    }
}

/// Bounded ingestion channel — replaces unbounded fire-and-forget spawns.
/// A single consumer task processes entries sequentially, preventing task
/// accumulation when SQLite is slow.
pub struct BatchIngestionService {
    tx: tokio::sync::mpsc::Sender<ActivityLogEntry>,
}

impl BatchIngestionService {
    /// Create a new batch ingestion service with the given buffer size.
    /// Spawns a background consumer task.
    pub fn new(service: Arc<ActivityIngestionService>, buffer: usize) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ActivityLogEntry>(buffer);
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                if let Err(e) = service.ingest(entry).await {
                    warn!("Activity batch ingestion failed: {e}");
                }
            }
        });
        Self { tx }
    }

    /// Non-blocking send. Drops the entry if the buffer is full (backpressure).
    pub fn ingest_nonblocking(&self, entry: ActivityLogEntry) {
        if self.tx.try_send(entry).is_err() {
            warn!("Activity ingestion buffer full — dropping entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::{content_hash, new_ulid, ChatMessageInput, ChatMessageNormalizer};
    use crate::types::{ActivityActor, ActivitySource};
    use chrono::Utc;

    async fn setup() -> (StoragePool, ActivityIngestionService) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Run migration
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::ActivityLog::migrations_static(),
        )
        .await
        .unwrap();
        let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());
        (pool, svc)
    }

    fn make_entry(action: &str) -> ActivityLogEntry {
        ActivityLogEntry {
            id: new_ulid(),
            timestamp: Utc::now(),
            source: ActivitySource::Chat,
            actor: ActivityActor::User,
            resource_type: None,
            resource_id: None,
            resource_name: None,
            action: action.into(),
            content_preview: Some("test content".into()),
            content_hash: Some(content_hash("test content")),
            metadata: None,
            app_name: None,
            project_id: None,
            work_context_id: None,
            embedding_id: None,
            duration_secs: None,
            session_key: None,
            is_sensitive: false,
        }
    }

    #[tokio::test]
    async fn test_ingest_single() {
        let (_pool, svc) = setup().await;
        let entry = make_entry("prompt");
        let id = svc.ingest(entry).await.unwrap();
        assert!(id.is_some());
    }

    #[tokio::test]
    async fn test_ingest_dedup() {
        let (_pool, svc) = setup().await;
        let e1 = make_entry("prompt");
        let e2 = make_entry("prompt"); // Same content_hash
        let id1 = svc.ingest(e1).await.unwrap();
        let id2 = svc.ingest(e2).await.unwrap();
        assert!(id1.is_some());
        assert!(id2.is_none()); // Deduped
    }

    #[tokio::test]
    async fn test_ingest_privacy_excluded() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::ActivityLog::migrations_static(),
        )
        .await
        .unwrap();
        let filter = PrivacyFilter::new(vec!["1Password".into()], vec![], vec![], vec![]);
        let svc = ActivityIngestionService::new(pool, filter);

        let mut entry = make_entry("view");
        entry.app_name = Some("1Password 7".into());
        let id = svc.ingest(entry).await.unwrap();
        assert!(id.is_none());
    }

    #[tokio::test]
    async fn test_ingest_batch() {
        let (_pool, svc) = setup().await;
        let entries = vec![
            {
                let mut e = make_entry("a");
                e.content_hash = Some(content_hash("batch-1"));
                e
            },
            {
                let mut e = make_entry("b");
                e.content_hash = Some(content_hash("batch-2"));
                e
            },
        ];
        let count = svc.ingest_batch(entries).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_normalize_and_ingest() {
        let (_pool, svc) = setup().await;
        let normalizer = ChatMessageNormalizer;
        let input = ChatMessageInput {
            session_key: "sk-test".into(),
            role: "user".into(),
            content: "Hello!".into(),
        };
        let id = svc.normalize_and_ingest(&normalizer, &input).await.unwrap();
        assert!(id.is_some());
    }

    #[tokio::test]
    async fn test_sensitive_flagging_on_ingest() {
        let (_pool, svc) = setup().await;
        let mut entry = make_entry("view");
        entry.app_name = Some("1Password".into());
        entry.content_hash = Some(content_hash("sensitive-test"));
        // Default filter flags 1Password as sensitive but doesn't exclude it
        let id = svc.ingest(entry).await.unwrap();
        assert!(id.is_some());
        // Verify it was flagged
        let results = ActivityLogRepo::query_range(
            &_pool,
            Utc::now() - chrono::Duration::hours(1),
            Utc::now() + chrono::Duration::hours(1),
            100,
            0,
        )
        .await
        .unwrap();
        assert!(results[0].is_sensitive);
    }

    #[tokio::test]
    async fn test_truncate_on_ingest() {
        let (_pool, svc) = setup().await;
        let mut entry = make_entry("prompt");
        entry.content_preview = Some("x".repeat(1000));
        entry.content_hash = Some(content_hash("truncate-test"));
        let id = svc.ingest(entry).await.unwrap();
        assert!(id.is_some());

        let results = ActivityLogRepo::query_range(
            &_pool,
            Utc::now() - chrono::Duration::hours(1),
            Utc::now() + chrono::Duration::hours(1),
            100,
            0,
        )
        .await
        .unwrap();
        assert!(results[0].content_preview.as_ref().unwrap().len() <= 500);
    }
}
