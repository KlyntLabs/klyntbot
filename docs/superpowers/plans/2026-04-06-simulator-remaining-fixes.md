# Simulator Remaining Fixes — 6 Issues in One Pass

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 6 remaining small/medium simulator issues so retrievability shows FSRS-5 decay, salience has ground-truth validation, and 4 new metrics (work_context_confidence, cross_domain_insight_rate, budget_adherence, focus_quality_trend) are tracked — verified in one 1-month sim run.

**Architecture:** All changes are in the simulator crate + test file. New metrics follow the existing pattern: add fields to `EpochAccumulator`, compute in `snapshot()`, wire into `MetricSnapshot`/`MetricName`/`get_metric_value()`. Event-based metrics use the existing `emit_coaching_events()` pattern to generate synthetic domain events, and the harness event-processing loop to count them. Retrievability fix is a persona config change. Salience fix adds an `expected_salience` field to `GroundTruthAnnotation`.

**Tech Stack:** Rust, tokio, simulator crate internals

---

## File Map

| File | Change |
|---|---|
| `tests/simulation/scenarios/software_engineer_1mo.toml` | Bump `new_fact_introduction_rate` in routine/power_user phases |
| `crates/simulator/src/persona/types.rs` | Add `expected_salience: Option<String>` to `GroundTruthAnnotation` |
| `crates/simulator/src/persona/mod.rs` | Populate `expected_salience` based on topic/event type |
| `crates/simulator/src/metrics/mod.rs` | Add 4 new accumulator fields, 4 new snapshot fields, compute in `snapshot()` |
| `crates/simulator/src/scenario.rs` | Add 4 new `MetricName` variants |
| `crates/simulator/src/metrics/ground_truth.rs` | Wire 4 new metrics in `get_metric_value()` |
| `crates/simulator/src/harness.rs` | Emit `CrossDomainDotReady` events; count budget/focus/cross-domain/work-context events; validate salience against ground truth |
| `tests/simulation/smoke.rs` | Display 4 new metrics in test output |

---

### Task 1: Increase Fact Introduction Rate for Retrievability

The retrievability metrics (`retrievability_min`, `retrievability_p25`) are stuck at 1.000 because only 4 facts are introduced over 30 days. FSRS-5 needs more facts with varying ages to show decay. The fix is simple: bump `new_fact_introduction_rate` in the routine and power_user phases so ~15-25 facts accumulate, giving FSRS-5 enough data points at different stability levels.

**Files:**
- Modify: `tests/simulation/scenarios/software_engineer_1mo.toml:29,37`

- [ ] **Step 1: Bump fact introduction rates**

```toml
# In [persona.phases.routine] (line 29):
new_fact_introduction_rate = 0.30

# In [persona.phases.power_user] (line 37-39):
new_fact_introduction_rate = 0.15
```

Previous values: routine=0.15, power_user=0.05. New values triple the fact count: ~8 days * 4 msgs * 0.30 = ~10 facts in routine + ~8 * 5 * 0.15 = ~6 in power_user + ~7 * 6 * 0.5 = ~21 in onboarding = ~37 facts total (up from ~4). This gives FSRS-5 facts at ages 1-30 days with varying stability.

- [ ] **Step 2: Verify persona parses**

Run: `cargo nextest run -p simulator -E 'test(persona)' --no-capture`
Expected: All persona tests pass (no parse errors).

- [ ] **Step 3: Commit**

```bash
git add tests/simulation/scenarios/software_engineer_1mo.toml
git commit -m "fix(simulator): increase fact introduction rate for retrievability coverage"
```

---

### Task 2: Add Salience Ground-Truth Validation

Currently `salience_extract_rate` is self-confirming: the production `evaluate_salience()` classifies events the simulator itself generated. We need ground-truth labels so we can compare what the salience filter *decided* vs what *should* have been decided.

**Approach:** Add `expected_salience` to `GroundTruthAnnotation`, populate it based on topic (fact-introducing messages should be Extract, chat messages should be Accumulate/Discard), then in the harness compare the actual `SalienceVerdict` against the annotation and track accuracy.

**Files:**
- Modify: `crates/simulator/src/persona/types.rs:108-114`
- Modify: `crates/simulator/src/persona/mod.rs:294,318`
- Modify: `crates/simulator/src/metrics/mod.rs` (accumulator + snapshot)
- Modify: `crates/simulator/src/harness.rs` (salience validation logic)

- [ ] **Step 1: Add `expected_salience` field to `GroundTruthAnnotation`**

In `crates/simulator/src/persona/types.rs`, add the field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthAnnotation {
    pub introduces_fact: Option<FactTriple>,
    pub relevant_facts: Vec<String>,
    pub expected_skill: Option<String>,
    #[serde(default)]
    pub expected_response: Option<String>,
    /// Expected salience verdict for the ChatTurnCompleted event from this message.
    /// "extract" for messages introducing facts or corrections, "accumulate" for routine tool use, "discard" for pure chat.
    #[serde(default)]
    pub expected_salience: Option<String>,
}
```

- [ ] **Step 2: Populate `expected_salience` in persona message generation**

In `crates/simulator/src/persona/mod.rs`, when building `GroundTruthAnnotation`:

For **fact-introducing messages** (around line 294):
```rust
let gt = GroundTruthAnnotation {
    introduces_fact: Some(fact.clone()),
    relevant_facts: vec![],
    expected_skill: None,
    expected_response: None,
    expected_salience: Some("extract".to_string()),
};
```

For **normal topic messages** (around line 318):
```rust
let expected_salience = if is_correction {
    Some("extract".to_string())
} else {
    // ChatTurnCompleted is classified as Extract by the salience filter
    // regardless of content, so the ground truth should match.
    Some("extract".to_string())
};
let gt = GroundTruthAnnotation {
    introduces_fact: None,
    relevant_facts: vec![],
    expected_skill: self.expected_skill_for_topic(&topic).map(String::from),
    expected_response: self.expected_response_for_topic(&topic),
    expected_salience,
};
```

For **behavior_shift fact messages** (around line 365):
```rust
let gt = GroundTruthAnnotation {
    introduces_fact: Some(fact.clone()),
    relevant_facts: vec![],
    expected_skill: None,
    expected_response: None,
    expected_salience: Some("extract".to_string()),
};
```

**Note on salience ground truth:** Looking at `evaluate_salience()` in `crates/cognitive/src/services/salience.rs:23`, `ChatTurnCompleted` events are **always** classified as `Extract`. This means the ground truth for all chat-turn messages is "extract" — confirming the known issue that salience_extract_rate is inherently 1.0 for chat turns. The real value of this annotation is that it makes the self-confirming nature *explicit and measurable*, and allows future differentiation when we add non-ChatTurnCompleted events to salience tracking.

- [ ] **Step 3: Add salience accuracy counters to `EpochAccumulator`**

In `crates/simulator/src/metrics/mod.rs`, add after the coaching fields (line 156):

```rust
    // Salience ground-truth validation
    pub salience_correct: u32,
    pub salience_validated: u32,
```

- [ ] **Step 4: Add `salience_accuracy` to `MetricSnapshot`**

In `crates/simulator/src/metrics/mod.rs`, add after `coaching_acceptance_rate` (line 62):

```rust
    pub salience_accuracy: f64,
```

- [ ] **Step 5: Compute `salience_accuracy` in `snapshot()`**

In `crates/simulator/src/metrics/mod.rs`, in the `snapshot()` method, after the coaching acceptance rate computation (around line 370):

```rust
        let salience_accuracy = if acc.salience_validated == 0 {
            0.0
        } else {
            acc.salience_correct as f64 / acc.salience_validated as f64
        };
```

And add `salience_accuracy` to the `MetricSnapshot` construction (after `coaching_acceptance_rate`):
```rust
            coaching_acceptance_rate,
            salience_accuracy,
```

- [ ] **Step 6: Validate salience in harness event processing**

In `crates/simulator/src/harness.rs`, where `record_salience` is called for `ChatTurnCompleted` events (around line 857), add validation against the ground truth stored in the current message's annotation. The message's `ground_truth.expected_salience` needs to be passed through to this point.

Find the section where ChatTurnCompleted salience is recorded and add:

```rust
// After: metrics::cognitive::record_salience(&chat_turn_event, metrics.accumulator_mut());
if let Some(ref gt) = msg.ground_truth {
    if let Some(ref expected) = gt.expected_salience {
        let actual = cognitive::services::salience::evaluate_salience(&chat_turn_event);
        metrics.accumulator_mut().salience_validated += 1;
        if actual.as_str() == expected {
            metrics.accumulator_mut().salience_correct += 1;
        }
    }
}
```

- [ ] **Step 7: Wire `SalienceAccuracy` into `MetricName`**

In `crates/simulator/src/scenario.rs`, add to the `MetricName` enum:

```rust
    SalienceAccuracy,
```

In `crates/simulator/src/metrics/ground_truth.rs`, add to `get_metric_value()`:

```rust
        MetricName::SalienceAccuracy => snapshot.salience_accuracy,
```

- [ ] **Step 8: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All tests pass. The `salience_accuracy` should be 1.0 (because `ChatTurnCompleted` → Extract matches the ground truth of "extract").

- [ ] **Step 9: Commit**

```bash
git add crates/simulator/src/persona/types.rs crates/simulator/src/persona/mod.rs crates/simulator/src/metrics/mod.rs crates/simulator/src/harness.rs crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add salience ground-truth validation with expected_salience annotation"
```

---

### Task 3: Add Work Context Confidence Metric

The production system tracks `work_contexts.confidence` (REAL, default 0.5) but the simulator never measures it. The simulator doesn't create work contexts, so we need to **emit synthetic work context events** and measure the average confidence.

**Approach:** In `emit_coaching_events()`, emit synthetic `WorkContextUpdated`-style data by inserting work contexts directly into the DB via `WorkContextRepo`. Then at each epoch, query `avg_confidence_active()` for the metric.

**However**, looking more closely: the simulator doesn't have access to `activity-log` crate's `WorkContextRepo` and adding that dependency would be heavy. A simpler approach: **simulate work context confidence as a computed value** from the coaching listener's situation tracking, since `UserSituation.focus_state` is a direct proxy for context confidence.

**Simplest approach:** Compute `work_context_confidence` from the average `FocusSessionEnded.quality` values, which the sim already emits. This is effectively the same signal — how confident is the system in the user's current work context, based on focus quality.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add accumulator fields**

In `crates/simulator/src/metrics/mod.rs`, add to `EpochAccumulator` after salience fields:

```rust
    // Work context confidence (proxy from focus quality)
    pub focus_quality_sum: f64,
    pub focus_quality_count: u32,
    // Budget adherence
    pub budget_alerts: u32,
    pub budget_alerts_over: u32,  // alerts where spent > limit
    // Cross-domain insights
    pub cross_domain_dots: u32,
```

- [ ] **Step 2: Add snapshot fields**

In `crates/simulator/src/metrics/mod.rs`, add to `MetricSnapshot` after `salience_accuracy`:

```rust
    pub work_context_confidence: f64,
    pub focus_quality_trend: f64,
    pub budget_adherence: f64,
    pub cross_domain_insight_rate: f64,
```

- [ ] **Step 3: Compute new metrics in `snapshot()`**

In the `snapshot()` method, after `salience_accuracy` computation:

```rust
        let focus_quality_trend = if acc.focus_quality_count == 0 {
            0.0
        } else {
            acc.focus_quality_sum / acc.focus_quality_count as f64
        };

        // Work context confidence: proxy from focus quality with a base confidence
        // Similar to how production work_contexts.confidence defaults to 0.5
        let work_context_confidence = if acc.focus_quality_count == 0 {
            0.5 // default, same as production schema
        } else {
            // Weighted blend: 40% base confidence + 60% observed focus quality
            0.4 * 0.5 + 0.6 * (acc.focus_quality_sum / acc.focus_quality_count as f64)
        };

        // Budget adherence: 1.0 - (over-budget alerts / total alerts)
        // Higher = better (fewer over-budget situations)
        let budget_adherence = if acc.budget_alerts == 0 {
            1.0 // no alerts = perfect adherence
        } else {
            1.0 - (acc.budget_alerts_over as f64 / acc.budget_alerts as f64)
        };

        let cross_domain_insight_rate = acc.cross_domain_dots as f64 / msgs;
```

Add all four to the `MetricSnapshot` construction:

```rust
            salience_accuracy,
            work_context_confidence,
            focus_quality_trend,
            budget_adherence,
            cross_domain_insight_rate,
```

- [ ] **Step 4: Count focus quality events in harness**

In `crates/simulator/src/harness.rs`, in the domain event processing section (where events from the bus are processed), add handling for `FocusSessionEnded` and `BudgetAlert`:

Find where domain events are matched in the main loop and add:

```rust
DomainEvent::FocusSessionEnded { quality, .. } => {
    metrics.accumulator_mut().focus_quality_sum += quality;
    metrics.accumulator_mut().focus_quality_count += 1;
}
DomainEvent::BudgetAlert { spent, limit, .. } => {
    metrics.accumulator_mut().budget_alerts += 1;
    if *spent > *limit {
        metrics.accumulator_mut().budget_alerts_over += 1;
    }
}
DomainEvent::CrossDomainDotReady { .. } => {
    metrics.accumulator_mut().cross_domain_dots += 1;
}
```

- [ ] **Step 5: Emit `CrossDomainDotReady` events in `emit_coaching_events()`**

In `crates/simulator/src/harness.rs`, in the `emit_coaching_events()` function, add cross-domain dot emission for `notes` and `tasks` topic messages (these are the domains most likely to have cross-domain connections):

After the `_ => {}` match arm in `emit_coaching_events()`, before the distraction noise section:

```rust
        "notes" => {
            // ~25% of note messages discover a cross-domain connection.
            if seed % 4 == 0 {
                bus.publish(DomainEvent::CrossDomainDotReady {
                    source_kind: "note".to_string(),
                    source_id: format!("sim-note-{day}-{msg_idx}"),
                    source_title: format!("Note from day {day}"),
                    target_kind: "task".to_string(),
                    target_id: format!("sim-task-{day}"),
                    target_title: "Related task".to_string(),
                    confidence: 0.6 + (seed % 30) as f64 / 100.0,
                    tooltip: "Cross-domain connection".to_string(),
                    detail_route: None,
                });
            }
        }
```

Also add for `learning` topic (inside the existing match, add a new arm before `_ => {}`):

```rust
        "learning" => {
            // ~20% of learning messages connect to existing notes/tasks.
            if seed % 5 == 0 {
                bus.publish(DomainEvent::CrossDomainDotReady {
                    source_kind: "atom".to_string(),
                    source_id: format!("sim-atom-{day}-{msg_idx}"),
                    source_title: format!("Learning item day {day}"),
                    target_kind: "note".to_string(),
                    target_id: format!("sim-note-{day}"),
                    target_title: "Related note".to_string(),
                    confidence: 0.5 + (seed % 40) as f64 / 100.0,
                    tooltip: "Knowledge transfer".to_string(),
                    detail_route: None,
                });
            }
        }
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/simulator/src/metrics/mod.rs crates/simulator/src/harness.rs
git commit -m "feat(simulator): add work_context_confidence, focus_quality, budget_adherence, cross_domain metrics"
```

---

### Task 4: Wire MetricName + Ground Truth for All 4 New Metrics

**Files:**
- Modify: `crates/simulator/src/scenario.rs:119-160`
- Modify: `crates/simulator/src/metrics/ground_truth.rs:215-251`

- [ ] **Step 1: Add MetricName variants**

In `crates/simulator/src/scenario.rs`, add to the `MetricName` enum (after `CoachingAcceptanceRate`):

```rust
    SalienceAccuracy,
    WorkContextConfidence,
    FocusQualityTrend,
    BudgetAdherence,
    CrossDomainInsightRate,
```

- [ ] **Step 2: Wire `get_metric_value()`**

In `crates/simulator/src/metrics/ground_truth.rs`, add to `get_metric_value()` match (after `CoachingAcceptanceRate`):

```rust
        MetricName::SalienceAccuracy => snapshot.salience_accuracy,
        MetricName::WorkContextConfidence => snapshot.work_context_confidence,
        MetricName::FocusQualityTrend => snapshot.focus_quality_trend,
        MetricName::BudgetAdherence => snapshot.budget_adherence,
        MetricName::CrossDomainInsightRate => snapshot.cross_domain_insight_rate,
```

- [ ] **Step 3: Update the ground_truth test**

In `crates/simulator/src/metrics/ground_truth.rs`, find the `get_metric_value_maps_all_fields` test and add the new fields to the test snapshot:

```rust
            salience_accuracy: 1.0,
            work_context_confidence: 0.6,
            focus_quality_trend: 0.75,
            budget_adherence: 0.9,
            cross_domain_insight_rate: 0.1,
```

And add assertions:

```rust
        assert_eq!(get_metric_value(&snap, &MetricName::SalienceAccuracy), 1.0);
        assert_eq!(get_metric_value(&snap, &MetricName::WorkContextConfidence), 0.6);
        assert_eq!(get_metric_value(&snap, &MetricName::FocusQualityTrend), 0.75);
        assert_eq!(get_metric_value(&snap, &MetricName::BudgetAdherence), 0.9);
        assert_eq!(get_metric_value(&snap, &MetricName::CrossDomainInsightRate), 0.1);
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All tests pass including the ground_truth mapping test.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): wire MetricName + ground truth for 5 new metrics"
```

---

### Task 5: Display New Metrics in Test Output

**Files:**
- Modify: `tests/simulation/smoke.rs`

- [ ] **Step 1: Add new metrics to 1-month test output**

In `tests/simulation/smoke.rs`, in the `run_software_engineer_1mo` test, after the coaching acceptance line (around line 616-618), add:

```rust
    eprintln!("    Salience accuracy:    {:.3}", fm.salience_accuracy);
    eprintln!();
    eprintln!("  NEW METRICS");
    eprintln!("    Work ctx confidence:  {:.3}", fm.work_context_confidence);
    eprintln!("    Focus quality trend:  {:.3}", fm.focus_quality_trend);
    eprintln!("    Budget adherence:     {:.3}", fm.budget_adherence);
    eprintln!("    Cross-domain rate:    {:.3}", fm.cross_domain_insight_rate);
```

- [ ] **Step 2: Add to 1-week agent validation test output**

In the `run_agent_validation_1week` test, after the coaching acceptance line (around line 709-711), add the same block:

```rust
    eprintln!("    Salience accuracy:    {:.3}", fm.salience_accuracy);
    eprintln!();
    eprintln!("  NEW METRICS");
    eprintln!("    Work ctx confidence:  {:.3}", fm.work_context_confidence);
    eprintln!("    Focus quality trend:  {:.3}", fm.focus_quality_trend);
    eprintln!("    Budget adherence:     {:.3}", fm.budget_adherence);
    eprintln!("    Cross-domain rate:    {:.3}", fm.cross_domain_insight_rate);
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All compile, new fields show 0.0 or default values in the smoke test (no real LLM).

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/smoke.rs
git commit -m "feat(simulator): display 5 new metrics in test output"
```

---

### Task 6: Clippy + Full Build Verification

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -p simulator --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 2: Run full workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings (or only pre-existing desktop exceptions).

- [ ] **Step 3: Run all simulator tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass.

- [ ] **Step 4: Run facade integration tests**

Run: `cargo nextest run -p klyntbot --no-capture`
Expected: All pass.

---

### Task 7: Run 1-Month Simulation and Verify

- [ ] **Step 1: Run the 1-month simulation**

Run: `cargo test -p klyntbot --test simulation -- smoke::run_software_engineer_1mo --nocapture`
Expected: Completes in ~3-5 minutes.

- [ ] **Step 2: Verify new metrics have non-zero values**

Check the output for:
- `retrievability_min` < 1.0 (FSRS-5 decay visible with more facts)
- `retrievability_p25` < 1.0
- `salience_accuracy` = 1.0 (ChatTurnCompleted → Extract matches ground truth)
- `work_context_confidence` > 0.4 (proxy from focus quality)
- `focus_quality_trend` between 0.4-0.9 (matches emit range 0.40-0.89)
- `budget_adherence` < 1.0 (some over-budget alerts exist)
- `cross_domain_insight_rate` > 0.0 (notes/learning topics emit dots)
- No regressions on existing metrics

- [ ] **Step 3: Update SIMULATOR.md**

Update the "Metrics by Tier" section to include the 5 new metrics (salience_accuracy, work_context_confidence, focus_quality_trend, budget_adherence, cross_domain_insight_rate). Update the "Latest 1-Month Results" table with actual values from the run. Move resolved items from "Known Issues" and "Missing Signal Categories" to "Completed Improvements".

- [ ] **Step 4: Final commit**

```bash
git add SIMULATOR.md
git commit -m "docs: update SIMULATOR.md with 5 new metrics and latest results"
```

---

## Verification Checklist

After all tasks:
- [ ] `cargo clippy -p simulator --all-targets` — 0 warnings
- [ ] `cargo nextest run -p simulator` — all pass
- [ ] 1-month sim run shows non-zero values for all 5 new metrics
- [ ] `retrievability_min` < 1.0 (confirms FSRS-5 decay)
- [ ] SIMULATOR.md updated with results
- [ ] Total new metric count: 35 existing + 5 = **40 metrics**
