//! Repository for querying review statistics from the `review_log` and `knowledge_atoms` tables.

use chrono::{NaiveDate, Utc};
use sqlx::SqlitePool;

// ── Stat types ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DailyReviewStat {
    pub date: String,
    pub review_count: i64,
    pub avg_rating: f64,
}

#[derive(Debug, Clone)]
pub struct DomainRetentionStat {
    pub domain: String,
    pub atom_count: i64,
    pub avg_retention: f64,
    pub reviews_last_7d: i64,
}

// ── Repository ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReviewStatsRepo {
    pool: SqlitePool,
}

impl ReviewStatsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Count consecutive days with at least 1 review in `review_log`, walking backwards from today.
    pub async fn current_streak(&self) -> Result<usize, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT d FROM (
                SELECT DATE(reviewed_at) as d FROM review_log
                UNION
                SELECT DATE(updated_at) as d FROM knowledge_atoms
                  WHERE status = 'active' AND salience >= 0.6
            ) ORDER BY d DESC LIMIT 60"#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let today = Utc::now().date_naive();
        let mut streak = 0usize;
        let mut expected = today;

        for (date_str,) in &rows {
            let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
                break;
            };
            if date == expected {
                streak += 1;
                expected = expected.pred_opt().unwrap_or(expected);
            } else if date < expected {
                // Gap found — stop counting
                break;
            }
        }

        Ok(streak)
    }

    /// Daily review counts for the last N days.
    pub async fn daily_reviews(&self, days: i64) -> Result<Vec<DailyReviewStat>, sqlx::Error> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();

        let rows: Vec<(String, i64, f64)> = sqlx::query_as(
            r#"
            SELECT DATE(reviewed_at) as d, COUNT(*) as cnt, AVG(rating) as avg_r
            FROM review_log
            WHERE reviewed_at > ?1
            GROUP BY d
            ORDER BY d DESC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(date, review_count, avg_rating)| DailyReviewStat {
                date,
                review_count,
                avg_rating,
            })
            .collect())
    }

    /// Per-domain stats from `knowledge_atoms`: domain, atom_count, avg_retention, reviews in last 7 days.
    pub async fn domain_retention_stats(&self) -> Result<Vec<DomainRetentionStat>, sqlx::Error> {
        let cutoff_7d = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();

        let rows: Vec<(String, i64, f64, i64)> = sqlx::query_as(
            r#"
            SELECT
                ka.domain,
                COUNT(*) as atom_count,
                AVG(ka.retention_pct) as avg_retention,
                COALESCE(
                    (SELECT COUNT(*)
                     FROM review_log rl
                     JOIN flashcards fc ON fc.id = rl.card_id
                     WHERE fc.atom_id IN (
                         SELECT id FROM knowledge_atoms WHERE domain = ka.domain AND status = 'active'
                     )
                     AND rl.reviewed_at > ?1),
                    0
                ) as reviews_last_7d
            FROM knowledge_atoms ka
            WHERE ka.status = 'active'
            GROUP BY ka.domain
            ORDER BY avg_retention ASC
            "#,
        )
        .bind(&cutoff_7d)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(domain, atom_count, avg_retention, reviews_last_7d)| DomainRetentionStat {
                    domain,
                    atom_count,
                    avg_retention,
                    reviews_last_7d,
                },
            )
            .collect())
    }

    /// Importance-weighted average retention across all active atoms.
    /// Returns 1.0 when no atoms exist (perfect score by default).
    pub async fn knowledge_retention_score(&self) -> Result<f64, sqlx::Error> {
        let row: (f64, f64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(retention_pct * personal_importance), 0.0),
                COALESCE(SUM(personal_importance), 0.0)
            FROM knowledge_atoms
            WHERE status = 'active'
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let (weighted_sum, importance_sum) = row;
        if importance_sum == 0.0 {
            Ok(1.0)
        } else {
            Ok(weighted_sum / importance_sum)
        }
    }

    /// Daily atom creation counts for the last N days.
    pub async fn daily_atoms_created(&self, days: i64) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        sqlx::query_as(
            r#"SELECT DATE(created_at) as d, COUNT(*) as cnt
               FROM knowledge_atoms
               WHERE status = 'active' AND created_at > ?1
               GROUP BY d ORDER BY d DESC"#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_current_streak_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);

        let streak = repo.current_streak().await.unwrap();
        assert_eq!(streak, 0);
    }

    #[tokio::test]
    async fn test_knowledge_retention_score_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);

        let score = repo.knowledge_retention_score().await.unwrap();
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_daily_reviews_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);

        let stats = repo.daily_reviews(30).await.unwrap();
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_domain_retention_stats_empty() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool);

        let stats = repo.domain_retention_stats().await.unwrap();
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_knowledge_retention_score_with_atoms() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool.clone());

        // Insert two active atoms with different importance and retention
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO knowledge_atoms
                (id, subject, atom_type, domain, retention_pct, stability, difficulty,
                 personal_importance, status, salience, created_at, updated_at)
            VALUES ('a1', 'atom1', 'concept', 'test', 0.8, 1.0, 5.0, 1.0, 'active', 1.0, ?1, ?1)"#,
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"INSERT INTO knowledge_atoms
                (id, subject, atom_type, domain, retention_pct, stability, difficulty,
                 personal_importance, status, salience, created_at, updated_at)
            VALUES ('a2', 'atom2', 'concept', 'test', 0.6, 1.0, 5.0, 0.5, 'active', 1.0, ?1, ?1)"#,
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Expected: (0.8*1.0 + 0.6*0.5) / (1.0 + 0.5) = 1.1 / 1.5 ≈ 0.7333
        let score = repo.knowledge_retention_score().await.unwrap();
        assert!((score - (1.1 / 1.5)).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_current_streak_includes_atoms() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool.clone());
        let now = Utc::now().to_rfc3339();

        // Insert an active atom created today (no reviews)
        sqlx::query(
            r#"INSERT INTO knowledge_atoms
                (id, subject, atom_type, domain, retention_pct, stability, difficulty,
                 personal_importance, status, salience, created_at, updated_at)
            VALUES ('a-streak', 'test', 'concept', 'test', 0.8, 1.0, 5.0, 1.0, 'active', 0.8, ?1, ?1)"#,
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let streak = repo.current_streak().await.unwrap();
        assert_eq!(
            streak, 1,
            "Atom acceptance alone should count as a streak day"
        );
    }

    #[tokio::test]
    async fn test_current_streak_consecutive() {
        let pool = cognitive_test_pool().await;
        let repo = ReviewStatsRepo::new(pool.clone());

        // Create a flashcard to reference
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO flashcards
                (id, deck, front, back, card_type, stability, difficulty, state, created_at, updated_at)
            VALUES ('fc1', 'general', 'q?', 'a!', 'basic', 1.0, 5.0, 'review', ?1, ?1)"#,
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Insert reviews for today and yesterday (2-day streak)
        let today = Utc::now().date_naive();
        let yesterday = today.pred_opt().unwrap();

        for (i, date) in [today, yesterday].iter().enumerate() {
            let ts = format!("{}T12:00:00+00:00", date);
            sqlx::query(
                r#"INSERT INTO review_log
                    (id, card_id, rating, elapsed_days, scheduled_days, state, reviewed_at)
                VALUES (?1, 'fc1', 3, 1.0, 1.0, 'review', ?2)"#,
            )
            .bind(format!("rev-{}", i))
            .bind(&ts)
            .execute(&pool)
            .await
            .unwrap();
        }

        let streak = repo.current_streak().await.unwrap();
        assert_eq!(streak, 2);
    }
}
