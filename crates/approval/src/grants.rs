use crate::class::{ApprovalClass, ApprovalLifetime};
use common::Result;
use sqlx::Row;
use storage::StoragePool;

#[derive(Debug, Clone)]
pub struct GrantRow {
    pub class: ApprovalClass,
    pub tool_name: String,
    pub action: Option<String>,
    pub resource_key: Option<String>,
    pub lifetime: ApprovalLifetime,
    pub session_id: Option<String>,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Clone)]
pub struct ApprovalGrantsRepo {
    pool: StoragePool,
}

impl ApprovalGrantsRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &GrantRow) -> Result<()> {
        let class = row.class.as_str();
        let lifetime = row.lifetime.as_str();
        sqlx::query(
            "INSERT OR IGNORE INTO approval_grants
             (class, tool_name, action, resource_key, lifetime, session_id, granted_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(class)
        .bind(&row.tool_name)
        .bind(&row.action)
        .bind(&row.resource_key)
        .bind(lifetime)
        .bind(&row.session_id)
        .bind(row.granted_at)
        .bind(row.expires_at)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn find(
        &self,
        class: ApprovalClass,
        tool: &str,
        action: Option<&str>,
        resource: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Option<GrantRow>> {
        let class_s = class.as_str();
        let row = sqlx::query(
            "SELECT class, tool_name, action, resource_key, lifetime, session_id, granted_at, expires_at
             FROM approval_grants
             WHERE class = ? AND tool_name = ?
               AND (action IS ? OR action = ?)
               AND (resource_key IS ? OR resource_key = ?)
               AND ((lifetime = 'forever' AND session_id IS NULL)
                 OR (lifetime = 'session' AND session_id = ?))
             LIMIT 1",
        )
        .bind(class_s)
        .bind(tool)
        .bind(action)
        .bind(action)
        .bind(resource)
        .bind(resource)
        .bind(session_id)
        .fetch_optional(self.pool.inner())
        .await?;
        row.map(row_to_grant).transpose()
    }

    pub async fn find_forever(
        &self,
        class: ApprovalClass,
        tool: &str,
        action: Option<&str>,
        resource: Option<&str>,
    ) -> Result<Option<GrantRow>> {
        self.find(class, tool, action, resource, None).await
    }

    pub async fn purge_session(&self, session_id: &str) -> Result<u64> {
        let res =
            sqlx::query("DELETE FROM approval_grants WHERE lifetime = 'session' AND session_id = ?")
                .bind(session_id)
                .execute(self.pool.inner())
                .await?;
        Ok(res.rows_affected())
    }
}

fn row_to_grant(row: sqlx::sqlite::SqliteRow) -> Result<GrantRow> {
    let class_s: String = row.try_get("class")?;
    let lifetime_s: String = row.try_get("lifetime")?;
    Ok(GrantRow {
        class: parse_class(&class_s)?,
        tool_name: row.try_get("tool_name")?,
        action: row.try_get("action")?,
        resource_key: row.try_get("resource_key")?,
        lifetime: parse_lifetime(&lifetime_s)?,
        session_id: row.try_get("session_id")?,
        granted_at: row.try_get("granted_at")?,
        expires_at: row.try_get("expires_at")?,
    })
}

fn parse_class(s: &str) -> Result<ApprovalClass> {
    match s {
        "safe" => Ok(ApprovalClass::Safe),
        "sensitive" => Ok(ApprovalClass::Sensitive),
        "destructive" => Ok(ApprovalClass::Destructive),
        "admin" => Ok(ApprovalClass::Admin),
        other => Err(common::ConfigError::Invalid(format!(
            "unknown approval class: {other}"
        )).into()),
    }
}

fn parse_lifetime(s: &str) -> Result<ApprovalLifetime> {
    match s {
        "once" => Ok(ApprovalLifetime::Once),
        "session" => Ok(ApprovalLifetime::Session),
        "forever" => Ok(ApprovalLifetime::Forever),
        other => Err(common::ConfigError::Invalid(format!(
            "unknown approval lifetime: {other}"
        )).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_find_session_grant() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool.clone());

        let row = GrantRow {
            class: ApprovalClass::Destructive,
            tool_name: "bash".into(),
            action: None,
            resource_key: Some("rm -rf /tmp/x".into()),
            lifetime: ApprovalLifetime::Session,
            session_id: Some("sess-1".into()),
            granted_at: 1_700_000_000,
            expires_at: None,
        };
        repo.insert(&row).await.unwrap();

        let found = repo
            .find(
                ApprovalClass::Destructive,
                "bash",
                None,
                Some("rm -rf /tmp/x"),
                Some("sess-1"),
            )
            .await
            .unwrap();
        assert!(found.is_some(), "session grant should be found");

        let other_session = repo
            .find(
                ApprovalClass::Destructive,
                "bash",
                None,
                Some("rm -rf /tmp/x"),
                Some("sess-2"),
            )
            .await
            .unwrap();
        assert!(other_session.is_none(), "should not match different session");
    }

    #[tokio::test]
    async fn forever_grant_matches_any_session() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        repo.insert(&GrantRow {
            class: ApprovalClass::Sensitive,
            tool_name: "notes".into(),
            action: Some("delete".into()),
            resource_key: None,
            lifetime: ApprovalLifetime::Forever,
            session_id: None,
            granted_at: 1,
            expires_at: None,
        })
        .await
        .unwrap();

        let found = repo
            .find_forever(ApprovalClass::Sensitive, "notes", Some("delete"), None)
            .await
            .unwrap();
        assert!(found.is_some());
    }
}
