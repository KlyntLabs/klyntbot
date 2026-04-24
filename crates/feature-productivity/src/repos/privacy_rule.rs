use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{MatchMode, PrivacyRule};

#[derive(sqlx::FromRow)]
struct PrivacyRuleRow {
    id: String,
    rule_type: String,
    pattern: String,
    match_mode: String,
    created_at: String,
}

impl From<PrivacyRuleRow> for PrivacyRule {
    fn from(row: PrivacyRuleRow) -> Self {
        let match_mode = row.match_mode.parse::<MatchMode>().unwrap_or_else(|_| {
            warn!(raw = %row.match_mode, "unknown match_mode in privacy_rules, defaulting to Exact");
            MatchMode::Exact
        });
        let created_at = row
            .created_at
            .parse::<jiff::Timestamp>()
            .unwrap_or_else(|_| jiff::Timestamp::now());

        Self {
            id: row.id,
            rule_type: row.rule_type,
            pattern: row.pattern,
            match_mode,
            created_at,
        }
    }
}

const COLUMNS: &str = "id, rule_type, pattern, match_mode, created_at";

#[derive(Debug, Clone)]
pub struct PrivacyRuleRepo {
    pool: SqlitePool,
}

impl PrivacyRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> common::Result<Vec<PrivacyRule>> {
        let rows = sqlx::query_as::<_, PrivacyRuleRow>(&format!(
            "SELECT {COLUMNS} FROM productivity_privacy_rules ORDER BY created_at ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(PrivacyRule::from).collect())
    }

    pub async fn create(&self, rule: &PrivacyRule) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_privacy_rules (id, rule_type, pattern, match_mode, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind(&rule.id)
        .bind(&rule.rule_type)
        .bind(&rule.pattern)
        .bind(rule.match_mode.to_string())
        .bind(rule.created_at.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> common::Result<()> {
        sqlx::query("DELETE FROM productivity_privacy_rules WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn sample_rule(id: &str) -> PrivacyRule {
        PrivacyRule {
            id: id.to_string(),
            rule_type: "exclude_app".to_string(),
            pattern: "1Password".to_string(),
            match_mode: MatchMode::Exact,
            created_at: jiff::Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_create_and_list_all() {
        let pool = setup_pool().await;
        let repo = PrivacyRuleRepo::new(pool);

        let rule = sample_rule("priv-1");
        repo.create(&rule).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].pattern, "1Password");
        assert_eq!(all[0].match_mode, MatchMode::Exact);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = setup_pool().await;
        let repo = PrivacyRuleRepo::new(pool);

        let r1 = sample_rule("priv-del-1");
        let r2 = PrivacyRule {
            id: "priv-del-2".to_string(),
            rule_type: "redact_title".to_string(),
            pattern: "banking".to_string(),
            match_mode: MatchMode::Contains,
            created_at: jiff::Timestamp::now(),
        };
        repo.create(&r1).await.unwrap();
        repo.create(&r2).await.unwrap();

        assert_eq!(repo.list_all().await.unwrap().len(), 2);

        repo.delete("priv-del-1").await.unwrap();

        let remaining = repo.list_all().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "priv-del-2");
    }
}
