use crate::StoragePool;
use common::Result;
use sqlx::Row;

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
    pool: StoragePool,
}

impl CodingApprovalHistoryRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn record(&self, entry: HistoryEntry) -> Result<()> {
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
        .execute(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn summary(
        &self,
        tool: &str,
        args_hash: &str,
        repo_id: &str,
    ) -> Result<ApprovalHistorySummary> {
        let row = sqlx::query(
            "SELECT \
                SUM(CASE WHEN decision = 'allow' THEN 1 ELSE 0 END) AS allow_count, \
                SUM(CASE WHEN decision = 'deny'  THEN 1 ELSE 0 END) AS deny_count, \
                MAX(created_at) AS last_at \
             FROM coding_approval_history WHERE tool = ? AND args_hash = ? AND repo_id = ?",
        )
        .bind(tool)
        .bind(args_hash)
        .bind(repo_id)
        .fetch_one(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(ApprovalHistorySummary {
            approval_count: row
                .try_get::<Option<i64>, _>("allow_count")
                .unwrap_or(None)
                .unwrap_or(0) as u32,
            denial_count: row
                .try_get::<Option<i64>, _>("deny_count")
                .unwrap_or(None)
                .unwrap_or(0) as u32,
            last_decided_at: row.try_get::<Option<i64>, _>("last_at").unwrap_or(None),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn clear_for_tool(&self, tool: &str, repo_id: Option<&str>) -> Result<u64> {
        let res = match repo_id {
            Some(rid) => {
                sqlx::query("DELETE FROM coding_approval_history WHERE tool = ? AND repo_id = ?")
                    .bind(tool)
                    .bind(rid)
                    .execute(self.pool.inner())
                    .await
            }
            None => {
                sqlx::query("DELETE FROM coding_approval_history WHERE tool = ?")
                    .bind(tool)
                    .execute(self.pool.inner())
                    .await
            }
        }
        .map_err(common::KlyntbotError::from)?;
        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn record_and_summary_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
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
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
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
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
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
