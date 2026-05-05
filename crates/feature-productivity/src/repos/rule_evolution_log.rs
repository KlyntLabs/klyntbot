use sqlx::SqlitePool;

use crate::types::RuleEvolutionEntry;

#[derive(Debug, Clone)]
pub struct RuleEvolutionLogRepo {
    pool: SqlitePool,
}

impl RuleEvolutionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, entry: &RuleEvolutionEntry) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_rule_evolution_log (id, rule_id, action, old_confidence, new_confidence, old_category, new_category, trigger_source, evidence_count, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
        )
        .bind(&entry.id)
        .bind(&entry.rule_id)
        .bind(&entry.action)
        .bind(entry.old_confidence)
        .bind(entry.new_confidence)
        .bind(&entry.old_category)
        .bind(&entry.new_category)
        .bind(&entry.trigger_source)
        .bind(entry.evidence_count)
        .bind(&entry.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_for_rule(&self, rule_id: &str) -> common::Result<Vec<RuleEvolutionEntry>> {
        let rows = sqlx::query_as::<_, RuleEvolutionEntry>(
            "SELECT * FROM productivity_rule_evolution_log WHERE rule_id = ?1 ORDER BY created_at DESC",
        )
        .bind(rule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{IntelligenceSessionType, MatchMode, RuleSource, RuleType, TrackingRule};
    

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    async fn insert_tracking_rule(pool: &SqlitePool, id: &str) {
        let now = jiff::Timestamp::now();
        let rule = TrackingRule {
            id: id.to_string(),
            rule_type: RuleType::App,
            match_field: "app_name".to_string(),
            match_pattern: "TestApp".to_string(),
            match_mode: MatchMode::Exact,
            category: "coding".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 10,
            source: RuleSource::Learned,
            confidence: 0.7,
            hit_count: 5,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        sqlx::query(
            r#"INSERT INTO productivity_tracking_rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count, is_active, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
        )
        .bind(&rule.id)
        .bind(rule.rule_type.to_string())
        .bind(&rule.match_field)
        .bind(&rule.match_pattern)
        .bind(rule.match_mode.to_string())
        .bind(&rule.category)
        .bind(rule.session_type.to_string())
        .bind(rule.priority)
        .bind(rule.source.to_string())
        .bind(rule.confidence)
        .bind(rule.hit_count)
        .bind(rule.is_active)
        .bind(rule.created_at.to_string())
        .bind(rule.updated_at.to_string())
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_create_and_list_for_rule() {
        let pool = setup_pool().await;
        let repo = RuleEvolutionLogRepo::new(pool.clone());

        insert_tracking_rule(&pool, "evo-rule-1").await;

        let entry1 = RuleEvolutionEntry {
            id: "evo-1".to_string(),
            rule_id: "evo-rule-1".to_string(),
            action: "created".to_string(),
            old_confidence: None,
            new_confidence: Some(0.5),
            old_category: None,
            new_category: Some("coding".to_string()),
            trigger_source: Some("ai_classification".to_string()),
            evidence_count: 3,
            created_at: "2026-03-09T10:00:00Z".to_string(),
        };
        let entry2 = RuleEvolutionEntry {
            id: "evo-2".to_string(),
            rule_id: "evo-rule-1".to_string(),
            action: "promoted".to_string(),
            old_confidence: Some(0.5),
            new_confidence: Some(0.8),
            old_category: Some("coding".to_string()),
            new_category: Some("coding".to_string()),
            trigger_source: Some("user_confirmation".to_string()),
            evidence_count: 10,
            created_at: "2026-03-09T12:00:00Z".to_string(),
        };

        repo.create(&entry1).await.unwrap();
        repo.create(&entry2).await.unwrap();

        let history = repo.list_for_rule("evo-rule-1").await.unwrap();
        assert_eq!(history.len(), 2);
        // Ordered DESC by created_at
        assert_eq!(history[0].action, "promoted");
        assert_eq!(history[1].action, "created");
    }

    #[tokio::test]
    async fn test_list_for_nonexistent_rule() {
        let pool = setup_pool().await;
        let repo = RuleEvolutionLogRepo::new(pool);

        let history = repo.list_for_rule("nonexistent").await.unwrap();
        assert!(history.is_empty());
    }
}
