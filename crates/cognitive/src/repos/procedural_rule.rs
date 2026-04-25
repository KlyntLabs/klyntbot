//! Repository for the `procedural_rules` table.

use sqlx::SqlitePool;

use crate::types::ProceduralRule;

/// Normalize a word: lowercase and strip common verb suffixes for basic stemming.
fn simple_stem(w: &str) -> String {
    let mut w = w.to_lowercase();
    if w.len() > 5 && w.ends_with("ed") {
        w.truncate(w.len() - 2);
    } else if w.len() > 5 && w.ends_with("ing") {
        w.truncate(w.len() - 3);
    }
    w
}

/// Compute word overlap ratio between two strings (Jaccard-like).
/// Applies lowercasing and simple stemming before comparison.
pub fn word_overlap_ratio(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<String> = a
        .split_whitespace()
        .map(|w| simple_stem(w.trim_matches(|c: char| !c.is_alphanumeric())))
        .filter(|w| w.len() > 2)
        .collect();
    let words_b: std::collections::HashSet<String> = b
        .split_whitespace()
        .map(|w| simple_stem(w.trim_matches(|c: char| !c.is_alphanumeric())))
        .filter(|w| w.len() > 2)
        .collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count() as f64;
    let union = words_a.union(&words_b).count() as f64;
    intersection / union
}

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
                signal_count, created_at, updated_at, active, project_id,
                effectiveness_score, stability, scope_repo_id, last_applied, application_count, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT (id) DO UPDATE SET
                rule_text = excluded.rule_text,
                confidence = excluded.confidence,
                signal_count = excluded.signal_count,
                updated_at = datetime('now'),
                active = excluded.active,
                project_id = excluded.project_id,
                effectiveness_score = excluded.effectiveness_score,
                stability = excluded.stability,
                scope_repo_id = excluded.scope_repo_id,
                last_applied = excluded.last_applied,
                application_count = excluded.application_count,
                metadata = excluded.metadata
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
        .bind(&rule.project_id)
        .bind(rule.effectiveness_score)
        .bind(rule.stability)
        .bind(&rule.scope_repo_id)
        .bind(&rule.last_applied)
        .bind(rule.application_count)
        .bind(&rule.metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a procedural rule (convenience alias for coding-memory phases).
    pub async fn insert(&self, rule: &ProceduralRule) -> Result<(), sqlx::Error> {
        self.upsert(rule).await
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

    /// Full-text search via FTS5.
    pub async fn search_fts(
        &self,
        query: &str,
        domain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ProceduralRule>, sqlx::Error> {
        let sql = r#"
            SELECT r.* FROM procedural_rules r
            INNER JOIN procedural_rules_fts fts ON r.id = fts.id
            WHERE procedural_rules_fts MATCH ?1
            AND (?2 IS NULL OR r.domain = ?2)
            AND r.active = 1
            ORDER BY fts.rank
            LIMIT ?3
        "#;
        sqlx::query_as::<_, ProceduralRule>(sql)
            .bind(query)
            .bind(domain)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
    }

    /// List all active rules across all domains.
    pub async fn list_all_active(&self) -> Result<Vec<ProceduralRule>, sqlx::Error> {
        sqlx::query_as::<_, ProceduralRule>(
            "SELECT * FROM procedural_rules WHERE active = 1 ORDER BY confidence DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Deactivate rules not updated in `days` days with fewer than `min_signals` signals.
    pub async fn deactivate_stale(&self, days: i64, min_signals: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE procedural_rules SET active = 0, updated_at = datetime('now')
             WHERE active = 1
             AND julianday('now') - julianday(updated_at) > ?1
             AND signal_count < ?2",
        )
        .bind(days)
        .bind(min_signals)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
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

    /// Find an active rule with similar text in the same domain via FTS5.
    pub async fn find_similar(
        &self,
        rule_text: &str,
        domain: &str,
    ) -> Result<Option<crate::types::ProceduralRule>, sqlx::Error> {
        // Build an OR query from significant words so FTS5 finds candidates even
        // when phrasing differs. The word_overlap_ratio check below applies the
        // real similarity threshold.
        let fts_query = rule_text
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 2)
            .collect::<Vec<_>>()
            .join(" OR ");
        if fts_query.is_empty() {
            return Ok(None);
        }
        let candidates = self.search_fts(&fts_query, Some(domain), 10).await?;
        for candidate in candidates {
            if word_overlap_ratio(&candidate.rule_text, rule_text) > 0.6 {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
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
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            effectiveness_score: 0.5,
            stability: 1.0,
            scope_repo_id: None,
            last_applied: None,
            application_count: 0,
            metadata: None,
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
    async fn test_list_all_active_across_domains() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r1 = test_rule("r1", "productivity", "Morning is peak time");
        let r2 = test_rule("r2", "finance", "Track daily expenses");
        let r3 = test_rule("r3", "productivity", "Break every 90 minutes");
        repo.upsert(&r1).await.unwrap();
        repo.upsert(&r2).await.unwrap();
        repo.upsert(&r3).await.unwrap();

        // Deactivate one to verify filtering
        repo.deactivate("r3").await.unwrap();

        let all_active = repo.list_all_active().await.unwrap();
        assert_eq!(all_active.len(), 2);
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

    #[test]
    fn test_word_overlap_high() {
        assert!(
            word_overlap_ratio(
                "User works best in mornings",
                "User works best in the mornings"
            ) > 0.6
        );
    }

    #[test]
    fn test_word_overlap_low() {
        assert!(word_overlap_ratio("User works best in mornings", "Track daily expenses") < 0.3);
    }

    #[test]
    fn test_word_overlap_empty() {
        assert!((word_overlap_ratio("", "anything") - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_find_similar_match() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);
        let r = test_rule(
            "r1",
            "productivity",
            "Suggest break after 90 minutes of focused work",
        );
        repo.upsert(&r).await.unwrap();
        let found = repo
            .find_similar(
                "Take a break after 90 minutes of focus work",
                "productivity",
            )
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "r1");
    }

    #[tokio::test]
    async fn test_find_similar_no_match() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);
        let r = test_rule("r1", "productivity", "Morning is peak time");
        repo.upsert(&r).await.unwrap();
        let found = repo
            .find_similar("Track daily expenses carefully", "productivity")
            .await
            .unwrap();
        assert!(found.is_none());
    }
}
