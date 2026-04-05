# Simulator Signal Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire four missing signal categories into the simulator so it measures cost efficiency, cache utilization, memory retention decay, and estimation accuracy — transforming the simulator from "does the pipeline work?" to "is the system getting smarter and more efficient over time?"

**Architecture:** Each metric adds new fields to `EpochAccumulator` and `MetricSnapshot`, new measurement functions in `metrics/`, and new data flowing from `harness.rs` (heuristic path) and `agent_harness.rs` (agent path). The simulation provider gains realistic cache token generation. Task completion actions gain duration data. All changes are additive — no existing metrics are altered.

**Tech Stack:** Rust, SQLite (`sqlx`), existing `cognitive::services::fsrs5`, existing `providers::Usage` type

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `crates/simulator/src/metrics/mod.rs` | Modify | Add 5 fields to `MetricSnapshot`, 8 fields to `EpochAccumulator`, add `cost_efficiency` to `BaselineMetrics` + regression checks |
| `crates/simulator/src/metrics/cognitive.rs` | Modify | Add `measure_retrievability_distribution()` that returns (avg, min, p25, p50) |
| `crates/simulator/src/metrics/cost.rs` | Create | New module: `measure_cost_efficiency()`, `measure_cache_hit_rate()` from `usage_records` |
| `crates/simulator/src/providers/simulation_provider.rs` | Modify | Generate nonzero `cache_read_tokens` / `cache_write_tokens` |
| `crates/simulator/src/persona/types.rs` | Modify | Add `estimated_duration_mins` field to `CompleteTask` variant |
| `crates/simulator/src/persona/mod.rs` | Modify | Generate estimated + actual duration when completing tasks |
| `crates/simulator/src/actions.rs` | Modify | Pass duration data through `TaskCompleted` event, emit `EstimationRecorded` |
| `crates/simulator/src/harness.rs` | Modify | Accumulate cost/cache/estimation counters, pass to `snapshot()`, insert `cache_read_tokens`/`cache_write_tokens` to `usage_records` |
| `crates/simulator/src/agent_harness.rs` | Modify | Capture `cache_read_tokens`/`cache_write_tokens` from `UsageReport` events |
| `crates/simulator/src/agent_types.rs` | Modify | Add cost/cache fields to `AgentResult` |
| `crates/simulator/src/report.rs` | No change | `MetricSnapshot` is serialized automatically via serde |
| `tests/simulation/scenarios/agent_validation_1week.toml` | Modify | Add checkpoint assertions for new metrics |

---

### Task 1: Cost Metrics Module

**Files:**
- Create: `crates/simulator/src/metrics/cost.rs`
- Modify: `crates/simulator/src/metrics/mod.rs` (add `pub mod cost;`)

This task adds two SQL-backed measurement functions that query the `usage_records` table. These functions will be called at epoch end from `harness.rs`, just like `measure_community_stability` and `measure_insight_usefulness`.

- [ ] **Step 1: Write the failing test for `measure_cost_efficiency`**

In `crates/simulator/src/metrics/cost.rs`:

```rust
//! Cost and token efficiency metrics.

/// Measure cost efficiency: total_cost_usd / total_outcomes.
///
/// Outcomes = tasks_completed + facts_extracted (passed in, not queried).
/// Returns `f64::INFINITY` when outcomes == 0, and `0.0` when no usage records exist.
pub async fn measure_cost_efficiency(
    pool: &sqlx::SqlitePool,
    since: &str,
    outcomes: u32,
) -> f64 {
    todo!()
}

/// Measure cache hit rate: sum(cache_read_tokens) / sum(prompt_tokens).
///
/// Returns 0.0 when no usage records exist or prompt_tokens is zero.
pub async fn measure_cache_hit_rate(pool: &sqlx::SqlitePool, since: &str) -> f64 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("in-memory pool");
        let inner = pool.inner().clone();
        std::mem::forget(pool);
        inner
    }

    #[tokio::test]
    async fn cost_efficiency_returns_zero_on_empty() {
        let pool = setup_pool().await;
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 5).await;
        assert!((r - 0.0).abs() < 1e-9, "no records → cost 0.0, got {r}");
    }

    #[tokio::test]
    async fn cost_efficiency_with_records() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 100, 50, 0.05, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 2).await;
        assert!((r - 0.025).abs() < 1e-9, "0.05 / 2 = 0.025, got {r}");
    }

    #[tokio::test]
    async fn cost_efficiency_infinity_on_zero_outcomes() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 100, 50, 0.05, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 0).await;
        assert!(r.is_infinite(), "zero outcomes → infinity, got {r}");
    }

    #[tokio::test]
    async fn cache_hit_rate_returns_zero_on_empty() {
        let pool = setup_pool().await;
        let r = measure_cache_hit_rate(&pool, "2026-01-01T00:00:00Z").await;
        assert!((r - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn cache_hit_rate_with_records() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              cache_read_tokens, cache_write_tokens, estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 200, 50, 80, 20, 0.01, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cache_hit_rate(&pool, "2026-01-01T00:00:00Z").await;
        // 80 / 200 = 0.4
        assert!((r - 0.4).abs() < 1e-9, "expected 0.4, got {r}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator -E 'test(cost_efficiency)' -E 'test(cache_hit_rate)' 2>&1 | tail -20`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement the functions**

Replace the `todo!()` bodies in `crates/simulator/src/metrics/cost.rs`:

```rust
pub async fn measure_cost_efficiency(
    pool: &sqlx::SqlitePool,
    since: &str,
    outcomes: u32,
) -> f64 {
    let total_cost: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) \
         FROM usage_records WHERE timestamp >= ?1",
    )
    .bind(since)
    .fetch_one(pool)
    .await
    .unwrap_or(0.0);

    if total_cost == 0.0 {
        return 0.0;
    }
    if outcomes == 0 {
        return f64::INFINITY;
    }
    total_cost / outcomes as f64
}

pub async fn measure_cache_hit_rate(pool: &sqlx::SqlitePool, since: &str) -> f64 {
    let (cache_read, prompt): (f64, f64) = sqlx::query_as(
        "SELECT \
             COALESCE(SUM(cache_read_tokens), 0.0), \
             COALESCE(SUM(prompt_tokens), 0.0) \
         FROM usage_records WHERE timestamp >= ?1",
    )
    .bind(since)
    .fetch_one(pool)
    .await
    .unwrap_or((0.0, 0.0));

    if prompt == 0.0 {
        return 0.0;
    }
    cache_read / prompt
}
```

- [ ] **Step 4: Register the module**

In `crates/simulator/src/metrics/mod.rs`, add after the existing `pub mod system;` line:

```rust
pub mod cost;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p simulator -E 'test(cost_efficiency)' -E 'test(cache_hit_rate)'`
Expected: 5 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/metrics/cost.rs crates/simulator/src/metrics/mod.rs
git commit -m "feat(simulator): add cost efficiency and cache hit rate metric functions"
```

---

### Task 2: Extend MetricSnapshot and EpochAccumulator

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs`

Add the new metric fields. Keep `snapshot()` signature unchanged for now — the new metrics are set directly after `snapshot()` is called (same pattern as `update_latest_cognitive()`).

- [ ] **Step 1: Add fields to MetricSnapshot**

In `crates/simulator/src/metrics/mod.rs`, inside `MetricSnapshot` struct, after the `wall_time_per_epoch_ms` field (line 52), add:

```rust
    // Tier 7 — cost economics
    pub cost_per_outcome_usd: f64,
    pub cache_hit_rate: f64,
    // Tier 4 — cognitive depth (extended)
    pub retrievability_min: f64,
    pub retrievability_p25: f64,
    // Tier 2 — behavioral quality (extended)
    pub estimation_deviation_avg: f64,
```

- [ ] **Step 2: Add fields to EpochAccumulator**

In the `EpochAccumulator` struct, after the `error_injected: u32` field (line 133), add:

```rust
    // Cost tracking
    pub total_cost_usd: f64,
    pub total_prompt_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_write_tokens: u64,
    // Estimation tracking
    pub estimation_deviation_sum: f64,
    pub estimation_count: u32,
    // Outcome tracking (for cost efficiency denominator)
    pub epoch_outcomes: u32,
```

- [ ] **Step 3: Add `cost_per_outcome_usd` and `cache_hit_rate` to BaselineMetrics**

In the `BaselineMetrics` struct, after `fact_extraction_accuracy: f64` (line 71), add:

```rust
    pub cost_per_outcome_usd: f64,
    pub cache_hit_rate: f64,
```

- [ ] **Step 4: Add update method for new metrics**

After the `update_latest_cognitive` method (around line 363), add:

```rust
    /// Update the latest snapshot with cost and estimation metrics computed externally.
    pub fn update_latest_cost_and_estimation(
        &mut self,
        cost_per_outcome_usd: f64,
        cache_hit_rate: f64,
        retrievability_min: f64,
        retrievability_p25: f64,
        estimation_deviation_avg: f64,
    ) {
        if let Some(snap) = self.timeline.last_mut() {
            snap.cost_per_outcome_usd = cost_per_outcome_usd;
            snap.cache_hit_rate = cache_hit_rate;
            snap.retrievability_min = retrievability_min;
            snap.retrievability_p25 = retrievability_p25;
            snap.estimation_deviation_avg = estimation_deviation_avg;
        }
    }
```

- [ ] **Step 5: Include new baselines in `compute_baselines`**

In `compute_baselines()`, inside the `for s in &self.timeline` loop (around line 383), add accumulation:

```rust
            bl.cost_per_outcome_usd += s.cost_per_outcome_usd;
            bl.cache_hit_rate += s.cache_hit_rate;
```

And after the existing divisors (after line 394), add:

```rust
        bl.cost_per_outcome_usd /= n;
        bl.cache_hit_rate /= n;
```

- [ ] **Step 6: Add regression checks for cost metrics**

In `check_regressions()`, add a cost regression check (higher cost = regression) in the "Token efficiency: regression = increase" block (after line 428). Add a new block right after it:

```rust
        // Cost per outcome: regression = increase.
        if bl.cost_per_outcome_usd > 0.0 {
            let pct = (latest.cost_per_outcome_usd - bl.cost_per_outcome_usd)
                / bl.cost_per_outcome_usd
                * 100.0;
            if pct > threshold_pct {
                alerts.push(RegressionAlert {
                    metric: "cost_per_outcome_usd".into(),
                    baseline: bl.cost_per_outcome_usd,
                    current: latest.cost_per_outcome_usd,
                    regression_pct: pct,
                });
            }
        }
```

And add `cache_hit_rate` to the `higher_is_better` checks array:

```rust
            (
                "cache_hit_rate",
                bl.cache_hit_rate,
                latest.cache_hit_rate,
                true, // skip when 0.0 — undefined, not regressed
            ),
```

- [ ] **Step 7: Run existing tests to verify nothing breaks**

Run: `cargo nextest run -p simulator -E 'test(snapshot)' -E 'test(baselines)' -E 'test(regression)' -E 'test(cumulative)'`
Expected: All existing tests PASS (new fields default to 0.0 via `Default`)

- [ ] **Step 8: Commit**

```bash
git add crates/simulator/src/metrics/mod.rs
git commit -m "feat(simulator): extend MetricSnapshot with cost, cache, retrievability, and estimation fields"
```

---

### Task 3: Retrievability Distribution

**Files:**
- Modify: `crates/simulator/src/metrics/cognitive.rs`

Replace the single average with a distribution that returns (avg, min, p25, p50). This replaces the existing `measure_average_retrievability` — the avg value is backwards-compatible.

- [ ] **Step 1: Write the failing test**

In `crates/simulator/src/metrics/cognitive.rs`, add to the `tests` module:

```rust
    #[tokio::test]
    async fn retrievability_distribution_with_facts() {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("pool");
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &cognitive::cognitive_migrations(),
        )
        .await
        .expect("migrations");
        std::mem::forget(pool);

        // Insert 4 facts with different stabilities and ages.
        // Fact 1: stability=10, recorded 1 day ago → R = 1/(1+1/90) ≈ 0.989
        // Fact 2: stability=1, recorded 9 days ago → R = 1/(1+9/9) = 0.5
        // Fact 3: stability=2, recorded 18 days ago → R = 1/(1+18/18) = 0.5
        // Fact 4: stability=0.5, recorded 9 days ago → R = 1/(1+9/4.5) = 0.333
        let now = "2026-01-10T00:00:00Z";
        for (id, stab, recorded) in [
            ("f1", 10.0, "2026-01-09T00:00:00Z"),
            ("f2", 1.0, "2026-01-01T00:00:00Z"),
            ("f3", 2.0, "2025-12-23T00:00:00Z"),
            ("f4", 0.5, "2026-01-01T00:00:00Z"),
        ] {
            sqlx::query(
                "INSERT INTO semantic_facts \
                 (id, domain, subject, predicate, object, confidence, source, \
                  recorded_at, stability, access_count, memory_type, scope_type) \
                 VALUES (?1, 'test', 's', 'p', 'o', 0.9, 'sim', ?2, ?3, 0, 'semantic', 'session')",
            )
            .bind(id)
            .bind(recorded)
            .bind(stab)
            .execute(&inner)
            .await
            .unwrap();
        }

        let dist = measure_retrievability_distribution(&inner, now).await;
        assert!(dist.avg > 0.3 && dist.avg < 0.8, "avg={}", dist.avg);
        assert!(dist.min > 0.2 && dist.min < 0.5, "min={}", dist.min);
        assert!(dist.p25 > 0.3 && dist.p25 < 0.6, "p25={}", dist.p25);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator -E 'test(retrievability_distribution)'`
Expected: FAIL — function doesn't exist

- [ ] **Step 3: Implement `RetrievabilityDistribution` and `measure_retrievability_distribution`**

At the top of `crates/simulator/src/metrics/cognitive.rs`, add the struct and function:

```rust
/// Distribution of retrievability scores across all active facts.
pub struct RetrievabilityDistribution {
    pub avg: f64,
    pub min: f64,
    pub p25: f64,
    pub p50: f64,
}

/// Measure retrievability distribution of all active semantic facts.
///
/// Returns percentiles (min, p25, p50) alongside the average, giving visibility
/// into the tail — are *any* facts being forgotten, even if the average looks healthy?
pub async fn measure_retrievability_distribution(
    pool: &sqlx::SqlitePool,
    simulated_now: &str,
) -> RetrievabilityDistribution {
    let rows: Vec<(f64, f64)> = sqlx::query_as(
        "SELECT stability, CAST(strftime('%s', recorded_at) AS REAL) \
         FROM semantic_facts \
         WHERE superseded_at IS NULL AND stability > 0",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return RetrievabilityDistribution {
            avg: 0.0,
            min: 0.0,
            p25: 0.0,
            p50: 0.0,
        };
    }

    let now = chrono::DateTime::parse_from_rfc3339(simulated_now)
        .unwrap_or_else(|_| chrono::Utc::now().into())
        .timestamp() as f64;

    let mut scores: Vec<f64> = rows
        .iter()
        .map(|&(stability, recorded_unix)| {
            let elapsed_days = ((now - recorded_unix) / 86400.0).max(0.0);
            cognitive::services::fsrs5::retrievability(elapsed_days, stability)
        })
        .collect();

    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = scores.len();
    let avg = scores.iter().sum::<f64>() / n as f64;
    let min = scores[0];
    let p25 = scores[n / 4];
    let p50 = scores[n / 2];

    RetrievabilityDistribution { avg, min, p25, p50 }
}
```

- [ ] **Step 4: Update `measure_average_retrievability` to delegate**

Replace the body of the existing `measure_average_retrievability` function to delegate:

```rust
pub async fn measure_average_retrievability(pool: &sqlx::SqlitePool, simulated_now: &str) -> f64 {
    let dist = measure_retrievability_distribution(pool, simulated_now).await;
    if dist.avg == 0.0 && dist.min == 0.0 {
        // Preserve legacy behavior: 1.0 when no facts exist.
        // The distribution function returns 0.0 for empty (correct), but callers
        // of the old function expect 1.0. This will be removed once all callers
        // switch to the distribution function.
        let count: Result<(i64,), _> = sqlx::query_as(
            "SELECT COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL AND stability > 0",
        )
        .fetch_one(pool)
        .await;
        if count.map(|(n,)| n).unwrap_or(0) == 0 {
            return 1.0;
        }
    }
    dist.avg
}
```

- [ ] **Step 5: Run all cognitive tests**

Run: `cargo nextest run -p simulator -E 'test(retrievab)'`
Expected: All tests PASS including the new distribution test and the existing `retrievability_returns_one_for_empty`

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/metrics/cognitive.rs
git commit -m "feat(simulator): add retrievability distribution (min, p25, p50) alongside average"
```

---

### Task 4: Realistic Cache Tokens in SimulationProvider

**Files:**
- Modify: `crates/simulator/src/providers/simulation_provider.rs`

The mock provider currently returns `cache_read_tokens: 0` and `cache_write_tokens: 0` for every response. Real providers return nonzero cache tokens when the prompt has shared prefixes (system prompt, tool definitions). Generate realistic values so cache_hit_rate is testable.

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/simulator/src/providers/simulation_provider.rs`:

```rust
    #[tokio::test]
    async fn cache_tokens_are_nonzero() {
        let provider = SimulationProvider::new(42);
        let messages = vec![providers::types::Message::user("test message")];
        let response = provider
            .chat(messages, &[], &Default::default())
            .await
            .unwrap();
        // After the first call, cache_write_tokens should be > 0 (writing system prompt to cache).
        // After subsequent calls, cache_read_tokens should be > 0 (reading from cache).
        // We test the second call.
        let messages2 = vec![providers::types::Message::user("second message")];
        let response2 = provider
            .chat(messages2, &[], &Default::default())
            .await
            .unwrap();
        assert!(
            response.usage.cache_write_tokens > 0 || response2.usage.cache_read_tokens > 0,
            "at least one call should have nonzero cache tokens: \
             first write={}, second read={}",
            response.usage.cache_write_tokens,
            response2.usage.cache_read_tokens,
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator -E 'test(cache_tokens_are_nonzero)'`
Expected: FAIL — both values are 0

- [ ] **Step 3: Generate realistic cache tokens**

In `crates/simulator/src/providers/simulation_provider.rs`, find where `cache_read_tokens: 0` and `cache_write_tokens: 0` are set in `Usage` construction (two places: the reactive/tool-calling response around line 253-257, and the direct mode response around line 281-284).

Replace both occurrences with cache token generation. The provider already has `self.call_count` (or use `self.rng`). Add a call counter to track whether this is the first call or a subsequent one.

First, add a call counter to `SimulationProvider`. Find the struct definition and add:

```rust
    call_count: std::sync::atomic::AtomicU32,
```

Initialize it in `new()`:

```rust
    call_count: std::sync::atomic::AtomicU32::new(0),
```

Then in the `chat` method, at the top, increment and read:

```rust
        let call_num = self.call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
```

Replace both `cache_read_tokens: 0, cache_write_tokens: 0` blocks with:

```rust
                    // Simulate prompt caching: first call writes, subsequent calls read.
                    // ~40% of prompt tokens are cacheable (system prompt + tool defs).
                    cache_read_tokens: if call_num > 0 {
                        (prompt_tokens as f64 * 0.4) as u32
                    } else {
                        0
                    },
                    cache_write_tokens: if call_num == 0 {
                        (prompt_tokens as f64 * 0.4) as u32
                    } else {
                        0
                    },
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator -E 'test(cache_tokens)' -E 'test(simulation_provider)' -E 'test(returns_tool_calls)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/providers/simulation_provider.rs
git commit -m "feat(simulator): generate realistic cache_read/write tokens in mock provider"
```

---

### Task 5: Estimation Data in Task Completion

**Files:**
- Modify: `crates/simulator/src/persona/types.rs`
- Modify: `crates/simulator/src/persona/mod.rs`
- Modify: `crates/simulator/src/actions.rs`

Wire estimation data through the task completion flow: persona generates estimated+actual duration, actions.rs emits `TaskCompleted` with durations and `EstimationRecorded`.

- [ ] **Step 1: Add fields to CompleteTask variant**

In `crates/simulator/src/persona/types.rs`, modify the `CompleteTask` variant (around line 126):

```rust
    CompleteTask {
        task_ref: String,
        estimated_duration_mins: Option<u32>,
        actual_duration_mins: Option<u32>,
    },
```

- [ ] **Step 2: Fix all existing CompleteTask construction sites**

In `crates/simulator/src/persona/mod.rs`, find `SimulatedToolAction::CompleteTask` (around line 167):

```rust
                    Some(SimulatedToolAction::CompleteTask {
                        task_ref: self.created_task_titles[idx].clone(),
                        estimated_duration_mins: Some(self.rng.random_range(15..120)),
                        actual_duration_mins: Some(self.rng.random_range(10..150)),
                    })
```

Also fix the test in `actions.rs` (around line 324):

```rust
        let action = SimulatedToolAction::CompleteTask {
            task_ref: "task-abc".into(),
            estimated_duration_mins: Some(30),
            actual_duration_mins: Some(45),
        };
```

And any other construction sites found via compiler errors — run `cargo build -p simulator 2>&1 | head -40` to find them.

- [ ] **Step 3: Update ActionExecutor to use durations**

In `crates/simulator/src/actions.rs`, modify the `CompleteTask` match arm:

```rust
            SimulatedToolAction::CompleteTask {
                task_ref,
                estimated_duration_mins,
                actual_duration_mins,
            } => {
                debug!(task_ref = %task_ref, "action: CompleteTask");

                let deviation_pct = match (estimated_duration_mins, actual_duration_mins) {
                    (Some(est), Some(act)) if *est > 0 => {
                        Some((*act as f64 - *est as f64) / *est as f64 * 100.0)
                    }
                    _ => None,
                };

                self.bus.publish(DomainEvent::TaskCompleted {
                    task_id: task_ref.clone(),
                    actual_duration_mins: actual_duration_mins.map(|m| m as i64),
                    estimated_duration_mins: estimated_duration_mins.map(|m| m as i64),
                    deviation_pct,
                });

                // Also emit EstimationRecorded for explicit tracking.
                if let (Some(est), Some(act)) = (estimated_duration_mins, actual_duration_mins) {
                    let dev = if *est > 0 {
                        (*act as f64 - *est as f64) / *est as f64 * 100.0
                    } else {
                        0.0
                    };
                    self.bus.publish(DomainEvent::EstimationRecorded {
                        task_id: task_ref.clone(),
                        estimated_mins: *est,
                        actual_mins: *act,
                        deviation_pct: dev,
                    });
                }

                // UPDATE the task row to completed.
                let _ = sqlx::query(
                    "UPDATE tasks SET status = 'completed', completed = 1, completed_at = ?, updated_at = ? WHERE title = ?",
                )
                .bind(simulated_now.to_rfc3339())
                .bind(simulated_now.to_rfc3339())
                .bind(task_ref)
                .execute(&self.pool)
                .await;
            }
```

- [ ] **Step 4: Run the actions tests**

Run: `cargo nextest run -p simulator -E 'test(complete_task)' -E 'test(create_task)'`
Expected: All PASS

- [ ] **Step 5: Write test for estimation event emission**

Add to the test module in `crates/simulator/src/actions.rs`:

```rust
    #[tokio::test]
    async fn complete_task_emits_estimation_recorded() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::CompleteTask {
            task_ref: "task-xyz".into(),
            estimated_duration_mins: Some(30),
            actual_duration_mins: Some(45),
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let _completed = rx.try_recv().expect("should receive TaskCompleted");
        let estimation = rx.try_recv().expect("should receive EstimationRecorded");
        assert!(
            matches!(
                estimation,
                DomainEvent::EstimationRecorded {
                    estimated_mins: 30,
                    actual_mins: 45,
                    ..
                }
            ),
            "got {estimation:?}"
        );
    }
```

- [ ] **Step 6: Run the new test**

Run: `cargo nextest run -p simulator -E 'test(estimation_recorded)'`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/simulator/src/persona/types.rs crates/simulator/src/persona/mod.rs crates/simulator/src/actions.rs
git commit -m "feat(simulator): wire estimation durations through task completion flow"
```

---

### Task 6: Wire Everything into the Harness

**Files:**
- Modify: `crates/simulator/src/harness.rs`
- Modify: `crates/simulator/src/agent_types.rs`
- Modify: `crates/simulator/src/agent_harness.rs`

This is the integration task. The harness accumulates cost/cache/estimation counters during each epoch, then passes them to the new `update_latest_cost_and_estimation()` method after `snapshot()`.

- [ ] **Step 1: Add cost fields to AgentResult**

In `crates/simulator/src/agent_types.rs`, add to `AgentResult` (after `breakpoints`):

```rust
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub cost_usd: f64,
```

- [ ] **Step 2: Capture tokens from agent events**

In `crates/simulator/src/agent_harness.rs`, the event drain task (line 283-326) currently destructures `UsageReport` for logging but doesn't accumulate the values. Change the drain to accumulate:

Before the `event_drain` spawn (around line 283), add shared accumulators:

```rust
        let drain_prompt = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let drain_completion = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let drain_cache_read = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let drain_cache_write = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        // Cost as integer millionths to avoid AtomicF64
        let drain_cost_micro = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

        let dp = drain_prompt.clone();
        let dc = drain_completion.clone();
        let dcr = drain_cache_read.clone();
        let dcw = drain_cache_write.clone();
        let dcm = drain_cost_micro.clone();
```

Inside the `UsageReport` match arm (around line 305-315), add accumulation:

```rust
                    AgentEvent::UsageReport {
                        prompt_tokens,
                        completion_tokens,
                        cache_read_tokens,
                        cache_write_tokens,
                        estimated_cost_usd,
                        response_time_ms,
                        ..
                    } => {
                        eprintln!(
                            "      $ {prompt_tokens}+{completion_tokens} tokens, ${estimated_cost_usd:.4}, {response_time_ms}ms"
                        );
                        dp.fetch_add(prompt_tokens, std::sync::atomic::Ordering::Relaxed);
                        dc.fetch_add(completion_tokens, std::sync::atomic::Ordering::Relaxed);
                        dcr.fetch_add(cache_read_tokens, std::sync::atomic::Ordering::Relaxed);
                        dcw.fetch_add(cache_write_tokens, std::sync::atomic::Ordering::Relaxed);
                        dcm.fetch_add(
                            (estimated_cost_usd * 1_000_000.0) as u64,
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    }
```

After `event_drain.await` (line 346), read the accumulated values and populate `AgentResult`:

Where `AgentResult` is constructed (find the struct construction), add:

```rust
            prompt_tokens: drain_prompt.load(std::sync::atomic::Ordering::Relaxed),
            completion_tokens: drain_completion.load(std::sync::atomic::Ordering::Relaxed),
            cache_read_tokens: drain_cache_read.load(std::sync::atomic::Ordering::Relaxed),
            cache_write_tokens: drain_cache_write.load(std::sync::atomic::Ordering::Relaxed),
            cost_usd: drain_cost_micro.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1_000_000.0,
```

- [ ] **Step 3: Accumulate agent cost in harness (agent path)**

In `crates/simulator/src/harness.rs`, after the agent result is processed (find where `metrics.accumulator_mut().agent_calls += 1` is, around line 800), add:

```rust
                    // Cost and cache tracking
                    let acc = metrics.accumulator_mut();
                    acc.total_cost_usd += agent_result.cost_usd;
                    acc.total_prompt_tokens += agent_result.prompt_tokens as u64;
                    acc.total_cache_read_tokens += agent_result.cache_read_tokens as u64;
                    acc.total_cache_write_tokens += agent_result.cache_write_tokens as u64;
```

- [ ] **Step 4: Accumulate estimation deviation in harness**

In `crates/simulator/src/harness.rs`, inside the tool action execution loop (around line 630, where `SimulatedToolAction::CompleteTask` is matched for salience), add estimation tracking:

```rust
                            SimulatedToolAction::CompleteTask {
                                estimated_duration_mins,
                                actual_duration_mins,
                                ..
                            } => {
                                if let (Some(est), Some(act)) =
                                    (estimated_duration_mins, actual_duration_mins)
                                {
                                    if *est > 0 {
                                        let deviation =
                                            ((*act as f64 - *est as f64) / *est as f64).abs();
                                        metrics.accumulator_mut().estimation_deviation_sum +=
                                            deviation;
                                        metrics.accumulator_mut().estimation_count += 1;
                                    }
                                }
                            }
```

Also count completed tasks as outcomes:

```rust
                            SimulatedToolAction::CompleteTask { .. } => {
                                metrics.accumulator_mut().epoch_outcomes += 1;
                            }
```

And count created facts as outcomes (find where `facts_extracted` is bumped):

```rust
                            // Also count fact extraction as an outcome
                            metrics.accumulator_mut().epoch_outcomes +=
                                metrics.accumulator_mut().facts_extracted;
```

Wait — this double-counts. Instead, compute outcomes at snapshot time. The `epoch_outcomes` field should be set right before snapshot. After the message processing loop for the epoch, before calling `metrics.snapshot(...)`, add:

```rust
            // Outcomes = tasks completed + facts extracted this epoch
            metrics.accumulator_mut().epoch_outcomes =
                metrics.accumulator_mut().tasks_completed + metrics.accumulator_mut().facts_extracted;
```

- [ ] **Step 5: Add cache tokens to heuristic-mode usage_records**

In `crates/simulator/src/harness.rs`, find the `INSERT INTO usage_records` statement (around line 970). The current insert doesn't include `cache_read_tokens` or `cache_write_tokens` columns. Update it:

```rust
                let cache_read = (prompt_tokens as f64 * 0.35) as i64; // heuristic: ~35% cache hits
                let cache_write = if day_counter <= 1 {
                    (prompt_tokens as f64 * 0.35) as i64
                } else {
                    0
                };
                let estimated_cost = prompt_tokens as f64 * 0.000003 + completion_tokens as f64 * 0.000015; // rough pricing

                let _ = sqlx::query(
                    "INSERT INTO usage_records \
                     (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
                      cache_read_tokens, cache_write_tokens, estimated_cost_usd, channel, strategy) \
                     VALUES (?, ?, ?, 'scripted-sim', 'simulator', ?, ?, ?, ?, ?, 'simulation', 'reactive')",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(msg.simulated_at.to_rfc3339())
                .bind(&request_id)
                .bind(prompt_tokens as i64)
                .bind(completion_tokens as i64)
                .bind(cache_read)
                .bind(cache_write)
                .bind(estimated_cost)
                .execute(&self.inner_pool)
                .await;

                // Accumulate cost/cache for heuristic mode
                metrics.accumulator_mut().total_cost_usd += estimated_cost;
                metrics.accumulator_mut().total_prompt_tokens += prompt_tokens;
                metrics.accumulator_mut().total_cache_read_tokens += cache_read as u64;
```

- [ ] **Step 6: Call the new update method after snapshot**

In `crates/simulator/src/harness.rs`, after the `metrics.update_latest_cognitive(...)` call (line 1123), add:

```rust
            // Cost, cache, retrievability distribution, and estimation metrics.
            let epoch_start_str_for_cost = plan.previous.to_rfc3339();
            let (cost_per_outcome, cache_rate, retrievability_dist) = tokio::join!(
                crate::metrics::cost::measure_cost_efficiency(
                    &self.inner_pool,
                    &epoch_start_str_for_cost,
                    metrics.timeline.last().map(|_| {
                        // Use the accumulator totals captured before snapshot reset.
                        // Since snapshot() already reset the accumulator, pull from
                        // the snapshot values directly.
                        let snap = metrics.timeline.last().unwrap();
                        // tasks_completed + facts_extracted for this epoch
                        // These are embedded in the cumulative rates; use the epoch_outcomes
                        // we stored before snapshot. Since the accumulator is reset, we
                        // need to compute from the snapshot. Use total_facts_extracted delta.
                        // Simplification: use the outcomes counter we set before snapshot.
                        0 // Will be replaced below
                    }).unwrap_or(0),
                ),
                crate::metrics::cost::measure_cache_hit_rate(
                    &self.inner_pool,
                    &epoch_start_str_for_cost,
                ),
                crate::metrics::cognitive::measure_retrievability_distribution(
                    &self.inner_pool,
                    &now_rfc3339,
                ),
            );
```

Actually, the outcomes count problem is that `snapshot()` resets the accumulator. We need to capture it before. Let me restructure. Before the `metrics.snapshot(...)` call, save the outcome count:

```rust
            let epoch_outcomes = metrics.accumulator_mut().epoch_outcomes;
            let epoch_estimation_avg = if metrics.accumulator_mut().estimation_count > 0 {
                metrics.accumulator_mut().estimation_deviation_sum
                    / metrics.accumulator_mut().estimation_count as f64
            } else {
                0.0
            };
```

Then after `metrics.update_latest_cognitive(...)`, add:

```rust
            let epoch_start_str_cost = plan.previous.to_rfc3339();
            let (cost_per_outcome, cache_rate, ret_dist) = tokio::join!(
                crate::metrics::cost::measure_cost_efficiency(
                    &self.inner_pool,
                    &epoch_start_str_cost,
                    epoch_outcomes,
                ),
                crate::metrics::cost::measure_cache_hit_rate(
                    &self.inner_pool,
                    &epoch_start_str_cost,
                ),
                crate::metrics::cognitive::measure_retrievability_distribution(
                    &self.inner_pool,
                    &now_rfc3339,
                ),
            );
            metrics.update_latest_cost_and_estimation(
                cost_per_outcome,
                cache_rate,
                ret_dist.min,
                ret_dist.p25,
                epoch_estimation_avg,
            );
```

- [ ] **Step 7: Build and fix any compilation errors**

Run: `cargo build -p simulator 2>&1 | head -60`

Fix any issues with field ordering, missing fields in struct construction, or type mismatches. Common fixes:
- Add `estimated_duration_mins: None, actual_duration_mins: None` to any `CompleteTask` construction you missed
- Add new fields with `0`/`0.0` defaults to any `AgentResult` construction
- Ensure the `epoch_outcomes` capture happens before `metrics.snapshot()`

- [ ] **Step 8: Run full test suite**

Run: `cargo nextest run -p simulator`
Expected: All tests PASS

- [ ] **Step 9: Commit**

```bash
git add crates/simulator/src/harness.rs crates/simulator/src/agent_harness.rs crates/simulator/src/agent_types.rs
git commit -m "feat(simulator): wire cost, cache, estimation, and retrievability distribution into harness"
```

---

### Task 7: Report Output and Scenario Assertions

**Files:**
- Modify: `tests/simulation/scenarios/agent_validation_1week.toml`

The `MetricSnapshot` changes are automatically picked up by serde serialization in `report.rs`, so the JSON report will include the new fields. Add checkpoint assertions to verify the new metrics produce meaningful (non-zero) values.

- [ ] **Step 1: Add checkpoint assertions to 1-week validation scenario**

In `tests/simulation/scenarios/agent_validation_1week.toml`, find the `[[checkpoints]]` section and add a checkpoint at the end of the simulation (find the last day, typically day 7):

```toml
[[checkpoints]]
at_day = 7
assertions = [
    # Existing assertions...
    # New: cost metrics should be non-zero (meaning the pipeline measured them)
    { type = "metric_above", metric = "cache_hit_rate", threshold = 0.05 },
    # Estimation: at least some tasks have duration data
    # (skip threshold — 0.0 is valid if no tasks were completed with estimates)
]
```

Note: `cost_per_outcome_usd` and `estimation_deviation_avg` may be 0.0 in short simulations where no tasks are completed. Only assert `cache_hit_rate > 0` since the mock provider now always produces cache tokens.

- [ ] **Step 2: Verify checkpoint assertion support**

Check that the `GroundTruthVerifier` in `metrics/ground_truth.rs` supports the `metric_above` assertion type for the new field names. If `MetricSnapshot` fields are accessed by string name, the verifier should already handle them via serde. If it uses an explicit match, add the new metric names.

Run: `cargo nextest run -p simulator -E 'test(checkpoint)' -E 'test(ground_truth)'`

- [ ] **Step 3: Run the 1-week validation**

Run: `cargo nextest run -p klyntbot -E 'test(agent_validation_1week)' -- --nocapture 2>&1 | tail -40`
Expected: PASS with `cache_hit_rate > 0.05` in the output

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/scenarios/agent_validation_1week.toml
git commit -m "feat(simulator): add cache_hit_rate checkpoint assertion to 1-week validation"
```

---

### Task 8: Verify End-to-End and Clean Up

**Files:**
- No new files — verification only

- [ ] **Step 1: Run the full simulator test suite**

Run: `cargo nextest run -p simulator`
Expected: All tests PASS

- [ ] **Step 2: Run workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | head -40`
Expected: 0 warnings (fix any new ones)

- [ ] **Step 3: Run cargo fmt**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 4: Run the 1-week simulation and verify new metrics appear in output**

Run: `cargo nextest run -p klyntbot -E 'test(agent_validation_1week)' -- --nocapture 2>&1 | grep -E 'cost_per_outcome|cache_hit|retrievability_min|estimation_deviation'`
Expected: All four new metrics appear in the output with non-default values

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore(simulator): clippy and formatting cleanup for signal completeness"
```
