use sqlx::SqlitePool;

use crate::types::VoiceJournalEntry;

#[derive(Debug, Clone)]
pub struct VoiceJournalRepo {
    pool: SqlitePool,
}

impl VoiceJournalRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, entry: &VoiceJournalEntry) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_voice_journals (id, recorded_at, duration_secs, transcript, extracted_facts, sentiment, session_id, processed, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        )
        .bind(&entry.id)
        .bind(&entry.recorded_at)
        .bind(entry.duration_secs)
        .bind(&entry.transcript)
        .bind(&entry.extracted_facts)
        .bind(&entry.sentiment)
        .bind(&entry.session_id)
        .bind(entry.processed)
        .bind(&entry.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> common::Result<Option<VoiceJournalEntry>> {
        let row = sqlx::query_as::<_, VoiceJournalEntry>(
            "SELECT * FROM productivity_voice_journals WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn list_range(
        &self,
        start: &str,
        end: &str,
    ) -> common::Result<Vec<VoiceJournalEntry>> {
        let rows = sqlx::query_as::<_, VoiceJournalEntry>(
            "SELECT * FROM productivity_voice_journals WHERE recorded_at >= ?1 AND recorded_at < ?2 ORDER BY recorded_at DESC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn mark_processed(&self, id: &str) -> common::Result<()> {
        sqlx::query("UPDATE productivity_voice_journals SET processed = TRUE WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn sample_entry(id: &str) -> VoiceJournalEntry {
        VoiceJournalEntry {
            id: id.to_string(),
            recorded_at: "2026-03-09T14:30:00Z".to_string(),
            duration_secs: 120,
            transcript: Some(
                "Today was a productive day, focused on the intelligence layer.".to_string(),
            ),
            extracted_facts: Some(
                r#"["worked on intelligence layer","felt productive"]"#.to_string(),
            ),
            sentiment: Some("positive".to_string()),
            session_id: None,
            processed: false,
            created_at: "2026-03-09T14:32:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let pool = setup_pool().await;
        let repo = VoiceJournalRepo::new(pool);

        let entry = sample_entry("vj-1");
        repo.create(&entry).await.unwrap();

        let fetched = repo.get("vj-1").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.duration_secs, 120);
        assert!(!fetched.processed);
    }

    #[tokio::test]
    async fn test_mark_processed() {
        let pool = setup_pool().await;
        let repo = VoiceJournalRepo::new(pool);

        let entry = sample_entry("vj-proc-1");
        repo.create(&entry).await.unwrap();

        repo.mark_processed("vj-proc-1").await.unwrap();

        let fetched = repo.get("vj-proc-1").await.unwrap().unwrap();
        assert!(fetched.processed);
    }
}
