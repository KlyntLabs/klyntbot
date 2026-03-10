//! Repository for the `procedural_rules` table.

use sqlx::SqlitePool;

use crate::types::ProceduralRule;

#[derive(Debug, Clone)]
pub struct ProceduralRuleRepo {
    pool: SqlitePool,
}

impl ProceduralRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a procedural rule.
    pub async fn upsert(&self, rule: &ProceduralRule) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO procedural_rules (id, domain, rule_text, confidence, source,
                signal_count, created_at, updated_at, active)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT (id) DO UPDATE SET
                rule_text = excluded.rule_text,
                confidence = excluded.confidence,
                signal_count = excluded.signal_count,
                updated_at = datetime('now'),
                active = excluded.active
            "#,
        )
        .bind(&rule.id)
        .bind(&rule.domain)
        .bind(&rule.rule_text)
        .bind(rule.confidence)
        .bind(&rule.source)
        .bind(rule.signal_count)
        .bind(&rule.created_at)
        .bind(&rule.updated_at)
        .bind(rule.active)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all active rules for a domain.
    pub async fn list_active(&self, domain: &str) -> Result<Vec<ProceduralRule>, sqlx::Error> {
        sqlx::query_as::<_, ProceduralRule>(
            "SELECT * FROM procedural_rules WHERE domain = ?1 AND active = 1 ORDER BY confidence DESC",
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await
    }

    /// Increment the signal count for a rule (pattern reinforcement).
    pub async fn increment_signal_count(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE procedural_rules SET signal_count = signal_count + 1, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count all active rules across all domains.
    pub async fn count_all_active(&self) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM procedural_rules WHERE active = 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Deactivate a rule.
    pub async fn deactivate(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE procedural_rules SET active = 0, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_rule(id: &str, domain: &str, rule_text: &str) -> ProceduralRule {
        ProceduralRule {
            id: id.into(),
            domain: domain.into(),
            rule_text: rule_text.into(),
            confidence: 0.6,
            source: "reflected".into(),
            signal_count: 1,
            created_at: "2026-03-06".into(),
            updated_at: "2026-03-06".into(),
            active: true,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_list_active() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r1 = test_rule("r1", "productivity", "Suggest break after 90min focus");
        let r2 = test_rule("r2", "productivity", "Morning is peak productivity time");
        repo.upsert(&r1).await.unwrap();
        repo.upsert(&r2).await.unwrap();

        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_increment_signal_count() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r = test_rule("r1", "productivity", "Morning is best");
        repo.upsert(&r).await.unwrap();

        repo.increment_signal_count("r1").await.unwrap();
        repo.increment_signal_count("r1").await.unwrap();

        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active[0].signal_count, 3); // started at 1, +2
    }

    #[tokio::test]
    async fn test_deactivate() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r = test_rule("r1", "productivity", "Outdated rule");
        repo.upsert(&r).await.unwrap();
        repo.deactivate("r1").await.unwrap();

        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 0);
    }
}
