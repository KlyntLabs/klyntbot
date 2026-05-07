use crate::error::StorageError;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone)]
pub struct PatternHistoryEntry {
    pub user_id: String,
    pub tool_name: String,
    pub path: Option<String>,
    pub decision: String,
    pub pattern_used: Option<String>,
    pub occurred_at: i64,
}

#[derive(Debug, Clone)]
pub struct ApprovalPatternHistoryRepo {
    pool: SqlitePool,
}

impl ApprovalPatternHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn record(&self, entry: PatternHistoryEntry) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO approval_pattern_history \
             (user_id, tool_name, path, decision, pattern_used, occurred_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.user_id)
        .bind(&entry.tool_name)
        .bind(&entry.path)
        .bind(&entry.decision)
        .bind(&entry.pattern_used)
        .bind(entry.occurred_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Return (approval_count, total_count) for a tool + path pattern
    /// within the last `window_days` days for the given user.
    pub async fn pattern_stats(
        &self,
        user_id: &str,
        tool_name: &str,
        path_like: &str,
        window_days: i64,
    ) -> Result<(u32, u32), StorageError> {
        let cutoff = jiff::Timestamp::now().as_second() - (window_days * 86_400);
        let row = sqlx::query(
            "SELECT \
                COALESCE(SUM(CASE WHEN decision = 'allow' OR decision = 'forever' THEN 1 ELSE 0 END), 0) AS allow_count, \
                COUNT(*) AS total_count \
             FROM approval_pattern_history \
             WHERE user_id = ? AND tool_name = ? AND path LIKE ? AND occurred_at >= ?",
        )
        .bind(user_id)
        .bind(tool_name)
        .bind(path_like)
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await?;
        let allow: i64 = row.try_get("allow_count").unwrap_or(0);
        let total: i64 = row.try_get("total_count").unwrap_or(0);
        Ok((allow as u32, total as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn record_and_stats_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalPatternHistoryRepo::new(pool.inner().clone());
        repo.record(PatternHistoryEntry {
            user_id: "u1".into(),
            tool_name: "edit".into(),
            path: Some("src/components/Button.tsx".into()),
            decision: "allow".into(),
            pattern_used: None,
            occurred_at: jiff::Timestamp::now().as_second(),
        })
        .await
        .unwrap();
        repo.record(PatternHistoryEntry {
            user_id: "u1".into(),
            tool_name: "edit".into(),
            path: Some("src/components/Modal.tsx".into()),
            decision: "allow".into(),
            pattern_used: None,
            occurred_at: jiff::Timestamp::now().as_second(),
        })
        .await
        .unwrap();
        let (allow, total) = repo
            .pattern_stats("u1", "edit", "src/components/%", 30)
            .await
            .unwrap();
        assert_eq!(allow, 2);
        assert_eq!(total, 2);
    }
}
