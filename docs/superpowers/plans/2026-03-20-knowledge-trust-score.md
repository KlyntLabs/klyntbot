# Knowledge Trust Score — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reactive "Knowledge Trust" score that measures fact extraction quality per domain, feeds it into the autotuner as `promotion_accuracy`, and surfaces it as a dashboard widget.

**Architecture:** Compute `(total_facts - fast_failures) / total_facts` per domain from existing `semantic_facts` supersession data over a 90-day rolling window. Wire as a new autotuner metric with a dedicated constraint. Surface via a `memory_health` Tauri command and `KnowledgeTrustWidget` component on the System > Memory tab.

**Tech Stack:** Rust (SQLite/sqlx, chrono, serde), React (TypeScript, Tailwind v4, useQuery)

**Spec:** `docs/superpowers/specs/2026-03-20-fact-correctness-tracking-design.md`

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/cognitive/src/repos/semantic_fact.rs` | Add `DomainHealthRow` + `fact_health_by_domain` query | 1 |
| `crates/autotuner/src/traits.rs` | Add `promotion_accuracy` to `MetricSnapshot` | 2 |
| `crates/autotuner/src/trial.rs` | Add `promotion_accuracy` to `TrialResult` | 2 |
| `crates/autotuner/src/metrics.rs` | Add `promotion_accuracy` to `aggregate_to_result` | 2 |
| `crates/config/src/schema/autotuner.rs` | Add `max_promotion_accuracy_drop` config field | 3 |
| `crates/autotuner/src/evaluator.rs` | Add promotion_accuracy constraint check | 3 |
| `crates/agent/src/autotuner/metric_collector.rs` | Wire `fact_health_by_domain` into `collect_metrics` | 4 |
| `crates/app-core/src/init/cron.rs` | Pass `SemanticFactRepo` to `AgentMetricCollector` | 4 |
| `crates/desktop-shared/src/cognitive_commands.rs` | Add `MemoryHealthResponse` + `DomainHealthEntry` types | 5 |
| `crates/app-core/src/handlers/cognitive/memory.rs` | Add `memory_health` handler on `AppCore` | 5 |
| `crates/desktop/src/commands/cognitive.rs` | Add `memory_health` Tauri command + `DEV_COMMANDS` + dev dispatch | 5 |
| `crates/desktop/src/main.rs` | Register `memory_health` in `tauri::generate_handler![]` | 5 |
| `desktop-ui/src/features/autotuner/components/KnowledgeTrustWidget.tsx` | Dashboard widget component | 6 |
| `desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx` | Mount widget above User Model section | 6 |

---

### Task 1: Add `fact_health_by_domain` to SemanticFactRepo

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`

- [ ] **Step 1: Add `DomainHealthRow` struct**

At the top of `semantic_fact.rs`, after the existing imports:

```rust
/// Per-domain fact health statistics for the Knowledge Trust score.
#[derive(Debug, Clone)]
pub struct DomainHealthRow {
    pub domain: String,
    pub total_facts: i64,
    pub active_facts: i64,
    pub fast_failures: i64,
}

impl DomainHealthRow {
    /// Health score: fraction of facts that were not fast failures.
    /// Returns 1.0 for empty domains (no data = no problems).
    pub fn health_score(&self) -> f64 {
        if self.total_facts == 0 {
            return 1.0;
        }
        ((self.total_facts - self.fast_failures) as f64 / self.total_facts as f64).clamp(0.0, 1.0)
    }
}
```

- [ ] **Step 2: Add `fact_health_by_domain` method**

Add to the `impl SemanticFactRepo` block:

```rust
/// Compute fact health per domain within a rolling window.
///
/// For each domain, returns the total facts created in the window,
/// how many are still active (not superseded), and how many were
/// "fast failures" (superseded within 7 days of creation).
pub async fn fact_health_by_domain(
    &self,
    window_days: i64,
) -> Result<Vec<DomainHealthRow>, sqlx::Error> {
    let window = format!("-{window_days} days");
    sqlx::query_as::<_, (String, i64, i64, i64)>(
        "SELECT domain,
                COUNT(*) as total_facts,
                SUM(CASE WHEN superseded_at IS NULL THEN 1 ELSE 0 END) as active_facts,
                SUM(CASE WHEN superseded_at IS NOT NULL
                     AND (julianday(superseded_at) - julianday(recorded_at)) < 7
                     THEN 1 ELSE 0 END) as fast_failures
         FROM semantic_facts
         WHERE recorded_at >= datetime('now', ?1)
         GROUP BY domain",
    )
    .bind(&window)
    .fetch_all(&self.pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(domain, total, active, fast)| DomainHealthRow {
                domain,
                total_facts: total,
                active_facts: active,
                fast_failures: fast,
            })
            .collect()
    })
}
```

- [ ] **Step 3: Add tests**

In the existing `#[cfg(test)] mod tests` block (or add one if none exists in semantic_fact.rs — check first):

```rust
#[tokio::test]
async fn fact_health_empty_returns_empty() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);
    let result = repo.fact_health_by_domain(90).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn fact_health_all_active() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);

    let fact = crate::types::SemanticFact {
        id: "f1".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "role".into(),
        object: "engineer".into(),
        confidence: 0.9,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&fact).await.unwrap();

    let result = repo.fact_health_by_domain(90).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].domain, "work");
    assert!((result[0].health_score() - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn fact_health_with_fast_failure() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);

    // Active fact
    let f1 = crate::types::SemanticFact {
        id: "f1".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "role".into(),
        object: "engineer".into(),
        confidence: 0.9,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&f1).await.unwrap();

    // Fast failure: superseded 2 days after creation
    let now = chrono::Utc::now();
    let f2 = crate::types::SemanticFact {
        id: "f2".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "team".into(),
        object: "wrong-team".into(),
        confidence: 0.6,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: (now - chrono::Duration::days(3)).to_rfc3339(),
        superseded_at: Some((now - chrono::Duration::days(1)).to_rfc3339()),
        superseded_by: Some("f3".into()),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&f2).await.unwrap();

    let result = repo.fact_health_by_domain(90).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].total_facts, 2);
    assert_eq!(result[0].fast_failures, 1);
    // health = (2 - 1) / 2 = 0.5
    assert!((result[0].health_score() - 0.5).abs() < f64::EPSILON);
}

#[tokio::test]
async fn fact_health_slow_supersession_not_penalized() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);

    let now = chrono::Utc::now();

    // Fact superseded after 30 days — NOT a fast failure (legitimate real-world change)
    let f1 = crate::types::SemanticFact {
        id: "f-slow".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "company".into(),
        object: "old-corp".into(),
        confidence: 0.9,
        source: "observed".into(),
        valid_from: "2026-01-01".into(),
        valid_until: None,
        recorded_at: (now - chrono::Duration::days(60)).to_rfc3339(),
        superseded_at: Some((now - chrono::Duration::days(20)).to_rfc3339()),
        superseded_by: Some("f-new".into()),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&f1).await.unwrap();

    let result = repo.fact_health_by_domain(90).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fast_failures, 0, "Slow supersession should NOT be a fast failure");
    // health = (1 - 0) / 1 = 1.0
    assert!((result[0].health_score() - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn fact_health_per_domain_independent() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);

    let now = chrono::Utc::now();

    // Work domain: 1 active fact, health = 1.0
    let f1 = crate::types::SemanticFact {
        id: "f-work".into(),
        domain: "work".into(),
        subject: "user".into(),
        predicate: "role".into(),
        object: "engineer".into(),
        confidence: 0.9,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: now.to_rfc3339(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&f1).await.unwrap();

    // Finance domain: 1 fast failure, health = 0.0
    let f2 = crate::types::SemanticFact {
        id: "f-finance".into(),
        domain: "finance".into(),
        subject: "user".into(),
        predicate: "bank".into(),
        object: "wrong-bank".into(),
        confidence: 0.5,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: (now - chrono::Duration::days(3)).to_rfc3339(),
        superseded_at: Some((now - chrono::Duration::days(1)).to_rfc3339()),
        superseded_by: Some("f-fix".into()),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    };
    repo.upsert(&f2).await.unwrap();

    let result = repo.fact_health_by_domain(90).await.unwrap();
    assert_eq!(result.len(), 2);

    let work = result.iter().find(|d| d.domain == "work").unwrap();
    let finance = result.iter().find(|d| d.domain == "finance").unwrap();
    assert!((work.health_score() - 1.0).abs() < f64::EPSILON, "Work should be 100%");
    assert!((finance.health_score() - 0.0).abs() < f64::EPSILON, "Finance should be 0%");
}

#[test]
fn domain_health_row_empty_returns_one() {
    let row = DomainHealthRow {
        domain: "test".into(),
        total_facts: 0,
        active_facts: 0,
        fast_failures: 0,
    };
    assert!((row.health_score() - 1.0).abs() < f64::EPSILON);
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p cognitive -E 'test(fact_health)' --no-fail-fast`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(cognitive): add fact_health_by_domain for Knowledge Trust score"
```

---

### Task 2: Add `promotion_accuracy` to MetricSnapshot + TrialResult

**Files:**
- Modify: `crates/autotuner/src/traits.rs`
- Modify: `crates/autotuner/src/trial.rs`
- Modify: `crates/autotuner/src/metrics.rs`

- [ ] **Step 1: Add field to `MetricSnapshot`**

In `crates/autotuner/src/traits.rs`, add after `memory_freshness`:

```rust
    // Phase 2: fact extraction quality (1.0 - fast_failure_rate)
    pub promotion_accuracy: f64,
```

- [ ] **Step 2: Add field to `TrialResult`**

In `crates/autotuner/src/trial.rs`, add after `memory_freshness`:

```rust
    pub promotion_accuracy: f64,
```

- [ ] **Step 3: Update `aggregate_to_result`**

In `crates/autotuner/src/metrics.rs`, add to the `TrialResult` construction after `memory_freshness`:

```rust
        promotion_accuracy: snapshots
            .iter()
            .map(|s| s.promotion_accuracy * w(s))
            .sum(),
```

- [ ] **Step 4: Fix all construction sites**

Adding a new field to `MetricSnapshot` and `TrialResult` will break all sites that construct them without `..Default::default()`. Search for `MetricSnapshot {` and `TrialResult {` across the workspace. Key files:
- `crates/autotuner/src/metrics.rs` — the `aggregate_to_result` function constructs `TrialResult` explicitly at line 29 (already handled in Step 3, but verify it compiles)
- `crates/agent/src/autotuner/metric_collector.rs` — add `promotion_accuracy: 0.0` (will be wired in Task 4)
- Any test files in `crates/autotuner/src/` that construct these structs without `..Default::default()`

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p autotuner --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(autotuner): add promotion_accuracy to MetricSnapshot and TrialResult"
```

---

### Task 3: Add promotion_accuracy constraint to evaluator

**Files:**
- Modify: `crates/config/src/schema/autotuner.rs`
- Modify: `crates/autotuner/src/evaluator.rs`

- [ ] **Step 1: Add config field**

In `crates/config/src/schema/autotuner.rs`, add to `AutoTunerConfig`:

```rust
    #[serde(default = "default_max_promotion_accuracy_drop")]
    pub max_promotion_accuracy_drop: f64,
```

Add to `Default` impl:

```rust
            max_promotion_accuracy_drop: default_max_promotion_accuracy_drop(),
```

Add the default function:

```rust
fn default_max_promotion_accuracy_drop() -> f64 {
    0.05
}
```

- [ ] **Step 2: Add constraint field to `ConstraintEvaluator`**

In `crates/autotuner/src/evaluator.rs`, add to the struct:

```rust
    /// promotion_accuracy must not drop by more than this absolute amount.
    max_promotion_accuracy_drop: f64,
```

Update `from_config`:

```rust
            max_promotion_accuracy_drop: config.max_promotion_accuracy_drop,
```

- [ ] **Step 3: Add constraint check**

In the `evaluate` method, add after the `correction_rate_regression` check:

```rust
        // --- Phase 2: promotion accuracy must not drop > threshold ---
        if baseline.promotion_accuracy > 0.0 {
            let accuracy_drop = baseline.promotion_accuracy - trial.promotion_accuracy;
            if accuracy_drop > self.max_promotion_accuracy_drop {
                failures.push(ConstraintFailure {
                    metric: "promotion_accuracy".into(),
                    threshold: self.max_promotion_accuracy_drop,
                    actual: accuracy_drop,
                    description: format!(
                        "promotion_accuracy dropped by {:.1}% but max allowed is {:.1}%",
                        accuracy_drop * 100.0,
                        self.max_promotion_accuracy_drop * 100.0,
                    ),
                });
            }
        }
```

- [ ] **Step 4: Add test**

```rust
#[test]
fn fails_when_promotion_accuracy_drops() {
    let evaluator = default_evaluator();
    let b = TrialResult {
        promotion_accuracy: 0.90,
        ..baseline()
    };

    // Drops from 0.90 to 0.80 = 0.10 drop, max allowed 0.05
    let trial = TrialResult {
        correction_rate: 0.18,
        promotion_accuracy: 0.80,
        ..b.clone()
    };

    let verdict = evaluator.evaluate(&trial, &b);
    assert!(
        verdict.failures.iter().any(|f| f.metric == "promotion_accuracy"),
        "Expected promotion_accuracy failure, got: {:?}",
        verdict.failures,
    );
}
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p autotuner -p config --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(autotuner): add promotion_accuracy constraint with dedicated config threshold"
```

---

### Task 4: Wire fact health into AgentMetricCollector

**Files:**
- Modify: `crates/agent/src/autotuner/metric_collector.rs`
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Add `SemanticFactRepo` to `AgentMetricCollector`**

In `crates/agent/src/autotuner/metric_collector.rs`:

Add to the struct:

```rust
    fact_repo: cognitive::SemanticFactRepo,
```

Update the constructor to accept a fifth argument:

```rust
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        event_log_repo: cognitive::EventLogRepo,
        usage_repo: storage::UsageRepo,
        trial_repo: storage::TrialRepo,
        fact_repo: cognitive::SemanticFactRepo,
    ) -> Self {
        Self {
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
            fact_repo,
        }
    }
```

- [ ] **Step 2: Wire `promotion_accuracy` into `collect_metrics`**

Add a new arm to the `tokio::join!` block:

```rust
            // Fact health (Knowledge Trust) for promotion_accuracy
            self.fact_repo.fact_health_by_domain(90),
```

After the join, compute the average:

```rust
        let promotion_accuracy = match fact_health_result {
            Ok(domains) if !domains.is_empty() => {
                domains.iter().map(|d| d.health_score()).sum::<f64>() / domains.len() as f64
            }
            _ => 1.0, // No data = assume healthy
        };
```

Replace the placeholder `promotion_accuracy: 0.0` in the `MetricSnapshot` construction with `promotion_accuracy`.

- [ ] **Step 3: Update wiring in `init/cron.rs`**

In `crates/app-core/src/init/cron.rs`, at the `AgentMetricCollector::new` call site (~line 121), add the fifth argument:

```rust
        let fact_repo = cognitive::SemanticFactRepo::new(repos.pool().clone());
        let metric_source: Arc<dyn autotuner::MetricSource> = Arc::new(
            agent::autotuner::metric_collector::AgentMetricCollector::new(
                strategy_repo,
                event_log_repo,
                usage_repo,
                trial_repo.clone(),
                fact_repo,
            ),
        );
```

- [ ] **Step 4: Fix existing tests in metric_collector**

The existing tests construct `AgentMetricCollector` with 4 args. Update to pass a `SemanticFactRepo` as the 5th:

```rust
let fact_repo = cognitive::SemanticFactRepo::new(inner.clone());
let collector = AgentMetricCollector::new(strategy_repo, event_log_repo, usage_repo, trial_repo, fact_repo);
```

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p agent -E 'test(metric)' --no-fail-fast`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(autotuner): wire fact health into AgentMetricCollector as promotion_accuracy"
```

---

### Task 5: Add `memory_health` Tauri command + handler

**Files:**
- Modify: `crates/desktop-shared/src/cognitive_commands.rs`
- Modify: `crates/app-core/src/handlers/cognitive/memory.rs`
- Modify: `crates/desktop/src/commands/cognitive.rs`

- [ ] **Step 1: Add response types to desktop-shared**

In `crates/desktop-shared/src/cognitive_commands.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHealthResponse {
    pub overall: f64,
    pub domains: Vec<DomainHealthEntry>,
    pub total_facts_90d: i64,
    pub fast_failures_90d: i64,
    pub trend_pct: Option<f64>,       // raw delta (e.g., 0.04 = score went up 4 points)
    pub computed_at: String,           // ISO timestamp for "last updated" display
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainHealthEntry {
    pub domain: String,
    pub score: f64,
    pub total_facts: i64,
    pub active_facts: i64,
    pub fast_failures: i64,
}
```

- [ ] **Step 2: Add handler on AppCore**

In `crates/app-core/src/handlers/cognitive/memory.rs`, add:

```rust
    pub async fn memory_health(&self) -> Result<MemoryHealthResponse, ApiError> {
        let fact_repo = cognitive::SemanticFactRepo::new(self.repos.pool().clone());
        let domains = fact_repo
            .fact_health_by_domain(90)
            .await
            .map_err(map_cognitive_err)?;

        let total_facts_90d: i64 = domains.iter().map(|d| d.total_facts).sum();
        let fast_failures_90d: i64 = domains.iter().map(|d| d.fast_failures).sum();

        let overall = if domains.is_empty() {
            1.0
        } else {
            domains.iter().map(|d| d.health_score()).sum::<f64>() / domains.len() as f64
        };

        // Week-over-week trend from LearningStateRepo snapshot
        // trend_pct is a raw score delta (e.g., 0.04 means score went up 4 percentage points)
        let trend_pct = if let Ok(Some(snapshot)) =
            self.repos.learning_state.get_value("knowledge_trust_snapshot").await
        {
            snapshot.get("score").and_then(|s| s.as_f64()).map(|prev| overall - prev)
        } else {
            None
        };

        // Persist current score for next trend computation
        let _ = self
            .repos
            .learning_state
            .set(
                "knowledge_trust_snapshot",
                &serde_json::json!({
                    "score": overall,
                    "computed_at": chrono::Utc::now().to_rfc3339(),
                }),
            )
            .await;

        let domain_entries: Vec<DomainHealthEntry> = domains
            .iter()
            .map(|d| DomainHealthEntry {
                domain: d.domain.clone(),
                score: d.health_score(),
                total_facts: d.total_facts,
                active_facts: d.active_facts,
                fast_failures: d.fast_failures,
            })
            .collect();

        Ok(MemoryHealthResponse {
            overall,
            domains: domain_entries,
            total_facts_90d,
            fast_failures_90d,
            trend_pct,
            computed_at: chrono::Utc::now().to_rfc3339(),
        })
    }
```

Note: Import `cognitive::repos::semantic_fact::DomainHealthRow` if the `health_score()` method isn't visible — it's on the `DomainHealthRow` struct. The `SemanticFactRepo` is already re-exported via `cognitive::SemanticFactRepo`.

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/cognitive.rs`, add the command function:

```rust
#[tauri::command]
pub async fn memory_health(
    state: State<'_, Arc<AppCore>>,
) -> Result<MemoryHealthResponse, ApiError> {
    state.memory_health().await
}
```

Add `"memory_health"` to `DEV_COMMANDS`.

Add to the `dispatch_dev` match:

```rust
        "memory_health" => dev::val(core.memory_health().await),
```

Also add `MemoryHealthResponse` to the `use desktop_shared::cognitive_commands::*` import (already covered by the wildcard).

- [ ] **Step 4: Register the Tauri command in `main.rs`**

In `crates/desktop/src/main.rs`, find the `tauri::generate_handler![]` macro (starts at ~line 287). Locate the cognitive command block and add `commands::cognitive::memory_health` to the list. The `dev_server_covers_all_tauri_commands` test enforces parity between `main.rs` and `DEV_COMMANDS` — both must include the new command.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p app-core -p desktop --no-fail-fast`

The `dev_server_covers_all_tauri_commands` test should pass with the new `"memory_health"` in `DEV_COMMANDS`.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(desktop): add memory_health Tauri command for Knowledge Trust widget"
```

---

### Task 6: Frontend KnowledgeTrustWidget

**Files:**
- Create: `desktop-ui/src/features/autotuner/components/KnowledgeTrustWidget.tsx`
- Modify: `desktop-ui/src/features/autotuner/index.ts`
- Modify: `desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx`

- [ ] **Step 1: Create the widget component**

Create `desktop-ui/src/features/autotuner/components/KnowledgeTrustWidget.tsx`:

```tsx
import { useQuery } from "@shared/hooks/useQuery";

interface MemoryHealthResponse {
  overall: number;
  domains: DomainHealthEntry[];
  totalFacts90d: number;
  fastFailures90d: number;
  trendPct: number | null;  // raw delta (e.g., 0.04 = +4 points)
  computedAt: string;
}

interface DomainHealthEntry {
  domain: string;
  score: number;
  totalFacts: number;
  activeFacts: number;
  fastFailures: number;
}

function scoreColor(score: number): string {
  if (score >= 0.85) return "bg-green-500/15 text-green-400 border-green-500/30";
  if (score >= 0.70) return "bg-amber-500/15 text-amber-400 border-amber-500/30";
  return "bg-red-500/15 text-red-400 border-red-500/30";
}

function trendArrow(delta: number | null): string {
  if (delta == null) return "";
  const points = Math.abs(Math.round(delta * 100)); // convert 0.04 → 4
  if (delta > 0) return ` ↑${points}%`;
  if (delta < 0) return ` ↓${points}%`;
  return "";
}

function trendColor(pct: number | null): string {
  if (pct == null) return "";
  return pct >= 0 ? "text-green-400" : "text-red-400";
}

export function KnowledgeTrustWidget() {
  const { data, loading } = useQuery<MemoryHealthResponse>("memory_health");

  if (loading) {
    return <div className="glass-card p-4 h-28 animate-pulse" />;
  }

  if (!data || data.totalFacts90d === 0) {
    return null; // Don't show widget when there are no facts yet
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-baseline justify-between">
        <div>
          <h3 className="text-[13px] font-medium text-foreground">
            Knowledge Trust
          </h3>
          <p className="text-[11px] text-dim">How well I know you</p>
        </div>
        <div className="text-right">
          <span className="text-2xl font-semibold text-foreground">
            {Math.round(data.overall * 100)}%
          </span>
          {data.trendPct != null && (
            <span className={`text-[11px] ml-1 ${trendColor(data.trendPct)}`}>
              {trendArrow(data.trendPct)}
            </span>
          )}
        </div>
      </div>

      <div className="flex flex-wrap gap-1.5">
        {data.domains.map((d) => (
          <span
            key={d.domain}
            className={`px-2 py-0.5 rounded-full text-[11px] font-medium border ${scoreColor(d.score)}`}
            title={`${d.totalFacts} facts, ${d.fastFailures} fast failures`}
          >
            {d.domain}: {Math.round(d.score * 100)}%
          </span>
        ))}
      </div>

      <p className="text-[10px] text-dim">
        {data.totalFacts90d} facts tracked
        {data.computedAt && ` · updated ${new Date(data.computedAt).toLocaleDateString()}`}
      </p>
    </div>
  );
}
```

- [ ] **Step 2: Export from index.ts**

In `desktop-ui/src/features/autotuner/index.ts`, add:

```typescript
export { KnowledgeTrustWidget } from "./components/KnowledgeTrustWidget";
```

- [ ] **Step 3: Mount in MemoryTab**

In `desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx`, import and render the widget at the top of the tab content (above the "User Model" section):

```tsx
import { KnowledgeTrustWidget } from "@features/autotuner";
```

Then in the JSX, add `<KnowledgeTrustWidget />` before the User Model section. Find the first `<h2>` or section header element and insert above it.

- [ ] **Step 4: Verify**

Run: `cd desktop-ui && bun run build`

Confirm no TypeScript errors.

- [ ] **Step 5: Visual verification**

Start the dev server (`cd desktop-ui && bun run dev`) and navigate to System > Memory tab. The Knowledge Trust widget should appear above the User Model section. If no facts exist, the widget should be hidden (returns null).

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(desktop): add KnowledgeTrustWidget to System > Memory tab"
```

---

### Task 7: Final verification

- [ ] **Step 1: Full workspace compile**

Run: `cargo check --workspace`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Format**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Test all modified crates**

Run: `cargo nextest run -p cognitive -p autotuner -p config -p agent -p app-core -p desktop --no-fail-fast`

- [ ] **Step 5: Frontend build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Frontend lint**

Run: `cd desktop-ui && bun run lint`

- [ ] **Step 7: Commit if fixes**

```bash
git commit -m "chore: fix lint/fmt from Knowledge Trust implementation"
```

---

## Dependency Graph

```
Task 1 (fact_health_by_domain) ──→ Task 2 (MetricSnapshot + TrialResult)
                                        │
                                        ├──→ Task 3 (evaluator constraint)
                                        │
                                        └──→ Task 4 (metric collector wiring)
                                                  │
                                                  └──→ Task 5 (Tauri command + handler)
                                                            │
                                                            └──→ Task 6 (frontend widget)
                                                                      │
                                                                      └──→ Task 7 (verification)
```

Tasks are strictly sequential — each builds on the previous.
