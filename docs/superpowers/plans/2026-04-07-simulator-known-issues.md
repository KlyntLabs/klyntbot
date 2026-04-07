# Simulator Known Issues — Measurement Accuracy Fixes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 3 measurement accuracy bugs in the simulator so metrics reflect real platform behavior instead of artifacts of the test harness. No production code changes. Issue 4 (ToolSelectionMismatch) is documented LLM variance — no code fix needed.

**Architecture:** All changes are in the simulator crate. Issue 1 is a SQL column fix (query reads wall-clock `recorded_at` instead of simulated `valid_from`). Issue 2 adds salience recording for coaching events already being emitted. Issue 3 adds cumulative tracking for response quality following the existing `cumulative_tasks_completed` pattern.

**Tech Stack:** Rust, SQLite, simulator crate internals

**Key constraint:** The simulator measures real platform behavior. Changes improve measurement accuracy — they don't game metrics to show better numbers.

---

## File Map

| File | Change |
|---|---|
| `crates/simulator/src/metrics/cognitive.rs:20` | Fix SQL: use `valid_from` instead of `recorded_at` for retrievability elapsed time |
| `crates/simulator/src/harness.rs:~600` | Record salience for coaching events (FocusSessionStarted/Ended, TaskDeferred, BudgetAlert, DistractionDetected) |
| `crates/simulator/src/metrics/mod.rs:163-200,310-317` | Add cumulative response quality tracking to MetricCollector |
| `SIMULATOR.md:96-104` | Update known issues to reflect fixes |

---

### Task 1: Fix Retrievability SQL — Use Simulated Time

**Root cause:** `measure_retrievability_distribution()` computes elapsed days as `simulated_now - recorded_at`. But `recorded_at` is set by `extraction.rs:93` to `Utc::now()` (wall-clock time), while `simulated_now` is the simulated clock (e.g. 2026-04-01). The entire 30-day simulation runs in ~40 minutes of wall time, so `recorded_at` for ALL facts is within 40 minutes of each other → elapsed_days ≈ 0 → retrievability ≈ 1.000.

The fix: use `valid_from` instead of `recorded_at`. `valid_from` is set from `observation.timestamp` (line 106), which is the simulated `msg.simulated_at` — the correct time axis. This is not gaming the metric: `valid_from` is when the fact was observed in the simulated world.

**Files:**
- Modify: `crates/simulator/src/metrics/cognitive.rs:20`

- [ ] **Step 1: Fix the SQL query**

Change line 20 in `crates/simulator/src/metrics/cognitive.rs`:

```rust
// Before:
"SELECT stability, CAST(strftime('%s', recorded_at) AS REAL) \
 FROM semantic_facts \
 WHERE superseded_at IS NULL AND stability > 0",

// After:
"SELECT stability, CAST(strftime('%s', valid_from) AS REAL) \
 FROM semantic_facts \
 WHERE superseded_at IS NULL AND stability > 0",
```

- [ ] **Step 2: Update the test to use different valid_from dates**

The existing test at `cognitive.rs:149-175` (`retrievability_distribution_with_facts`) already inserts facts with different `recorded_at` values AND sets `valid_from` to the same value (line 162: `VALUES (?1, 'test', 's', 'p', 'o', 0.9, 'sim', ?2, ?2, ?3, ...)`). Since both columns use `?2`, the test already passes with either column. No test change needed.

- [ ] **Step 3: Run the test**

Run: `cargo nextest run -p simulator -E 'test(retrievability)' --no-capture`
Expected: All retrievability tests pass. The `retrievability_distribution_with_facts` test should show unchanged results since its `valid_from` and `recorded_at` are identical.

- [ ] **Step 4: Verify with the smoke test**

Run: `cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: PASS. Retrievability will now show decay for facts created in earlier simulated days.

---

### Task 2: Diversify Salience Ground-Truth Events

**Root cause:** Only `ChatTurnCompleted` events are recorded for salience, which always evaluate to "extract". The coaching pipeline already emits `FocusSessionStarted` (→ Accumulate), `FocusSessionEnded` (→ Accumulate), `TaskDeferred` (→ Accumulate), `BudgetAlert` (→ Extract), `DistractionDetected` (→ Accumulate). These are real domain events with real verdicts — recording them gives a realistic Extract/Accumulate mix.

**Files:**
- Modify: `crates/simulator/src/harness.rs:~600` (after `emit_coaching_events` call, inside the existing coaching-event metrics block)

- [ ] **Step 1: Add salience recording for coaching events**

In `harness.rs`, find the block starting with `// Count metrics from the coaching events we just emitted.` (around line 580). This block already mirrors `emit_coaching_events` logic. Add salience recording + ground-truth validation alongside the existing metric counting.

Replace the entire `// Count metrics from the coaching events we just emitted.` block with:

```rust
// Count metrics from the coaching events we just emitted.
// Also record salience + ground-truth for non-ChatTurnCompleted events.
{
    let seed = msg_idx.wrapping_mul(31).wrapping_add(day_counter as usize);
    match msg.topic.as_str() {
        "productivity" | "coaching" => {
            let quality = 0.4 + (seed % 50) as f64 / 100.0;
            metrics.accumulator_mut().focus_quality_sum += quality;
            metrics.accumulator_mut().focus_quality_count += 1;

            // Salience: FocusSessionStarted → Accumulate, FocusSessionEnded → Accumulate
            let start_evt = DomainEvent::FocusSessionStarted {
                session_type: "pomodoro".to_string(),
                target_mins: 25 + (seed % 35) as i64,
            };
            let end_evt = DomainEvent::FocusSessionEnded {
                duration_secs: 1500,
                quality,
                interruptions: (seed % 4) as i32,
            };
            crate::metrics::cognitive::record_salience(&start_evt, metrics.accumulator_mut());
            crate::metrics::cognitive::record_salience(&end_evt, metrics.accumulator_mut());
            // Ground-truth: both should be Accumulate
            metrics.accumulator_mut().salience_validated += 2;
            if cognitive::services::salience::evaluate_salience(&start_evt).as_str() == "accumulate" {
                metrics.accumulator_mut().salience_correct += 1;
            }
            if cognitive::services::salience::evaluate_salience(&end_evt).as_str() == "accumulate" {
                metrics.accumulator_mut().salience_correct += 1;
            }
        }
        "finance" => {
            if seed % 10 < 4 {
                let spent = 80.0 + (seed % 40) as f64;
                metrics.accumulator_mut().budget_alerts += 1;
                if spent > 100.0 {
                    metrics.accumulator_mut().budget_alerts_over += 1;
                }

                // Salience: BudgetAlert → Extract
                let evt = DomainEvent::BudgetAlert {
                    category: "dining".to_string(),
                    spent,
                    limit: 100.0,
                };
                crate::metrics::cognitive::record_salience(&evt, metrics.accumulator_mut());
                metrics.accumulator_mut().salience_validated += 1;
                if cognitive::services::salience::evaluate_salience(&evt).as_str() == "extract" {
                    metrics.accumulator_mut().salience_correct += 1;
                }
            }
        }
        "tasks" => {
            // TaskDeferred events (~30% of task messages)
            if seed % 10 < 3 {
                let evt = DomainEvent::TaskDeferred {
                    task_id: format!("sim-task-{day_counter}-{msg_idx}"),
                    times_deferred: 1 + (seed % 3) as i32,
                };
                crate::metrics::cognitive::record_salience(&evt, metrics.accumulator_mut());
                metrics.accumulator_mut().salience_validated += 1;
                if cognitive::services::salience::evaluate_salience(&evt).as_str() == "accumulate" {
                    metrics.accumulator_mut().salience_correct += 1;
                }
            }
        }
        "notes" => {
            if seed % 4 == 0 {
                metrics.accumulator_mut().cross_domain_dots += 1;
            }
        }
        "learning" => {
            if seed % 5 == 0 {
                metrics.accumulator_mut().cross_domain_dots += 1;
            }
        }
        _ => {}
    }

    // DistractionDetected (~20% of ALL messages) → Accumulate
    if seed % 5 == 0 {
        let evt = DomainEvent::DistractionDetected {
            app: "social_media".to_string(),
            duration_secs: Some(30 + (seed % 90) as i64),
            context: format!("day{day_counter}_msg{msg_idx}"),
        };
        crate::metrics::cognitive::record_salience(&evt, metrics.accumulator_mut());
        metrics.accumulator_mut().salience_validated += 1;
        if cognitive::services::salience::evaluate_salience(&evt).as_str() == "accumulate" {
            metrics.accumulator_mut().salience_correct += 1;
        }
    }
}
```

**Expected outcome:** `salience_accuracy` will now reflect a mix of event types. With productivity → 2 Accumulate, finance → 1 Extract, tasks → 1 Accumulate, all → ~20% Accumulate (distraction), plus the existing ChatTurnCompleted → Extract per message, the accuracy should still be near 1.0 (since we use the real `evaluate_salience()` to validate), but the underlying data is no longer self-confirming.

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All 95 tests pass. The `salience_extract_rate` metric will shift since we're now recording more Accumulate events.

- [ ] **Step 3: Run smoke test**

Run: `cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: PASS. `salience_accuracy` still near 1.0 (correct classification), `salience_extract_rate` lower than before (reflecting the Accumulate events).

---

### Task 3: Make Response Quality Cumulative

**Root cause:** `response_quality` in `MetricSnapshot` is computed from per-epoch accumulator only. When the last epoch has all agent errors (`agent_response_quality_count == 0`), it falls to 0.0. `final_metrics = timeline.last()` reports only the last epoch. This is the same pattern that `task_completion_rate` and `coaching_acceptance_rate` already solve with cumulative tracking.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs`

- [ ] **Step 1: Add cumulative fields to MetricCollector**

In the `MetricCollector` struct (after `cumulative_coaching_stop: u32`), add:

```rust
    cumulative_response_quality_sum: f64,
    cumulative_response_quality_count: u32,
```

In `MetricCollector::new()`, initialize both to `0.0` / `0`:

```rust
            cumulative_response_quality_sum: 0.0,
            cumulative_response_quality_count: 0,
```

- [ ] **Step 2: Accumulate in snapshot()**

In `snapshot()`, after the existing `self.cumulative_coaching_stop += acc.coaching_stop;` line, add:

```rust
        self.cumulative_response_quality_sum +=
            acc.agent_response_quality_sum + acc.response_quality_sum;
        self.cumulative_response_quality_count +=
            acc.agent_response_quality_count + acc.response_quality_count;
```

- [ ] **Step 3: Change response_quality computation to use cumulative values**

Replace the existing response_quality computation (the block that checks `acc.agent_response_quality_count > 0` then `acc.response_quality_count > 0`) with:

```rust
        let response_quality = if self.cumulative_response_quality_count > 0 {
            self.cumulative_response_quality_sum / self.cumulative_response_quality_count as f64
        } else {
            0.0
        };
```

This follows the exact same pattern as `coaching_acceptance_rate` and `task_completion_rate`.

- [ ] **Step 4: Run the unit tests**

Run: `cargo nextest run -p simulator -E 'test(snapshot_computes_rates)' --no-capture`
Expected: PASS — the existing test sets `agent_response_quality_count = 0` so the result stays 0.0 (cumulative from single epoch with no agent quality data).

Run: `cargo nextest run -p simulator --no-capture`
Expected: All 95 tests pass.

- [ ] **Step 5: Run smoke test**

Run: `cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: PASS.

---

### Task 4: Update SIMULATOR.md + Commit

- [ ] **Step 1: Update SIMULATOR.md Known Issues**

Replace the Known Issues section with updated text reflecting the fixes:

- **retrievability_min/p25** — now uses `valid_from` (simulated time) for elapsed-day calculation. With bumped fact introduction rates and correct time axis, FSRS-5 decay is visible across 30-day runs.
- **salience_accuracy** — now validates a mix of event types (ChatTurnCompleted, FocusSessionStarted/Ended, TaskDeferred, BudgetAlert, DistractionDetected) instead of only ChatTurnCompleted.
- **response_quality** — now cumulative across all epochs, matching the pattern of task_completion_rate and coaching_acceptance_rate. Error-heavy epochs no longer zero out the metric.
- **ToolSelectionMismatch** — unchanged, documented LLM variance.

- [ ] **Step 2: Run full test suite**

Run: `cargo nextest run -p simulator --no-capture && cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/simulator/src/metrics/cognitive.rs \
       crates/simulator/src/metrics/mod.rs \
       crates/simulator/src/harness.rs \
       SIMULATOR.md
git commit -m "fix(simulator): improve measurement accuracy for 3 metrics

- retrievability: use valid_from (simulated time) instead of recorded_at
  (wall-clock) for elapsed-day calculation, making FSRS-5 decay visible
- salience_accuracy: record salience for coaching events (FocusSession,
  TaskDeferred, BudgetAlert, Distraction) alongside ChatTurnCompleted
- response_quality: cumulative tracking across epochs instead of last-epoch
  only, preventing error-heavy epochs from zeroing out the metric"
```

---

## Verification

After all tasks, run the 1-month real-LLM simulation to verify:

```bash
DEEPSEEK_API_KEY=<key> cargo nextest run --test simulation -E 'test(run_software_engineer_1mo)' --no-capture
```

**Expected changes in the output:**
- `retrievability_min` < 1.0 (should show FSRS-5 decay for facts from earlier days)
- `salience_accuracy` still near 1.0 (correct classification), but `salience_extract_rate` < 1.0 (reflecting Accumulate events)
- `response_quality` > 0.0 even if the last epoch had errors (cumulative average from earlier good epochs)
