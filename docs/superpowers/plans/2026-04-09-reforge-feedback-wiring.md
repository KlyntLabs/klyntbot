# Reforge Phase A — Feedback Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire existing feedback signals (tool failures, corrections, behavioral metrics, graph health) into the Reforge nightly cycle so it can automatically improve skills, memory, and parameters based on real usage data.

**Architecture:** Extend the Reforge Phase 1 collector to read 8+ existing SQL tables. Feed tool failure patterns and correction summaries into Phase 3 Review prompt. Persist Reforge's own output (ContextPrioritySuggestions, CrossSessionPatterns) so they feed back into the next cycle. No new data collection — just read what's already stored.

**Tech Stack:** Rust, SQLite (storage/cognitive crates), existing Reforge cycle (cognitive crate), agent crate for correction attribution

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/services/reforge/feedback.rs` | All feedback data loading functions (tool failures, corrections, behavioral metrics, graph health) |
| `crates/storage/migrations/010_reforge_suggestions.sql` | `reforge_suggestions` table for persisting Reforge's own output |
| `crates/storage/src/repos/reforge_suggestion.rs` | Repo for `reforge_suggestions` CRUD |

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/src/services/reforge/types.rs` | Add ToolFailureSummary, CorrectionSummary, BehavioralMetrics, GraphHealthMetrics, ReforgeSuggestion; extend ReforgeCollected |
| `crates/cognitive/src/services/reforge/mod.rs` | Export new `feedback` module |
| `crates/cognitive/src/services/reforge/service.rs` | Persist ContextPrioritySuggestions + CrossSessionPatterns in Phase 5; feed CompactionResult to stats |
| `crates/cognitive/src/services/reforge/collector.rs` | Call feedback loading functions |
| `crates/agent/src/adapters/reforge_handlers.rs` | Extend REVIEW_PROMPT and format_review_input with tool failures + corrections |
| `crates/agent/src/agent_loop/mod.rs` | Populate `active_skill` on UserCorrectedAI events |
| `crates/storage/src/repos/mod.rs` | Export ReforgesSuggestionRepo |
| `crates/storage/src/repos/outcome.rs` | Add `tool_failure_stats_since()` method |
| `crates/storage/src/repos/retrieval_feedback.rs` | Add `avg_precision_by_domain_since()` method |
| `crates/app-core/src/init/cron.rs` | Pass new repos to Reforge handler |
| `tests/integration/cognitive.rs` | Integration test for feedback-enhanced Reforge cycle |

---

### Task 1: Add feedback types to Reforge

**Files:**
- Modify: `crates/cognitive/src/services/reforge/types.rs`

- [ ] **Step 1: Add feedback types**

Add these types after the `AutotunerContext` struct in `types.rs`:

```rust
// ── Feedback types (Phase A) ─────────────────────────────────

/// Aggregated tool failure stats for a single tool since last Reforge run.
#[derive(Debug, Clone, Serialize)]
pub struct ToolFailureSummary {
    pub tool_name: String,
    pub total_calls: u32,
    pub failure_count: u32,
    pub failure_rate: f64,
    pub error_types: Vec<String>,
}

/// Aggregated corrections attributed to a specific skill.
#[derive(Debug, Clone, Serialize)]
pub struct CorrectionSummary {
    pub skill_name: String,
    pub correction_count: u32,
    pub sample_corrections: Vec<String>,
}

/// Behavioral metrics collected from feature crates.
#[derive(Debug, Clone, Serialize, Default)]
pub struct BehavioralMetrics {
    pub task_estimation_bias: Option<f64>,
    pub coaching_acceptance_rate: Option<f64>,
    pub focus_quality_trend: Option<f64>,
    pub suggestion_dismiss_rate: Option<f64>,
    pub forecast_accuracy: Option<f64>,
}

/// Knowledge graph health snapshot.
#[derive(Debug, Clone, Serialize, Default)]
pub struct GraphHealthMetrics {
    pub active_facts: u32,
    pub active_rules: u32,
    pub co_activation_pairs: u32,
    pub facts_per_domain: Vec<(String, u32)>,
    pub avg_fact_stability: f64,
}

/// A persisted suggestion from a previous Reforge cycle.
#[derive(Debug, Clone, Serialize)]
pub struct ReforgeSuggestion {
    pub suggestion_type: String,
    pub content: String,
    pub reason: String,
    pub confidence: f64,
    pub cycle_run_at: String,
}
```

- [ ] **Step 2: Extend ReforgeCollected**

Add these fields to the `ReforgeCollected` struct:

```rust
    // Phase A: Feedback wiring
    pub tool_failures: Vec<ToolFailureSummary>,
    pub correction_summaries: Vec<CorrectionSummary>,
    pub retrieval_precision_by_domain: Vec<(String, f64)>,
    pub behavioral_metrics: BehavioralMetrics,
    pub graph_health: GraphHealthMetrics,
    pub previous_suggestions: Vec<ReforgeSuggestion>,
    pub extraction_yield_by_domain: Vec<(String, f64)>,
```

- [ ] **Step 3: Extend ReviewInput**

Add to `ReviewInput`:

```rust
    pub tool_failure_summary: Option<String>,
    pub correction_summary: Option<String>,
    pub previous_suggestions_summary: Option<String>,
```

- [ ] **Step 4: Extend ReforgeResult**

Add to `ReforgeResult`:

```rust
    pub suggestions_persisted: u32,
    pub patterns_persisted: u32,
```

- [ ] **Step 5: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles with warnings about unused fields (OK at this stage).

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add feedback types to Reforge"
```

---

### Task 2: Storage layer — tool failure query + suggestions table + per-domain precision

**Files:**
- Modify: `crates/storage/src/repos/outcome.rs`
- Modify: `crates/storage/src/repos/retrieval_feedback.rs`
- Create: `crates/storage/src/repos/reforge_suggestion.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/migrations/001_initial.sql`

- [ ] **Step 1: Add tool_failure_stats_since to OutcomeRepo**

In `crates/storage/src/repos/outcome.rs`, add after the existing `count_stats()` method:

```rust
/// Aggregate tool failure stats grouped by tool name since a timestamp.
pub async fn tool_failure_stats_since(
    &self,
    since: DateTime<Utc>,
) -> Result<Vec<ToolFailureStatsRow>, StorageError> {
    let rows = sqlx::query_as::<_, ToolFailureStatsRow>(
        "SELECT tool_name,
                COUNT(*) AS total_calls,
                SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failure_count,
                GROUP_CONCAT(DISTINCT CASE WHEN success = 0 THEN error_category ELSE NULL END) AS error_types
         FROM learning_outcomes
         WHERE created_at > ?1
         GROUP BY tool_name
         HAVING failure_count > 0
         ORDER BY failure_count DESC
         LIMIT 20",
    )
    .bind(since.to_rfc3339())
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

Add the row type in the same file:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ToolFailureStatsRow {
    pub tool_name: String,
    pub total_calls: i64,
    pub failure_count: i64,
    pub error_types: Option<String>,
}
```

- [ ] **Step 2: Add avg_precision_by_domain_since to RetrievalFeedbackRepo**

In `crates/storage/src/repos/retrieval_feedback.rs`, add after the existing `avg_precision_since()` method:

```rust
/// Average retrieval precision grouped by fact domain.
/// Joins retrieval_feedback against semantic_facts to determine domain.
pub async fn avg_precision_by_domain_since(
    &self,
    days: i64,
) -> Result<Vec<(String, f64)>, sqlx::Error> {
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT f.domain, AVG(rf.precision) as avg_precision
         FROM retrieval_feedback rf,
              json_each(rf.retrieved_fact_ids) je
         JOIN semantic_facts f ON f.id = je.value
         WHERE rf.created_at > ?1
         GROUP BY f.domain
         HAVING COUNT(*) >= 3
         ORDER BY avg_precision ASC",
    )
    .bind(&since)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 3: Create reforge_suggestions table**

Add to `crates/storage/migrations/001_initial.sql` (pre-release, can modify in-place):

```sql
-- Reforge self-feedback: persisted suggestions and patterns from previous cycles
CREATE TABLE IF NOT EXISTS reforge_suggestions (
    id TEXT PRIMARY KEY,
    suggestion_type TEXT NOT NULL,
    content TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    confidence REAL NOT NULL DEFAULT 0.0,
    cycle_run_at TEXT NOT NULL,
    acted_upon INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_reforge_suggestions_type
    ON reforge_suggestions(suggestion_type, created_at);
```

- [ ] **Step 4: Create ReforgeSuggestionRepo**

Create `crates/storage/src/repos/reforge_suggestion.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReforgeSuggestionRow {
    pub id: String,
    pub suggestion_type: String,
    pub content: String,
    pub reason: String,
    pub confidence: f64,
    pub cycle_run_at: String,
    pub acted_upon: bool,
    pub created_at: String,
}

pub struct ReforgeSuggestionRepo {
    pool: SqlitePool,
}

impl ReforgeSuggestionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &ReforgeSuggestionRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO reforge_suggestions (id, suggestion_type, content, reason, confidence, cycle_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&row.id)
        .bind(&row.suggestion_type)
        .bind(&row.content)
        .bind(&row.reason)
        .bind(row.confidence)
        .bind(&row.cycle_run_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load recent suggestions not yet acted upon, for feeding back into the next cycle.
    pub async fn recent_unacted(
        &self,
        limit: u32,
    ) -> Result<Vec<ReforgeSuggestionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ReforgeSuggestionRow>(
            "SELECT * FROM reforge_suggestions
             WHERE acted_upon = 0
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark a suggestion as acted upon.
    pub async fn mark_acted(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE reforge_suggestions SET acted_upon = 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete suggestions older than N days.
    pub async fn cleanup(&self, max_age_days: u32) -> Result<u32, StorageError> {
        let cutoff =
            (Utc::now() - chrono::Duration::days(max_age_days as i64)).to_rfc3339();
        let result = sqlx::query(
            "DELETE FROM reforge_suggestions WHERE created_at < ?1",
        )
        .bind(&cutoff)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as u32)
    }
}
```

- [ ] **Step 5: Export the new repo**

In `crates/storage/src/repos/mod.rs`, add:

```rust
pub mod reforge_suggestion;
pub use reforge_suggestion::ReforgeSuggestionRepo;
```

- [ ] **Step 6: Verify**

Run: `cargo build -p storage`
Expected: Compiles cleanly.

- [ ] **Step 7: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add tool failure stats, per-domain precision, and reforge suggestions repo"
```

---

### Task 3: Populate active_skill on UserCorrectedAI events

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`

- [ ] **Step 1: Read the current correction emission code**

Read `crates/agent/src/agent_loop/mod.rs` and find the `emit_correction_signal()` method (around line 196) and all call sites (around lines 180-188). Understand how `active_skill: None` is currently passed.

- [ ] **Step 2: Find where the active skill is known**

Search for `last_routed_skill`, `active_skill`, or `current_skill` in the agent loop. The skill is known from the `SkillRouter` result during intent analysis. Check if the `AgentLoop` struct or `RunContext` stores the last routed skill name.

If a `last_skill` field doesn't exist on the loop state, the simplest approach is to read it from the `StrategyRepo` — the most recent `strategy_records` row for this session has a `predicted_strategy` column which is the skill name. Alternatively, thread the skill name from the intent pipeline result through to the correction handler.

- [ ] **Step 3: Populate active_skill**

Update the call to `emit_correction_signal()` to pass the actual skill name instead of `None`. The exact approach depends on what Step 2 finds. The simplest pattern:

```rust
// In the correction detection path, before calling emit_correction_signal:
let active_skill = self.strategy_repo
    .get_latest_for_session(&session_key)
    .await
    .ok()
    .and_then(|s| s.predicted_strategy);

// Then pass it:
self.emit_correction_signal(
    chat_id,
    original,
    correction,
    kind,
    strength,
    session_key,
    active_skill, // was: None
).await;
```

If `get_latest_for_session` doesn't exist, a simpler approach is to store the skill name from the most recent `process_message()` call on the `AgentLoop` struct as `last_active_skill: Option<String>` and read it during correction detection.

The implementer should read the agent loop struct fields and pick the cleanest approach. The key constraint: the skill name must be available at the point where `emit_correction_signal` is called, which is during message processing after the skill has already been selected.

- [ ] **Step 4: Verify**

Run: `cargo build -p agent`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): populate active_skill on UserCorrectedAI events"
```

---

### Task 4: Feedback loading functions

**Files:**
- Create: `crates/cognitive/src/services/reforge/feedback.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`

- [ ] **Step 1: Create the feedback module**

Create `crates/cognitive/src/services/reforge/feedback.rs` with all feedback loading functions. The cognitive crate doesn't depend on feature crates, so behavioral metrics from feature tables must be loaded via raw SQL against the shared pool:

```rust
//! Feedback signal loading for Reforge Phase 1.
//!
//! Reads existing SQL tables to surface tool failures, corrections,
//! behavioral metrics, and graph health for the Reforge cycle.

use std::collections::HashMap;

use tracing::warn;

use super::types::*;

/// Load tool failure stats from the outcome_records table.
pub async fn load_tool_failures(
    outcome_repo: &storage::OutcomeRepo,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<ToolFailureSummary> {
    match outcome_repo.tool_failure_stats_since(since).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| ToolFailureSummary {
                tool_name: r.tool_name,
                total_calls: r.total_calls as u32,
                failure_count: r.failure_count as u32,
                failure_rate: if r.total_calls > 0 {
                    r.failure_count as f64 / r.total_calls as f64
                } else {
                    0.0
                },
                error_types: r
                    .error_types
                    .unwrap_or_default()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            })
            .collect(),
        Err(e) => {
            warn!("Reforge feedback: failed to load tool failures: {e}");
            vec![]
        }
    }
}

/// Load correction summaries grouped by skill from the domain_event_log.
pub async fn load_correction_summaries(
    event_log_repo: &crate::repos::EventLogRepo,
    since: &str,
) -> Vec<CorrectionSummary> {
    let now_str = chrono::Utc::now().to_rfc3339();
    let events = match event_log_repo
        .query_domain_events_range(since, &now_str)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Reforge feedback: failed to load correction events: {e}");
            return vec![];
        }
    };

    let mut by_skill: HashMap<String, Vec<String>> = HashMap::new();
    for event in &events {
        if event.event_type != "UserCorrectedAI" {
            continue;
        }
        let payload: serde_json::Value = match serde_json::from_str(&event.payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let skill = payload
            .get("active_skill")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let correction = payload
            .get("correction")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        by_skill.entry(skill).or_default().push(correction);
    }

    by_skill
        .into_iter()
        .map(|(skill_name, corrections)| CorrectionSummary {
            correction_count: corrections.len() as u32,
            sample_corrections: corrections.into_iter().take(3).collect(),
            skill_name,
        })
        .collect()
}

/// Load behavioral metrics from feature crate tables.
/// Uses raw SQL because cognitive doesn't depend on feature crates.
pub async fn load_behavioral_metrics(
    pool: &sqlx::SqlitePool,
) -> BehavioralMetrics {
    let mut metrics = BehavioralMetrics::default();

    // Task estimation bias (from task_estimation_history)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT AVG(deviation_pct) FROM task_estimation_history
         WHERE created_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.task_estimation_bias = row.map(|r| r.0);
    }

    // Coaching acceptance rate (from coaching_strategies)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT CAST(SUM(times_accepted) AS REAL) / NULLIF(SUM(times_used), 0)
         FROM coaching_strategies
         WHERE times_used > 0",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.coaching_acceptance_rate = row.and_then(|r| if r.0 > 0.0 { Some(r.0) } else { None });
    }

    // Focus quality trend (7d avg vs 14d avg from daily_summaries)
    if let Ok(row) = sqlx::query_as::<_, (f64, f64)>(
        "SELECT
           (SELECT AVG(avg_session_quality) FROM daily_summaries WHERE date > date('now', '-7 days')),
           (SELECT AVG(avg_session_quality) FROM daily_summaries WHERE date > date('now', '-14 days') AND date <= date('now', '-7 days'))",
    )
    .fetch_optional(pool)
    .await
    {
        if let Some((recent, prev)) = row {
            if prev > 0.0 {
                metrics.focus_quality_trend = Some(recent - prev);
            }
        }
    }

    // Suggestion dismiss rate (from task_suggestions)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT CAST(SUM(CASE WHEN status = 'dismissed' THEN 1 ELSE 0 END) AS REAL)
              / NULLIF(COUNT(*), 0)
         FROM task_suggestions
         WHERE resolved_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.suggestion_dismiss_rate = row.map(|r| r.0);
    }

    // Forecast accuracy (from productivity_forecasts)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT AVG(ABS(prediction_error)) FROM productivity_forecasts
         WHERE created_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.forecast_accuracy = row.map(|r| r.0);
    }

    metrics
}

/// Load knowledge graph health metrics.
pub async fn load_graph_health(
    fact_repo: &crate::repos::SemanticFactRepo,
    rule_repo: &crate::repos::ProceduralRuleRepo,
    co_activation_repo: &crate::repos::CoActivationRepo,
) -> GraphHealthMetrics {
    let active_facts = fact_repo.count_active().await.unwrap_or(0) as u32;
    let active_rules = rule_repo.count_active_rules().await.unwrap_or(0) as u32;
    let co_activation_pairs = co_activation_repo.count_all().await.unwrap_or(0) as u32;

    let facts_per_domain = fact_repo
        .count_by_domain()
        .await
        .unwrap_or_default();

    let avg_fact_stability = fact_repo
        .avg_stability()
        .await
        .unwrap_or(1.0);

    GraphHealthMetrics {
        active_facts,
        active_rules,
        co_activation_pairs,
        facts_per_domain,
        avg_fact_stability,
    }
}

/// Load previous Reforge suggestions that haven't been acted upon.
pub async fn load_previous_suggestions(
    suggestion_repo: &storage::ReforgeSuggestionRepo,
) -> Vec<ReforgeSuggestion> {
    match suggestion_repo.recent_unacted(10).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| ReforgeSuggestion {
                suggestion_type: r.suggestion_type,
                content: r.content,
                reason: r.reason,
                confidence: r.confidence,
                cycle_run_at: r.cycle_run_at,
            })
            .collect(),
        Err(e) => {
            warn!("Reforge feedback: failed to load previous suggestions: {e}");
            vec![]
        }
    }
}

/// Load extraction yield by domain from pipeline_event_log.
pub async fn load_extraction_yield(
    event_log_repo: &crate::repos::EventLogRepo,
    since: &str,
) -> Vec<(String, f64)> {
    // The pipeline_event_log stores facts_extracted per observation.
    // Group by domain and compute avg yield.
    match event_log_repo.extraction_yield_by_domain(since).await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Reforge feedback: failed to load extraction yield: {e}");
            vec![]
        }
    }
}
```

Note: Some repo methods referenced above (`count_active`, `count_active_rules`, `count_all`, `count_by_domain`, `avg_stability`, `extraction_yield_by_domain`) may not exist yet. The implementer should check each repo and add simple `SELECT COUNT(*)` / `SELECT AVG()` queries where missing. These are 3-5 line methods each.

- [ ] **Step 2: Export the feedback module**

In `crates/cognitive/src/services/reforge/mod.rs`, add:

```rust
pub mod feedback;
```

- [ ] **Step 3: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles (some methods may need stubs if repos lack count/avg queries).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add feedback loading functions for Reforge"
```

---

### Task 5: Extend collector to load feedback data

**Files:**
- Modify: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: Add feedback loading to collect()**

The `collect()` function needs additional repo parameters for feedback loading. However, to avoid growing the already-large parameter list, create a new `FeedbackSources` struct:

```rust
/// Optional data sources for feedback collection.
/// All fields are optional — missing sources are gracefully skipped.
pub struct FeedbackSources<'a> {
    pub outcome_repo: Option<&'a storage::OutcomeRepo>,
    pub event_log_repo: Option<&'a crate::repos::EventLogRepo>,
    pub co_activation_repo: Option<&'a crate::repos::CoActivationRepo>,
    pub suggestion_repo: Option<&'a storage::ReforgeSuggestionRepo>,
    pub pool: Option<&'a sqlx::SqlitePool>,
}
```

- [ ] **Step 2: Add feedback_sources parameter to collect()**

Add `feedback_sources: Option<&FeedbackSources<'_>>` as the last parameter of `collect()`. At the end of the function (before the `Some(ReforgeCollected { ... })` return), load feedback data:

```rust
    // --- Feedback signals (Phase A) ---
    let (tool_failures, correction_summaries, behavioral_metrics, graph_health,
         previous_suggestions, retrieval_precision_by_domain, extraction_yield_by_domain) =
        if let Some(fb) = feedback_sources {
            let since_dt = chrono::DateTime::parse_from_rfc3339(since)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now() - chrono::Duration::days(7));

            let tool_failures = if let Some(repo) = fb.outcome_repo {
                super::feedback::load_tool_failures(repo, since_dt).await
            } else {
                vec![]
            };

            let correction_summaries = if let Some(repo) = fb.event_log_repo {
                super::feedback::load_correction_summaries(repo, since).await
            } else {
                vec![]
            };

            let behavioral_metrics = if let Some(pool) = fb.pool {
                super::feedback::load_behavioral_metrics(pool).await
            } else {
                BehavioralMetrics::default()
            };

            let graph_health = super::feedback::load_graph_health(
                fact_repo, rule_repo,
                fb.co_activation_repo.unwrap_or(&crate::repos::CoActivationRepo::new(
                    fact_repo.pool().clone()
                )),
            ).await;

            let previous_suggestions = if let Some(repo) = fb.suggestion_repo {
                super::feedback::load_previous_suggestions(repo).await
            } else {
                vec![]
            };

            let retrieval_precision_by_domain = if let Some(repo) = feedback_repo {
                repo.avg_precision_by_domain_since(
                    match last_run_at {
                        Some(ts) => chrono::DateTime::parse_from_rfc3339(ts)
                            .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days().max(1))
                            .unwrap_or(7),
                        None => 7,
                    }
                ).await.unwrap_or_default()
            } else {
                vec![]
            };

            let extraction_yield_by_domain = if let Some(repo) = fb.event_log_repo {
                super::feedback::load_extraction_yield(repo, since).await
            } else {
                vec![]
            };

            (tool_failures, correction_summaries, behavioral_metrics, graph_health,
             previous_suggestions, retrieval_precision_by_domain, extraction_yield_by_domain)
        } else {
            (vec![], vec![], BehavioralMetrics::default(), GraphHealthMetrics::default(),
             vec![], vec![], vec![])
        };
```

Then include all fields in the returned `ReforgeCollected`.

- [ ] **Step 3: Update all callers to pass feedback_sources**

In `crates/app-core/src/init/cron.rs` (the Reforge handler), construct a `FeedbackSources` and pass it. In the integration test, pass `None`.

- [ ] **Step 4: Verify**

Run: `cargo build -p cognitive -p app-core`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/ crates/app-core/
git commit -m "feat(cognitive): wire feedback data sources into Reforge collector"
```

---

### Task 6: Extend Review prompt with tool failures and corrections

**Files:**
- Modify: `crates/agent/src/adapters/reforge_handlers.rs`
- Modify: `crates/cognitive/src/services/reforge/service.rs`

- [ ] **Step 1: Extend REVIEW_PROMPT**

Add to the REVIEW_PROMPT constant (before the JSON schema):

```
When tool failure data is provided, analyze error patterns and propose skill edits that fix the root cause.
For example, if a tool consistently receives wrong parameter types, update the skill's Critical Rules to clarify the correct parameter usage.

When correction data is provided, identify which skill instructions led to the mistake and propose targeted fixes.

When previous suggestions are provided, evaluate whether they should still be acted upon or have become stale.
```

- [ ] **Step 2: Extend format_review_input()**

Add sections to `format_review_input()`:

```rust
    // Tool failures
    if let Some(ref summary) = input.tool_failure_summary {
        writeln!(&mut out, "\n## Tool Health (since last cycle)\n{summary}").unwrap();
    }

    // Correction patterns per skill
    if let Some(ref summary) = input.correction_summary {
        writeln!(&mut out, "\n## Correction Patterns\n{summary}").unwrap();
    }

    // Previous suggestions
    if let Some(ref summary) = input.previous_suggestions_summary {
        writeln!(&mut out, "\n## Previous Cycle Suggestions (unacted)\n{summary}").unwrap();
    }
```

- [ ] **Step 3: Build feedback strings in service.rs**

In `build_review_input()` in `service.rs`, format the new fields:

```rust
    let tool_failure_summary = if !collected.tool_failures.is_empty() {
        Some(
            collected
                .tool_failures
                .iter()
                .map(|f| {
                    format!(
                        "- {}:{} — {}/{} calls failed ({:.0}%) — errors: {}",
                        "tool", f.tool_name, f.failure_count, f.total_calls,
                        f.failure_rate * 100.0,
                        f.error_types.join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    let correction_summary = if !collected.correction_summaries.is_empty() {
        Some(
            collected
                .correction_summaries
                .iter()
                .map(|c| {
                    let samples = c.sample_corrections.iter()
                        .map(|s| format!("    \"{s}\""))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("- {} skill: {} corrections\n{samples}", c.skill_name, c.correction_count)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };

    let previous_suggestions_summary = if !collected.previous_suggestions.is_empty() {
        Some(
            collected
                .previous_suggestions
                .iter()
                .map(|s| format!("- [{}] {}: {} (confidence: {:.2})", s.suggestion_type, s.content, s.reason, s.confidence))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };
```

Include these in the `ReviewInput` constructor.

- [ ] **Step 4: Verify**

Run: `cargo build -p agent -p cognitive`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/ crates/cognitive/
git commit -m "feat(agent): extend Review prompt with tool failures, corrections, and previous suggestions"
```

---

### Task 7: Persist Reforge's own output (ContextPrioritySuggestions + CrossSessionPatterns)

**Files:**
- Modify: `crates/cognitive/src/services/reforge/service.rs`

- [ ] **Step 1: Add suggestion_repo parameter to run_reforge**

Add `suggestion_repo: Option<&storage::ReforgeSuggestionRepo>` to the `run_reforge()` function signature.

- [ ] **Step 2: Persist ContextPrioritySuggestions after Phase 3**

After the Phase 3 Review block (where `review_output` is assigned), add:

```rust
    // Persist context priority suggestions for the next cycle's feedback loop.
    if let Some(ref review) = review_output {
        if let Some(repo) = suggestion_repo {
            let now = Utc::now().to_rfc3339();
            for suggestion in &review.context_priority_suggestions {
                let row = storage::repos::reforge_suggestion::ReforgeSuggestionRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    suggestion_type: "context_priority".to_string(),
                    content: suggestion.suggestion.clone(),
                    reason: suggestion.reason.clone(),
                    confidence: 0.8,
                    cycle_run_at: now.clone(),
                    acted_upon: false,
                    created_at: now.clone(),
                };
                if let Err(e) = repo.insert(&row).await {
                    warn!("Reforge: failed to persist context priority suggestion: {e}");
                } else {
                    result.suggestions_persisted += 1;
                }
            }
        }
    }
```

- [ ] **Step 3: Persist CrossSessionPatterns after Phase 2**

After the Phase 2 Synthesize block, add:

```rust
    // Persist high-confidence cross-session patterns as episodic memories.
    if let Some(ref syn) = synthesize_output {
        for pattern in &syn.cross_session_patterns {
            if pattern.confidence >= 0.7 {
                let mem = EpisodicMemory {
                    id: uuid::Uuid::new_v4().to_string(),
                    domain: SOURCE_REFORGE.to_string(),
                    content: pattern.pattern.clone(),
                    summary: Some("Cross-session pattern".to_string()),
                    importance: pattern.confidence,
                    occurred_at: Utc::now().to_rfc3339(),
                    recorded_at: Utc::now().to_rfc3339(),
                    stability: 3.0,
                    last_accessed: None,
                    access_count: 0,
                    project_id: None,
                    scope_type: "system".to_string(),
                    scope_id: None,
                };
                if let Err(e) = episodic_repo.insert(&mem).await {
                    warn!("Reforge: failed to persist cross-session pattern: {e}");
                } else {
                    result.patterns_persisted += 1;
                }
            }
        }
    }
```

- [ ] **Step 4: Include CompactionResult in stats JSON**

In the Phase 7 Compact block, capture the `CompactionResult` and include it in the `stats_json`:

```rust
    // In the Phase 7 block, change:
    let compaction_stats = match crate::services::compaction::run_compaction(...).await {
        Ok(cr) => {
            debug!(...);
            Some(serde_json::json!({
                "facts_archived": cr.facts_archived,
                "episodic_deleted": cr.episodic_deleted,
                "rules_deactivated": cr.rules_deactivated,
            }))
        }
        Err(e) => { ... None }
    };

    // Then in the stats_json, add:
    // "compaction": compaction_stats,
```

- [ ] **Step 5: Update all callers to pass suggestion_repo**

In `crates/app-core/src/init/cron.rs`, construct and pass the `ReforgeSuggestionRepo`. In the integration test, pass `None`.

- [ ] **Step 6: Verify**

Run: `cargo build -p cognitive -p app-core`

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(reforge)'`
Expected: All existing reforge tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/ crates/app-core/ crates/storage/
git commit -m "feat(cognitive): persist Reforge output — context suggestions, cross-session patterns, compaction stats"
```

---

### Task 8: Wire everything in cron handler

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Read the current Reforge cron handler**

Read the `JOB_REFORGE_NIGHTLY` handler block in `cron.rs`. Understand what repos are captured in the closure and how `run_reforge` is called.

- [ ] **Step 2: Add new repos to the closure**

The `repos_reforge: Repos` clone already includes `outcomes: OutcomeRepo` and `strategies: StrategyRepo`. You need to add:

```rust
let event_log_repo = cognitive::EventLogRepo::new(pool.clone());
let co_activation_repo = cognitive::CoActivationRepo::new(pool.clone());
let suggestion_repo = storage::ReforgeSuggestionRepo::new(pool.clone());
```

Build the `FeedbackSources` struct inside the handler:

```rust
let feedback_sources = cognitive::services::reforge::collector::FeedbackSources {
    outcome_repo: Some(&repos_reforge.outcomes),
    event_log_repo: Some(&event_log_repo),
    co_activation_repo: Some(&co_activation_repo),
    suggestion_repo: Some(&suggestion_repo),
    pool: Some(&pool),
};
```

- [ ] **Step 3: Pass feedback_sources to collect() and suggestion_repo to run_reforge()**

Update the `run_reforge()` call to pass the new parameters.

- [ ] **Step 4: Verify full workspace builds**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/
git commit -m "feat(app-core): wire feedback sources into Reforge cron handler"
```

---

### Task 9: Integration test — feedback-enhanced Reforge cycle

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add test for feedback-enhanced cycle**

```rust
#[tokio::test]
async fn test_reforge_with_feedback_signals() {
    use klyntbot::cognitive::services::reforge::service::run_reforge;
    use klyntbot::cognitive::services::reforge::skill_files::SkillFileManager;
    use klyntbot::cognitive::services::reforge::types::*;
    // ... same setup as existing test ...

    // Seed a tool failure in outcome_records
    sqlx::query(
        "INSERT INTO learning_outcomes (id, session_key, tool_name, success, error_category, duration_ms, created_at)
         VALUES ('tf1', 'test-session', 'finance:tx_add', 0, 'InvalidParams', 100, datetime('now'))"
    )
    .execute(&inner)
    .await
    .unwrap();

    // Seed a correction event in domain_event_log
    sqlx::query(
        "INSERT INTO domain_event_log (id, event_type, domain, salience, payload, timestamp)
         VALUES ('corr1', 'UserCorrectedAI', 'chat', 'extract',
                 '{\"original\":\"wrong\",\"correction\":\"right\",\"kind\":\"KeywordPrefix\",\"strength\":0.8,\"session_key\":\"test\",\"active_skill\":\"finance-management\"}',
                 datetime('now'))"
    )
    .execute(&inner)
    .await
    .unwrap();

    // Run Reforge with feedback sources
    let result = run_reforge(
        &reforge_state_repo, &skill_version_repo, &session_memory_repo,
        &fact_repo, &episodic_repo, &rule_repo, &handler, &skill_mgr,
        None, None, None, None, None, None, // bridges + autotuner ctx
    )
    .await;

    let r = result.unwrap();
    assert!(r.phase_errors.is_empty(), "Expected no errors: {:?}", r.phase_errors);
    // The cycle should complete successfully with feedback data available
}
```

The implementer should adapt this test based on the final `run_reforge` signature, ensuring feedback sources are properly constructed for the test.

- [ ] **Step 2: Run test**

Run: `cargo nextest run -E 'test(reforge_with_feedback)'`
Expected: PASS

- [ ] **Step 3: Run all reforge tests**

Run: `cargo nextest run -E 'test(reforge)'`
Expected: All pass (including existing tests).

- [ ] **Step 4: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add tests/
git commit -m "test: add Reforge feedback-enhanced cycle integration test"
```

---

## Summary

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | Feedback types | 1 | compile check |
| 2 | Storage queries + suggestions table | 4 | compile check |
| 3 | Skill attribution on corrections | 1 | compile check |
| 4 | Feedback loading functions | 2 | compile check |
| 5 | Extend collector with feedback sources | 2 | compile check |
| 6 | Extend Review prompt | 2 | compile check |
| 7 | Persist Reforge output | 2 | existing tests pass |
| 8 | Wire cron handler | 1 | workspace build |
| 9 | Integration test | 1 | 1 new integration test |

**Total: ~15 files modified/created, 1 integration test, 9 commits**

---

## Missing Repo Methods

The implementer will encounter repos missing simple aggregate queries. These are trivial additions (3-5 lines each):

| Repo | Missing Method | Query |
|------|---------------|-------|
| `SemanticFactRepo` | `count_active()` | `SELECT COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL` |
| `SemanticFactRepo` | `count_by_domain()` | `SELECT domain, COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL GROUP BY domain` |
| `SemanticFactRepo` | `avg_stability()` | `SELECT AVG(stability) FROM semantic_facts WHERE superseded_at IS NULL` |
| `ProceduralRuleRepo` | `count_active_rules()` | `SELECT COUNT(*) FROM procedural_rules WHERE active = 1` |
| `CoActivationRepo` | `count_all()` | `SELECT COUNT(*) FROM co_activation_edges` |
| `EventLogRepo` | `extraction_yield_by_domain()` | `SELECT domain, AVG(facts_extracted) FROM pipeline_event_log WHERE timestamp > ?1 GROUP BY domain` |

Add these as encountered during Task 4 implementation.
