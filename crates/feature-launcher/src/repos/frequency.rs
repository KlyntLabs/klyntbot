use jiff::Timestamp;
use sqlx::SqlitePool;
use storage::StorageError;

const HALF_LIFE_HOURS: f64 = 72.0;

#[derive(Debug, Clone)]
pub struct FrequencyRepo {
    pool: SqlitePool,
}

impl FrequencyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a usage event.
    pub async fn record_usage(&self, item_id: &str, kind: &str) -> Result<(), StorageError> {
        let now = Timestamp::now().to_string();
        sqlx::query("INSERT INTO launcher_usage_log (item_id, kind, used_at) VALUES (?, ?, ?)")
            .bind(item_id)
            .bind(kind)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Compute frecency scores for a batch of items.
    pub async fn get_frecency_batch(
        &self,
        items: &[(String, String)],
    ) -> Result<Vec<(f64, Option<String>)>, StorageError> {
        if items.is_empty() {
            return Ok(vec![]);
        }

        let conditions: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, _)| format!("(item_id = ?{} AND kind = ?{})", i * 2 + 1, i * 2 + 2))
            .collect();
        let cutoff = (Timestamp::now() - jiff::SignedDuration::from_hours(90 * 24)).to_string();
        let sql = format!(
            "SELECT item_id, kind, used_at FROM launcher_usage_log WHERE ({}) AND used_at > ?{} ORDER BY used_at DESC",
            conditions.join(" OR "),
            items.len() * 2 + 1,
        );

        let mut query = sqlx::query_as::<_, (String, String, String)>(&sql);
        for (item_id, kind) in items {
            query = query.bind(item_id).bind(kind);
        }
        query = query.bind(&cutoff);
        let rows = query.fetch_all(&self.pool).await?;

        let now = Timestamp::now();
        let lambda = (2.0_f64).ln() / HALF_LIFE_HOURS;

        let mut results = Vec::with_capacity(items.len());
        for (item_id, kind) in items {
            let mut score = 0.0_f64;
            let mut last_used: Option<String> = None;
            for (rid, rk, used_at) in &rows {
                if rid == item_id && rk == kind {
                    if last_used.is_none() {
                        last_used = Some(used_at.clone());
                    }
                    if let Ok(dt) = used_at.parse::<Timestamp>() {
                        let hours =
                            (now.as_millisecond() - dt.as_millisecond()) as f64 / 3_600_000.0;
                        score += (-lambda * hours).exp();
                    }
                }
            }
            results.push((score, last_used));
        }
        Ok(results)
    }

    /// Get top N items by frecency for the default view.
    pub async fn top_frecency(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String, f64)>, StorageError> {
        let cutoff = (Timestamp::now() - jiff::SignedDuration::from_hours(30 * 24)).to_string();
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT item_id, kind, used_at FROM launcher_usage_log WHERE used_at > ? ORDER BY used_at DESC LIMIT 500",
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        let now = Timestamp::now();
        let lambda = (2.0_f64).ln() / HALF_LIFE_HOURS;

        let mut scores: std::collections::HashMap<(String, String), f64> =
            std::collections::HashMap::new();
        for (item_id, kind, used_at) in &rows {
            if let Ok(dt) = used_at.parse::<Timestamp>() {
                let hours = (now.as_millisecond() - dt.as_millisecond()) as f64 / 3_600_000.0;
                *scores.entry((item_id.clone(), kind.clone())).or_default() +=
                    (-lambda * hours).exp();
            }
        }

        let mut sorted: Vec<_> = scores
            .into_iter()
            .map(|((id, kind), score)| (id, kind, score))
            .collect();
        sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(limit);
        Ok(sorted)
    }

    /// Prune entries older than 90 days.
    pub async fn prune_old_entries(&self) -> Result<u64, StorageError> {
        let cutoff = (Timestamp::now() - jiff::SignedDuration::from_hours(90 * 24)).to_string();
        let result = sqlx::query("DELETE FROM launcher_usage_log WHERE used_at < ?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    // Keep old increment() for backwards compat during migration
    pub async fn increment(&self, item_id: &str, kind: &str) -> Result<(), StorageError> {
        self.record_usage(item_id, kind).await
    }

    // Keep old get_boosts_batch for callers that haven't migrated yet
    pub async fn get_boosts_batch(
        &self,
        items: &[(String, String)],
    ) -> Result<Vec<(f64, Option<String>)>, StorageError> {
        self.get_frecency_batch(items).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> FrequencyRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::LauncherFeature::migrations_static(),
        )
        .await
        .unwrap();
        FrequencyRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_record_and_frecency() {
        let repo = setup().await;
        repo.record_usage("com.apple.Safari", "app").await.unwrap();
        repo.record_usage("com.apple.Safari", "app").await.unwrap();
        let items = vec![("com.apple.Safari".to_string(), "app".to_string())];
        let results = repo.get_frecency_batch(&items).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].0 > 0.0);
        assert!(results[0].1.is_some());
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_zero() {
        let repo = setup().await;
        let items = vec![("nonexistent".to_string(), "app".to_string())];
        let results = repo.get_frecency_batch(&items).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0.0);
        assert!(results[0].1.is_none());
    }

    #[tokio::test]
    async fn test_frecency_batch_ordering() {
        let repo = setup().await;
        for _ in 0..5 {
            repo.record_usage("safari", "app").await.unwrap();
        }
        repo.record_usage("slack", "app").await.unwrap();

        let items = vec![
            ("safari".to_string(), "app".to_string()),
            ("slack".to_string(), "app".to_string()),
            ("missing".to_string(), "app".to_string()),
        ];
        let results = repo.get_frecency_batch(&items).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results[0].0 > results[1].0); // safari > slack
        assert_eq!(results[2].0, 0.0); // missing = 0
        assert!(results[0].1.is_some());
        assert!(results[1].1.is_some());
        assert!(results[2].1.is_none());
    }

    #[tokio::test]
    async fn test_top_frecency() {
        let repo = setup().await;
        repo.record_usage("app_a", "app").await.unwrap();
        repo.record_usage("app_a", "app").await.unwrap();
        repo.record_usage("app_b", "app").await.unwrap();
        let top = repo.top_frecency(10).await.unwrap();
        assert!(!top.is_empty());
        assert!(top[0].2 >= top[top.len() - 1].2);
    }

    #[tokio::test]
    async fn test_increment_delegates_to_record_usage() {
        let repo = setup().await;
        repo.increment("com.apple.Safari", "app").await.unwrap();
        let items = vec![("com.apple.Safari".to_string(), "app".to_string())];
        let results = repo.get_boosts_batch(&items).await.unwrap();
        assert!(results[0].0 > 0.0);
    }
}
