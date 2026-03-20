# Autotuner Phase 2: Memory Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the autotuner to optimize memory retrieval parameters — FSRS retention, accumulation thresholds, vector search tuning, and the remaining 3 relevance weights — using the same shadow-score-and-promote infrastructure built in Phase 1.

**Architecture:** Add 8 new `TrialParams` fields, implement `ShadowRetriever` trait for shadow memory retrieval, add Phase 2 metrics (retrieval precision proxy, memory freshness), add Phase 2 constraints to the evaluator, gate Phase 2 behind champion stability (7 days stable + 14 days running), and extend the nightly cycle to score memory retrieval variants.

**Tech Stack:** Rust (SQLite/sqlx, tokio, serde, chrono), cognitive crate (FSRS, vector search, relevance scoring)

**Spec:** `docs/superpowers/specs/2026-03-19-autoresearch-design.md` (Phase 2 Sketch, lines 527-574)

**Depends on:** Phase 1 complete (commits `28ef2e53` through `46f8f264`)

---

## Scope Check

This is one plan because the 8 tasks form a linear dependency chain — each builds on the previous. The `ShadowRetriever` can't work without the new `TrialParams` fields, the metrics can't be computed without the shadow retrieval log, and the constraints can't be checked without the metrics.

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/common/src/autotuner.rs` | Add 8 new TrialParams fields + update `resolve_relevance_weights` | 1 |
| `crates/autotuner/src/cycle.rs` | Update `affected_param_names` + `trial_params_as_array` for 16 fields | 1 |
| `crates/autotuner/src/generator.rs` | Add Phase 2 parameter bounds to LLM prompt | 1 |
| `crates/autotuner/src/traits.rs` | Add `ShadowRetriever` trait + extend `MetricSnapshot` with 2 new fields | 2 |
| `crates/autotuner/src/trial.rs` | Add 2 new fields to `TrialResult` | 2 |
| `crates/autotuner/src/metrics.rs` | Update `aggregate_to_result` for new fields | 2 |
| `crates/cognitive/src/services/memory_retriever.rs` | Add `retrieve_with_overrides` method to `UnifiedMemoryService` | 3 |
| `crates/agent/src/autotuner/shadow_retriever.rs` | Create — implements `ShadowRetriever` using `UnifiedMemoryService` | 4 |
| `crates/storage/src/repos/trial_repo.rs` | Add `autotuner_shadow_retrieval_log` table + insert/query methods | 5 |
| `crates/agent/src/autotuner/hooks.rs` | Add `shadow_retriever` field, run shadow retrieval per-message | 5 |
| `crates/agent/src/autotuner/metric_collector.rs` | Compute `retrieval_precision` + `memory_freshness` from shadow retrieval log | 6 |
| `crates/config/src/schema/autotuner.rs` | Add Phase 2 constraint thresholds to `AutoTunerConfig` | 7 |
| `crates/autotuner/src/evaluator.rs` | Add Phase 2 constraint checks | 7 |
| `crates/agent/src/autotuner/mod.rs` | Add Phase 2 readiness gate in nightly cycle | 8 |
| `crates/app-core/src/init/cron.rs` | Wire `ShadowRetriever` into hook | 8 |

---

### Task 1: Extend TrialParams with 8 new fields

**Files:**
- Modify: `crates/common/src/autotuner.rs`
- Modify: `crates/autotuner/src/cycle.rs`
- Modify: `crates/autotuner/src/generator.rs`
- Modify: `crates/autotuner/src/evaluator.rs`

- [ ] **Step 1: Add 8 new fields to TrialParams**

In `crates/common/src/autotuner.rs`, add after the existing 8 fields:

```rust
    // Phase 2: FSRS tuning
    pub fsrs_desired_retention: Option<f64>,          // default 0.9, bounds [0.70, 0.99]

    // Phase 2: Accumulation thresholds
    // NOTE: These are read ONCE at startup by BackgroundConsolidationService and cannot
    // be dynamically overridden mid-run. They only take effect when the champion is promoted
    // and the service is restarted. Shadow scoring cannot evaluate these — they are
    // "promotion-time" params, not "per-message" params.
    pub accumulate_promote_threshold: Option<usize>,  // default 5, bounds [2, 15]
    pub accumulate_min_days: Option<usize>,           // default 3, bounds [1, 10]

    // Phase 2: Vector search
    pub vector_top_k: Option<usize>,                  // default 30, bounds [10, 100]
    pub min_similarity: Option<f64>,                  // default 0.55, bounds [0.30, 0.80]

    // Phase 2: Remaining 3 relevance weights (completes the 6-factor set)
    pub relevance_weight_importance: Option<f64>,     // default 0.15, bounds [0.05, 0.40]
    pub relevance_weight_frequency: Option<f64>,      // default 0.10, bounds [0.02, 0.30]
    pub relevance_weight_temporal: Option<f64>,        // default 0.05, bounds [0.01, 0.20]
```

- [ ] **Step 2: Update `resolve_relevance_weights`**

The existing method takes 3 default args for the missing weights. Now all 6 can come from `TrialParams`. Update the signature to only take `config_defaults: &[f64; 6]` (the full 6-element Config default array), and resolve all 6 from `TrialParams` with Config fallback:

```rust
pub fn resolve_relevance_weights(&self, defaults: &[f64; 6]) -> [f64; 6] {
    let raw = [
        self.relevance_weight_semantic.unwrap_or(defaults[0]),
        self.relevance_weight_retrievability.unwrap_or(defaults[1]),
        self.relevance_weight_importance.unwrap_or(defaults[2]),
        self.relevance_weight_frequency.unwrap_or(defaults[3]),
        self.relevance_weight_situation.unwrap_or(defaults[4]),
        self.relevance_weight_temporal.unwrap_or(defaults[5]),
    ];
    // Normalize to sum to 1.0
    let sum: f64 = raw.iter().sum();
    if sum > 0.0 {
        raw.map(|w| w / sum)
    } else {
        defaults.clone()
    }
}
```

Update all call sites of `resolve_relevance_weights` to pass the full 6-element defaults array. The main call site is the existing test in `crates/common/src/autotuner.rs` — update it from 3 positional `f64` args to a `&[f64; 6]` array. Also check `crates/agent/` and `crates/cognitive/` for any other call sites.

- [ ] **Step 3: Update `affected_param_names` in `cycle.rs`**

Add 8 new `check_field!` calls:

```rust
check_field!(fsrs_desired_retention);
check_field!(accumulate_promote_threshold);
check_field!(accumulate_min_days);
check_field!(vector_top_k);
check_field!(min_similarity);
check_field!(relevance_weight_importance);
check_field!(relevance_weight_frequency);
check_field!(relevance_weight_temporal);
```

- [ ] **Step 4: Update `trial_params_as_array` in `evaluator.rs`**

Change the return type from `[f64; 8]` to `[f64; 16]` and add the 8 new fields:

```rust
fn trial_params_as_array(p: &TrialParams) -> [f64; 16] {
    [
        // existing 8...
        p.fsrs_desired_retention.unwrap_or(0.0),
        p.accumulate_promote_threshold.map(|v| v as f64).unwrap_or(0.0),
        p.accumulate_min_days.map(|v| v as f64).unwrap_or(0.0),
        p.vector_top_k.map(|v| v as f64).unwrap_or(0.0),
        p.min_similarity.unwrap_or(0.0),
        p.relevance_weight_importance.unwrap_or(0.0),
        p.relevance_weight_frequency.unwrap_or(0.0),
        p.relevance_weight_temporal.unwrap_or(0.0),
    ]
}
```

- [ ] **Step 5: Update parameter bounds in `build_generation_prompt`**

In `crates/autotuner/src/generator.rs`, add 8 new rows to the bounds table string:

```
| fsrs_desired_retention          | 0.70  | 0.99 | 0.01 | FSRS target retention for spaced repetition |
| accumulate_promote_threshold    | 2     | 15   | 1    | Min observations before promoting to fact |
| accumulate_min_days             | 1     | 10   | 1    | Min days of observation before promotion |
| vector_top_k                    | 10    | 100  | 5    | Number of candidate vectors to retrieve |
| min_similarity                  | 0.30  | 0.80 | 0.05 | Cosine similarity threshold for retrieval |
| relevance_weight_importance     | 0.05  | 0.40 | 0.05 | Weight for fact importance in ranking |
| relevance_weight_frequency      | 0.02  | 0.30 | 0.02 | Weight for access frequency in ranking |
| relevance_weight_temporal       | 0.01  | 0.20 | 0.01 | Weight for temporal recency in ranking |
```

- [ ] **Step 6: Write backward-compat deserialization test**

```rust
#[test]
fn phase1_champion_deserializes_with_phase2_fields() {
    // Simulate a Phase 1 champion JSON (no Phase 2 fields)
    let json = r#"{"skill_keyword_weight": 0.7, "skill_semantic_weight": 0.3}"#;
    let params: TrialParams = serde_json::from_str(json).unwrap();
    assert!(params.fsrs_desired_retention.is_none());
    assert!(params.vector_top_k.is_none());
    assert!(params.relevance_weight_importance.is_none());
}
```

- [ ] **Step 7: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p common -p autotuner --no-fail-fast`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(autotuner): add 8 Phase 2 TrialParams fields for memory optimization"
```

---

### Task 2: Extend MetricSnapshot + TrialResult + ShadowRetriever trait

**Files:**
- Modify: `crates/autotuner/src/traits.rs`
- Modify: `crates/autotuner/src/trial.rs`
- Modify: `crates/autotuner/src/metrics.rs`

- [ ] **Step 1: Add `ShadowRetriever` trait to `traits.rs`**

```rust
/// Runs memory retrieval with trial parameter overrides for shadow scoring.
#[async_trait]
pub trait ShadowRetriever: Send + Sync {
    async fn retrieve_shadow(
        &self,
        query: &str,
        context: &ShadowContext,
        params: &common::TrialParams,
    ) -> common::Result<Vec<ShadowRetrievalResult>>;
}

/// Result of a shadow memory retrieval for one trial variant.
#[derive(Debug, Clone)]
pub struct ShadowRetrievalResult {
    pub memory_ids: Vec<String>,
    pub avg_score: f64,
    pub avg_age_days: f64,
    pub total_retrieved: usize,
}
```

**Design deviation from spec:** The spec (line 554) defines the return type as `Result<Vec<MemoryEntry>>`. We deliberately use `Result<ShadowRetrievalResult>` (a summary struct) instead, to avoid a dependency from `autotuner` (L4) on `context_engine` (L3). The concrete `AgentShadowRetriever` in the `agent` crate (L5) converts `Vec<MemoryEntry>` to `ShadowRetrievalResult`. This is an intentional architecture choice — flag it in the code comment.

- [ ] **Step 2: Add 2 new fields to `MetricSnapshot`**

```rust
pub struct MetricSnapshot {
    // ... existing 8 fields ...
    pub retrieval_precision: f64,   // Phase 2: fraction of messages where shadow retrieval overlapped with control
    pub memory_freshness: f64,      // Phase 2: average age (days) of retrieved memories
    // NOTE: The spec also defines retrieval_recall and promotion_accuracy.
    // These are deliberately deferred — retrieval_recall requires knowing the ground-truth
    // relevant set (no current data source), and promotion_accuracy requires tracking fact
    // correctness over time. Both can be added later without breaking the schema.
    // The retrieval_recall constraint (spec line 572) uses retrieval_precision as a proxy
    // until the real metric is available.
}
```

- [ ] **Step 3: Add 2 new fields to `TrialResult`**

In `crates/autotuner/src/trial.rs`, add to the `TrialResult` struct:

```rust
    pub retrieval_precision: f64,
    pub memory_freshness: f64,
```

- [ ] **Step 4: Update `aggregate_to_result` in `metrics.rs`**

Add the new fields to the aggregation, following the existing volume-weighted pattern.

- [ ] **Step 5: Fix all compilation errors**

Adding new fields to `MetricSnapshot` and `TrialResult` will break all construction sites. Search for `MetricSnapshot {` and `TrialResult {` across the workspace. Add `retrieval_precision: 0.0, memory_freshness: 0.0` to each. Key files:
- `crates/agent/src/autotuner/metric_collector.rs` — `collect_metrics` return
- `crates/autotuner/src/cycle.rs` — test constructions
- `crates/autotuner/src/evaluator.rs` — test constructions
- `crates/autotuner/src/metrics.rs` — test constructions

- [ ] **Step 6: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p autotuner --no-fail-fast`

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(autotuner): add ShadowRetriever trait and Phase 2 metric fields"
```

---

### Task 3: Add `retrieve_with_overrides` to UnifiedMemoryService

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add override method**

The existing `fetch_facts` method (line ~75) builds `RetrievalParams` from `self.config`. Add a new method that accepts param overrides:

```rust
/// Retrieve memories using overridden retrieval parameters (for shadow scoring).
pub async fn retrieve_with_overrides(
    &self,
    query: &str,
    vector_top_k: usize,
    min_similarity: f64,
    relevance_weights: [f64; 6],
) -> common::Result<Vec<MemoryEntry>> {
    // IMPORTANT: RetrievalParams has additional fields beyond weights:
    // - situational_boost: f64 — MUST call self.current_situational_boost().await
    // - max_stability: f64 — from self.config
    // - scope_chain: Vec<String> — from self.config or empty
    // Do NOT use a `..default()` pattern — construct ALL fields explicitly.
    let situational_boost = self.current_situational_boost().await;
    let params = RetrievalParams {
        vector_top_k,
        min_similarity,
        relevance_weight_semantic: relevance_weights[0],
        relevance_weight_retrievability: relevance_weights[1],
        relevance_weight_importance: relevance_weights[2],
        relevance_weight_frequency: relevance_weights[3],
        relevance_weight_situation: relevance_weights[4],
        relevance_weight_temporal: relevance_weights[5],
        situational_boost,
        max_stability: self.config.max_stability,
        scope_chain: self.config.scope_chain.clone(),
        // Check for any other fields on RetrievalParams — read the struct definition
    };
    // Follow the retrieval path in fetch_facts or retrieve_scoped,
    // calling retrieve_relevant_facts(&self.embedder, query, &params, &self.pool)
}
```

Read the file carefully to understand the exact `RetrievalParams` struct fields. The `current_situational_boost()` method is async — it MUST be awaited. The `max_stability` and `scope_chain` fields must also be provided (from `self.config`).

- [ ] **Step 2: Verify**

Run: `cargo check -p cognitive` then `cargo nextest run -p cognitive --no-fail-fast`

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(cognitive): add retrieve_with_overrides for shadow memory retrieval"
```

---

### Task 4: Implement AgentShadowRetriever

**Files:**
- Create: `crates/agent/src/autotuner/shadow_retriever.rs`
- Modify: `crates/agent/src/autotuner/mod.rs` (add `pub mod shadow_retriever;`)

- [ ] **Step 1: Create `shadow_retriever.rs`**

Follow the pattern of `shadow_classifier.rs`:

```rust
use async_trait::async_trait;
use autotuner::{ShadowContext, ShadowRetriever, ShadowRetrievalResult};
use cognitive::UnifiedMemoryService;
use common::TrialParams;
use std::sync::Arc;

pub struct AgentShadowRetriever {
    memory_service: Arc<UnifiedMemoryService>,
    config_defaults: [f64; 6], // 6 default relevance weights from CognitiveConfig
}

impl AgentShadowRetriever {
    pub fn new(memory_service: Arc<UnifiedMemoryService>, config_defaults: [f64; 6]) -> Self {
        Self { memory_service, config_defaults }
    }
}

#[async_trait]
impl ShadowRetriever for AgentShadowRetriever {
    async fn retrieve_shadow(
        &self,
        query: &str,
        _context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowRetrievalResult> {
        let weights = params.resolve_relevance_weights(&self.config_defaults);
        let top_k = params.vector_top_k.unwrap_or(30);
        let min_sim = params.min_similarity.unwrap_or(0.55);

        let entries = self.memory_service
            .retrieve_with_overrides(query, top_k, min_sim, weights)
            .await?;

        let total = entries.len();
        let avg_score = if total > 0 {
            entries.iter().map(|e| e.score).sum::<f64>() / total as f64
        } else { 0.0 };

        // Compute avg age — MemoryEntry doesn't have a timestamp,
        // so use score as a proxy (higher score = more relevant = fresher).
        // Real age computation would need fact timestamps from the DB.
        let avg_age_days = 0.0; // Placeholder — wire when SemanticFact timestamps are accessible

        Ok(ShadowRetrievalResult {
            memory_ids: entries.iter().map(|e| e.id.clone()).collect(),
            avg_score,
            avg_age_days,
            total_retrieved: total,
        })
    }
}
```

- [ ] **Step 2: Add module to `mod.rs`**

In `crates/agent/src/autotuner/mod.rs`, add:
```rust
pub mod shadow_retriever;
```

- [ ] **Step 3: Add test**

```rust
#[tokio::test]
async fn shadow_retriever_returns_empty_for_no_memories() {
    // Setup with empty in-memory DB
    // Verify ShadowRetrievalResult has total_retrieved = 0
}
```

- [ ] **Step 4: Verify**

Run: `cargo check -p agent` then `cargo nextest run -p agent -E 'test(shadow_retriever)' --no-fail-fast`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(autotuner): implement AgentShadowRetriever for shadow memory scoring"
```

---

### Task 5: Shadow retrieval logging + per-message hook

**Files:**
- Modify: `crates/storage/src/repos/trial_repo.rs`
- Modify: `crates/agent/src/autotuner/hooks.rs`

- [ ] **Step 1: Add `autotuner_shadow_retrieval_log` table**

In `crates/storage/src/repos/trial_repo.rs`, extend `MIGRATION_SQL` with:

```sql
CREATE TABLE IF NOT EXISTS autotuner_shadow_retrieval_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trial_id TEXT NOT NULL REFERENCES autotuner_trials(id),
    chat_id TEXT NOT NULL,
    message_timestamp TEXT NOT NULL,
    variant_retrieved_count INTEGER NOT NULL,
    control_retrieved_count INTEGER NOT NULL,
    overlap_count INTEGER NOT NULL,
    variant_avg_score REAL NOT NULL,
    variant_avg_age_days REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shadow_retrieval_log_trial
    ON autotuner_shadow_retrieval_log(trial_id);
```

- [ ] **Step 2: Add insert method**

```rust
pub async fn insert_shadow_retrieval_log(
    &self,
    trial_id: &str,
    chat_id: &str,
    message_timestamp: &str,
    variant_retrieved_count: i64,
    control_retrieved_count: i64,
    overlap_count: i64,
    variant_avg_score: f64,
    variant_avg_age_days: f64,
) -> Result<(), StorageError> { ... }
```

- [ ] **Step 3: Add query methods**

```rust
/// Retrieval precision for a trial: average overlap_count / variant_retrieved_count
pub async fn retrieval_precision_for_trial(
    &self, trial_id: &str, since: DateTime<Utc>,
) -> Result<f64, StorageError> { ... }

/// Average memory freshness for a trial
pub async fn avg_memory_freshness_for_trial(
    &self, trial_id: &str, since: DateTime<Utc>,
) -> Result<f64, StorageError> { ... }
```

- [ ] **Step 4: Add `shadow_retriever` to `AutoTunerHookImpl`**

In `crates/agent/src/autotuner/hooks.rs`, add:

```rust
shadow_retriever: Option<Arc<dyn ShadowRetriever>>,
```

Update the constructor. In `on_message_received`, after the shadow classification loop, add a shadow retrieval loop for active trials that have Phase 2 params set:

```rust
// Shadow retrieval (Phase 2)
if let Some(ref retriever) = self.shadow_retriever {
    for trial in &active_trials {
        let params: TrialParams = serde_json::from_str(&trial.params)?;
        // Only run if trial has memory-related params
        if params.has_memory_params() {
            let result = retriever.retrieve_shadow(message, &context, &params).await?;
            // Also retrieve with control (champion) params for comparison
            let control = retriever.retrieve_shadow(message, &context, &champion_params).await?;
            let overlap = count_id_overlap(&result.memory_ids, &control.memory_ids);
            self.trial_repo.insert_shadow_retrieval_log(
                &trial.id, chat_id, &timestamp,
                result.total_retrieved as i64,
                control.total_retrieved as i64,
                overlap as i64,
                result.avg_score,
                result.avg_age_days,
            ).await?;
        }
    }
}
```

Add a helper `has_memory_params` on `TrialParams` that returns `true` if any Phase 2 field is `Some`.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p storage -p agent --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(autotuner): shadow retrieval logging per-message for Phase 2 trials"
```

---

### Task 6: Compute Phase 2 metrics in collector

**Files:**
- Modify: `crates/agent/src/autotuner/metric_collector.rs`

- [ ] **Step 1: Wire retrieval_precision + memory_freshness**

In `collect_metrics`, add queries to the `tokio::join!` block:

```rust
// Phase 2 metrics (from shadow retrieval log)
async {
    if let Some(tid) = trial_id_str.as_deref() {
        let precision = self.trial_repo
            .retrieval_precision_for_trial(tid, since).await.unwrap_or(0.0);
        let freshness = self.trial_repo
            .avg_memory_freshness_for_trial(tid, since).await.unwrap_or(0.0);
        (precision, freshness)
    } else {
        (0.0, 0.0)
    }
},
```

Set the fields on `MetricSnapshot`:
```rust
retrieval_precision,
memory_freshness,
```

- [ ] **Step 2: Verify**

Run: `cargo nextest run -p agent -E 'test(metric)' --no-fail-fast`

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(autotuner): compute retrieval_precision and memory_freshness from shadow log"
```

---

### Task 7: Phase 2 constraints in evaluator

**Files:**
- Modify: `crates/config/src/schema/autotuner.rs`
- Modify: `crates/autotuner/src/evaluator.rs`

- [ ] **Step 1: Add Phase 2 constraint thresholds to AutoTunerConfig**

```rust
    pub max_retrieval_precision_drop: f64,       // default 0.05
    pub min_retrieval_recall_improvement: f64,    // default 0.05 (not computed yet — use precision proxy)
    pub max_correction_rate_increase: f64,        // default 0.03 (protect Phase 1 gains)
```

- [ ] **Step 2: Add constraint checks to evaluator**

In the `evaluate` method, add after the existing 5 constraints:

Note: `ConstraintFailure` has 4 fields: `metric: String`, `threshold: f64`, `actual: f64`, `description: String`. NOT a `message` field. Follow the existing pattern in the evaluator.

```rust
// Phase 2 Constraint: retrieval precision must not drop > threshold
if baseline.retrieval_precision > 0.0 {
    let precision_drop = baseline.retrieval_precision - trial.retrieval_precision;
    if precision_drop > self.max_retrieval_precision_drop {
        failures.push(ConstraintFailure {
            metric: "retrieval_precision".into(),
            threshold: self.max_retrieval_precision_drop,
            actual: precision_drop,
            description: format!("Precision dropped {:.1}% (max {:.1}%)",
                precision_drop * 100.0, self.max_retrieval_precision_drop * 100.0),
        });
    }
}

// Phase 2 Constraint: correction rate must not increase > threshold (protect Phase 1)
if trial.correction_rate > baseline.correction_rate {
    let increase = trial.correction_rate - baseline.correction_rate;
    if increase > self.max_correction_rate_increase {
        failures.push(ConstraintFailure {
            metric: "correction_rate_regression".into(),
            threshold: self.max_correction_rate_increase,
            actual: increase,
            description: format!("Correction rate increased {:.1}% (max {:.1}%)",
                increase * 100.0, self.max_correction_rate_increase * 100.0),
        });
    }
}
```

- [ ] **Step 3: Add tests**

```rust
#[test]
fn fails_when_retrieval_precision_drops() { ... }

#[test]
fn fails_when_correction_rate_regresses() { ... }
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p autotuner --no-fail-fast`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(autotuner): add Phase 2 constraint checks for retrieval precision and correction regression"
```

---

### Task 8: Phase 2 readiness gate + wiring

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Add Phase 2 readiness check**

In `crates/agent/src/autotuner/mod.rs`, add a method to the orchestrator:

```rust
/// Check if Phase 2 (memory optimization) is ready to activate.
/// Requires: autotuner running >= 14 days AND champion stable >= 7 days.
pub fn is_phase2_ready(&self) -> bool {
    let champion = self.champion.try_read().ok();
    if let Some(champion) = champion {
        let days_since_promotion = (Utc::now() - champion.promoted_at).num_days();
        let stable_enough = days_since_promotion >= 7;
        // Check autotuner has been running >= 14 days
        // Use the first experiment's created_at as proxy for start date
        // (or a dedicated learning_state key)
        stable_enough // simplified — add the 14-day check via learning_state
    } else {
        false
    }
}
```

Store `autotuner_started_at` in `LearningStateRepo` during first bootstrap or first nightly cycle.

- [ ] **Step 2: Gate shadow retrieval in hooks**

In the `on_message_received` hook, wrap the shadow retrieval section:

```rust
if self.orchestrator.is_phase2_ready() {
    // Run shadow retrieval for Phase 2 trials
    ...
}
```

- [ ] **Step 3: Wire ShadowRetriever in `init/cron.rs`**

Construct `AgentShadowRetriever` from the `UnifiedMemoryService` (which is built during cognitive init) and pass it to the hook:

```rust
let shadow_retriever = if let Some(ref memory_service) = cognitive_result.memory_service {
    Some(Arc::new(agent::autotuner::shadow_retriever::AgentShadowRetriever::new(
        Arc::clone(memory_service),
        config.cognitive.relevance_weight_defaults(),
    )) as Arc<dyn autotuner::ShadowRetriever>)
} else {
    None
};
```

Pass to `AutoTunerHookImpl::new()` as an additional parameter.

- [ ] **Step 4: Wire in builder**

Update `AgentLoopBuilder` to thread the shadow retriever through to the hook construction.

- [ ] **Step 5: Add `autotuner_started_at` persistence**

In the nightly cycle callback, on first run, store:
```rust
if orch.learning_state_repo().get_value("autotuner_started_at").await?.is_none() {
    orch.learning_state_repo().set(
        "autotuner_started_at",
        &serde_json::Value::String(Utc::now().to_rfc3339()),
    ).await?;
}
```

- [ ] **Step 6: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p agent -p app-core --no-fail-fast`

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(autotuner): Phase 2 readiness gate + wire ShadowRetriever into hook"
```

---

### Task 9: Final verification

- [ ] **Step 1: Full workspace compile**

Run: `cargo check --workspace`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Format**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Test all modified crates**

Run: `cargo nextest run -p common -p autotuner -p cognitive -p agent -p storage -p app-core --no-fail-fast`

- [ ] **Step 5: Frontend build** (if types changed)

Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Commit if fixes**

```bash
git commit -m "chore: fix lint/fmt from Phase 2 memory optimization"
```

---

## Dependency Graph

```
Task 1 (TrialParams + 8 fields) ──→ Task 2 (MetricSnapshot + ShadowRetriever trait)
                                          │
                                          ├──→ Task 3 (retrieve_with_overrides on cognitive)
                                          │         │
                                          │         └──→ Task 4 (AgentShadowRetriever impl)
                                          │                   │
                                          │                   └──→ Task 5 (shadow retrieval log + hook)
                                          │                             │
                                          │                             └──→ Task 6 (metric computation)
                                          │
                                          └──→ Task 7 (Phase 2 constraints)
                                                        │
                                                        └──→ Task 8 (readiness gate + wiring)
                                                                      │
                                                                      └──→ Task 9 (verification)
```

Tasks 3 and 7 can run in parallel (both depend on Task 2, independent of each other).
Tasks 4-6 are sequential (each builds on the previous).
Task 8 depends on Tasks 6 and 7.
