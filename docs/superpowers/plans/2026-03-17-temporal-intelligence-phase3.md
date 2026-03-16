# Temporal Intelligence (Phase 3) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add temporal intelligence to the cognitive memory system — fact history tracking, contradiction detection in the consolidation pipeline, change summaries, and a temporal recency weight in retrieval scoring.

**Architecture:** A new `TemporalService` in the cognitive crate (L5) provides three capabilities: fact history queries (active + archive), structured change summaries (LLM-free), and a contradiction detection hook in `BackgroundConsolidationService`. The existing 5-factor retrieval scoring formula in `decay.rs` gains a 6th temporal recency factor. No new tables needed — temporal data already lives in `semantic_facts.valid_from` and `semantic_facts_archive`.

**Tech Stack:** Rust (SQLite, chrono), no new dependencies.

**Spec:** `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` (§4: Temporal Intelligence)

**Depends on:** Phase 0+1 (complete) — EntityRepo, SemanticFactRepo with archive methods, ContradictionDetected domain event.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/services/temporal.rs` | TemporalService: fact history, change summaries |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/services/mod.rs` | Register temporal module |
| `crates/cognitive/src/services/decay.rs` | Add temporal_score to 6-factor formula (RelevanceWeights + relevance_score) |
| `crates/cognitive/src/services/retrieval.rs` | Add temporal weight to RetrievalParams, pass valid_from to scoring |
| `crates/cognitive/src/services/background.rs` | Add contradiction detection after execute_memory_ops() |
| `crates/cognitive/src/lib.rs` | Re-export TemporalService |

---

## Chunk 1: TemporalService + Scoring

### Task 1: TemporalService — Fact History

**Files:**
- Create: `crates/cognitive/src/services/temporal.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Create TemporalService with fact_history and types**

Create `crates/cognitive/src/services/temporal.rs`:

```rust
//! Temporal intelligence: fact history, change summaries, temporal scoring.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::repos::SemanticFactRepo;
use crate::types::SemanticFact;

/// A versioned snapshot of a fact — current, superseded, or archived.
#[derive(Debug, Clone, Serialize)]
pub struct FactVersion {
    pub fact: SemanticFact,
    pub is_archived: bool,
}

/// Structured summary of knowledge changes over a time period.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeSummary {
    pub period: (String, String),
    pub new_facts: Vec<SemanticFact>,
    pub updated_facts: Vec<(SemanticFact, SemanticFact)>, // (old, new)
    pub superseded_facts: Vec<SemanticFact>,
    pub by_domain: std::collections::HashMap<String, usize>,
}

/// Temporal intelligence service — queries fact evolution over time.
#[derive(Clone)]
pub struct TemporalService {
    fact_repo: SemanticFactRepo,
}

impl TemporalService {
    pub fn new(fact_repo: SemanticFactRepo) -> Self {
        Self { fact_repo }
    }

    /// Get the full history of a subject+predicate pair across active and archived facts.
    /// Returns versions ordered by valid_from DESC (newest first).
    pub async fn get_fact_history(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<FactVersion>, sqlx::Error> {
        // Query active facts matching subject+predicate
        let active = self
            .fact_repo
            .find_by_subject_predicate(subject, predicate)
            .await?;

        // Query archived facts matching subject+predicate
        let archived = self
            .fact_repo
            .search_archived_by_subject_predicate(subject, predicate)
            .await?;

        let mut versions: Vec<FactVersion> = Vec::with_capacity(active.len() + archived.len());

        for fact in active {
            versions.push(FactVersion {
                fact,
                is_archived: false,
            });
        }
        for fact in archived {
            versions.push(FactVersion {
                fact,
                is_archived: true,
            });
        }

        // Sort by valid_from DESC (newest first)
        versions.sort_by(|a, b| b.fact.valid_from.cmp(&a.fact.valid_from));

        Ok(versions)
    }

    /// Generate a structured change summary for a given time period.
    /// No LLM call — pure SQL queries.
    pub async fn change_summary(
        &self,
        since: DateTime<Utc>,
        domains: Option<&[&str]>,
    ) -> Result<ChangeSummary, sqlx::Error> {
        let since_str = since.to_rfc3339();
        let now_str = Utc::now().to_rfc3339();

        // New facts: recorded_at >= since, no superseded_at
        let new_facts = self
            .fact_repo
            .list_created_since(&since_str, domains)
            .await?;

        // Superseded facts: superseded_at >= since
        let superseded_facts = self
            .fact_repo
            .list_superseded_since(&since_str, domains)
            .await?;

        // Updated facts: find pairs where old was superseded and new was created in the period
        let mut updated_facts = Vec::new();
        for old in &superseded_facts {
            if let Some(ref new_id) = old.superseded_by {
                if let Ok(Some(new_fact)) = self.fact_repo.get(new_id).await {
                    updated_facts.push((old.clone(), new_fact));
                }
            }
        }

        // Count by domain
        let mut by_domain = std::collections::HashMap::new();
        for f in &new_facts {
            *by_domain.entry(f.domain.clone()).or_insert(0) += 1;
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
}
```

- [ ] **Step 2: Register module**

In `crates/cognitive/src/services/mod.rs`, add:
```rust
pub mod temporal;
```

- [ ] **Step 3: Add missing SemanticFactRepo query methods**

The TemporalService needs 3 new query methods on SemanticFactRepo. In `crates/cognitive/src/repos/semantic_fact.rs`, add:

```rust
    /// Find active facts by exact subject + predicate match.
    pub async fn find_by_subject_predicate(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE subject = ?1 AND predicate = ?2 ORDER BY valid_from DESC",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(&self.pool)
        .await
    }

    /// Search archived facts by exact subject + predicate match.
    pub async fn search_archived_by_subject_predicate(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT id, domain, subject, predicate, object, confidence, source, valid_from, valid_until, recorded_at, superseded_at, superseded_by, stability, last_accessed, access_count, project_id, memory_type FROM semantic_facts_archive WHERE subject = ?1 AND predicate = ?2 ORDER BY valid_from DESC",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(&self.pool)
        .await
    }

    /// List facts created since a given timestamp, optionally filtered by domains.
    pub async fn list_created_since(
        &self,
        since: &str,
        domains: Option<&[&str]>,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        match domains {
            Some(ds) if !ds.is_empty() => {
                let placeholders: Vec<String> = (0..ds.len()).map(|i| format!("?{}", i + 2)).collect();
                let sql = format!(
                    "SELECT * FROM semantic_facts WHERE recorded_at >= ?1 AND superseded_at IS NULL AND domain IN ({}) ORDER BY recorded_at DESC",
                    placeholders.join(", ")
                );
                let mut query = sqlx::query_as::<_, SemanticFact>(&sql).bind(since);
                for d in ds {
                    query = query.bind(*d);
                }
                query.fetch_all(&self.pool).await
            }
            _ => {
                sqlx::query_as::<_, SemanticFact>(
                    "SELECT * FROM semantic_facts WHERE recorded_at >= ?1 AND superseded_at IS NULL ORDER BY recorded_at DESC",
                )
                .bind(since)
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    /// List facts superseded since a given timestamp, optionally filtered by domains.
    pub async fn list_superseded_since(
        &self,
        since: &str,
        domains: Option<&[&str]>,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        match domains {
            Some(ds) if !ds.is_empty() => {
                let placeholders: Vec<String> = (0..ds.len()).map(|i| format!("?{}", i + 2)).collect();
                let sql = format!(
                    "SELECT * FROM semantic_facts WHERE superseded_at >= ?1 AND domain IN ({}) ORDER BY superseded_at DESC",
                    placeholders.join(", ")
                );
                let mut query = sqlx::query_as::<_, SemanticFact>(&sql).bind(since);
                for d in ds {
                    query = query.bind(*d);
                }
                query.fetch_all(&self.pool).await
            }
            _ => {
                sqlx::query_as::<_, SemanticFact>(
                    "SELECT * FROM semantic_facts WHERE superseded_at >= ?1 ORDER BY superseded_at DESC",
                )
                .bind(since)
                .fetch_all(&self.pool)
                .await
            }
        }
    }
```

- [ ] **Step 4: Add tests to temporal.rs**

Append to `temporal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::SemanticFactRepo;
    use crate::types::{SemanticFact, DEFAULT_MEMORY_TYPE};

    async fn setup() -> (SemanticFactRepo, TemporalService) {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = SemanticFactRepo::new(pool);
        let service = TemporalService::new(repo.clone());
        (repo, service)
    }

    fn make_fact(id: &str, subject: &str, predicate: &str, object: &str, valid_from: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "work".into(),
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "user_stated".into(),
            valid_from: valid_from.into(),
            valid_until: None,
            recorded_at: valid_from.into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
        }
    }

    #[tokio::test]
    async fn test_fact_history_returns_active_facts() {
        let (repo, service) = setup().await;
        repo.upsert(&make_fact("f1", "user", "prefers_db", "PostgreSQL", "2026-03-01")).await.unwrap();
        repo.upsert(&make_fact("f2", "user", "prefers_db", "DynamoDB", "2026-02-01")).await.unwrap();

        let history = service.get_fact_history("user", "prefers_db").await.unwrap();
        assert_eq!(history.len(), 2);
        // Newest first
        assert_eq!(history[0].fact.object, "PostgreSQL");
        assert!(!history[0].is_archived);
    }

    #[tokio::test]
    async fn test_change_summary_counts_new_facts() {
        let (repo, service) = setup().await;
        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        repo.upsert(&make_fact("f1", "user", "role", "engineer", &Utc::now().to_rfc3339())).await.unwrap();

        let summary = service.change_summary(since, None).await.unwrap();
        assert_eq!(summary.new_facts.len(), 1);
        assert!(summary.by_domain.contains_key("work"));
    }

    #[tokio::test]
    async fn test_change_summary_empty_when_no_changes() {
        let (_repo, service) = setup().await;
        let since = chrono::Utc::now() + chrono::Duration::hours(1);
        let summary = service.change_summary(since, None).await.unwrap();
        assert!(summary.new_facts.is_empty());
        assert!(summary.superseded_facts.is_empty());
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(temporal)'`
Expected: all 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/temporal.rs crates/cognitive/src/services/mod.rs crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): add TemporalService with fact history and change summaries"
```

---

### Task 2: Add Temporal Recency to Scoring Formula

**Files:**
- Modify: `crates/cognitive/src/services/decay.rs`
- Modify: `crates/cognitive/src/services/retrieval.rs`

- [ ] **Step 1: Extend RelevanceWeights and relevance_score**

In `crates/cognitive/src/services/decay.rs`:

1. Add `temporal` field to `RelevanceWeights`:
```rust
pub struct RelevanceWeights {
    pub semantic: f64,
    pub retrievability: f64,
    pub importance: f64,
    pub frequency: f64,
    pub situation: f64,
    pub temporal: f64,  // NEW: recency weight
}
```

2. Update `Default` impl — steal 0.05 from `situation` (0.25 → 0.20):
```rust
impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            semantic: 0.3,
            retrievability: 0.2,
            importance: 0.15,
            frequency: 0.1,
            situation: 0.20,  // was 0.25
            temporal: 0.05,   // NEW
        }
    }
}
```

3. Add `temporal_recency` parameter to `relevance_score`:
```rust
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
    temporal_recency: f64,  // NEW
    weights: &RelevanceWeights,
) -> f64 {
    (semantic_similarity * weights.semantic
        + retrievability * weights.retrievability
        + importance * weights.importance
        + access_frequency * weights.frequency
        + situational_boost * weights.situation
        + temporal_recency * weights.temporal)  // NEW
        .clamp(0.0, 1.0)
}
```

4. Add the `temporal_recency_score` helper function:
```rust
/// Compute temporal recency score from a valid_from timestamp string.
/// Returns a value between 0.1 and 1.0 — recent facts score higher.
/// Uses an inverse decay: 1 / (1 + age_days / 30).
pub fn temporal_recency_score(valid_from: &str) -> f64 {
    let now = chrono::Utc::now().naive_utc();
    let age_days = valid_from
        .parse::<chrono::NaiveDateTime>()
        .or_else(|_| chrono::NaiveDate::parse_from_str(valid_from, "%Y-%m-%d").map(|d| d.and_hms_opt(0, 0, 0).unwrap()))
        .map(|vf| (now - vf).num_days().max(0) as f64)
        .unwrap_or(30.0);
    (1.0 / (1.0 + age_days / 30.0)).max(0.1)
}
```

- [ ] **Step 2: Update all relevance_score call sites in decay.rs tests**

Update the test constants and calls to pass the new `temporal_recency` parameter (use `0.5` as a neutral value in existing tests):
- `test_relevance_score_combines_factors`: add `0.5` argument
- `test_relevance_score_clamps`: add `1.0` and `0.0` respectively
- `test_relevance_score_custom_weights`: add `temporal: 0.0` to custom weights, add `0.0` argument

- [ ] **Step 3: Update retrieval.rs**

In `crates/cognitive/src/services/retrieval.rs`:

1. Add `relevance_weight_temporal` to `RetrievalParams`:
```rust
pub struct RetrievalParams {
    // ... existing fields ...
    pub relevance_weight_temporal: f64,  // NEW
}
```

Update `RetrievalParams::new()` default:
```rust
relevance_weight_temporal: 0.05,
```

2. Update `RelevanceWeights` construction (around line 85-91) to include the new field:
```rust
let weights = RelevanceWeights {
    semantic: params.relevance_weight_semantic,
    retrievability: params.relevance_weight_retrievability,
    importance: params.relevance_weight_importance,
    frequency: params.relevance_weight_frequency,
    situation: params.relevance_weight_situation,
    temporal: params.relevance_weight_temporal,  // NEW
};
```

3. In `vector_path` (around line 198), add temporal score to `relevance_score` call:
```rust
let temporal = crate::decay::temporal_recency_score(&fact.valid_from);
let score = relevance_score(
    similarity,
    r,
    fact.confidence,
    freq,
    situational_boost,
    temporal,  // NEW
    weights,
);
```

4. In `fallback_path` (around line 234), same change:
```rust
let temporal = crate::decay::temporal_recency_score(&fact.valid_from);
let score = relevance_score(0.5, r, fact.confidence, freq, situational_boost, temporal, weights);
```

- [ ] **Step 4: Update default_params in retrieval tests**

The `default_params` helper and test weight constants need to be updated. In existing retrieval tests, add `relevance_weight_temporal: 0.05` to `RetrievalParams::new()`.

- [ ] **Step 5: Add temporal scoring test**

Add to `decay.rs` tests:

```rust
#[test]
fn test_temporal_recency_score_recent() {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let score = temporal_recency_score(&now);
    assert!(score > 0.95, "Today's fact should score near 1.0");
}

#[test]
fn test_temporal_recency_score_old() {
    let old = "2020-01-01T00:00:00";
    let score = temporal_recency_score(old);
    assert!(score < 0.2, "Very old fact should score low");
    assert!(score >= 0.1, "Score should be at least 0.1");
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p cognitive`
Expected: all tests pass (existing + new temporal tests).

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p cognitive --all-targets`
Expected: 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/decay.rs crates/cognitive/src/services/retrieval.rs
git commit -m "feat(cognitive): add temporal recency to 6-factor retrieval scoring"
```

---

## Chunk 2: Contradiction Detection + Re-exports

### Task 3: Contradiction Detection in BackgroundConsolidationService

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

The `ContradictionDetected` domain event already exists (added in Phase 0). The consolidation service already calls `execute_memory_ops()` and has access to the domain event bus. We need to add contradiction detection logic AFTER `execute_memory_ops()` completes.

- [ ] **Step 1: Add contradiction detection after execute_memory_ops**

In `crates/cognitive/src/services/background.rs`, find the block after `execute_memory_ops()` call (around line 423-429). After line 429 (after the `.await;`), add:

```rust
                        // ── Contradiction detection ──────────────────────────
                        // Surface when a user-stated, high-confidence fact is
                        // updated to a different value in a different session.
                        if let Some(ref bus) = domain_bus {
                            for (candidate, op) in candidates.iter().zip(ops.iter()) {
                                if let crate::types::MemoryOp::Update { id: _, old_id } = op {
                                    let new = &candidate.candidate;
                                    // Only surface contradictions for user-stated, high-confidence facts
                                    if new.confidence < 0.7 || new.source != "user_stated" {
                                        continue;
                                    }
                                    // Look up the old fact to compare objects
                                    if let Ok(Some(old_fact)) = repo.get(old_id).await {
                                        if old_fact.object != new.object {
                                            // Skip same-session updates (compare recorded_at)
                                            if is_same_session(&old_fact.recorded_at, &session_start) {
                                                continue;
                                            }
                                            let _ = bus.publish(bus::DomainEvent::ContradictionDetected {
                                                existing_subject: old_fact.subject.clone(),
                                                existing_predicate: old_fact.predicate.clone(),
                                                existing_object: old_fact.object.clone(),
                                                new_object: new.object.clone(),
                                                confidence: new.confidence,
                                            });
                                        }
                                    }
                                }
                            }
                        }
```

- [ ] **Step 2: Add the `is_same_session` helper and `session_start` capture**

Add at the bottom of `background.rs` (before the last closing brace of the module, or alongside other helpers):

```rust
/// Check if a fact was recorded in the current session (within last 5 minutes of session start).
fn is_same_session(recorded_at: &str, session_start: &str) -> bool {
    let recorded = recorded_at
        .parse::<chrono::NaiveDateTime>()
        .unwrap_or_default();
    let start = session_start
        .parse::<chrono::NaiveDateTime>()
        .unwrap_or_default();
    // If recorded is within 5 minutes before session start, it's same session
    (recorded - start).num_seconds().abs() < 300
}
```

The `session_start` variable needs to be captured at the start of the background service. Look for where `BackgroundConsolidationService` initializes or where the main loop starts. Capture it as:
```rust
let session_start = chrono::Utc::now().to_rfc3339();
```
Place this at the top of the background task spawn block (before the loop that processes batches). If the service already has a session concept, use that instead.

- [ ] **Step 3: Verify `domain_bus` is accessible**

Check that `domain_bus` (or `bus`) is available in the scope where `execute_memory_ops` is called. The background service should already have it — it's passed during construction. If it's named differently (e.g., `event_bus`), use that name instead.

- [ ] **Step 4: Build**

Run: `cargo build -p cognitive`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): add contradiction detection after consolidation"
```

---

### Task 4: Re-exports + Final Verification

**Files:**
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Re-export TemporalService types**

In `crates/cognitive/src/lib.rs`, add to the re-exports:
```rust
pub use services::temporal::{ChangeSummary, FactVersion, TemporalService};
```

- [ ] **Step 2: Full build + tests**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: all tests pass (2560+ tests).

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 new warnings.

- [ ] **Step 4: Format**

Run: `cargo fmt --all`

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): re-export TemporalService types"
```

If formatting changes are needed:
```bash
cargo fmt --all
git add -A && git commit -m "style: format Phase 3 implementation"
```
