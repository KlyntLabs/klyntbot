//! Append-only changelog for semantic fact mutations.
//!
//! Records every create/update/supersede/archive operation for auditing.
//! Use `prune()` on a cron schedule to prevent unbounded growth.

use sqlx::SqlitePool;

/// The type of mutation recorded in the changelog.
#[derive(Debug, Clone, Copy)]
pub enum FactChangeType {
    Create,
    Update,
    Supersede,
    Archive,
}

impl FactChangeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Supersede => "supersede",
            Self::Archive => "archive",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChangelogEntry {
    pub id: i64,
    pub fact_id: String,
    pub change_type: String,
    pub field_changed: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub source: Option<String>,
    pub changed_at: String,
}

#[derive(Debug, Clone)]
pub struct FactChangelogRepo {
    pool: SqlitePool,
}

impl FactChangelogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        fact_id: &str,
        change_type: FactChangeType,
        field_changed: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        source: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO fact_changelog (fact_id, change_type, field_changed, old_value, new_value, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(fact_id)
        .bind(change_type.as_str())
        .bind(field_changed)
        .bind(old_value)
        .bind(new_value)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn history(
        &self,
        fact_id: &str,
        limit: u32,
    ) -> Result<Vec<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT * FROM fact_changelog WHERE fact_id = ?1 ORDER BY changed_at DESC LIMIT ?2",
        )
        .bind(fact_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn changes_since(
        &self,
        since: &str,
        limit: u32,
    ) -> Result<Vec<ChangelogEntry>, sqlx::Error> {
        sqlx::query_as::<_, ChangelogEntry>(
            "SELECT * FROM fact_changelog WHERE changed_at > ?1 ORDER BY changed_at DESC LIMIT ?2",
        )
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn prune(&self, max_age_days: u32) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM fact_changelog WHERE changed_at < datetime('now', ?1)")
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
    async fn test_record_and_history() {
        let pool = setup().await;
        let repo = FactChangelogRepo::new(pool);

        repo.record(
            "f1",
            FactChangeType::Create,
            None,
            None,
            Some("user peak_hours 10am"),
            Some("extraction"),
        )
        .await
        .unwrap();
        repo.record(
            "f1",
            FactChangeType::Update,
            Some("object"),
            Some("10am"),
            Some("9am"),
            Some("consolidation"),
        )
        .await
        .unwrap();

        let history = repo.history("f1", 100).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].change_type, "update"); // newest first
        assert_eq!(history[1].change_type, "create");
    }

    #[tokio::test]
    async fn test_changes_since() {
        let pool = setup().await;
        let repo = FactChangelogRepo::new(pool);

        repo.record("f1", FactChangeType::Create, None, None, Some("test"), None)
            .await
            .unwrap();

        let changes = repo.changes_since("2020-01-01", 100).await.unwrap();
        assert!(!changes.is_empty());
    }
}
