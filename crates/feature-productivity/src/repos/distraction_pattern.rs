use chrono::Utc;
use sqlx::SqlitePool;

use crate::types::DistractionPattern;

#[derive(sqlx::FromRow)]
struct PatternRow {
    id: Option<i64>,
    date: String,
    hour_of_day: i32,
    hours_active_today: f64,
    mins_since_break: f64,
    preceding_app: Option<String>,
    preceding_category: Option<String>,
    preceding_duration_mins: Option<f64>,
    distraction_app: String,
    distraction_category: Option<String>,
    recovery_secs: Option<i64>,
    created_at: String,
}

impl From<PatternRow> for DistractionPattern {
    fn from(row: PatternRow) -> Self {
        let created_at =
            common::utils::date::parse_datetime(&row.created_at, "UTC").unwrap_or_else(Utc::now);
        Self {
            id: row.id,
            date: row.date,
            hour_of_day: row.hour_of_day,
            hours_active_today: row.hours_active_today,
            mins_since_break: row.mins_since_break,
            preceding_app: row.preceding_app,
            preceding_category: row.preceding_category,
            preceding_duration_mins: row.preceding_duration_mins,
            distraction_app: row.distraction_app,
            distraction_category: row.distraction_category,
            recovery_secs: row.recovery_secs,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DistractionPatternRepo {
    pool: SqlitePool,
}

impl DistractionPatternRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, pattern: &DistractionPattern) -> common::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO distraction_patterns (date, hour_of_day, hours_active_today, mins_since_break, preceding_app, preceding_category, preceding_duration_mins, distraction_app, distraction_category, recovery_secs)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               RETURNING id"#,
        )
        .bind(&pattern.date)
        .bind(pattern.hour_of_day)
        .bind(pattern.hours_active_today)
        .bind(pattern.mins_since_break)
        .bind(&pattern.preceding_app)
        .bind(&pattern.preceding_category)
        .bind(pattern.preceding_duration_mins)
        .bind(&pattern.distraction_app)
        .bind(&pattern.distraction_category)
        .bind(pattern.recovery_secs)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn update_recovery(&self, id: i64, recovery_secs: i64) -> common::Result<()> {
        sqlx::query("UPDATE distraction_patterns SET recovery_secs = ?1 WHERE id = ?2")
            .bind(recovery_secs)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<DistractionPattern>> {
        let rows = sqlx::query_as::<_, PatternRow>(
            r#"SELECT id, date, hour_of_day, hours_active_today, mins_since_break,
                      preceding_app, preceding_category, preceding_duration_mins,
                      distraction_app, distraction_category, recovery_secs, created_at
               FROM distraction_patterns
               WHERE date >= ?1 AND date <= ?2
               ORDER BY created_at ASC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(DistractionPattern::from).collect())
    }

    pub async fn avg_recovery_secs(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Option<f64>> {
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(CAST(recovery_secs AS REAL)) FROM distraction_patterns WHERE date >= ?1 AND date <= ?2 AND recovery_secs IS NOT NULL",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(avg)
    }

    pub async fn count_by_hour(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<(i32, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            hour_of_day: i32,
            cnt: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT hour_of_day, COUNT(*) as cnt FROM distraction_patterns WHERE date >= ?1 AND date <= ?2 GROUP BY hour_of_day ORDER BY hour_of_day",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|r| (r.hour_of_day, r.cnt)).collect())
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

    #[tokio::test]
    async fn test_insert_and_query() {
        let pool = setup_pool().await;
        let repo = DistractionPatternRepo::new(pool);

        let pattern = DistractionPattern {
            id: None,
            date: "2026-03-06".to_string(),
            hour_of_day: 14,
            hours_active_today: 5.5,
            mins_since_break: 90.0,
            preceding_app: Some("VS Code".to_string()),
            preceding_category: Some("coding".to_string()),
            preceding_duration_mins: Some(45.0),
            distraction_app: "Reddit".to_string(),
            distraction_category: Some("social_media".to_string()),
            recovery_secs: None,
            created_at: Utc::now(),
        };

        let id = repo.insert(&pattern).await.unwrap();
        assert!(id > 0);

        let results = repo.list_range("2026-03-06", "2026-03-06").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].distraction_app, "Reddit");
        assert_eq!(results[0].hour_of_day, 14);
    }

    #[tokio::test]
    async fn test_update_recovery() {
        let pool = setup_pool().await;
        let repo = DistractionPatternRepo::new(pool);

        let pattern = DistractionPattern {
            id: None,
            date: "2026-03-06".to_string(),
            hour_of_day: 10,
            hours_active_today: 2.0,
            mins_since_break: 30.0,
            preceding_app: None,
            preceding_category: None,
            preceding_duration_mins: None,
            distraction_app: "Twitter".to_string(),
            distraction_category: None,
            recovery_secs: None,
            created_at: Utc::now(),
        };

        let id = repo.insert(&pattern).await.unwrap();
        repo.update_recovery(id, 45).await.unwrap();

        let results = repo.list_range("2026-03-06", "2026-03-06").await.unwrap();
        assert_eq!(results[0].recovery_secs, Some(45));
    }

    #[tokio::test]
    async fn test_avg_recovery_secs() {
        let pool = setup_pool().await;
        let repo = DistractionPatternRepo::new(pool);

        for (i, recovery) in [30i64, 60, 90].iter().enumerate() {
            let pattern = DistractionPattern {
                id: None,
                date: "2026-03-06".to_string(),
                hour_of_day: 10 + i as i32,
                hours_active_today: 3.0,
                mins_since_break: 45.0,
                preceding_app: None,
                preceding_category: None,
                preceding_duration_mins: None,
                distraction_app: "Reddit".to_string(),
                distraction_category: None,
                recovery_secs: Some(*recovery),
                created_at: Utc::now(),
            };
            repo.insert(&pattern).await.unwrap();
        }

        let avg = repo
            .avg_recovery_secs("2026-03-06", "2026-03-06")
            .await
            .unwrap();
        assert!((avg.unwrap() - 60.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_count_by_hour() {
        let pool = setup_pool().await;
        let repo = DistractionPatternRepo::new(pool);

        for hour in [10, 10, 14] {
            let pattern = DistractionPattern {
                id: None,
                date: "2026-03-06".to_string(),
                hour_of_day: hour,
                hours_active_today: 3.0,
                mins_since_break: 30.0,
                preceding_app: None,
                preceding_category: None,
                preceding_duration_mins: None,
                distraction_app: "Reddit".to_string(),
                distraction_category: None,
                recovery_secs: None,
                created_at: Utc::now(),
            };
            repo.insert(&pattern).await.unwrap();
        }

        let counts = repo
            .count_by_hour("2026-03-06", "2026-03-06")
            .await
            .unwrap();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0], (10, 2));
        assert_eq!(counts[1], (14, 1));
    }
}
