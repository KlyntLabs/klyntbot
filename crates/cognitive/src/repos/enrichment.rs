//! Repos for Phase B2 enrichment: conversation density scoring and knowledge graph snapshots.

use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// ConversationDensityRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationDensityRow {
    pub id: String,
    pub session_key: String,
    pub content_preview: String,
    pub density_score: f64,
    pub tier: String,
    pub entity_signal: f64,
    pub action_signal: f64,
    pub decision_signal: f64,
    pub novelty_signal: f64,
    pub enriched: bool,
    pub computed_at: String,
}

#[derive(Debug, Clone)]
pub struct ConversationDensityRepo {
    pool: SqlitePool,
}

impl ConversationDensityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a density score for a conversation turn.
    pub async fn insert(
        &self,
        id: &str,
        session_key: &str,
        content_preview: &str,
        score: &crate::services::value_density::DensityScore,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO conversation_density
             (id, session_key, content_preview, density_score, tier,
              entity_signal, action_signal, decision_signal, novelty_signal)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(id)
        .bind(session_key)
        .bind(content_preview)
        .bind(score.total)
        .bind(score.tier.as_str())
        .bind(score.entity_signal)
        .bind(score.action_signal)
        .bind(score.decision_signal)
        .bind(score.novelty_signal)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count unenriched turns by tier since a timestamp.
    pub async fn count_pending_by_tier(&self, tier: &str, since: &str) -> Result<u32, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM conversation_density
             WHERE tier = ?1 AND enriched = 0 AND computed_at > ?2",
        )
        .bind(tier)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u32)
    }

    /// Load unenriched medium-density turns for Phase 6.5 batch processing.
    pub async fn load_pending_medium(
        &self,
        limit: u32,
    ) -> Result<Vec<ConversationDensityRow>, sqlx::Error> {
        sqlx::query_as::<_, ConversationDensityRow>(
            "SELECT * FROM conversation_density
             WHERE tier = 'medium' AND enriched = 0
             ORDER BY density_score DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Mark turns as enriched after graph processing.
    pub async fn mark_enriched(&self, ids: &[String]) -> Result<(), sqlx::Error> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "UPDATE conversation_density SET enriched = 1 WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KnowledgeSnapshotRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeSnapshotRow {
    pub id: i64,
    pub fact_count: i64,
    pub entity_count: i64,
    pub relationship_count: i64,
    pub domain_summary: Option<String>,
    pub top_entities: Option<String>,
    pub graph_metrics: Option<String>,
    pub snapshot_at: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeSnapshotRepo {
    pool: SqlitePool,
}

impl KnowledgeSnapshotRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a nightly knowledge graph snapshot.
    pub async fn insert(
        &self,
        fact_count: u32,
        entity_count: u32,
        relationship_count: u32,
        domain_summary: Option<&str>,
        top_entities: Option<&str>,
        graph_metrics: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO knowledge_snapshots
             (fact_count, entity_count, relationship_count, domain_summary, top_entities, graph_metrics)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(fact_count)
        .bind(entity_count)
        .bind(relationship_count)
        .bind(domain_summary)
        .bind(top_entities)
        .bind(graph_metrics)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load the N most recent snapshots for trend analysis.
    pub async fn recent(&self, limit: u32) -> Result<Vec<KnowledgeSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeSnapshotRow>(
            "SELECT * FROM knowledge_snapshots ORDER BY snapshot_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete snapshots older than N days.
    pub async fn prune(&self, max_age_days: u32) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM knowledge_snapshots WHERE snapshot_at < datetime('now', ?1)")
                .bind(format!("-{max_age_days} days"))
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn density_insert_and_load_pending() {
        let pool = setup().await;
        let repo = ConversationDensityRepo::new(pool);

        let score = crate::services::value_density::DensityScore {
            total: 0.55,
            entity_signal: 0.4,
            action_signal: 0.3,
            decision_signal: 0.2,
            novelty_signal: 0.1,
            tier: crate::services::value_density::DensityTier::Medium,
        };
        repo.insert("t1", "sess1", "test content", &score)
            .await
            .unwrap();

        let pending = repo.load_pending_medium(10).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "t1");
    }

    #[tokio::test]
    async fn density_mark_enriched() {
        let pool = setup().await;
        let repo = ConversationDensityRepo::new(pool);

        let score = crate::services::value_density::DensityScore {
            total: 0.55,
            entity_signal: 0.4,
            action_signal: 0.3,
            decision_signal: 0.2,
            novelty_signal: 0.1,
            tier: crate::services::value_density::DensityTier::Medium,
        };
        repo.insert("t2", "sess1", "content", &score).await.unwrap();
        repo.mark_enriched(&["t2".to_string()]).await.unwrap();

        let pending = repo.load_pending_medium(10).await.unwrap();
        assert!(
            pending.is_empty(),
            "Enriched turns should not appear in pending"
        );
    }

    #[tokio::test]
    async fn snapshot_insert_and_recent() {
        let pool = setup().await;
        let repo = KnowledgeSnapshotRepo::new(pool);

        repo.insert(100, 50, 30, Some(r#"{"work":42}"#), None, None)
            .await
            .unwrap();
        repo.insert(105, 52, 32, Some(r#"{"work":44}"#), None, None)
            .await
            .unwrap();

        let recent = repo.recent(5).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Both rows inserted; fact counts should be present (order may vary within same second)
        let counts: Vec<i64> = recent.iter().map(|r| r.fact_count).collect();
        assert!(counts.contains(&100));
        assert!(counts.contains(&105));
    }
}
