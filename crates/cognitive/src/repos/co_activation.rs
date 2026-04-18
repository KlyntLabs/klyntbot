//! Repository for co-activation tracking between semantic facts.
//!
//! When facts are retrieved together, they form co-activation pairs with increasing
//! strength (Hebbian learning). This enables the retrieval system to boost facts
//! that historically co-occur in useful retrievals.

use jiff::Timestamp;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct CoActivationRepo {
    pool: SqlitePool,
}

impl CoActivationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record that a set of facts were co-retrieved. Inserts/increments strength
    /// for every ordered pair (a, b) where a < b.
    pub async fn record_co_retrieval(&self, fact_ids: &[String]) -> Result<(), sqlx::Error> {
        if fact_ids.len() < 2 {
            return Ok(());
        }
        let now = Timestamp::now().to_string();
        let mut sorted = fact_ids.to_vec();
        sorted.sort();
        sorted.dedup();

        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                sqlx::query(
                    "INSERT INTO co_activation (fact_id_a, fact_id_b, strength, last_fired)
                     VALUES (?1, ?2, 1.0, ?3)
                     ON CONFLICT(fact_id_a, fact_id_b)
                     DO UPDATE SET strength = strength + 1.0, last_fired = ?3",
                )
                .bind(&sorted[i])
                .bind(&sorted[j])
                .bind(&now)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Sum co-activation strengths between `fact_id` and each peer in `peer_ids`.
    /// Checks both orderings since pairs are stored with a < b.
    pub async fn sum_strength_with_peers(
        &self,
        fact_id: &str,
        peer_ids: &[String],
    ) -> Result<f64, sqlx::Error> {
        if peer_ids.is_empty() {
            return Ok(0.0);
        }
        let placeholders: Vec<String> =
            (0..peer_ids.len()).map(|i| format!("?{}", i + 2)).collect();
        let ph = placeholders.join(",");

        let sql = format!(
            "SELECT COALESCE(SUM(strength), 0.0) FROM co_activation
             WHERE (fact_id_a = ?1 AND fact_id_b IN ({ph}))
                OR (fact_id_b = ?1 AND fact_id_a IN ({ph}))"
        );

        let mut query = sqlx::query_as::<_, (f64,)>(&sql).bind(fact_id);
        // Bind peer_ids twice (once for each IN clause)
        for peer in peer_ids {
            query = query.bind(peer);
        }
        for peer in peer_ids {
            query = query.bind(peer);
        }
        let (total,) = query.fetch_one(&self.pool).await?;
        Ok(total)
    }

    /// Delete pairs whose `last_fired` is older than `max_age_days`, regardless of strength.
    pub async fn expire_stale(&self, max_age_days: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM co_activation \
             WHERE julianday('now') - julianday(last_fired) > ?1",
        )
        .bind(max_age_days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Multiply all strengths by `factor` and delete pairs below `min_strength`.
    pub async fn decay_all(&self, factor: f64, min_strength: f64) -> Result<u64, sqlx::Error> {
        sqlx::query("UPDATE co_activation SET strength = strength * ?1")
            .bind(factor)
            .execute(&self.pool)
            .await?;
        let result = sqlx::query("DELETE FROM co_activation WHERE strength < ?1")
            .bind(min_strength)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Load all co-activation pairs (for graph building).
    pub async fn list_all_pairs(&self) -> Result<Vec<(String, String, f64)>, sqlx::Error> {
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            "SELECT fact_id_a, fact_id_b, strength FROM co_activation ORDER BY strength DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count total co-activation pairs.
    pub async fn count_all(&self) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM co_activation")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_record_co_retrieval() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        // 3 facts = 3 pairs: (a,b), (a,c), (b,c)
        repo.record_co_retrieval(&["c".into(), "a".into(), "b".into()])
            .await
            .unwrap();
        assert_eq!(repo.count_all().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_sum_strength_with_peers() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        // Record twice to get strength 2.0
        let ids = vec!["x".into(), "y".into()];
        repo.record_co_retrieval(&ids).await.unwrap();
        repo.record_co_retrieval(&ids).await.unwrap();

        let sum = repo
            .sum_strength_with_peers("x", &["y".into()])
            .await
            .unwrap();
        assert!((sum - 2.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_sum_strength_no_peers() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        let sum = repo.sum_strength_with_peers("x", &[]).await.unwrap();
        assert!((sum - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_decay_and_prune() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        repo.record_co_retrieval(&["a".into(), "b".into()])
            .await
            .unwrap();
        // strength=1.0, decay by 0.05 -> 0.05, below threshold 0.1 -> pruned
        let pruned = repo.decay_all(0.05, 0.1).await.unwrap();
        assert_eq!(pruned, 1);
        assert_eq!(repo.count_all().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_single_fact_no_pairs() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        repo.record_co_retrieval(&["only_one".into()])
            .await
            .unwrap();
        assert_eq!(repo.count_all().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_expire_stale_keeps_recent() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool);
        repo.record_co_retrieval(&["a".into(), "b".into()])
            .await
            .unwrap();
        // Just created — should not expire with a 90-day window
        let expired = repo.expire_stale(90).await.unwrap();
        assert_eq!(expired, 0);
        assert_eq!(repo.count_all().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_expire_stale_removes_old() {
        let pool = cognitive_test_pool().await;
        let repo = CoActivationRepo::new(pool.clone());
        repo.record_co_retrieval(&["a".into(), "b".into()])
            .await
            .unwrap();
        // Backdate last_fired to 100 days ago
        sqlx::query("UPDATE co_activation SET last_fired = datetime('now', '-100 days')")
            .execute(&pool)
            .await
            .unwrap();
        let expired = repo.expire_stale(90).await.unwrap();
        assert_eq!(expired, 1);
        assert_eq!(repo.count_all().await.unwrap(), 0);
    }
}
