# Contextual Query Rewriting Phase 3 — Autotuner Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the query rewriter self-optimizing by integrating its key parameters into the autotuner's shadow experiment → metric collection → champion promotion pipeline, so the system learns the best rewrite aggressiveness, confidence thresholds, and signal weights per-user over time.

**Architecture:** Add 3 rewrite-specific fields to `TrialParams` (shared via the existing `champion_overrides` lock). Instrument the rewriter to log structured events for metric collection. Extend the autotuner's metric pipeline, generator prompt, and evaluator constraints to cover rewrite quality. Shadow rewrite scoring compares trial params against the champion's rewrite behavior without affecting live traffic.

**Tech Stack:** Rust, existing autotuner infrastructure (`TrialParams`, `MetricSnapshot`, `TrialResult`, `ConstraintEvaluator`, `AgentMetricCollector`, generator prompt), `strategy_records` table for instrumentation, `tokio::sync::RwLock` for live param injection.

**Spec:** `docs/superpowers/specs/2026-03-23-contextual-query-rewriting-design.md` (Phase 3 section, Metrics section)

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/common/src/autotuner.rs` | Modify | Add 3 rewrite param fields to `TrialParams` + `has_rewrite_params()` |
| `crates/agent/src/adapters/query_rewriter.rs` | Modify | Accept `champion_overrides` lock, read tunable params per-call, log rewrite events to `strategy_records` |
| `crates/context_engine/src/rewriter.rs` | Modify | Add `RewriteEvent` struct for instrumentation |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Record rewrite metadata in `strategy_records` at Step 10 |
| `crates/storage/src/repos/strategy_repo.rs` | Modify | Add `rewrite_triggered` + `rewrite_source` columns to `strategy_records`, add query for rewrite metrics |
| `crates/storage/migrations/001_initial.sql` | Modify | Add 2 columns to `strategy_records` (pre-release, direct ALTER) |
| `crates/autotuner/src/traits.rs` | Modify | Add `rewrite_trigger_rate` + `rewrite_engagement_rate` to `MetricSnapshot` |
| `crates/autotuner/src/trial.rs` | Modify | Add same fields to `TrialResult` |
| `crates/autotuner/src/metrics.rs` | Modify | Aggregate new fields in volume-weighted merge |
| `crates/agent/src/autotuner/metric_collector.rs` | Modify | Collect rewrite metrics from `strategy_records` |
| `crates/autotuner/src/generator.rs` | Modify | Add Phase 3 params to bounds table |
| `crates/autotuner/src/evaluator.rs` | Modify | Add rewrite engagement constraint |
| `crates/config/src/schema/autotuner.rs` | Modify | Add `max_rewrite_engagement_drop: f64` config field |
| `crates/storage/src/rows/learning.rs` | Modify | Add `rewrite_triggered: i32` + `rewrite_source: Option<String>` to `StrategyRecordRow` |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Pass `champion_overrides` lock to `ContextualQueryRewriter` |

---

## Task 1: Add rewrite params to `TrialParams`

**Files:**
- Modify: `crates/common/src/autotuner.rs:8-42`

- [ ] **Step 1: Write test for new fields**

Add to the test module:

```rust
#[test]
fn has_rewrite_params_detects_phase3_fields() {
    let empty = TrialParams::default();
    assert!(!empty.has_rewrite_params());

    let with_rewrite = TrialParams {
        rewrite_confidence_threshold: Some(0.5),
        ..Default::default()
    };
    assert!(with_rewrite.has_rewrite_params());
}

#[test]
fn phase2_champion_deserializes_with_phase3_fields() {
    let json = r#"{"vector_top_k": 50, "min_similarity": 0.6}"#;
    let params: TrialParams = serde_json::from_str(json).unwrap();
    assert!(params.rewrite_confidence_threshold.is_none());
    assert!(params.rewrite_max_signals.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p common -E 'test(has_rewrite_params) + test(phase2_champion_deserializes_with_phase3)'`
Expected: FAIL

- [ ] **Step 3: Add fields to `TrialParams`**

After the Phase 2 fields (line ~41), add:

```rust
    // Phase 3: Query rewriting
    /// Minimum enrichment confidence to inject into InsightForge (bounds [0.3, 0.95]).
    /// Higher = more selective (fewer rewrites), lower = more aggressive.
    pub rewrite_confidence_threshold: Option<f64>,
    /// Max context signals to include in heuristic enrichment (bounds [1, 6]).
    /// Higher = richer enrichment at the cost of noise.
    pub rewrite_max_signals: Option<usize>,
    /// Minimum enriched query length to accept (bounds [5, 30]).
    /// Shorter enrichments are discarded as too vague.
    pub rewrite_min_enrichment_length: Option<usize>,
```

Add `has_rewrite_params()` method:

```rust
    pub fn has_rewrite_params(&self) -> bool {
        self.rewrite_confidence_threshold.is_some()
            || self.rewrite_max_signals.is_some()
            || self.rewrite_min_enrichment_length.is_some()
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p common`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/autotuner.rs
git commit -m "feat(common): add Phase 3 rewrite params to TrialParams"
```

---

## Task 2: Instrument the rewriter — log rewrite events

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql` (add columns)
- Modify: `crates/storage/src/repos/strategy_repo.rs` (add columns to INSERT + query)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (record rewrite metadata)

- [ ] **Step 1: Add columns to `strategy_records`**

In `crates/storage/migrations/001_initial.sql`, find the `CREATE TABLE strategy_records` statement and add after `retrieved_memory_count`:

```sql
    rewrite_triggered INTEGER DEFAULT 0,
    rewrite_source TEXT
```

Note: Pre-release project — direct schema modification, no migration script needed.

- [ ] **Step 2: Update `StrategyRecordRow` struct**

In `crates/storage/src/rows/learning.rs`, find the `StrategyRecordRow` struct and add:

```rust
    pub rewrite_triggered: i32,
    pub rewrite_source: Option<String>,
```

This is required because the repo uses `RETURNING *` which maps all columns to the struct via `sqlx::FromRow`.

- [ ] **Step 3: Update `StrategyRepo` INSERT**

In `crates/storage/src/repos/strategy_repo.rs`, find the `record_strategy` method's INSERT statement. Add the two new columns to both the column list and values.

Also add new query methods (use `DateTime<Utc>` binding directly, matching existing repo patterns like `memory_relevance_since`):

```rust
/// Fraction of messages where rewrite_triggered = 1, since `since`.
pub async fn rewrite_trigger_rate_since(
    &self,
    since: DateTime<Utc>,
) -> Result<f64, StorageError> {
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*) as total,
                SUM(CASE WHEN rewrite_triggered = 1 THEN 1 ELSE 0 END) as triggered
         FROM strategy_records
         WHERE timestamp >= ?1",
    )
    .bind(since)
    .fetch_one(&self.pool)
    .await?;
    Ok(if row.0 == 0 { 0.0 } else { row.1 as f64 / row.0 as f64 })
}

/// Fraction of rewritten messages where retrieved_memory_count > 0.
pub async fn rewrite_engagement_rate_since(
    &self,
    since: DateTime<Utc>,
) -> Result<f64, StorageError> {
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*) as total,
                SUM(CASE WHEN retrieved_memory_count > 0 THEN 1 ELSE 0 END) as engaged
         FROM strategy_records
         WHERE timestamp >= ?1 AND rewrite_triggered = 1",
    )
    .bind(since)
    .fetch_one(&self.pool)
    .await?;
    Ok(if row.0 == 0 { 0.0 } else { row.1 as f64 / row.0 as f64 })
}
```

- [ ] **Step 4: Record rewrite metadata in `AgentRuntime`**

In `crates/agent/src/agent_runtime/runtime.rs`, at Step 10 where `record_strategy` is called, pass the rewrite info. The `enriched` variable from Step 5.5/6 needs to be carried through to Step 10.

Add to the runtime's internal state tracking (alongside existing `mode_used`, `classification`):

```rust
let rewrite_triggered = enriched.is_some();
let rewrite_source = enriched.as_ref().map(|r| match r.source {
    context_engine::RewriteSource::Heuristic => "heuristic".to_string(),
    context_engine::RewriteSource::Llm => "llm".to_string(),
});
```

Pass these to `record_strategy()`.

- [ ] **Step 5: Verify compilation + existing tests**

Run: `cargo check --workspace && cargo nextest run -p storage -p agent`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add crates/storage/ crates/agent/src/agent_runtime/
git commit -m "feat(storage,agent): instrument query rewriting in strategy_records"
```

---

## Task 3: Add rewrite metrics to autotuner pipeline

**Files:**
- Modify: `crates/autotuner/src/traits.rs:62-83` (`MetricSnapshot`)
- Modify: `crates/autotuner/src/trial.rs:62-80` (`TrialResult`)
- Modify: `crates/autotuner/src/metrics.rs` (aggregation)
- Modify: `crates/agent/src/autotuner/metric_collector.rs:42-170` (collection)

- [ ] **Step 1: Add fields to `MetricSnapshot`**

In `crates/autotuner/src/traits.rs`, add after `knowledge_retention_score`:

```rust
    /// Phase 3: Fraction of messages where query rewriting was triggered.
    pub rewrite_trigger_rate: f64,
    /// Phase 3: Fraction of rewritten messages where retrieved memories were engaged.
    pub rewrite_engagement_rate: f64,
```

- [ ] **Step 2: Add fields to `TrialResult`**

In `crates/autotuner/src/trial.rs`, add after `knowledge_retention_score`:

```rust
    // Phase 3: Query rewriting
    pub rewrite_trigger_rate: f64,
    pub rewrite_engagement_rate: f64,
```

- [ ] **Step 3: Update aggregation in `metrics.rs`**

In `crates/autotuner/src/metrics.rs`, in the `aggregate_to_result` function, the `TrialResult { ... }` struct literal constructs every field explicitly (no `..Default::default()`). You MUST add the two new fields to this struct literal:

```rust
        // Phase 3: Query rewriting
        rewrite_trigger_rate: snapshots.iter().map(|s| w(s) * s.rewrite_trigger_rate).sum(),
        rewrite_engagement_rate: snapshots.iter().map(|s| w(s) * s.rewrite_engagement_rate).sum(),
```

Also update any test `MetricSnapshot` or `TrialResult` struct literals in `metrics.rs` tests — they also construct every field explicitly. Add `rewrite_trigger_rate: 0.0, rewrite_engagement_rate: 0.0` to each.

- [ ] **Step 4: Collect metrics in `AgentMetricCollector`**

In `crates/agent/src/autotuner/metric_collector.rs`, add to the `tokio::join!` block:

```rust
            // Phase 3: rewrite metrics
            self.strategy_repo.rewrite_trigger_rate_since(since),
            self.strategy_repo.rewrite_engagement_rate_since(since),
```

In the destructuring, add the results. Use `.unwrap_or(0.0)` for error handling (matching the existing pattern — e.g., `routing_stability.unwrap_or(1.0)`):

```rust
let rewrite_trigger_rate = rewrite_trigger_result.unwrap_or(0.0);
let rewrite_engagement_rate = rewrite_engagement_result.unwrap_or(0.0);
```

Add these to the `MetricSnapshot` construction.

- [ ] **Step 5: Fix all test `MetricSnapshot` and `TrialResult` struct literals**

The new fields have defaults (0.0) so existing tests should compile with `..Default::default()`. If any tests construct these structs with explicit fields, add the new fields.

- [ ] **Step 6: Verify**

Run: `cargo nextest run -p autotuner -p agent -E 'test(metric) + test(aggregate) + test(collect)'`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/autotuner/ crates/agent/src/autotuner/
git commit -m "feat(autotuner): add rewrite_trigger_rate and rewrite_engagement_rate metrics"
```

---

## Task 4: Inject champion params into the rewriter

**Files:**
- Modify: `crates/agent/src/adapters/query_rewriter.rs:321-335` (struct + constructor)
- Modify: `crates/agent/src/agent_loop/builder.rs:817-826` (wiring)

- [ ] **Step 1: Write test for champion param override**

```rust
#[tokio::test]
async fn champion_overrides_confidence_threshold() {
    let rewriter = ContextualQueryRewriter::heuristic_only();
    let overrides = Arc::new(std::sync::RwLock::new(Some(common::TrialParams {
        rewrite_confidence_threshold: Some(0.95), // Very high — should suppress most rewrites
        ..Default::default()
    })));
    let rewriter = rewriter.with_champion_overrides(overrides);
    let ctx = finance_context();
    // Heuristic produces confidence ~0.75, but threshold is 0.95 → should return None
    let result = rewriter.rewrite("how are we doing?", &ctx).await;
    assert!(result.is_none(), "High confidence threshold should suppress low-confidence rewrites");
}

#[tokio::test]
async fn champion_overrides_max_signals() {
    let overrides = Arc::new(std::sync::RwLock::new(Some(common::TrialParams {
        rewrite_max_signals: Some(1), // Only 1 signal allowed
        ..Default::default()
    })));
    let rewriter = ContextualQueryRewriter::heuristic_only().with_champion_overrides(overrides);
    let ctx = RetrievalContext {
        active_skill: Some("finance-management".into()),
        active_task: Some(ActiveTaskContext {
            title: "March budget".into(),
            project_name: Some("Q1 Finance".into()),
            domain: Some("finance".into()),
        }),
        situation: Some(context_engine::UserSituationSnapshot {
            energy_level: 0.2, // Aggressive mode would normally allow 4 signals
            ..Default::default()
        }),
        ..Default::default()
    };
    let result = rewriter.rewrite("how are we doing?", &ctx).await;
    assert!(result.is_some());
    // With max_signals=1, enrichment should be shorter (fewer signals)
    let enriched = result.unwrap().enriched_query;
    // Should contain at most 1 signal (correction or task or skill)
    assert!(!enriched.contains(", "), "Max 1 signal should not have comma-separated signals");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(champion_overrides)'`
Expected: FAIL

- [ ] **Step 3: Add champion overrides to `ContextualQueryRewriter`**

Add field and builder method:

```rust
pub struct ContextualQueryRewriter {
    llm_provider: Option<providers::DynProvider>,
    rewriter_model: Option<String>,
    timeout_ms: u64,
    champion_overrides: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
}

impl ContextualQueryRewriter {
    // ... existing new() and heuristic_only() ...

    pub fn with_champion_overrides(
        mut self,
        overrides: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        self.champion_overrides = Some(overrides);
        self
    }

    fn read_champion(&self) -> Option<common::TrialParams> {
        self.champion_overrides.as_ref()
            .and_then(|lock| lock.read().ok())
            .and_then(|guard| guard.clone())
    }

    fn effective_max_signals(&self, ctx: &RetrievalContext) -> usize {
        if let Some(params) = self.read_champion() {
            if let Some(max) = params.rewrite_max_signals {
                return max;
            }
        }
        // Default: energy-adaptive
        if self.is_aggressive(ctx) { 4 } else { 2 }
    }

    fn effective_confidence_threshold(&self) -> f32 {
        self.read_champion()
            .and_then(|p| p.rewrite_confidence_threshold)
            .map(|t| t as f32)
            .unwrap_or(0.0) // Default: no threshold (accept all)
    }

    fn effective_min_enrichment_length(&self) -> usize {
        self.read_champion()
            .and_then(|p| p.rewrite_min_enrichment_length)
            .unwrap_or(10) // Current hardcoded value
    }
}
```

- [ ] **Step 4: Use champion params in `heuristic_rewrite` and `rewrite`**

In `heuristic_rewrite`, replace `self.max_signals(ctx)` with `self.effective_max_signals(ctx)`.

Replace the hardcoded `enriched_query.len() < 10` check with `enriched_query.len() < self.effective_min_enrichment_length()`.

In `rewrite()` (the `QueryRewriter` impl), after producing a result, check confidence against threshold:

```rust
// Apply champion confidence threshold
if let Some(ref result) = result {
    if result.confidence < self.effective_confidence_threshold() {
        debug!(
            confidence = result.confidence,
            threshold = self.effective_confidence_threshold(),
            "⏭️ QueryRewriter: below champion confidence threshold"
        );
        return None;
    }
}
```

- [ ] **Step 5: Wire champion overrides in builder**

In `crates/agent/src/agent_loop/builder.rs`, after constructing `ContextualQueryRewriter::new(...)`, add:

```rust
        // Phase 3: Wire autotuner champion overrides for rewrite params
        if let Some(ref orchestrator) = self.autotuner {
            if let Some(sink) = orchestrator.memory_param_sink() {
                query_rewriter = query_rewriter.with_champion_overrides(sink);
            }
        }
```

Note: This reuses the SAME `memory_param_sink` lock that `UnifiedMemoryService` uses. The `TrialParams` struct carries ALL params in one place — both memory and rewrite fields.

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p agent -E 'test(query_rewriter) + test(champion_overrides)'`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/adapters/query_rewriter.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): inject autotuner champion params into ContextualQueryRewriter"
```

---

## Task 5: Update generator prompt and evaluator constraints

**Files:**
- Modify: `crates/autotuner/src/generator.rs:161-182` (bounds table)
- Modify: `crates/autotuner/src/evaluator.rs:36-75` (constraints)
- Modify: `crates/config/src/schema/autotuner.rs` (new config field)

- [ ] **Step 1: Add Phase 3 params to the generator bounds table**

In `crates/autotuner/src/generator.rs`, after the Phase 2 rows in the parameter table (line ~182), add:

```rust
| rewrite_confidence_threshold   | 0.30  | 0.95  | 0.05  | Minimum confidence to accept an enrichment |\n\
| rewrite_max_signals            | 1     | 6     | 1     | Max context signals in heuristic enrichment |\n\
| rewrite_min_enrichment_length  | 5     | 30    | 5     | Min chars for an enrichment to be accepted |\n\n",
```

- [ ] **Step 2: Add rewrite engagement constraint to evaluator**

In `crates/autotuner/src/evaluator.rs`, add:

```rust
    // Phase 3 constraint
    /// rewrite_engagement_rate must not decrease by more than this absolute amount.
    max_rewrite_engagement_drop: f64,
```

Wire from config:
1. In `crates/config/src/schema/autotuner.rs`, add to `AutoTunerConfig`:
```rust
    #[serde(default = "default_max_rewrite_engagement_drop")]
    pub max_rewrite_engagement_drop: f64,
```
And add the default function: `fn default_max_rewrite_engagement_drop() -> f64 { 0.10 }`

2. In `ConstraintEvaluator::from_config()`, add: `max_rewrite_engagement_drop: config.max_rewrite_engagement_drop,`

Add check in `evaluate()`:

```rust
        // Phase 3: Rewrite engagement
        let engagement_drop = baseline.rewrite_engagement_rate - trial.rewrite_engagement_rate;
        if engagement_drop > self.max_rewrite_engagement_drop {
            failures.push(ConstraintFailure {
                metric: "rewrite_engagement_rate".into(),
                threshold: self.max_rewrite_engagement_drop,
                actual: engagement_drop,
                description: format!(
                    "Rewrite engagement dropped by {engagement_drop:.4} (max allowed: {:.4})",
                    self.max_rewrite_engagement_drop
                ),
            });
        }
```

- [ ] **Step 3: Update promotion constraints in generator prompt**

In the promotion constraints section (line ~186-195), add:

```rust
- `rewrite_engagement_rate` must not decrease by **> 10%** absolute from champion\n\
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p autotuner`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add crates/autotuner/ crates/config/
git commit -m "feat(autotuner): add Phase 3 rewrite params to generator prompt and evaluator"
```

---

## Task 6: Final verification and integration test

- [ ] **Step 1: Write integration test**

In `crates/agent/src/adapters/query_rewriter.rs` tests:

```rust
#[tokio::test]
async fn autotuner_champion_overrides_affect_rewrite_behavior() {
    // Simulate the full autotuner → rewriter flow
    let overrides = Arc::new(std::sync::RwLock::new(None));
    let rewriter = ContextualQueryRewriter::heuristic_only()
        .with_champion_overrides(Arc::clone(&overrides));
    let ctx = finance_context();

    // No champion → default behavior
    let result1 = rewriter.rewrite("how are we doing?", &ctx).await;
    assert!(result1.is_some());

    // Promote a champion with high threshold
    *overrides.write().unwrap() = Some(common::TrialParams {
        rewrite_confidence_threshold: Some(0.95),
        ..Default::default()
    });

    // Same query, now suppressed by threshold
    let result2 = rewriter.rewrite("how are we doing?", &ctx).await;
    assert!(result2.is_none(), "Champion threshold should suppress low-confidence rewrite");

    // Clear champion
    *overrides.write().unwrap() = None;

    // Back to default behavior
    let result3 = rewriter.rewrite("how are we doing?", &ctx).await;
    assert!(result3.is_some());
}
```

- [ ] **Step 2: Run full verification**

Run: `cargo fmt --all --check`
Run: `cargo clippy --workspace --all-targets --all-features`
Run: `cargo nextest run --workspace`
Run: `cargo test --workspace --doc`
Expected: ALL PASS, 0 warnings

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test(agent): integration test for autotuner → rewriter champion override flow"
```

---

## Summary

| Task | Description | Lines | Dependencies |
|------|-------------|-------|-------------|
| 1 | Add rewrite params to `TrialParams` | ~20 | None |
| 2 | Instrument rewriter in strategy_records | ~60 | Task 1 |
| 3 | Add rewrite metrics to autotuner pipeline | ~40 | Task 2 |
| 4 | Inject champion params into rewriter | ~80 | Tasks 1, 3 |
| 5 | Generator prompt + evaluator constraints | ~30 | Tasks 1, 3 |
| 6 | Final verification + integration test | ~30 | Tasks 4, 5 |

**Total: ~260 lines of new/changed code**

**What Phase 3 delivers:**
- Rewrite confidence threshold, max signals, and min enrichment length are autotuner-tunable
- The nightly autotuner cycle generates trial variants that experiment with different rewrite aggressiveness levels
- Shadow evaluation scores rewrite quality via `rewrite_trigger_rate` and `rewrite_engagement_rate`
- Champion promotion requires rewrite engagement not to drop >10%
- The system learns per-user whether aggressive (low threshold, many signals) or conservative (high threshold, few signals) rewriting produces better retrieval engagement
- Zero manual tuning needed — the system self-optimizes over days/weeks
