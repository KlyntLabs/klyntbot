use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{IntelligenceSessionType, MatchMode, RuleSource, RuleType, TrackingRule};

#[derive(sqlx::FromRow)]
struct TrackingRuleRow {
    id: String,
    rule_type: String,
    match_field: String,
    match_pattern: String,
    match_mode: String,
    category: String,
    session_type: String,
    priority: i32,
    source: String,
    confidence: f64,
    hit_count: i64,
    last_hit_at: Option<String>,
    is_active: bool,
    created_at: String,
    updated_at: String,
}

impl From<TrackingRuleRow> for TrackingRule {
    fn from(row: TrackingRuleRow) -> Self {
        let rule_type = row.rule_type.parse::<RuleType>().unwrap_or_else(|_| {
            warn!(raw = %row.rule_type, "unknown rule_type in DB, defaulting to App");
            RuleType::App
        });
        let match_mode = row.match_mode.parse::<MatchMode>().unwrap_or_else(|_| {
            warn!(raw = %row.match_mode, "unknown match_mode in DB, defaulting to Exact");
            MatchMode::Exact
        });
        let session_type = row
            .session_type
            .parse::<IntelligenceSessionType>()
            .unwrap_or_else(|_| {
                warn!(raw = %row.session_type, "unknown session_type in DB, defaulting to Focus");
                IntelligenceSessionType::Focus
            });
        let source = row.source.parse::<RuleSource>().unwrap_or_else(|_| {
            warn!(raw = %row.source, "unknown source in DB, defaulting to System");
            RuleSource::System
        });
        let last_hit_at = row
            .last_hit_at
            .as_deref()
            .and_then(|s| common::parse_datetime(s, "UTC"));
        let created_at =
            common::parse_datetime(&row.created_at, "UTC").unwrap_or_else(chrono::Utc::now);
        let updated_at =
            common::parse_datetime(&row.updated_at, "UTC").unwrap_or_else(chrono::Utc::now);

        Self {
            id: row.id,
            rule_type,
            match_field: row.match_field,
            match_pattern: row.match_pattern,
            match_mode,
            category: row.category,
            session_type,
            priority: row.priority,
            source,
            confidence: row.confidence,
            hit_count: row.hit_count,
            last_hit_at,
            is_active: row.is_active,
            created_at,
            updated_at,
        }
    }
}

const COLUMNS: &str = "id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count, last_hit_at, is_active, created_at, updated_at";

#[derive(Debug, Clone)]
pub struct TrackingRuleRepo {
    pool: SqlitePool,
}

impl TrackingRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_active(&self) -> common::Result<Vec<TrackingRule>> {
        let rows = sqlx::query_as::<_, TrackingRuleRow>(&format!(
            "SELECT {COLUMNS} FROM productivity_tracking_rules WHERE is_active = TRUE ORDER BY priority ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(TrackingRule::from).collect())
    }

    pub async fn get(&self, id: &str) -> common::Result<Option<TrackingRule>> {
        let row = sqlx::query_as::<_, TrackingRuleRow>(&format!(
            "SELECT {COLUMNS} FROM productivity_tracking_rules WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(TrackingRule::from))
    }

    pub async fn create(&self, rule: &TrackingRule) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_tracking_rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count, last_hit_at, is_active, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
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
        .bind(rule.last_hit_at)
        .bind(rule.is_active)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn update(&self, rule: &TrackingRule) -> common::Result<()> {
        sqlx::query(
            r#"UPDATE productivity_tracking_rules SET
                   rule_type = ?2, match_field = ?3, match_pattern = ?4, match_mode = ?5,
                   category = ?6, session_type = ?7, priority = ?8, source = ?9,
                   confidence = ?10, hit_count = ?11, last_hit_at = ?12, is_active = ?13,
                   updated_at = ?14
               WHERE id = ?1"#,
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
        .bind(rule.last_hit_at)
        .bind(rule.is_active)
        .bind(rule.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn upsert(&self, rule: &TrackingRule) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_tracking_rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source, confidence, hit_count, last_hit_at, is_active, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
               ON CONFLICT(id) DO UPDATE SET
                   rule_type = excluded.rule_type, match_field = excluded.match_field,
                   match_pattern = excluded.match_pattern, match_mode = excluded.match_mode,
                   category = excluded.category, session_type = excluded.session_type,
                   priority = excluded.priority, source = excluded.source,
                   confidence = excluded.confidence, hit_count = excluded.hit_count,
                   last_hit_at = excluded.last_hit_at, is_active = excluded.is_active,
                   updated_at = excluded.updated_at"#,
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
        .bind(rule.last_hit_at)
        .bind(rule.is_active)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn increment_hit(&self, id: &str) -> common::Result<()> {
        let now = chrono::Utc::now();
        sqlx::query(
            "UPDATE productivity_tracking_rules SET hit_count = hit_count + 1, last_hit_at = ?2, updated_at = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_by_source(&self, source: &str) -> common::Result<Vec<TrackingRule>> {
        let rows = sqlx::query_as::<_, TrackingRuleRow>(&format!(
            "SELECT {COLUMNS} FROM productivity_tracking_rules WHERE source = ?1 ORDER BY priority ASC"
        ))
        .bind(source)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(TrackingRule::from).collect())
    }

    pub async fn deactivate(&self, id: &str) -> common::Result<()> {
        let now = chrono::Utc::now();
        sqlx::query(
            "UPDATE productivity_tracking_rules SET is_active = FALSE, updated_at = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(now)
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
    use chrono::Utc;

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

    fn sample_rule(id: &str) -> TrackingRule {
        let now = Utc::now();
        TrackingRule {
            id: id.to_string(),
            rule_type: RuleType::App,
            match_field: "app_name".to_string(),
            match_pattern: "TestApp".to_string(),
            match_mode: MatchMode::Exact,
            category: "coding".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 10,
            source: RuleSource::User,
            confidence: 0.9,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let pool = setup_pool().await;
        let repo = TrackingRuleRepo::new(pool);

        let rule = sample_rule("test-rule-1");
        repo.create(&rule).await.unwrap();

        let fetched = repo.get("test-rule-1").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.match_pattern, "TestApp");
        assert_eq!(fetched.rule_type, RuleType::App);
        assert_eq!(fetched.source, RuleSource::User);
    }

    #[tokio::test]
    async fn test_list_active() {
        let pool = setup_pool().await;
        let repo = TrackingRuleRepo::new(pool);

        // Migration seeds system rules, so there should already be active rules
        let active = repo.list_active().await.unwrap();
        assert!(!active.is_empty());

        // Add a custom active rule
        let rule = sample_rule("custom-active-1");
        repo.create(&rule).await.unwrap();

        let active_after = repo.list_active().await.unwrap();
        assert_eq!(active_after.len(), active.len() + 1);
    }

    #[tokio::test]
    async fn test_increment_hit() {
        let pool = setup_pool().await;
        let repo = TrackingRuleRepo::new(pool);

        let rule = sample_rule("hit-test-1");
        repo.create(&rule).await.unwrap();

        repo.increment_hit("hit-test-1").await.unwrap();
        repo.increment_hit("hit-test-1").await.unwrap();

        let fetched = repo.get("hit-test-1").await.unwrap().unwrap();
        assert_eq!(fetched.hit_count, 2);
        assert!(fetched.last_hit_at.is_some());
    }

    #[tokio::test]
    async fn test_deactivate() {
        let pool = setup_pool().await;
        let repo = TrackingRuleRepo::new(pool);

        let rule = sample_rule("deact-test-1");
        repo.create(&rule).await.unwrap();

        let before = repo.list_active().await.unwrap();
        let before_count = before.len();

        repo.deactivate("deact-test-1").await.unwrap();

        let after = repo.list_active().await.unwrap();
        assert_eq!(after.len(), before_count - 1);

        let fetched = repo.get("deact-test-1").await.unwrap().unwrap();
        assert!(!fetched.is_active);
    }
}
