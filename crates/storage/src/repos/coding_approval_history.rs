use crate::error::StorageError;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub tool: String,
    pub args_hash: String,
    pub repo_id: String,
    pub decision: String,
    pub decided_by: String,
    pub layer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalHistorySummary {
    pub approval_count: u32,
    pub denial_count: u32,
    pub last_decided_at: Option<i64>,
}

impl ApprovalHistorySummary {
    pub fn poisoned(&self) -> bool {
        self.denial_count > 0
    }
}

#[derive(Debug, Clone)]
pub struct CodingApprovalHistoryRepo {
    pool: SqlitePool,
}

impl CodingApprovalHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn record(&self, entry: HistoryEntry) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO coding_approval_history \
             (tool, args_hash, repo_id, decision, decided_by, layer) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.tool)
        .bind(&entry.args_hash)
        .bind(&entry.repo_id)
        .bind(&entry.decision)
        .bind(&entry.decided_by)
        .bind(&entry.layer)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn summary(
        &self,
        tool: &str,
        args_hash: &str,
        repo_id: &str,
    ) -> Result<ApprovalHistorySummary, StorageError> {
        let row = sqlx::query(
            "SELECT \
                COALESCE(SUM(CASE WHEN decision = 'allow' THEN 1 ELSE 0 END), 0) AS allow_count, \
                COALESCE(SUM(CASE WHEN decision = 'deny'  THEN 1 ELSE 0 END), 0) AS deny_count, \
                MAX(created_at) AS last_at \
             FROM coding_approval_history WHERE tool = ? AND args_hash = ? AND repo_id = ?",
        )
        .bind(tool)
        .bind(args_hash)
        .bind(repo_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(ApprovalHistorySummary {
            approval_count: row.try_get::<i64, _>("allow_count").unwrap_or(0) as u32,
            denial_count: row.try_get::<i64, _>("deny_count").unwrap_or(0) as u32,
            last_decided_at: row.try_get::<Option<i64>, _>("last_at").unwrap_or(None),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn clear_for_tool(
        &self,
        tool: &str,
        repo_id: Option<&str>,
    ) -> Result<u64, StorageError> {
        let res = match repo_id {
            Some(rid) => {
                sqlx::query("DELETE FROM coding_approval_history WHERE tool = ? AND repo_id = ?")
                    .bind(tool)
                    .bind(rid)
                    .execute(&self.pool)
                    .await
            }
            None => {
                sqlx::query("DELETE FROM coding_approval_history WHERE tool = ?")
                    .bind(tool)
                    .execute(&self.pool)
                    .await
            }
        }?;
        Ok(res.rows_affected())
    }

    /// Return per-args_hash stats for a tool, filtered to entries decided within
    /// the last `window_days` days. Each row is (args_hash, approval_count, total_count).
    /// If `args_hash` is provided, only stats for that hash are returned.
    pub async fn tool_pattern_stats(
        &self,
        tool: &str,
        args_hash: Option<&str>,
        window_days: i64,
    ) -> Result<Vec<(String, u32, u32)>, StorageError> {
        let cutoff = jiff::Timestamp::now().as_second() - (window_days * 86_400);
        let rows = if let Some(hash) = args_hash {
            sqlx::query(
                "SELECT args_hash, \
                    COALESCE(SUM(CASE WHEN decision = 'allow' THEN 1 ELSE 0 END), 0) AS allow_count, \
                    COUNT(*) AS total_count \
                 FROM coding_approval_history \
                 WHERE tool = ? AND args_hash = ? AND created_at >= ? \
                 GROUP BY args_hash",
            )
            .bind(tool)
            .bind(hash)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT args_hash, \
                    COALESCE(SUM(CASE WHEN decision = 'allow' THEN 1 ELSE 0 END), 0) AS allow_count, \
                    COUNT(*) AS total_count \
                 FROM coding_approval_history \
                 WHERE tool = ? AND created_at >= ? \
                 GROUP BY args_hash",
            )
            .bind(tool)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| {
                let hash: String = r.try_get("args_hash").unwrap_or_default();
                let allow: i64 = r.try_get("allow_count").unwrap_or(0);
                let total: i64 = r.try_get("total_count").unwrap_or(0);
                (hash, allow as u32, total as u32)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn record_and_summary_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.inner().clone());
        repo.record(HistoryEntry {
            tool: "bash".into(),
            args_hash: "abc".into(),
            repo_id: "r1".into(),
            decision: "allow".into(),
            decided_by: "user".into(),
            layer: "ask".into(),
        })
        .await
        .unwrap();
        repo.record(HistoryEntry {
            tool: "bash".into(),
            args_hash: "abc".into(),
            repo_id: "r1".into(),
            decision: "allow".into(),
            decided_by: "user".into(),
            layer: "ask".into(),
        })
        .await
        .unwrap();
        let summary = repo.summary("bash", "abc", "r1").await.unwrap();
        assert_eq!(summary.approval_count, 2);
        assert_eq!(summary.denial_count, 0);
    }

    #[tokio::test]
    async fn single_denial_marks_history_poisoned() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.inner().clone());
        for _ in 0..10 {
            repo.record(HistoryEntry {
                tool: "bash".into(),
                args_hash: "x".into(),
                repo_id: "r".into(),
                decision: "allow".into(),
                decided_by: "user".into(),
                layer: "ask".into(),
            })
            .await
            .unwrap();
        }
        repo.record(HistoryEntry {
            tool: "bash".into(),
            args_hash: "x".into(),
            repo_id: "r".into(),
            decision: "deny".into(),
            decided_by: "user".into(),
            layer: "ask".into(),
        })
        .await
        .unwrap();
        let s = repo.summary("bash", "x", "r").await.unwrap();
        assert_eq!(s.denial_count, 1);
        assert!(s.poisoned());
    }

    #[tokio::test]
    async fn clear_for_tool_empties_summary() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.inner().clone());
        repo.record(HistoryEntry {
            tool: "bash".into(),
            args_hash: "z".into(),
            repo_id: "r".into(),
            decision: "allow".into(),
            decided_by: "user".into(),
            layer: "ask".into(),
        })
        .await
        .unwrap();
        repo.clear_for_tool("bash", Some("r")).await.unwrap();
        let s = repo.summary("bash", "z", "r").await.unwrap();
        assert_eq!(s.approval_count, 0);
    }
}
