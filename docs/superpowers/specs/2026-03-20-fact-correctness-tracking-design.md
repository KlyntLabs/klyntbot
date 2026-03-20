# Fact Correctness Tracking — Knowledge Trust Score

**Date:** 2026-03-20
**Status:** Design approved, pending implementation plan
**Depends on:** Autotuner Phase 2 (complete), cognitive memory pipeline (existing)

## Overview

Add a reactive "Knowledge Trust" score that measures how trustworthy the AI's extracted knowledge is, per domain. Computed from existing supersession data (zero user effort), surfaced as a dashboard widget, and fed into the autotuner as the `promotion_accuracy` metric for the closed-loop threshold tuning.

## Why This Matters

The autotuner Phase 2 tunes `accumulate_promote_threshold` and `accumulate_min_days` — controlling how aggressively observations get promoted to semantic facts. Without a quality signal, the autotuner can't evaluate whether its tuning produces good facts or bad ones. This metric closes that loop: bad threshold → more fast-failing facts → lower score → autotuner blocks that variant.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Correctness signal | Reactive (supersession-based) | Zero user effort, computable from existing data. Proactive validation (thumbs-up/down) deferred to Phase 2. |
| Threshold adjustment | Through autotuner experiments | Avoids oscillation risk from direct feedback. Nightly cadence is a feature — fact supersession is never an emergency. |
| Score granularity | Per-domain + global average | Per-domain gives actionable insight ("Finance: 71%"). Global is just `AVG(domain_scores)` — free. |
| UI label | "Knowledge Trust" | Personal and second-brain-feeling. Internal metric name stays `promotion_accuracy`. |
| Rolling window | 90 days | Old superseded facts shouldn't drag down the score. Measures current extraction quality. |

---

## Section 1: Health Score Computation

### Formula

```
domain_health = (active_facts - fast_failures) / total_facts
```

Where (within a 90-day rolling window per domain):
- `total_facts` = all facts created in the window
- `active_facts` = facts where `superseded_at IS NULL`
- `fast_failures` = facts superseded within 7 days of `recorded_at`

**Why fast-failure distinction:** Facts superseded within 7 days are likely extraction errors. Facts superseded after 30+ days are often legitimate real-world changes (user changed jobs, moved, etc.). Fast failures are the signal; slow supersessions are natural evolution.

### SQL Query

```sql
SELECT domain,
       COUNT(*) as total_facts,
       SUM(CASE WHEN superseded_at IS NULL THEN 1 ELSE 0 END) as active_facts,
       SUM(CASE WHEN superseded_at IS NOT NULL
            AND (julianday(superseded_at) - julianday(recorded_at)) < 7
            THEN 1 ELSE 0 END) as fast_failures
FROM semantic_facts
WHERE recorded_at >= datetime('now', '-90 days')
GROUP BY domain
```

### Implementation

**New method on `SemanticFactRepo`:**

```rust
pub struct DomainHealthRow {
    pub domain: String,
    pub total_facts: i64,
    pub active_facts: i64,
    pub fast_failures: i64,
}

impl DomainHealthRow {
    pub fn health_score(&self) -> f64 {
        if self.total_facts == 0 { return 1.0; }
        ((self.active_facts - self.fast_failures) as f64 / self.total_facts as f64)
            .clamp(0.0, 1.0)
    }
}

/// Compute fact health per domain within a rolling window.
pub async fn fact_health_by_domain(&self, window_days: i64) -> Result<Vec<DomainHealthRow>> { ... }
```

**File:** `crates/cognitive/src/repos/semantic_fact.rs`

---

## Section 2: Autotuner Integration

### New Metric Fields

Add `promotion_accuracy: f64` to `MetricSnapshot` and `TrialResult` (following the exact pattern of `retrieval_precision`, `retrieval_recall`, `memory_freshness`).

### Data Flow

1. `AgentMetricCollector::collect_metrics()` calls `fact_repo.fact_health_by_domain(90)`
2. Averages domain scores into a single `promotion_accuracy` value
3. Also stores per-domain breakdown as a JSON map alongside for diagnostics
4. `aggregate_to_result()` volume-weights it like every other metric

### Constraint

Add to `ConstraintEvaluator`: **promotion_accuracy must not drop > 5%** (same threshold as `retrieval_precision`). Reuses `max_retrieval_precision_drop` config field.

### Closed Loop Behavior

When the autotuner tests a more aggressive `accumulate_promote_threshold`:
- If the trial produces more fast-failing facts → `promotion_accuracy` drops
- The >5% constraint blocks promotion of that variant
- The tuner learns to be more conservative in domains where extraction quality is fragile

**No direct threshold manipulation.** The autotuner's existing experiment cycle handles it. We just give it a new signal to optimize against.

### Where the `SemanticFactRepo` Comes From

`AgentMetricCollector` currently holds `StrategyRepo`, `EventLogRepo`, `UsageRepo`, and `TrialRepo`. Add `SemanticFactRepo` as a fifth dependency. It's already constructed during cognitive init and available at the `init/cron.rs` wiring site.

---

## Section 3: Dashboard Widget

### Placement

System > Memory tab, at the top above the existing "User Model" domain cards.

### Tauri Command

New command: `memory_health` → calls `SemanticFactRepo::fact_health_by_domain(90)`.

### Response Type

```rust
pub struct MemoryHealthResponse {
    pub overall: f64,                    // average of domain scores
    pub domains: Vec<DomainHealthEntry>,
    pub total_facts_90d: i64,
    pub fast_failures_90d: i64,
    pub trend_pct: Option<f64>,          // week-over-week change
}

pub struct DomainHealthEntry {
    pub domain: String,
    pub score: f64,
    pub total_facts: i64,
    pub active_facts: i64,
    pub fast_failures: i64,
}
```

### Frontend Component

**File:** `desktop-ui/src/features/autotuner/components/KnowledgeTrustWidget.tsx`

**Visual design:**
- Glass card (`.glass-card`)
- Title: "Knowledge Trust" with subtitle "How well I know you"
- Large overall percentage with trend arrow (e.g., "87% ↑4%")
- Row of domain pills colored by score:
  - Green: >= 85%
  - Amber: 70-84%
  - Red: < 70%
- Clicking a domain pill opens a popover listing 2-3 facts superseded this month
- "Last updated: 2 days ago" line at bottom

**Data fetching:** `useQuery("memory_health")` — no custom hook needed.

### Integration Point

**File:** `desktop-ui/src/features/settings/pages/` or System > Memory tab — insert `KnowledgeTrustWidget` above the "User Model" section.

---

## Section 4: Week-Over-Week Trend

To compute the trend arrow, the `memory_health` handler runs the same query twice:
- Once for the current 90-day window
- Once for the previous 7-day snapshot (stored in `LearningStateRepo` as `"knowledge_trust_snapshot"`)

After computing the current score, persist it:
```rust
learning_state.set("knowledge_trust_snapshot", &json!({
    "score": overall,
    "computed_at": Utc::now().to_rfc3339(),
})).await;
```

The trend is `current - previous_snapshot`. If no previous snapshot exists, `trend_pct = None`.

---

## Files Modified

| File | Change | Description |
|------|--------|-------------|
| `crates/cognitive/src/repos/semantic_fact.rs` | Modify | Add `fact_health_by_domain` method + `DomainHealthRow` |
| `crates/autotuner/src/traits.rs` | Modify | Add `promotion_accuracy` to `MetricSnapshot` |
| `crates/autotuner/src/trial.rs` | Modify | Add `promotion_accuracy` to `TrialResult` |
| `crates/autotuner/src/metrics.rs` | Modify | Add `promotion_accuracy` to `aggregate_to_result` |
| `crates/autotuner/src/evaluator.rs` | Modify | Add promotion_accuracy constraint check |
| `crates/agent/src/autotuner/metric_collector.rs` | Modify | Wire `fact_health_by_domain` into `collect_metrics` |
| `crates/app-core/src/handlers/cognitive/` | Modify | Add `memory_health` handler |
| `crates/desktop/src/commands/` | Modify | Add `memory_health` Tauri command |
| `crates/desktop-shared/src/` | Modify | Add response types |
| `desktop-ui/src/features/autotuner/components/KnowledgeTrustWidget.tsx` | Create | Dashboard widget |
| System > Memory page | Modify | Mount `KnowledgeTrustWidget` |

---

## Non-Goals (v1)

- **Proactive validation UI** (thumbs-up/down on individual facts) — Phase 2
- **"Want to review?" nudges** after supersession — Phase 2
- **Coaching tie-in** (UserSituation integration) — Phase 2
- **Weekly reflection line item** — Phase 2
- **Per-domain autotuner thresholds** — the autotuner tunes global params; per-domain is future

---

## Testing Strategy

| Test | Crate | What it validates |
|------|-------|-------------------|
| `fact_health_empty_returns_one` | `cognitive` | No facts → health = 1.0 |
| `fact_health_all_active` | `cognitive` | No supersessions → health = 1.0 |
| `fact_health_with_fast_failures` | `cognitive` | Fast failures penalized correctly |
| `fact_health_slow_supersession_not_penalized` | `cognitive` | Superseded after 30 days → only counts once, not as fast failure |
| `fact_health_per_domain` | `cognitive` | Different domains computed independently |
| `promotion_accuracy_constraint` | `autotuner` | Evaluator blocks trial when accuracy drops > 5% |
| `metric_collector_wires_promotion_accuracy` | `agent` | `collect_metrics` includes non-zero `promotion_accuracy` |
