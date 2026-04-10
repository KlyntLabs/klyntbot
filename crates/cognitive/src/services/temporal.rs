//! Temporal reasoning service — fact history and change summaries.
//!
//! Provides time-oriented queries over the semantic fact store:
//! - Full version history for a given subject/predicate pair
//! - Change summaries over a time window (new, updated, superseded facts)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::repos::{FactChangelogRepo, SemanticFactRepo};
use crate::types::SemanticFact;

/// A fact together with a flag indicating whether it lives in the archive.
#[derive(Debug, Clone, Serialize)]
pub struct FactVersion {
    pub fact: SemanticFact,
    pub is_archived: bool,
}

/// A summary of memory changes over a time period.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeSummary {
    /// `(from, to)` timestamps for the period (ISO-8601 strings).
    pub period: (String, String),
    /// Facts first recorded during the period (not yet superseded).
    pub new_facts: Vec<SemanticFact>,
    /// Pairs of `(old_fact, new_fact)` where the old was superseded and
    /// the new superseder can be resolved.
    pub updated_facts: Vec<(SemanticFact, SemanticFact)>,
    /// Facts superseded during the period whose superseder could not be
    /// resolved (deleted / never written).
    pub superseded_facts: Vec<SemanticFact>,
    /// Count of changed facts per domain (new + updated + superseded).
    pub by_domain: HashMap<String, usize>,
}

/// A knowledge diff between two timestamps.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeDiff {
    pub period: (String, String),
    pub added: Vec<SemanticFact>,
    pub updated: Vec<(SemanticFact, SemanticFact)>,
    pub removed: Vec<SemanticFact>,
    pub by_domain: HashMap<String, usize>,
}

/// A decision point — a fact that changed value multiple times.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionPoint {
    pub subject: String,
    pub predicate: String,
    pub version_count: usize,
    pub latest_value: Option<String>,
    pub earliest_value: Option<String>,
}

/// Service for temporal queries over the semantic fact store.
#[derive(Clone)]
pub struct TemporalService {
    fact_repo: SemanticFactRepo,
    changelog_repo: Option<FactChangelogRepo>,
}

impl TemporalService {
    pub fn new(fact_repo: SemanticFactRepo) -> Self {
        Self {
            fact_repo,
            changelog_repo: None,
        }
    }

    pub fn with_changelog(mut self, repo: FactChangelogRepo) -> Self {
        self.changelog_repo = Some(repo);
        self
    }

    /// Return all versions of the fact identified by `subject` + `predicate`,
    /// both active and archived, sorted by `valid_from` descending (newest first).
    pub async fn get_fact_history(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<FactVersion>, sqlx::Error> {
        let active = self
            .fact_repo
            .find_by_subject_predicate(subject, predicate)
            .await?;
        let archived = self
            .fact_repo
            .search_archived_by_subject_predicate(subject, predicate)
            .await?;

        let mut versions: Vec<FactVersion> = active
            .into_iter()
            .map(|f| FactVersion {
                fact: f,
                is_archived: false,
            })
            .chain(archived.into_iter().map(|f| FactVersion {
                fact: f,
                is_archived: true,
            }))
            .collect();

        // Sort newest valid_from first (lexicographic works for ISO-8601 dates).
        versions.sort_by(|a, b| b.fact.valid_from.cmp(&a.fact.valid_from));

        Ok(versions)
    }

    /// Summarise memory changes that occurred on or after `since`.
    ///
    /// When `domains` is `Some`, only facts whose `domain` field is in the
    /// slice are considered.
    pub async fn change_summary(
        &self,
        since: DateTime<Utc>,
        domains: Option<&[&str]>,
    ) -> Result<ChangeSummary, sqlx::Error> {
        let since_str = since.to_rfc3339();
        let now_str = Utc::now().to_rfc3339();

        let new_facts = self
            .fact_repo
            .list_created_since(&since_str, domains)
            .await?;

        let superseded = self
            .fact_repo
            .list_superseded_since(&since_str, domains)
            .await?;

        // Build a lookup of new_facts by id for fast superseder resolution.
        let new_by_id: HashMap<&str, &SemanticFact> =
            new_facts.iter().map(|f| (f.id.as_str(), f)).collect();

        let mut updated_facts: Vec<(SemanticFact, SemanticFact)> = Vec::new();
        let mut superseded_facts: Vec<SemanticFact> = Vec::new();

        for old_fact in superseded {
            if let Some(new_id) = &old_fact.superseded_by {
                if let Some(&new_fact) = new_by_id.get(new_id.as_str()) {
                    updated_facts.push((old_fact, new_fact.clone()));
                } else {
                    // Superseder exists but wasn't created in this window — still
                    // report as an update if we can resolve it via the DB.
                    match self.fact_repo.get(new_id).await? {
                        Some(new_fact) => updated_facts.push((old_fact, new_fact)),
                        None => superseded_facts.push(old_fact),
                    }
                }
            } else {
                superseded_facts.push(old_fact);
            }
        }

        // Count changes per domain.
        let mut by_domain: HashMap<String, usize> = HashMap::new();
        for f in &new_facts {
            *by_domain.entry(f.domain.clone()).or_insert(0) += 1;
        }
        for (old, _new) in &updated_facts {
            *by_domain.entry(old.domain.clone()).or_insert(0) += 1;
        }
        for f in &superseded_facts {
            *by_domain.entry(f.domain.clone()).or_insert(0) += 1;
        }

        Ok(ChangeSummary {
            period: (since_str, now_str),
            new_facts,
            updated_facts,
            superseded_facts,
            by_domain,
        })
    }

    /// Return the state of a fact at a given point in time.
    pub async fn facts_as_of(
        &self,
        subject: &str,
        predicate: &str,
        as_of: &str,
    ) -> Result<Option<FactVersion>, sqlx::Error> {
        let history = self.get_fact_history(subject, predicate).await?;
        for version in &history {
            if version.fact.valid_from.as_str() <= as_of {
                if let Some(ref superseded_at) = version.fact.superseded_at {
                    if superseded_at.as_str() <= as_of {
                        continue;
                    }
                }
                return Ok(Some(version.clone()));
            }
        }
        Ok(None)
    }

    /// Find when a subject+predicate combination was first mentioned.
    pub async fn first_mention(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let history = self.get_fact_history(subject, predicate).await?;
        Ok(history.last().map(|v| v.fact.valid_from.clone()))
    }

    /// Find competing truths — active facts with the same subject+predicate
    /// but different objects.
    pub async fn competing_truths(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        self.fact_repo
            .find_by_subject_predicate(subject, predicate)
            .await
    }

    /// Compute a knowledge diff between two timestamps.
    pub async fn knowledge_diff(
        &self,
        from: &str,
        to: &str,
        domains: Option<&[&str]>,
    ) -> Result<KnowledgeDiff, sqlx::Error> {
        let from_dt = chrono::DateTime::parse_from_rfc3339(from)
            .map(|d| d.with_timezone(&Utc))
            .map_err(|_| {
                sqlx::Error::Protocol(format!("invalid RFC3339 'from' timestamp: {from}"))
            })?;

        let summary = self.change_summary(from_dt, domains).await?;

        Ok(KnowledgeDiff {
            period: (from.to_string(), to.to_string()),
            added: summary.new_facts,
            updated: summary.updated_facts,
            removed: summary.superseded_facts,
            by_domain: summary.by_domain,
        })
    }

    /// Find decision points — facts with multiple historical values.
    ///
    /// Looks back 180 days to avoid loading the entire superseded history.
    pub async fn decision_points(
        &self,
        domain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DecisionPoint>, sqlx::Error> {
        let since = (Utc::now() - chrono::Duration::days(180))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let domain_arr: Option<Vec<&str>> = domain.map(|d| vec![d]);
        let superseded = self
            .fact_repo
            .list_superseded_since(&since, domain_arr.as_deref().map(|s| s.as_ref()))
            .await?;

        let mut seen: HashMap<(String, String), Vec<SemanticFact>> = HashMap::new();
        for fact in superseded {
            seen.entry((fact.subject.clone(), fact.predicate.clone()))
                .or_default()
                .push(fact);
        }

        let mut points: Vec<DecisionPoint> = seen
            .into_iter()
            .filter(|(_, versions)| versions.len() >= 2)
            .map(|((subject, predicate), versions)| DecisionPoint {
                subject,
                predicate,
                version_count: versions.len(),
                latest_value: versions.first().map(|f| f.object.clone()),
                earliest_value: versions.last().map(|f| f.object.clone()),
            })
            .collect();

        points.sort_by(|a, b| b.version_count.cmp(&a.version_count));
        points.truncate(limit);
        Ok(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DEFAULT_MEMORY_TYPE;

    async fn setup() -> (sqlx::SqlitePool, TemporalService) {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = SemanticFactRepo::new(pool.clone());
        let service = TemporalService::new(repo);
        (pool, service)
    }

    fn make_fact(
        id: &str,
        domain: &str,
        subject: &str,
        predicate: &str,
        valid_from: &str,
        recorded_at: &str,
    ) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: domain.into(),
            subject: subject.into(),
            predicate: predicate.into(),
            object: "value".into(),
            confidence: 0.8,
            source: "test".into(),
            valid_from: valid_from.into(),
            valid_until: None,
            recorded_at: recorded_at.into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
        }
    }

    #[tokio::test]
    async fn test_fact_history_returns_active_facts() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let f1 = make_fact(
            "h1",
            "productivity",
            "user",
            "peak_hours",
            "2026-01-01",
            "2026-01-01",
        );
        let f2 = make_fact(
            "h2",
            "productivity",
            "user",
            "peak_hours",
            "2026-02-01",
            "2026-02-01",
        );

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let history = service
            .get_fact_history("user", "peak_hours")
            .await
            .unwrap();

        assert_eq!(history.len(), 2, "expected 2 versions in history");

        // Sorted newest first by valid_from.
        assert_eq!(history[0].fact.id, "h2");
        assert_eq!(history[1].fact.id, "h1");

        // Both are active (not archived).
        assert!(!history[0].is_archived);
        assert!(!history[1].is_archived);
    }

    #[tokio::test]
    async fn test_change_summary_counts_new_facts() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        // Use a recorded_at that is clearly in the past but after our `since`.
        let fact = make_fact(
            "cs1",
            "productivity",
            "user",
            "peak_hours",
            "2026-03-10",
            "2026-03-10T00:00:00Z",
        );
        repo.upsert(&fact).await.unwrap();

        // Query since one week before the fact was recorded.
        let since = DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let summary = service.change_summary(since, None).await.unwrap();

        assert!(
            !summary.new_facts.is_empty(),
            "expected at least one new fact"
        );
        assert!(
            summary.new_facts.iter().any(|f| f.id == "cs1"),
            "cs1 should appear in new_facts"
        );
        assert!(
            summary.by_domain.contains_key("productivity"),
            "productivity domain should be counted"
        );
    }

    #[tokio::test]
    async fn test_facts_as_of() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let f1 = make_fact("asof1", "work", "user", "role", "2026-01-01", "2026-01-01");
        repo.upsert(&f1).await.unwrap();

        let result = service
            .facts_as_of("user", "role", "2026-06-01")
            .await
            .unwrap();
        assert!(result.is_some(), "Should find fact valid at 2026-06-01");

        let result = service
            .facts_as_of("user", "role", "2025-01-01")
            .await
            .unwrap();
        assert!(result.is_none(), "Should not find fact before valid_from");
    }

    #[tokio::test]
    async fn test_first_mention() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let f1 = make_fact("fm1", "work", "user", "lang", "2026-01-15", "2026-01-15");
        let f2 = make_fact("fm2", "work", "user", "lang", "2026-03-01", "2026-03-01");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let mention = service.first_mention("user", "lang").await.unwrap();
        assert_eq!(mention, Some("2026-01-15".to_string()));
    }

    #[tokio::test]
    async fn test_competing_truths() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        let mut f1 = make_fact(
            "ct1",
            "work",
            "user",
            "fav_lang",
            "2026-01-01",
            "2026-01-01",
        );
        f1.object = "Python".to_string();
        let mut f2 = make_fact(
            "ct2",
            "work",
            "user",
            "fav_lang",
            "2026-02-01",
            "2026-02-01",
        );
        f2.object = "Rust".to_string();
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let truths = service.competing_truths("user", "fav_lang").await.unwrap();
        assert_eq!(truths.len(), 2, "Should find 2 competing truths");
    }

    #[tokio::test]
    async fn test_change_summary_empty_when_no_changes() {
        let (_pool, service) = setup().await;
        let repo = service.fact_repo.clone();

        // Insert a fact with an old recorded_at.
        let fact = make_fact(
            "old1",
            "productivity",
            "user",
            "break_pattern",
            "2025-01-01",
            "2025-01-01T00:00:00Z",
        );
        repo.upsert(&fact).await.unwrap();

        // Query since a future date — nothing should appear.
        let since = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let summary = service.change_summary(since, None).await.unwrap();

        assert!(summary.new_facts.is_empty(), "expected no new facts");
        assert!(
            summary.updated_facts.is_empty(),
            "expected no updated facts"
        );
        assert!(
            summary.superseded_facts.is_empty(),
            "expected no superseded facts"
        );
        assert!(summary.by_domain.is_empty(), "expected empty domain map");
    }
}
