//! Repository for the extraction critic log (KCA Track 5).

use storage::StoragePool;

#[derive(Debug, Clone)]
pub struct ExtractionCriticLogEntry {
    pub id: String,
    pub fact_id: String,
    pub verdict: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ExtractionCriticLogRepo {
    pool: StoragePool,
}

impl ExtractionCriticLogRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, fact_id: &str, verdict: &str, reason: &str) -> common::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO extraction_critic_log (id, fact_id, verdict, reason) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(fact_id)
        .bind(verdict)
        .bind(reason)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("critic_log insert: {e}")))?;
        Ok(())
    }

    pub async fn list_unreviewed(
        &self,
        limit: usize,
    ) -> common::Result<Vec<ExtractionCriticLogEntry>> {
        let lim = limit as i64;
        let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
            "SELECT id, fact_id, verdict, reason, created_at FROM extraction_critic_log WHERE reviewed_by_reforge_at IS NULL ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(lim)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(id, fact_id, verdict, reason, created_at)| ExtractionCriticLogEntry {
                    id,
                    fact_id,
                    verdict,
                    reason,
                    created_at,
                },
            )
            .collect())
    }

    pub async fn mark_reviewed(&self, ids: &[String]) -> common::Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "UPDATE extraction_critic_log SET reviewed_by_reforge_at = datetime('now') WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&sql);
        for id in ids {
            q = q.bind(id);
        }
        q.execute(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn insert_and_list_unreviewed() {
        let pool = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool);
        let repo = ExtractionCriticLogRepo::new(pool.clone());

        // Need a fact to reference.
        let f = crate::types::SemanticFact {
            id: "f1".into(),
            domain: "test".into(),
            subject: "A".into(),
            predicate: "p".into(),
            object: "B".into(),
            confidence: 0.5,
            source: "test".into(),
            valid_from: "2026-04-29".into(),
            recorded_at: "2026-04-29".into(),
            stability: 1.0,
            ..Default::default()
        };
        crate::repos::SemanticFactRepo::new(pool.inner().clone())
            .upsert(&f)
            .await
            .unwrap();

        repo.insert(&f.id, "hallucinated", "no anchor")
            .await
            .unwrap();
        let list = repo.list_unreviewed(10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].verdict, "hallucinated");
    }
}
