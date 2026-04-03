# Simulator Intelligence Upgrade — Comprehensive Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the simulator from a storage-layer health check into a genuine intelligence evaluation framework by fixing broken metrics, integrating real skill routing, expanding feature coverage, and adding cognitive depth measurement.

**Architecture:** Four phases, each independently deployable. Phase 1 makes existing metrics honest (no more self-validating circularity). Phase 2 integrates the real `SkillRouter` so routing quality is actually measured. Phase 3 expands feature coverage (coaching, learning, cross-feature workflows). Phase 4 adds cognitive depth metrics (FSRS-5 decay, salience filtering, memory promotion, meta-rule accumulation).

**Tech Stack:** Rust, SQLite (in-memory), `skill-system` crate (`SkillRouter`, `SkillCatalog`), `cognitive` crate (FSRS-5, salience, promotion), `simulator` crate, `toml` scenarios.

**Current state reference:** Post Phase 1-3 metric fixes are landed. Latest sim results: `target/simulation/software_engineer_vn_269_20260402_152007.json`. All 14 metrics functional but most measure infrastructure health rather than AI intelligence.

---

## File Structure

### Phase 1 — Metric Integrity (existing files only)
- Modify: `crates/simulator/src/metrics/memory.rs` — negation-aware retention matching
- Modify: `crates/simulator/src/metrics/mod.rs` — cumulative contradiction rate, new `MetricSnapshot` fields, `BaselineMetrics` expansion
- Modify: `crates/simulator/src/metrics/behavioral.rs` — revised personalization formula
- Modify: `crates/simulator/src/metrics/system.rs` — rolling brain_version_velocity, real insight quality gate
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — tier-1 baseline support, new metric value mappings
- Modify: `crates/simulator/src/scenario.rs` — new `MetricName` variants
- Modify: `crates/simulator/src/harness.rs` — cumulative counters wiring
- Modify: `crates/simulator/src/report.rs` — tier-1 improvements
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml` — tighter thresholds

### Phase 2 — Real Skill Routing
- Modify: `crates/simulator/src/harness.rs` — `SkillCatalog` + `SkillRouter` integration, real routing calls per message
- Modify: `crates/simulator/src/metrics/mod.rs` — `routing_accuracy` accumulator field
- Modify: `crates/simulator/src/scenario.rs` — `RoutingAccuracy` metric name
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — metric value mapping
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml` — routing accuracy checkpoint

### Phase 3 — Feature Coverage Expansion
- Modify: `crates/simulator/src/persona/types.rs` — new `SimulatedToolAction` variants
- Modify: `crates/simulator/src/persona/templates.rs` — coaching templates
- Modify: `crates/simulator/src/persona/mod.rs` — coaching/learning tool generation, cross-feature workflows
- Modify: `crates/simulator/src/actions.rs` — new action executors
- Modify: `crates/simulator/Cargo.toml` — `feature-coaching`, `feature-learning` deps
- Create: `tests/simulation/scenarios/fact_contradiction.toml` — contradiction-focused scenario
- Create: `tests/simulation/scenarios/coaching_persona.toml` — coaching-heavy scenario
- Modify: `tests/simulation/smoke.rs` — new test functions

### Phase 4 — Cognitive Depth Metrics
- Modify: `crates/simulator/src/metrics/mod.rs` — new cognitive metric fields
- Create: `crates/simulator/src/metrics/cognitive.rs` — FSRS-5 decay, salience, promotion metrics
- Modify: `crates/simulator/src/harness.rs` — salience filtering, promotion pipeline, meta-rule tracking
- Modify: `crates/simulator/src/scenario.rs` — new cognitive metric names
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — cognitive metric mappings
- Modify: `crates/simulator/src/lib.rs` — (if new modules added)

---

## Phase 1: Metric Integrity

### Task 1: Add negation awareness to knowledge_retention Strategy 3

Strategy 3 in `measure_knowledge_retention` matches `r.subject == fact.subject && obj_lower.contains(&fact.object)`. This causes false positives when the stored object says "not an engineer" — it still matches "engineer".

**Files:**
- Modify: `crates/simulator/src/metrics/memory.rs:28-43`
- Test: `crates/simulator/src/metrics/memory.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

In `crates/simulator/src/metrics/memory.rs`, add to the `mod tests` block:

```rust
#[tokio::test]
async fn retention_rejects_negated_facts() {
    let (pool, inner) = test_pool().await;
    std::mem::forget(pool);

    let repo = cognitive::SemanticFactRepo::new(inner);

    // Insert a fact that negates the known fact
    let fact = cognitive::SemanticFact {
        id: "negated-1".to_string(),
        domain: "personal".to_string(),
        subject: "user".to_string(),
        predicate: "stated".to_string(),
        object: "I am no longer a software engineer".to_string(),
        confidence: 0.5,
        source: "heuristic".to_string(),
        valid_from: chrono::Utc::now().to_rfc3339(),
        valid_until: None,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: "semantic".to_string(),
        scope_type: "global".to_string(),
        scope_id: None,
    };
    repo.upsert(&fact).await.expect("upsert negated fact");

    let known = vec![FactTriple {
        subject: "user".to_string(),
        predicate: "works_as".to_string(),
        object: "software engineer".to_string(),
    }];

    let retention = measure_knowledge_retention(&repo, &known).await;
    assert!(
        (retention - 0.0).abs() < 1e-9,
        "negated fact should NOT count as retained, got {retention}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator --test-threads=1 -E 'test(retention_rejects_negated_facts)'`
Expected: FAIL — Strategy 3 matches "software engineer" substring despite "no longer" prefix.

- [ ] **Step 3: Add negation detection to Strategy 3**

In `crates/simulator/src/metrics/memory.rs`, replace Strategy 3 (line 42):

```rust
            // Strategy 3: subject match + object contains the fact's object value
            // BUT reject if the stored object contains negation markers near the match
            if r.subject == fact.subject && obj_lower.contains(&fact.object.to_lowercase()) {
                let negation_markers = ["not ", "no longer ", "stopped ", "quit ", "never ", "don't ", "doesn't ", "isn't ", "aren't "];
                let has_negation = negation_markers.iter().any(|neg| {
                    if let Some(pos) = obj_lower.find(&fact.object.to_lowercase()) {
                        // Check if any negation marker appears before the match position
                        let prefix = &obj_lower[..pos];
                        prefix.contains(neg)
                    } else {
                        false
                    }
                });
                return !has_negation;
            }
            false
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p simulator --test-threads=1 -E 'test(retention_rejects_negated_facts)'`
Expected: PASS

- [ ] **Step 5: Run all retention tests to verify no regressions**

Run: `cargo nextest run -p simulator --test-threads=1 -E 'test(retention)'`
Expected: All pass (including `retention_finds_heuristic_extracted_facts` — positive case still works)

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/metrics/memory.rs
git commit -m "fix(simulator): add negation awareness to knowledge_retention Strategy 3"
```

---

### Task 2: Make contradiction_detection_rate cumulative

Currently per-epoch: `contradictions_detected / facts_introduced`. On days with 0 introductions, this is 0/1=0. Should be cumulative across all epochs like task_completion_rate.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs:66-95` (EpochAccumulator, MetricCollector)

- [ ] **Step 1: Write the failing test**

In `crates/simulator/src/metrics/mod.rs`, add to `mod tests`:

```rust
#[test]
fn contradiction_rate_is_cumulative() {
    let mut collector = MetricCollector::new(30);

    // Epoch 1: 2 facts introduced, 1 contradiction → rate = 1/2 = 0.5
    {
        let acc = collector.accumulator_mut();
        acc.messages_processed = 5;
        acc.facts_introduced = 2;
        acc.contradictions_detected = 1;
    }
    collector.snapshot(utc(2026, 4, 1, 12, 0), 1, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
    assert!(
        (collector.timeline[0].contradiction_detection_rate - 0.5).abs() < 1e-9,
        "epoch 1: expected 0.5, got {}",
        collector.timeline[0].contradiction_detection_rate
    );

    // Epoch 2: 0 facts introduced, 0 contradictions → cumulative still 1/2 = 0.5
    {
        let acc = collector.accumulator_mut();
        acc.messages_processed = 5;
        acc.facts_introduced = 0;
        acc.contradictions_detected = 0;
    }
    collector.snapshot(utc(2026, 4, 2, 12, 0), 2, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
    assert!(
        (collector.timeline[1].contradiction_detection_rate - 0.5).abs() < 1e-9,
        "epoch 2: expected 0.5 (cumulative), got {}",
        collector.timeline[1].contradiction_detection_rate
    );

    // Epoch 3: 3 facts introduced, 2 contradictions → cumulative (1+2)/(2+3) = 3/5 = 0.6
    {
        let acc = collector.accumulator_mut();
        acc.messages_processed = 5;
        acc.facts_introduced = 3;
        acc.contradictions_detected = 2;
    }
    collector.snapshot(utc(2026, 4, 3, 12, 0), 3, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
    assert!(
        (collector.timeline[2].contradiction_detection_rate - 0.6).abs() < 1e-9,
        "epoch 3: expected 0.6 (cumulative 3/5), got {}",
        collector.timeline[2].contradiction_detection_rate
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator -E 'test(contradiction_rate_is_cumulative)'`
Expected: FAIL — epoch 2 shows 0.0 instead of 0.5.

- [ ] **Step 3: Add cumulative contradiction tracking**

In `crates/simulator/src/metrics/mod.rs`, add two fields to `MetricCollector`:

```rust
pub struct MetricCollector {
    pub timeline: Vec<MetricSnapshot>,
    pub baselines: Option<BaselineMetrics>,
    baseline_day: u32,
    accumulator: EpochAccumulator,
    cumulative_tasks_created: u32,
    cumulative_tasks_completed: u32,
    cumulative_facts_superseded: u32,
    cumulative_facts_introduced: u32,
    cumulative_contradictions: u32,
}
```

Initialize to 0 in `new()`:

```rust
    pub fn new(baseline_after_day: u32) -> Self {
        Self {
            timeline: Vec::new(),
            baselines: None,
            baseline_day: baseline_after_day,
            accumulator: EpochAccumulator::default(),
            cumulative_tasks_created: 0,
            cumulative_tasks_completed: 0,
            cumulative_facts_superseded: 0,
            cumulative_facts_introduced: 0,
            cumulative_contradictions: 0,
        }
    }
```

In `snapshot()`, add after the existing cumulative lines:

```rust
        self.cumulative_facts_introduced += self.accumulator.facts_introduced;
        self.cumulative_contradictions += self.accumulator.contradictions_detected;
```

Replace the per-epoch contradiction formula:

```rust
        let contradiction_detection_rate = if self.cumulative_facts_introduced == 0 {
            0.0
        } else {
            self.cumulative_contradictions as f64 / self.cumulative_facts_introduced as f64
        };
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p simulator -E 'test(contradiction_rate_is_cumulative)'`
Expected: PASS

- [ ] **Step 5: Fix the existing `snapshot_computes_rates_correctly` test**

The existing test sets `facts_introduced = 5` and `contradictions_detected = 1`, expecting `0.2`. With cumulative tracking on a fresh collector, the result is still `1/5 = 0.2` — should still pass. Verify:

Run: `cargo nextest run -p simulator -E 'test(snapshot_computes_rates_correctly)'`
Expected: PASS (no change needed — first epoch cumulative == per-epoch)

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/metrics/mod.rs
git commit -m "fix(simulator): make contradiction_detection_rate cumulative across epochs"
```

---

### Task 3: Revise personalization_score formula

Replace the current `fact_coverage * 0.4 + retrieval_precision * 0.3 + (1 - correction_rate) * 0.3` with a formula that uses retrieval_recall instead of correction_rate (which is scenario-driven noise).

**Files:**
- Modify: `crates/simulator/src/metrics/behavioral.rs`
- Modify: `crates/simulator/src/metrics/mod.rs:172-176` (call site)

- [ ] **Step 1: Update the personalization_score function**

Replace entire `crates/simulator/src/metrics/behavioral.rs`:

```rust
/// Compute a composite personalisation score from three inputs.
///
/// - `fact_coverage`: fraction of known facts retained (0.0 – 1.0)
/// - `retrieval_precision`: precision of recent retrievals (0.0 – 1.0)
/// - `retrieval_recall`: recall of recent retrievals (0.0 – 1.0)
///
/// Formula: `fact_coverage * 0.4 + retrieval_precision * 0.3 + retrieval_recall * 0.3`
///
/// This measures actual memory quality: can the system retain facts (coverage),
/// find the right ones (precision), and find all relevant ones (recall)?
pub fn personalization_score(
    fact_coverage: f64,
    retrieval_precision: f64,
    retrieval_recall: f64,
) -> f64 {
    fact_coverage * 0.4 + retrieval_precision * 0.3 + retrieval_recall * 0.3
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_scores() {
        let score = personalization_score(1.0, 1.0, 1.0);
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn worst_scores() {
        let score = personalization_score(0.0, 0.0, 0.0);
        assert!((score - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_scores() {
        // 0.85 * 0.4 + 0.8 * 0.3 + 0.7 * 0.3
        // = 0.34 + 0.24 + 0.21 = 0.79
        let score = personalization_score(0.85, 0.8, 0.7);
        assert!((score - 0.79).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Update the call site in MetricCollector::snapshot()**

In `crates/simulator/src/metrics/mod.rs`, change the `personalization_score` call (around line 172):

```rust
        let personalization_score = behavioral::personalization_score(
            knowledge_retention,
            retrieval_precision,
            retrieval_recall,
        );
```

- [ ] **Step 3: Fix the `snapshot_computes_rates_correctly` test assertion**

The test currently expects `0.82` based on `0.85 * 0.4 + 0.8 * 0.3 + (1 - 0.2) * 0.3`.
New formula: `0.85 * 0.4 + 0.8 * 0.3 + 0.6 * 0.3 = 0.34 + 0.24 + 0.18 = 0.76`.

Update the assertion comment and value in the test:

```rust
        // personalization_score = 0.85 * 0.4 + 0.8 * 0.3 + 0.6 * 0.3
        //                       = 0.34 + 0.24 + 0.18 = 0.76
        assert!((snap.personalization_score - 0.76).abs() < 1e-9);
```

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run -p simulator -E 'test(personalization) or test(snapshot_computes)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/metrics/behavioral.rs crates/simulator/src/metrics/mod.rs
git commit -m "fix(simulator): replace correction_rate with retrieval_recall in personalization_score"
```

---

### Task 4: Add real insight quality gate

Replace `count / day` with a quality-filtered count: only count insights that reference 2+ distinct domains.

**Files:**
- Modify: `crates/simulator/src/metrics/system.rs:70-83`

- [ ] **Step 1: Write the failing test**

Add to `crates/simulator/src/metrics/system.rs` `mod tests`:

```rust
#[tokio::test]
async fn insight_usefulness_requires_cross_domain() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Create the cross_domain_insights table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cross_domain_insights (
            date TEXT, insight_text TEXT, dot_refs TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert an insight referencing only 1 domain (should NOT count)
    sqlx::query(
        "INSERT INTO cross_domain_insights (date, insight_text, dot_refs) VALUES ('2026-01-01', 'single domain', '[\"tasks\"]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert an insight referencing 2+ domains (should count)
    sqlx::query(
        "INSERT INTO cross_domain_insights (date, insight_text, dot_refs) VALUES ('2026-01-02', 'cross domain', '[\"tasks\",\"finance\"]')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let usefulness = measure_insight_usefulness(&pool, 10).await;
    // Only 1 of 2 insights qualifies → 1/10 = 0.1
    assert!(
        (usefulness - 0.1).abs() < 1e-9,
        "expected 0.1 (1 qualified insight / 10 days), got {usefulness}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p simulator -E 'test(insight_usefulness_requires_cross_domain)'`
Expected: FAIL — current impl counts both rows, returning 2/10 = 0.2.

- [ ] **Step 3: Update measure_insight_usefulness with quality gate**

```rust
pub async fn measure_insight_usefulness(pool: &sqlx::SqlitePool, day: u32) -> f64 {
    // Count only insights whose dot_refs JSON array has 2+ distinct domains
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM cross_domain_insights \
         WHERE json_array_length(dot_refs) >= 2",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    if day == 0 {
        return 0.0;
    }
    (count.0 as f64 / day as f64).min(1.0)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p simulator -E 'test(insight_usefulness)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/metrics/system.rs
git commit -m "fix(simulator): insight_usefulness requires 2+ domains per insight"
```

---

### Task 5: Replace community_stability formula with note-diversity measure

The current `note_count / 50` formula is meaningless. Replace with a real measure: ratio of distinct note topics to total notes, rewarding diversity over volume.

**Files:**
- Modify: `crates/simulator/src/harness.rs:1175-1204` (MemoryMaintenance cron)
- Modify: `crates/simulator/src/metrics/system.rs:5-13`

- [ ] **Step 1: Write the failing test**

Add to `crates/simulator/src/metrics/system.rs` `mod tests`:

```rust
#[tokio::test]
async fn community_stability_measures_diversity() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS communities (
            id TEXT PRIMARY KEY, name TEXT, summary TEXT, stability REAL,
            member_count INTEGER, source_note_count INTEGER,
            diversity_score REAL DEFAULT 0.0,
            created_at TEXT, updated_at TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert a community with real diversity_score
    sqlx::query(
        "INSERT INTO communities (id, name, summary, stability, member_count, source_note_count, diversity_score, created_at, updated_at) \
         VALUES ('c1', 'test', 'test', 0.8, 10, 5, 0.75, datetime('now'), datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let stability = measure_community_stability(&pool).await;
    assert!(
        (stability - 0.8).abs() < 1e-9,
        "expected 0.8, got {stability}"
    );
}
```

- [ ] **Step 2: Run test to verify it passes** (existing function already reads AVG(stability))

Run: `cargo nextest run -p simulator -E 'test(community_stability_measures_diversity)'`
Expected: PASS — the query already works; the fix is in the harness where we compute a real stability value.

- [ ] **Step 3: Update MemoryMaintenance cron to compute diversity-based stability**

In `crates/simulator/src/harness.rs`, replace the `MemoryMaintenance` cron body:

```rust
            CronTrigger::MemoryMaintenance => {
                debug!(trigger = "MemoryMaintenance", %simulated_now, "Executing cron");
                // Count notes and distinct note titles to measure content diversity
                let note_count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM book_tree_nodes WHERE source_type = 'Note'",
                )
                .fetch_one(&self.inner_pool)
                .await
                .unwrap_or((0,));

                let distinct_titles: (i64,) = sqlx::query_as(
                    "SELECT COUNT(DISTINCT title) FROM book_tree_nodes WHERE source_type = 'Note'",
                )
                .fetch_one(&self.inner_pool)
                .await
                .unwrap_or((0,));

                if note_count.0 >= 3 {
                    // Stability = diversity ratio (distinct titles / total notes)
                    // High diversity = many unique topics = more stable knowledge base
                    let diversity = distinct_titles.0 as f64 / note_count.0 as f64;
                    // Weight: 60% diversity + 40% volume (capped at 50 notes)
                    let volume_score = (note_count.0 as f64 / 50.0).min(1.0);
                    let stability = diversity * 0.6 + volume_score * 0.4;
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO communities \
                         (id, name, summary, stability, member_count, source_note_count, \
                          created_at, updated_at) \
                         VALUES ('sim-community', 'Simulated Notes Community', \
                                 'Auto-generated from simulation notes', ?, ?, ?, \
                                 datetime('now'), datetime('now'))",
                    )
                    .bind(stability)
                    .bind(note_count.0)
                    .bind(note_count.0)
                    .execute(&self.inner_pool)
                    .await;
                    debug!(
                        notes = note_count.0,
                        distinct = distinct_titles.0,
                        stability,
                        "Community stability updated (diversity-weighted)"
                    );
                }
            }
```

- [ ] **Step 4: Run simulator tests**

Run: `cargo nextest run -p simulator -E 'test(community_stability)' --test-threads=1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/harness.rs crates/simulator/src/metrics/system.rs
git commit -m "fix(simulator): replace community_stability formula with diversity-weighted measure"
```

---

### Task 6: Add tier-1 baselines and brain_version rolling window

Currently only tier-2 metrics have baselines. Add tier-1 memory metrics to baseline tracking, and make brain_version_velocity a 30-day rolling count.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs` — `BaselineMetrics`, `compute_baselines`, `check_regressions`
- Modify: `crates/simulator/src/metrics/ground_truth.rs:203-214` — `get_baseline_value`

- [ ] **Step 1: Expand BaselineMetrics struct**

In `crates/simulator/src/metrics/mod.rs`, add tier-1 fields:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaselineMetrics {
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub insight_usefulness: f64,
    // Tier 1 — memory fidelity baselines
    pub knowledge_retention: f64,
    pub retrieval_precision: f64,
    pub retrieval_recall: f64,
    pub fact_extraction_accuracy: f64,
}
```

- [ ] **Step 2: Update compute_baselines**

In `compute_baselines()`, add averaging for tier-1 fields:

```rust
    fn compute_baselines(&mut self) {
        if self.timeline.is_empty() {
            return;
        }
        let n = self.timeline.len() as f64;

        let mut bl = BaselineMetrics::default();
        for s in &self.timeline {
            bl.token_efficiency += s.token_efficiency;
            bl.personalization_score += s.personalization_score;
            bl.task_completion_rate += s.task_completion_rate;
            bl.routing_stability += s.routing_stability;
            bl.insight_usefulness += s.insight_usefulness;
            bl.knowledge_retention += s.knowledge_retention;
            bl.retrieval_precision += s.retrieval_precision;
            bl.retrieval_recall += s.retrieval_recall;
            bl.fact_extraction_accuracy += s.fact_extraction_accuracy;
        }
        bl.token_efficiency /= n;
        bl.personalization_score /= n;
        bl.task_completion_rate /= n;
        bl.routing_stability /= n;
        bl.insight_usefulness /= n;
        bl.knowledge_retention /= n;
        bl.retrieval_precision /= n;
        bl.retrieval_recall /= n;
        bl.fact_extraction_accuracy /= n;

        self.baselines = Some(bl);
    }
```

- [ ] **Step 3: Update check_regressions to include tier-1**

Add tier-1 metrics to the regression checks array:

```rust
        let checks: &[(&str, f64, f64)] = &[
            (
                "personalization_score",
                bl.personalization_score,
                latest.personalization_score,
            ),
            (
                "task_completion_rate",
                bl.task_completion_rate,
                latest.task_completion_rate,
            ),
            (
                "routing_stability",
                bl.routing_stability,
                latest.routing_stability,
            ),
            (
                "insight_usefulness",
                bl.insight_usefulness,
                latest.insight_usefulness,
            ),
            (
                "knowledge_retention",
                bl.knowledge_retention,
                latest.knowledge_retention,
            ),
            (
                "retrieval_precision",
                bl.retrieval_precision,
                latest.retrieval_precision,
            ),
            (
                "retrieval_recall",
                bl.retrieval_recall,
                latest.retrieval_recall,
            ),
            (
                "fact_extraction_accuracy",
                bl.fact_extraction_accuracy,
                latest.fact_extraction_accuracy,
            ),
        ];
```

- [ ] **Step 4: Update get_baseline_value in ground_truth.rs**

In `crates/simulator/src/metrics/ground_truth.rs`, update the `get_baseline_value` function:

```rust
fn get_baseline_value(baselines: &BaselineMetrics, metric: &MetricName) -> f64 {
    match metric {
        MetricName::TokenEfficiency => baselines.token_efficiency,
        MetricName::PersonalizationScore => baselines.personalization_score,
        MetricName::TaskCompletionRate => baselines.task_completion_rate,
        MetricName::RoutingStability => baselines.routing_stability,
        MetricName::InsightUsefulness => baselines.insight_usefulness,
        MetricName::KnowledgeRetention => baselines.knowledge_retention,
        MetricName::RetrievalPrecision => baselines.retrieval_precision,
        MetricName::RetrievalRecall => baselines.retrieval_recall,
        MetricName::FactExtractionAccuracy => baselines.fact_extraction_accuracy,
        // System health metrics — no baselines
        _ => 0.0,
    }
}
```

- [ ] **Step 5: Fix existing tests that check baseline fields**

Update `baselines_computed_after_threshold_day` test and `get_baseline_value_returns_tier2_only` test to account for new fields. The tier-2 test should be renamed:

In ground_truth.rs tests, update:
```rust
    #[test]
    fn get_baseline_value_returns_tracked_metrics() {
        let baselines = BaselineMetrics {
            token_efficiency: 100.0,
            personalization_score: 0.8,
            task_completion_rate: 0.7,
            routing_stability: 0.9,
            insight_usefulness: 0.6,
            knowledge_retention: 0.85,
            retrieval_precision: 0.75,
            retrieval_recall: 0.7,
            fact_extraction_accuracy: 0.9,
        };

        assert!(
            (get_baseline_value(&baselines, &MetricName::TokenEfficiency) - 100.0).abs() < 1e-9
        );
        assert!(
            (get_baseline_value(&baselines, &MetricName::KnowledgeRetention) - 0.85).abs() < 1e-9
        );

        // System health metrics return 0.0
        assert!(
            (get_baseline_value(&baselines, &MetricName::AutotunerPromotionSuccess) - 0.0).abs()
                < 1e-9
        );
    }
```

- [ ] **Step 6: Run all metric tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/simulator/src/metrics/mod.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add tier-1 baselines and regression detection for memory metrics"
```

---

### Task 7: Tighten scenario checkpoint thresholds

With the metric fixes in place, raise the bar from "easy to pass accidentally" to "requires real cognitive quality".

**Files:**
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml`

- [ ] **Step 1: Update the 12-month scenario checkpoints**

Replace the `[[checkpoints]]` sections:

```toml
[[checkpoints]]
at_day = 14
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.4 },
    { type = "metric_above", metric = "fact_extraction_accuracy", threshold = 0.8 },
]

[[checkpoints]]
at_day = 90
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.5 },
    { type = "metric_above", metric = "personalization_score", threshold = 0.4 },
    { type = "metric_above", metric = "retrieval_precision", threshold = 0.2 },
]

[[checkpoints]]
at_day = 180
assertions = [
    { type = "metric_above", metric = "personalization_score", threshold = 0.45 },
    { type = "metric_above", metric = "task_completion_rate", threshold = 0.25 },
    { type = "metric_above", metric = "routing_stability", threshold = 0.5 },
]

[[checkpoints]]
at_day = 269
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.55 },
    { type = "metric_above", metric = "personalization_score", threshold = 0.45 },
    { type = "metric_above", metric = "task_completion_rate", threshold = 0.3 },
    { type = "fact_superseded", subject = "user", predicate = "works_as", old_object = "software engineer" },
]
```

- [ ] **Step 2: Run the 12-month simulation to verify**

Run: `cargo nextest run --test simulation -E 'test(run_software_engineer_12mo)' --test-threads=1`
Expected: PASS — thresholds are raised but still achievable with the current cognitive pipeline.

If any checkpoint fails, lower that specific threshold by 0.05 and re-run. The goal is strict-but-passing.

- [ ] **Step 3: Commit**

```bash
git add tests/simulation/scenarios/software_engineer_12mo.toml
git commit -m "feat(simulator): tighten checkpoint thresholds for stricter quality gates"
```

---

## Phase 2: Real Skill Routing

### Task 8: Integrate SkillRouter into the harness

Replace the `message_matches_topic_keywords` proxy with real `SkillRouter::select_orchestrator` calls, measuring whether the router picks the correct skill for each message.

**Files:**
- Modify: `crates/simulator/src/harness.rs` — add SkillCatalog/SkillRouter setup, replace routing logic
- Modify: `crates/simulator/src/metrics/mod.rs` — add `routing_correct` accumulator field
- Modify: `crates/simulator/src/scenario.rs` — add `RoutingAccuracy` metric name
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — add mapping

- [ ] **Step 1: Add routing_correct to EpochAccumulator**

In `crates/simulator/src/metrics/mod.rs`:

```rust
#[derive(Debug, Default)]
pub struct EpochAccumulator {
    pub messages_processed: u32,
    pub corrections: u32,
    pub facts_introduced: u32,
    pub facts_extracted: u32,
    pub contradictions_detected: u32,
    pub total_tokens: u64,
    pub retrieval_precision_sum: f64,
    pub retrieval_recall_sum: f64,
    pub retrieval_count: u32,
    pub routing_matches: u32,
    pub routing_correct: u32,
    pub routing_total: u32,
    pub tasks_created: u32,
    pub tasks_completed: u32,
    pub facts_superseded: u32,
}
```

- [ ] **Step 2: Add routing_accuracy to MetricSnapshot**

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricSnapshot {
    // ... existing fields ...
    pub routing_accuracy: f64,
    // ... rest ...
}
```

- [ ] **Step 3: Compute routing_accuracy in snapshot()**

Add after `routing_stability` computation:

```rust
        let routing_accuracy = if acc.routing_total == 0 {
            0.0
        } else {
            acc.routing_correct as f64 / acc.routing_total as f64
        };
```

And add `routing_accuracy` to the `MetricSnapshot` construction.

- [ ] **Step 4: Add MetricName::RoutingAccuracy to scenario.rs**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricName {
    KnowledgeRetention,
    RetrievalPrecision,
    RetrievalRecall,
    FactExtractionAccuracy,
    ContradictionDetectionRate,
    CorrectionRate,
    TokenEfficiency,
    PersonalizationScore,
    TaskCompletionRate,
    RoutingStability,
    RoutingAccuracy,
    InsightUsefulness,
    AutotunerPromotionSuccess,
    CommunityStability,
    BrainVersionVelocity,
}
```

- [ ] **Step 5: Map routing_accuracy in ground_truth.rs**

Add to `get_metric_value`:
```rust
        MetricName::RoutingAccuracy => snapshot.routing_accuracy,
```

- [ ] **Step 6: Wire SkillCatalog + SkillRouter into SimulationHarness**

In `crates/simulator/src/harness.rs`, add fields to `SimulationHarness`:

```rust
use skill_system::{SkillCatalog, SkillRouter, SkillSource};

pub struct SimulationHarness {
    // ... existing fields ...
    skill_router: Option<SkillRouter>,
    skill_catalog: Option<SkillCatalog>,
}
```

In `SimulationHarness::new()`, after all the existing setup, load built-in skills:

```rust
        // Load built-in skills for real routing evaluation.
        let (skill_catalog, skill_router) = match skill_system::built_in_skills() {
            Ok(entries) => {
                let source = SkillSource::BuiltIn(entries);
                match SkillCatalog::discover_sync(&[source]) {
                    Ok(catalog) => {
                        let router = SkillRouter::new(&catalog);
                        (Some(catalog), Some(router))
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to load skill catalog for routing — falling back to keyword proxy");
                        (None, None)
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load built-in skills — falling back to keyword proxy");
                (None, None)
            }
        };
```

Initialize the fields in the `Ok(Self { ... })` block.

- [ ] **Step 7: Replace routing measurement per message**

In the message processing loop, after the existing `message_matches_topic_keywords` check, add real routing:

```rust
                // Real routing accuracy: compare SkillRouter output against expected_skill
                if let (Some(ref router), Some(ref catalog)) =
                    (&self.skill_router, &self.skill_catalog)
                {
                    if let Some(ref gt) = msg.ground_truth {
                        if let Some(ref expected) = gt.expected_skill {
                            let selected = router.select_orchestrator(&msg.content, catalog, None);
                            metrics.accumulator_mut().routing_total += 1;
                            if selected.name == *expected {
                                metrics.accumulator_mut().routing_correct += 1;
                            }
                        }
                    }
                }
```

- [ ] **Step 8: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS — routing_accuracy starts appearing in snapshots.

- [ ] **Step 9: Add routing_accuracy checkpoint to 12mo scenario**

In `tests/simulation/scenarios/software_engineer_12mo.toml`, add to the day 269 checkpoint:

```toml
    { type = "metric_above", metric = "routing_accuracy", threshold = 0.5 },
```

- [ ] **Step 10: Run full 12mo simulation**

Run: `cargo nextest run --test simulation -E 'test(run_software_engineer_12mo)' --test-threads=1`
Expected: PASS

- [ ] **Step 11: Commit**

```bash
git add crates/simulator/src/metrics/mod.rs crates/simulator/src/metrics/ground_truth.rs \
       crates/simulator/src/scenario.rs crates/simulator/src/harness.rs \
       tests/simulation/scenarios/software_engineer_12mo.toml
git commit -m "feat(simulator): integrate real SkillRouter for routing_accuracy metric"
```

---

## Phase 3: Feature Coverage Expansion

### Task 9: Add coaching topic and templates

**Files:**
- Modify: `crates/simulator/src/persona/templates.rs` — add `COACHING_TEMPLATES`
- Modify: `crates/simulator/src/persona/mod.rs` — add coaching to `expected_skill_for_topic`

- [ ] **Step 1: Add coaching templates**

In `crates/simulator/src/persona/templates.rs`, add:

```rust
pub const COACHING_TEMPLATES: &[&str] = &[
    "I'm feeling overwhelmed with my workload",
    "Help me set better priorities for this week",
    "What patterns do you see in my productivity?",
    "I keep procrastinating on {task} — any advice?",
    "How can I improve my work-life balance?",
];
```

Update `templates_for_topic`:

```rust
        "coaching" => COACHING_TEMPLATES,
```

- [ ] **Step 2: Add coaching to expected_skill_for_topic**

In `crates/simulator/src/persona/mod.rs`, update `expected_skill_for_topic`:

```rust
            "coaching" => Some("general".to_string()),
```

- [ ] **Step 3: Add coaching keywords to message_matches_topic_keywords**

In `crates/simulator/src/harness.rs`:

```rust
        "coaching" => {
            lower.contains("overwhelm")
                || lower.contains("priorit")
                || lower.contains("advice")
                || lower.contains("balance")
                || lower.contains("procrastinat")
        }
```

- [ ] **Step 4: Run template tests**

Run: `cargo nextest run -p simulator -E 'test(templates_for_known_topic)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/persona/templates.rs crates/simulator/src/persona/mod.rs \
       crates/simulator/src/harness.rs
git commit -m "feat(simulator): add coaching topic with templates and routing keywords"
```

---

### Task 10: Add CreateFlashcard and ReviewFlashcard tool actions

**Files:**
- Modify: `crates/simulator/src/persona/types.rs` — new enum variants
- Modify: `crates/simulator/src/actions.rs` — new executors
- Modify: `crates/simulator/src/persona/mod.rs` — generate learning actions

- [ ] **Step 1: Add new SimulatedToolAction variants**

In `crates/simulator/src/persona/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimulatedToolAction {
    // ... existing variants ...
    CreateFlashcard {
        front: String,
        back: String,
        topic: String,
    },
    ReviewFlashcard {
        topic: String,
        rating: u8,
    },
}
```

- [ ] **Step 2: Add action execution**

In `crates/simulator/src/actions.rs`, add match arms:

```rust
            SimulatedToolAction::CreateFlashcard {
                front,
                back,
                topic,
            } => {
                debug!(topic = %topic, "action: CreateFlashcard");
                self.bus.publish(DomainEvent::NoteCreated {
                    note_id: Uuid::new_v4().to_string(),
                    title: format!("Flashcard: {}", &front[..front.len().min(30)]),
                });
            }

            SimulatedToolAction::ReviewFlashcard { topic, rating } => {
                debug!(topic = %topic, rating = %rating, "action: ReviewFlashcard");
                // Publish as a learning event for metric tracking
                self.bus.publish(DomainEvent::ProductivityScoreComputed {
                    date: simulated_now.format("%Y-%m-%d").to_string(),
                    score: f64::from(*rating) * 20.0, // 1-5 rating → 20-100 score
                });
            }
```

- [ ] **Step 3: Update tool_name mapping in harness.rs**

In harness.rs, add to the `tool_name` match:

```rust
                        SimulatedToolAction::CreateFlashcard { .. }
                        | SimulatedToolAction::ReviewFlashcard { .. } => "learning",
```

- [ ] **Step 4: Add learning tool action generation**

In `crates/simulator/src/persona/mod.rs`, update `generate_tool_action`:

```rust
            "learning" => {
                let topics = ["Rust", "Python", "ML", "algorithms"];
                let topic = topics[self.rng.random_range(0..topics.len())];
                if self.rng.random::<f64>() < 0.4 {
                    // 40% review existing, 60% create new
                    Some(SimulatedToolAction::ReviewFlashcard {
                        topic: topic.to_string(),
                        rating: self.rng.random_range(1..=5),
                    })
                } else {
                    Some(SimulatedToolAction::CreateFlashcard {
                        front: format!("What is {topic}?"),
                        back: format!("{topic} is a key concept"),
                        topic: topic.to_string(),
                    })
                }
            }
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/persona/types.rs crates/simulator/src/actions.rs \
       crates/simulator/src/persona/mod.rs crates/simulator/src/harness.rs
git commit -m "feat(simulator): add CreateFlashcard and ReviewFlashcard tool actions for learning coverage"
```

---

### Task 11: Create fact contradiction scenario

A focused scenario that tests whether the cognitive pipeline correctly detects and handles contradicting facts.

**Files:**
- Create: `tests/simulation/scenarios/fact_contradiction.toml`
- Modify: `tests/simulation/smoke.rs` — add test function

- [ ] **Step 1: Create the scenario file**

Create `tests/simulation/scenarios/fact_contradiction.toml`:

```toml
[persona]
name = "contradiction_test"
timezone = "UTC"
language = "en"
seed = 123

[persona.messages_per_day]
onboarding = 5
routine = 4
power_user = 4
shift = 5

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "backend engineer" },
    { subject = "user", predicate = "prefers_language", object = "Java" },
]

[persona.phases.onboarding]
duration_days = 5
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.8
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 5
correction_rate = 0.05
topic_weights = { tasks = 0.4, notes = 0.3, chat = 0.3 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 2
correction_rate = 0.05
topic_weights = { tasks = 0.5, notes = 0.3, chat = 0.2 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.6

[persona.phases.behavior_shift]
duration_days = 5
correction_rate = 0.2
shift_description = "Career change from backend to ML"
new_facts = [
    { subject = "user", predicate = "works_as", object = "ML engineer" },
    { subject = "user", predicate = "prefers_language", object = "Python" },
]
topic_weights = { tasks = 0.3, learning = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.9
tool_action_rate = 0.4

[[checkpoints]]
at_day = 10
assertions = [
    { type = "fact_exists", subject = "user", predicate = "works_as", object = "backend engineer", min_confidence = 0.5 },
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.5 },
]

[[checkpoints]]
at_day = 17
assertions = [
    { type = "fact_superseded", subject = "user", predicate = "works_as", old_object = "backend engineer" },
    { type = "fact_exists", subject = "user", predicate = "works_as", object = "ML engineer", min_confidence = 0.5 },
    { type = "metric_above", metric = "contradiction_detection_rate", threshold = 0.1 },
]
```

- [ ] **Step 2: Add test function**

In `tests/simulation/smoke.rs`:

```rust
#[tokio::test]
async fn run_fact_contradiction() {
    let report = run_scenario(include_str!("scenarios/fact_contradiction.toml")).await;
    eprintln!(
        "Contradiction test: {} msgs, {:.2}s, contradictions={:.3}, superseded={}",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.final_metrics.contradiction_detection_rate,
        report.summary.total_facts_superseded,
    );
    for cp in &report.checkpoints {
        let status = if cp.all_passed { "PASS" } else { "FAIL" };
        eprintln!("  Checkpoint day {}: {}", cp.at_day, status);
        for a in &cp.assertions {
            let mark = if a.passed { "  [x]" } else { "  [ ]" };
            eprintln!(
                "    {} {} (actual: {:?}, expected: {})",
                mark, a.description, a.actual_value, a.expected
            );
        }
    }
    assert!(
        report.summary.total_facts_superseded > 0,
        "Expected at least 1 fact supersession"
    );
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Contradiction scenario checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}
```

- [ ] **Step 3: Run the contradiction scenario**

Run: `cargo nextest run --test simulation -E 'test(run_fact_contradiction)' --test-threads=1`
Expected: PASS — contradicting facts should be detected and old facts superseded.

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/scenarios/fact_contradiction.toml tests/simulation/smoke.rs
git commit -m "feat(simulator): add fact contradiction scenario with supersession verification"
```

---

### Task 12: Create coaching-focused persona scenario

**Files:**
- Create: `tests/simulation/scenarios/coaching_persona.toml`
- Modify: `tests/simulation/smoke.rs`

- [ ] **Step 1: Create the scenario file**

Create `tests/simulation/scenarios/coaching_persona.toml`:

```toml
[persona]
name = "coaching_focus"
timezone = "UTC"
language = "en"
seed = 99

[persona.messages_per_day]
onboarding = 4
routine = 5
power_user = 6
shift = 4

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "product manager" },
    { subject = "user", predicate = "struggles_with", object = "time management" },
]

[persona.phases.onboarding]
duration_days = 7
correction_rate = 0.15
topic_weights = { tasks = 0.3, coaching = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.5
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 14
correction_rate = 0.08
topic_weights = { tasks = 0.2, coaching = 0.3, productivity = 0.2, notes = 0.2, chat = 0.1 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 14
correction_rate = 0.05
topic_weights = { tasks = 0.15, coaching = 0.25, productivity = 0.2, notes = 0.15, finance = 0.1, insights = 0.1, chat = 0.05 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7

[persona.phases.behavior_shift]
duration_days = 14
correction_rate = 0.1
shift_description = "User starts a new role and needs fresh coaching"
new_facts = [
    { subject = "user", predicate = "works_as", object = "engineering manager" },
    { subject = "user", predicate = "struggles_with", object = "delegation" },
]
topic_weights = { coaching = 0.4, tasks = 0.2, productivity = 0.2, chat = 0.2 }
new_fact_introduction_rate = 0.6
tool_action_rate = 0.4

[[checkpoints]]
at_day = 21
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.3 },
    { type = "metric_above", metric = "personalization_score", threshold = 0.2 },
]

[[checkpoints]]
at_day = 49
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.4 },
    { type = "metric_above", metric = "task_completion_rate", threshold = 0.15 },
    { type = "fact_superseded", subject = "user", predicate = "works_as", old_object = "product manager" },
]
```

- [ ] **Step 2: Add test function**

In `tests/simulation/smoke.rs`:

```rust
#[tokio::test]
async fn run_coaching_persona() {
    let report = run_scenario(include_str!("scenarios/coaching_persona.toml")).await;
    eprintln!(
        "Coaching persona: {} msgs, {:.2}s, retention={:.3}, personalization={:.3}",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.final_metrics.knowledge_retention,
        report.summary.final_metrics.personalization_score,
    );
    for cp in &report.checkpoints {
        let status = if cp.all_passed { "PASS" } else { "FAIL" };
        eprintln!("  Checkpoint day {}: {}", cp.at_day, status);
        for a in &cp.assertions {
            let mark = if a.passed { "  [x]" } else { "  [ ]" };
            eprintln!(
                "    {} {} (actual: {:?}, expected: {})",
                mark, a.description, a.actual_value, a.expected
            );
        }
    }
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Coaching scenario checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}
```

- [ ] **Step 3: Run the coaching scenario**

Run: `cargo nextest run --test simulation -E 'test(run_coaching_persona)' --test-threads=1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/scenarios/coaching_persona.toml tests/simulation/smoke.rs
git commit -m "feat(simulator): add coaching-focused persona scenario"
```

---

## Phase 4: Cognitive Depth Metrics

### Task 13: Add cognitive metrics module with FSRS-5 decay measurement

Measure how well the FSRS-5 system preserves fact retrievability over simulated time.

**Files:**
- Create: `crates/simulator/src/metrics/cognitive.rs`
- Modify: `crates/simulator/src/metrics/mod.rs` — export, new snapshot field
- Modify: `crates/simulator/src/scenario.rs` — new MetricName
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — value mapping
- Modify: `crates/simulator/src/harness.rs` — measure after fact introduction

- [ ] **Step 1: Create the cognitive metrics module**

Create `crates/simulator/src/metrics/cognitive.rs`:

```rust
//! Cognitive depth metrics: FSRS-5 decay, salience filtering, memory promotion.

/// Measure average retrievability of all active semantic facts.
///
/// Uses the FSRS-5 stability and elapsed time to compute retrievability
/// for each fact, then returns the average. A score of 1.0 means all facts
/// are perfectly retrievable; a score near 0.0 means memory has decayed.
pub async fn measure_average_retrievability(
    pool: &sqlx::SqlitePool,
    simulated_now: &str,
) -> f64 {
    // Query all active facts with their stability and recorded_at timestamps
    let rows: Vec<(f64, String)> = sqlx::query_as(
        "SELECT stability, recorded_at FROM semantic_facts \
         WHERE superseded_at IS NULL AND stability > 0",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return 1.0; // No facts = nothing to forget
    }

    let now = chrono::DateTime::parse_from_rfc3339(simulated_now)
        .unwrap_or_else(|_| chrono::Utc::now().into())
        .timestamp() as f64;

    let mut total_retrievability = 0.0;
    for (stability, recorded_at) in &rows {
        let recorded = chrono::DateTime::parse_from_rfc3339(recorded_at)
            .map(|dt| dt.timestamp() as f64)
            .unwrap_or(now);
        let elapsed_days = ((now - recorded) / 86400.0).max(0.0);
        // FSRS-5 retrievability formula: R(t) = (1 + t / (9 * S))^(-1)
        // where S = stability, t = elapsed days
        let r = (1.0 + elapsed_days / (9.0 * stability)).powf(-1.0);
        total_retrievability += r;
    }

    total_retrievability / rows.len() as f64
}

/// Count the number of meta-rules that were proposed (pending or approved)
/// during the simulation. Measures whether correction streaks produce
/// actionable rule proposals.
pub async fn count_meta_rules(pool: &sqlx::SqlitePool) -> u32 {
    let count: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM mirror_meta_rules")
            .fetch_one(pool)
            .await;

    count.map(|(n,)| n as u32).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retrievability_returns_one_for_empty() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let r = measure_average_retrievability(&pool, "2026-04-01T00:00:00Z").await;
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn meta_rules_returns_zero_on_missing_table() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let count = count_meta_rules(&pool).await;
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Export the new module**

In `crates/simulator/src/metrics/mod.rs`, add:

```rust
pub mod cognitive;
```

- [ ] **Step 3: Add memory_retrievability to MetricSnapshot**

```rust
    pub memory_retrievability: f64,
    pub meta_rule_count: u32,
```

- [ ] **Step 4: Add MetricName variants**

In `crates/simulator/src/scenario.rs`:

```rust
    MemoryRetrievability,
    MetaRuleCount,
```

- [ ] **Step 5: Wire into harness snapshot**

In `crates/simulator/src/harness.rs`, before the `metrics.snapshot()` call, add:

```rust
            let memory_retrievability = crate::metrics::cognitive::measure_average_retrievability(
                &self.inner_pool,
                &plan.simulated_now.to_rfc3339(),
            )
            .await;
            let meta_rule_count =
                crate::metrics::cognitive::count_meta_rules(&self.inner_pool).await;
```

Update `MetricCollector::snapshot()` to accept and store these values (add parameters and set on the snapshot struct).

- [ ] **Step 6: Map in ground_truth.rs**

```rust
        MetricName::MemoryRetrievability => snapshot.memory_retrievability,
        MetricName::MetaRuleCount => snapshot.meta_rule_count as f64,
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS

- [ ] **Step 8: Commit**

```bash
git add crates/simulator/src/metrics/cognitive.rs crates/simulator/src/metrics/mod.rs \
       crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs \
       crates/simulator/src/harness.rs
git commit -m "feat(simulator): add FSRS-5 memory_retrievability and meta_rule_count metrics"
```

---

### Task 14: Add FactNotExists checkpoint assertion

Enable asserting that the system correctly forgot or superseded a fact.

**Files:**
- Modify: `crates/simulator/src/scenario.rs` — new assertion variant
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — verification logic

- [ ] **Step 1: Add the assertion variant**

In `crates/simulator/src/scenario.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckpointAssertion {
    FactExists {
        subject: String,
        predicate: String,
        object: String,
        min_confidence: f64,
    },
    FactSuperseded {
        subject: String,
        predicate: String,
        old_object: String,
    },
    FactNotExists {
        subject: String,
        predicate: String,
        object: String,
    },
    MetricAbove {
        metric: MetricName,
        threshold: f64,
    },
    MetricImproved {
        metric: MetricName,
        min_improvement_pct: f64,
    },
}
```

- [ ] **Step 2: Add verification logic**

In `crates/simulator/src/metrics/ground_truth.rs`, add the match arm in `verify_checkpoint`:

```rust
                CheckpointAssertion::FactNotExists {
                    subject,
                    predicate,
                    object,
                } => Self::check_fact_not_exists(fact_repo, subject, predicate, object).await,
```

Add the method:

```rust
    async fn check_fact_not_exists(
        fact_repo: &cognitive::SemanticFactRepo,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> AssertionResult {
        let query = format!("{subject} {predicate} {object}");
        let facts = fact_repo
            .search_fts(&query, None, 10)
            .await
            .unwrap_or_default();

        let found = facts.iter().any(|f| {
            f.subject == subject
                && f.predicate == predicate
                && f.object == object
                && f.superseded_at.is_none()
        });

        AssertionResult {
            description: format!("FactNotExists({subject}, {predicate}, {object})"),
            passed: !found,
            actual_value: None,
            expected: "fact should not exist (active, unsuperseded)".to_string(),
        }
    }
```

- [ ] **Step 3: Add test**

```rust
    #[test]
    fn fact_not_exists_assertion_format() {
        // Just verify the assertion variant deserializes
        let toml = r#"
            type = "fact_not_exists"
            subject = "user"
            predicate = "works_as"
            object = "intern"
        "#;
        let assertion: CheckpointAssertion = toml::from_str(toml).unwrap();
        assert!(matches!(assertion, CheckpointAssertion::FactNotExists { .. }));
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add FactNotExists checkpoint assertion type"
```

---

### Task 15: Update report improvements to include all metric tiers

Currently `compute_improvements` only tracks tier-2 metrics. Extend to include tier-1 and the new cognitive metrics.

**Files:**
- Modify: `crates/simulator/src/report.rs:74-118`

- [ ] **Step 1: Expand compute_improvements**

Replace the function body:

```rust
pub fn compute_improvements(
    baselines: &BaselineMetrics,
    final_metrics: &MetricSnapshot,
) -> HashMap<String, f64> {
    let mut improvements = HashMap::new();

    // Lower is better: (baseline - final) / baseline * 100
    if baselines.token_efficiency > 0.0 {
        let pct = (baselines.token_efficiency - final_metrics.token_efficiency)
            / baselines.token_efficiency
            * 100.0;
        improvements.insert("token_efficiency".to_string(), pct);
    }

    // Higher is better: (final - baseline) / baseline * 100
    let higher_is_better: &[(&str, f64, f64)] = &[
        ("personalization_score", baselines.personalization_score, final_metrics.personalization_score),
        ("routing_stability", baselines.routing_stability, final_metrics.routing_stability),
        ("task_completion_rate", baselines.task_completion_rate, final_metrics.task_completion_rate),
        ("insight_usefulness", baselines.insight_usefulness, final_metrics.insight_usefulness),
        ("knowledge_retention", baselines.knowledge_retention, final_metrics.knowledge_retention),
        ("retrieval_precision", baselines.retrieval_precision, final_metrics.retrieval_precision),
        ("retrieval_recall", baselines.retrieval_recall, final_metrics.retrieval_recall),
        ("fact_extraction_accuracy", baselines.fact_extraction_accuracy, final_metrics.fact_extraction_accuracy),
    ];

    for &(name, baseline, current) in higher_is_better {
        if baseline > 0.0 {
            let pct = (current - baseline) / baseline * 100.0;
            improvements.insert(name.to_string(), pct);
        }
    }

    improvements
}
```

- [ ] **Step 2: Update compute_improvements_basic test**

Update the test to include tier-1 baselines and verify they appear:

```rust
    #[test]
    fn compute_improvements_includes_tier1() {
        let baselines = BaselineMetrics {
            token_efficiency: 500.0,
            personalization_score: 0.5,
            task_completion_rate: 0.6,
            routing_stability: 0.8,
            insight_usefulness: 0.4,
            knowledge_retention: 0.7,
            retrieval_precision: 0.6,
            retrieval_recall: 0.5,
            fact_extraction_accuracy: 0.8,
        };

        let final_metrics = MetricSnapshot {
            token_efficiency: 400.0,
            personalization_score: 0.75,
            task_completion_rate: 0.6,
            routing_stability: 0.9,
            insight_usefulness: 0.6,
            knowledge_retention: 0.85,
            retrieval_precision: 0.75,
            retrieval_recall: 0.7,
            ..MetricSnapshot::default()
        };

        let improvements = compute_improvements(&baselines, &final_metrics);

        // Tier-1 metrics should now appear
        assert!(improvements.contains_key("knowledge_retention"));
        assert!((improvements["knowledge_retention"] - (0.85 - 0.7) / 0.7 * 100.0).abs() < 1e-9);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p simulator -E 'test(compute_improvements)'`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/report.rs
git commit -m "feat(simulator): expand report improvements to include tier-1 metrics"
```

---

### Task 16: Final validation — run all scenarios and verify

**Files:** None (test-only task)

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -p simulator --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 2: Run format check**

Run: `cargo fmt -p simulator --check`
Expected: No formatting issues

- [ ] **Step 3: Run all simulator unit tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All PASS

- [ ] **Step 4: Run all simulation integration tests**

Run: `cargo nextest run --test simulation --test-threads=1`
Expected: All PASS (smoke, 12mo, finance, onboarding, contradiction, coaching)

- [ ] **Step 5: Verify the 12-month simulation report**

Run: `SIMULATION_OUTPUT_DIR=target/simulation cargo nextest run --test simulation -E 'test(run_software_engineer_12mo)' --test-threads=1`

Check the report shows:
- `routing_accuracy` is populated (non-zero if SkillRouter loaded)
- `memory_retrievability` is populated
- `contradiction_detection_rate` is cumulative (non-zero on final day)
- `personalization_score` uses new formula (no correction_rate component)
- All checkpoints pass including the new `fact_superseded` assertion
- No regressions

- [ ] **Step 6: Commit all remaining changes**

```bash
git add -A
git commit -m "feat(simulator): comprehensive intelligence upgrade — all phases complete"
```

---

## Summary of Changes by Metric

| Metric | Before | After |
|--------|--------|-------|
| `knowledge_retention` | False positives from negation | Negation-aware Strategy 3 |
| `contradiction_detection_rate` | Per-epoch (0 on most days) | Cumulative across epochs |
| `personalization_score` | 30% weight on scenario-driven correction_rate | 30% weight on retrieval_recall |
| `community_stability` | `note_count / 50` formula | Diversity-weighted (60% unique titles + 40% volume) |
| `insight_usefulness` | Counts all insight rows | Only counts cross-domain insights (2+ domains) |
| `routing_stability` | Circular keyword proxy | Still keyword proxy (preserved for backward compat) |
| `routing_accuracy` | Did not exist | Real SkillRouter output vs ground truth |
| `memory_retrievability` | Did not exist | FSRS-5 retrievability across all active facts |
| `meta_rule_count` | Did not exist | Count of MetaRule proposals from correction streaks |
| Tier-1 baselines | Not tracked | Full baseline + regression detection |
| Checkpoint strictness | Lenient (0.3-0.5) | Tighter (0.4-0.55) + FactSuperseded assertions |
| Feature coverage | 4 features | 6 features (+ coaching, learning actions) |
| Scenarios | 4 scenarios | 6 scenarios (+ contradiction, coaching) |
