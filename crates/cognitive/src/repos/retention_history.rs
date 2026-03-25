//! Repository for querying daily retention history from `review_log` + `knowledge_atoms`.
//!
//! Since atoms only store current `retention_pct`, historical retention is approximated
//! from review success rates (rating >= 3) over time.

use std::collections::HashMap;

use sqlx::SqlitePool;

// ── Data types ─────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyRetentionPoint {
    pub date: String,
    pub avg_retention: f64,
    pub review_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DomainRetentionHistory {
    pub domain: String,
    pub points: Vec<DailyRetentionPoint>,
}

// ── Repository ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RetentionHistoryRepo {
    pool: SqlitePool,
}

impl RetentionHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get daily retention data for the last N days (all domains combined).
    ///
    /// Retention is approximated as the fraction of reviews with rating >= 3 (pass).
    pub async fn daily_retention(
        &self,
        days: i64,
    ) -> Result<Vec<DailyRetentionPoint>, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        sqlx::query_as::<_, (String, f64, i64)>(
            r#"
            SELECT DATE(rl.reviewed_at) as d,
                   AVG(CASE WHEN rl.rating >= 3 THEN 1.0 ELSE 0.0 END) as success_rate,
                   COUNT(*) as cnt
            FROM review_log rl
            WHERE rl.reviewed_at > ?1
            GROUP BY d
            ORDER BY d ASC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(date, avg, count)| DailyRetentionPoint {
                    date,
                    avg_retention: avg,
                    review_count: count,
                })
                .collect()
        })
    }

    /// Get per-domain daily retention for chart breakdown.
    ///
    /// Returns one `DomainRetentionHistory` per domain that had reviews in the last N days.
    pub async fn domain_retention_history(
        &self,
        days: i64,
    ) -> Result<Vec<DomainRetentionHistory>, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let rows: Vec<(String, String, f64, i64)> = sqlx::query_as(
            r#"
            SELECT ka.domain, DATE(rl.reviewed_at) as d,
                   AVG(CASE WHEN rl.rating >= 3 THEN 1.0 ELSE 0.0 END) as success_rate,
                   COUNT(*) as cnt
            FROM review_log rl
            JOIN flashcards fc ON fc.id = rl.card_id
            JOIN knowledge_atoms ka ON ka.id = fc.atom_id
            WHERE rl.reviewed_at > ?1
            GROUP BY ka.domain, d
            ORDER BY ka.domain, d ASC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        let mut map: HashMap<String, Vec<DailyRetentionPoint>> = HashMap::new();
        for (domain, date, avg, count) in rows {
            map.entry(domain).or_default().push(DailyRetentionPoint {
                date,
                avg_retention: avg,
                review_count: count,
            });
        }
        let mut result: Vec<_> = map
            .into_iter()
            .map(|(domain, points)| DomainRetentionHistory { domain, points })
            .collect();
        result.sort_by(|a, b| a.domain.cmp(&b.domain));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_daily_retention_empty() {
        let pool = cognitive_test_pool().await;
        let repo = RetentionHistoryRepo::new(pool);
        let points = repo.daily_retention(30).await.unwrap();
        assert!(points.is_empty());
    }

    #[tokio::test]
    async fn test_domain_retention_history_empty() {
        let pool = cognitive_test_pool().await;
        let repo = RetentionHistoryRepo::new(pool);
        let history = repo.domain_retention_history(30).await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_daily_retention_with_reviews() {
        let pool = cognitive_test_pool().await;
        let repo = RetentionHistoryRepo::new(pool.clone());

        let now = chrono::Utc::now();
        let today_ts = format!("{}T12:00:00+00:00", now.date_naive());

        // Insert a flashcard to reference (no atom needed for overall retention)
        sqlx::query(
            r#"INSERT INTO flashcards
                (id, deck, front, back, card_type, stability, difficulty, state, created_at, updated_at)
            VALUES ('fc1', 'general', 'q?', 'a!', 'basic', 1.0, 5.0, 'review', ?1, ?1)"#,
        )
        .bind(&today_ts)
        .execute(&pool)
        .await
        .unwrap();

        // Insert two reviews: one pass (rating 4) and one fail (rating 1)
        sqlx::query(
            r#"INSERT INTO review_log
                (id, card_id, rating, elapsed_days, scheduled_days, state, reviewed_at)
            VALUES ('rev1', 'fc1', 4, 1.0, 1.0, 'review', ?1),
                   ('rev2', 'fc1', 1, 1.0, 1.0, 'review', ?1)"#,
        )
        .bind(&today_ts)
        .execute(&pool)
        .await
        .unwrap();

        let points = repo.daily_retention(7).await.unwrap();
        assert_eq!(points.len(), 1);
        // 1 pass out of 2 reviews → 0.5 success rate
        assert!((points[0].avg_retention - 0.5).abs() < 0.001);
        assert_eq!(points[0].review_count, 2);
    }
}
