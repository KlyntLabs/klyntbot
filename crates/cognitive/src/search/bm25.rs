//! BM25 full-text search across cognitive memory tables via SQLite FTS5.

use sqlx::SqlitePool;

/// A BM25 search result with its FTS5 rank score.
#[derive(Debug, Clone)]
pub struct Bm25Result {
    pub id: String,
    /// FTS5 rank (negative BM25 — lower = better match). Negated here so higher = better.
    pub score: f64,
    /// Which table this result came from.
    pub source_table: &'static str,
}

/// Search semantic_facts via FTS5.
pub async fn search_semantic_facts(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM semantic_facts_fts fts
        INNER JOIN semantic_facts f ON f.id = fts.id
        WHERE semantic_facts_fts MATCH ?1
        AND (?2 IS NULL OR f.domain = ?2)
        AND f.superseded_at IS NULL
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "semantic_facts",
                })
                .collect()
        })
}

/// Search episodic_memories via FTS5.
pub async fn search_episodic_memories(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM episodic_memories_fts fts
        INNER JOIN episodic_memories e ON e.id = fts.id
        WHERE episodic_memories_fts MATCH ?1
        AND (?2 IS NULL OR e.domain = ?2)
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "episodic_memories",
                })
                .collect()
        })
}

/// Search procedural_rules via FTS5.
pub async fn search_procedural_rules(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM procedural_rules_fts fts
        INNER JOIN procedural_rules r ON r.id = fts.id
        WHERE procedural_rules_fts MATCH ?1
        AND (?2 IS NULL OR r.domain = ?2)
        AND r.active = 1
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "procedural_rules",
                })
                .collect()
        })
}

/// Unified BM25 search across all cognitive tables.
/// Returns results sorted by score descending, limited to `limit` total.
pub async fn bm25_search_all(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    // Query all three tables with per-table limit, then merge
    let per_table_limit = limit * 2; // over-fetch to get good candidates
    let (facts, episodes, rules) = tokio::try_join!(
        search_semantic_facts(pool, query, domain, per_table_limit),
        search_episodic_memories(pool, query, domain, per_table_limit),
        search_procedural_rules(pool, query, domain, per_table_limit),
    )?;

    let mut all: Vec<Bm25Result> = Vec::with_capacity(facts.len() + episodes.len() + rules.len());
    all.extend(facts);
    all.extend(episodes);
    all.extend(rules);

    // Sort by score descending
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(limit);

    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::EpisodicMemoryRepo;
    use crate::repos::ProceduralRuleRepo;
    use crate::repos::SemanticFactRepo;
    use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact};

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: "fact".to_string(),
        }
    }

    fn test_memory(id: &str, content: &str) -> EpisodicMemory {
        EpisodicMemory {
            id: id.into(),
            domain: "productivity".into(),
            content: content.into(),
            summary: None,
            importance: 0.5,
            occurred_at: "2026-03-06T10:00:00".into(),
            recorded_at: "2026-03-06T10:00:00".into(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
        }
    }

    fn test_rule(id: &str, rule_text: &str) -> ProceduralRule {
        ProceduralRule {
            id: id.into(),
            domain: "productivity".into(),
            rule_text: rule_text.into(),
            confidence: 0.8,
            source: "reflection".into(),
            signal_count: 1,
            created_at: "2026-03-06".into(),
            updated_at: "2026-03-06".into(),
            active: true,
            project_id: None,
        }
    }

    #[tokio::test]
    async fn test_fts5_semantic_facts_basic_search() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", "morning 9-11am"))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", "every 90 minutes"))
            .await
            .unwrap();
        repo.upsert(&test_fact("f3", "coffee_preference", "black espresso"))
            .await
            .unwrap();

        let results = search_semantic_facts(&pool, "morning hours", None, 10)
            .await
            .unwrap();
        assert!(!results.is_empty(), "Should find facts matching 'morning hours'");
        assert_eq!(results[0].source_table, "semantic_facts");
    }

    #[tokio::test]
    async fn test_fts5_semantic_facts_domain_filter() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        let mut f1 = test_fact("f1", "peak_hours", "morning");
        f1.domain = "productivity".into();
        let mut f2 = test_fact("f2", "budget", "morning routine costs");
        f2.domain = "finance".into();

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = search_semantic_facts(&pool, "morning", Some("productivity"), 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");
    }

    #[tokio::test]
    async fn test_fts5_excludes_superseded_facts() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", "morning 9am"))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "peak_hours", "morning 10am"))
            .await
            .unwrap();
        repo.supersede("f1", "f2").await.unwrap();

        let results = search_semantic_facts(&pool, "morning peak", None, 10)
            .await
            .unwrap();
        // f1 is superseded, only f2 should appear
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert!(!ids.contains(&"f1"), "Superseded fact should be excluded");
    }

    #[tokio::test]
    async fn test_fts5_episodic_memories_search() {
        let pool = setup().await;
        let repo = EpisodicMemoryRepo::new(pool.clone());

        repo.insert(&test_memory(
            "e1",
            "Had a productive morning coding session on the Rust project",
        ))
        .await
        .unwrap();
        repo.insert(&test_memory("e2", "Went for a walk in the afternoon park"))
            .await
            .unwrap();

        let results = search_episodic_memories(&pool, "coding Rust", None, 10)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "e1");
    }

    #[tokio::test]
    async fn test_fts5_procedural_rules_search() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool.clone());

        repo.upsert(&test_rule(
            "r1",
            "When user mentions deadlines, check task priorities first",
        ))
        .await
        .unwrap();
        repo.upsert(&test_rule("r2", "Always confirm before deleting any data"))
            .await
            .unwrap();

        let results = search_procedural_rules(&pool, "deadline priorities", None, 10)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "r1");
    }

    #[tokio::test]
    async fn test_fts5_excludes_inactive_rules() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool.clone());

        let mut rule = test_rule("r1", "Some old inactive rule about deadlines");
        rule.active = false;
        repo.upsert(&rule).await.unwrap();

        let results = search_procedural_rules(&pool, "deadlines", None, 10)
            .await
            .unwrap();
        assert!(results.is_empty(), "Inactive rules should be excluded");
    }

    #[tokio::test]
    async fn test_bm25_search_all_merges_tables() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episode_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        fact_repo
            .upsert(&test_fact("f1", "morning routine", "coffee then code"))
            .await
            .unwrap();
        episode_repo
            .insert(&test_memory("e1", "Great morning coding with coffee"))
            .await
            .unwrap();
        rule_repo
            .upsert(&test_rule(
                "r1",
                "Morning coding sessions are most productive",
            ))
            .await
            .unwrap();

        let results = bm25_search_all(&pool, "morning coding", None, 10)
            .await
            .unwrap();
        assert!(
            results.len() >= 2,
            "Should find results across multiple tables"
        );

        let tables: Vec<&str> = results.iter().map(|r| r.source_table).collect();
        let unique_tables: std::collections::HashSet<&str> = tables.into_iter().collect();
        assert!(
            unique_tables.len() >= 2,
            "Results should come from multiple tables"
        );
    }

    #[tokio::test]
    async fn test_bm25_search_respects_limit() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        for i in 0..10 {
            repo.upsert(&test_fact(&format!("f{i}"), "coding", &format!("project {i}")))
                .await
                .unwrap();
        }

        let results = bm25_search_all(&pool, "coding project", None, 3)
            .await
            .unwrap();
        assert!(results.len() <= 3);
    }

    #[tokio::test]
    async fn test_bm25_empty_query_returns_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());
        repo.upsert(&test_fact("f1", "peak_hours", "morning"))
            .await
            .unwrap();

        // FTS5 MATCH with empty string should error or return empty
        let results = search_semantic_facts(&pool, "", None, 10).await;
        // Either returns empty or errors — both acceptable
        assert!(results.is_err() || results.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fts5_porter_stemming() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "running", "daily morning runs"))
            .await
            .unwrap();

        // "run" should match "running" and "runs" via porter stemmer
        let results = search_semantic_facts(&pool, "run", None, 10)
            .await
            .unwrap();
        assert!(
            !results.is_empty(),
            "Porter stemmer should match 'run' to 'running'/'runs'"
        );
    }
}
