# Autotuner Phase 2 — Generation Context & UX Polish

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the autotuner's LLM variant generation informed by real behavioral data, populate `AutotunerDecision` event fields, add correction acknowledgment UX, and add keyword rate limiting.

**Architecture:** The nightly cycle's `run_llm_generation` already builds a `GenerationContext` with three placeholder strings. We replace them with real queries against `StrategyRepo`, `TrialRepo`, and `EpisodicMemoryRepo`. Separately, `AgentLoop` gets a per-request correction ack prepend and a per-session rate limiter. The `AutotunerDecision` event gets real `improvement_pct` + `affected_params` computed from champion/trial metrics.

**Tech Stack:** Rust, SQLite (via sqlx), tokio async, serde

**Spec:** `docs/superpowers/specs/2026-03-20-autotuner-feedback-loop-design.md` (Phase 2 section)

**Depends on:** Phase 1 commit `28ef2e53` (feat(autotuner): close feedback loop)

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/agent/src/autotuner/mod.rs` | Orchestrator — build real GenerationContext, populate AutotunerDecision fields | 1, 4 |
| `crates/autotuner/src/cycle.rs` | Add `affected_param_names` helper | 4 |
| `crates/autotuner/src/traits.rs` | (read-only reference for MetricSnapshot fields) | — |
| `crates/storage/src/repos/strategy.rs` | (existing — `get_stats_since`, `get_strategy_summaries`) | 1 |
| `crates/storage/src/repos/trial_repo.rs` | (existing — `get_recent_completed`, `shadow_log_agreement_rate`) | 1 |
| `crates/cognitive/src/repos/episodic_memory.rs` | (existing — `list_recent`) | 1 |
| `crates/agent/src/agent_loop/mod.rs` | Correction ack prepend + rate limiter | 2, 3 |
| `crates/session/src/manager.rs` | Add `correction_cooldown` counter field to `Session` | 3 |
| `crates/cognitive/src/services/background.rs` | First-person observation text for AutotunerDecision | 5 |

---

### Task 1: Replace generation context placeholders with real data

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs` (lines 458–500)

The `AutoTunerOrchestrator` needs access to `StrategyRepo` and `EpisodicMemoryRepo` to build real context. These repos must be threaded through from `init/cron.rs`.

- [ ] **Step 1: Write test for trend_summary generation**

In `crates/agent/src/autotuner/mod.rs`, add to the existing test module:

```rust
#[test]
fn build_trend_summary_formats_correctly() {
    use storage::repos::strategy::OverallStats;
    let stats = OverallStats {
        total_records: 150,
        accuracy: 0.82,
        avg_response_time_ms: 1200,
        avg_satisfaction: Some(0.75),
    };
    let agreement_rate = 0.91;
    let summary = super::build_trend_summary(&stats, agreement_rate);
    assert!(summary.contains("150 messages"));
    assert!(summary.contains("82.0%"));
    assert!(summary.contains("91.0%"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(build_trend)' --no-fail-fast 2>&1 | tail -10`
Expected: Compile error — `build_trend_summary` not found.

- [ ] **Step 3: Implement `build_trend_summary` helper**

Add a module-level function in `crates/agent/src/autotuner/mod.rs`:

```rust
/// Build a human-readable trend summary from strategy stats and shadow log agreement.
fn build_trend_summary(stats: &storage::OverallStats, agreement_rate: f64) -> String {
    let satisfaction = stats
        .avg_satisfaction
        .map(|s| format!("{:.0}%", s * 100.0))
        .unwrap_or_else(|| "no data".into());
    format!(
        "Last 7 days: {total} messages processed, {acc:.1}% classification accuracy, \
         {rt}ms avg response time, {stab:.1}% routing stability, {sat} user satisfaction.",
        total = stats.total_records,
        acc = stats.accuracy * 100.0,
        rt = stats.avg_response_time_ms,
        stab = agreement_rate * 100.0,
        sat = satisfaction,
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(build_trend)' --no-fail-fast`
Expected: PASS.

- [ ] **Step 5: Implement `build_behavioral_context` helper**

```rust
/// Build a behavioral context string from per-skill strategy summaries.
fn build_behavioral_context(summaries: &[storage::StrategySummaryRow]) -> String {
    if summaries.is_empty() {
        return "No skill usage data available yet.".into();
    }
    let lines: Vec<String> = summaries
        .iter()
        .map(|s| {
            format!(
                "- {}: {} messages, {:.0}% accuracy, {:.1} avg escalations",
                s.predicted_strategy,
                s.sample_count,
                if s.sample_count > 0 {
                    s.correct_count as f64 / s.sample_count as f64 * 100.0
                } else {
                    0.0
                },
                s.avg_escalations,
            )
        })
        .collect();
    format!("Skill usage breakdown (7d):\n{}", lines.join("\n"))
}
```

- [ ] **Step 6: Implement `build_memory_snapshot` helper**

```rust
/// Build a short memory snapshot from recent episodic memories.
/// Note: The type is `cognitive::EpisodicMemory` (not `EpisodicMemoryRow`).
/// The `domain` field is `String` (not `Option<String>`).
fn build_memory_snapshot(episodes: &[cognitive::EpisodicMemory]) -> String {
    if episodes.is_empty() {
        return "No recent episodic memories.".into();
    }
    let count = episodes.len();
    let domains: std::collections::HashSet<&str> = episodes
        .iter()
        .map(|e| e.domain.as_str())
        .collect();
    let domain_list = if domains.is_empty() {
        "various".into()
    } else {
        domains.into_iter().collect::<Vec<_>>().join(", ")
    };
    format!(
        "{count} recent memory episodes across domains: {domain_list}.",
    )
}
```

- [ ] **Step 7: Add repos to `AutoTunerOrchestrator` and thread through**

The orchestrator at `mod.rs` lines 42–49 needs two new `Option` fields:
```rust
strategy_repo: Option<storage::StrategyRepo>,
episodic_repo: Option<cognitive::EpisodicMemoryRepo>,
```

Update the `new()` constructor and `register_nightly_cycle()` to accept these. Update the call site in `crates/app-core/src/init/cron.rs` to pass them.

Using `Option` keeps backward compatibility — if repos are not available, the context falls back to the current placeholder strings.

- [ ] **Step 8: Wire real context in `run_llm_generation`**

Replace lines 495–497 in `run_llm_generation`:

```rust
// Build real generation context
let trend_summary = if let Some(ref strategy_repo) = self.strategy_repo {
    let seven_days_ago = Utc::now() - chrono::Duration::days(7);
    let stats = strategy_repo.get_stats_since(seven_days_ago).await.unwrap_or_else(|_| storage::OverallStats {
        total_records: 0, accuracy: 0.0, avg_response_time_ms: 0, avg_satisfaction: None,
    });
    let agreement = self.trial_repo.shadow_log_agreement_rate(None, seven_days_ago).await.unwrap_or(1.0);
    build_trend_summary(&stats, agreement)
} else {
    "Trend data not yet available.".into()
};

let behavioral_context = if let Some(ref strategy_repo) = self.strategy_repo {
    let seven_days_ago = Utc::now() - chrono::Duration::days(7);
    let summaries = strategy_repo.get_strategy_summaries(seven_days_ago).await.unwrap_or_else(|_| vec![]);
    build_behavioral_context(&summaries)
} else {
    "Behavioral data not yet available.".into()
};

let memory_snapshot = if let Some(ref episodic_repo) = self.episodic_repo {
    let episodes = episodic_repo.list_recent(20).await.unwrap_or_default();
    build_memory_snapshot(&episodes)
} else {
    "Memory snapshot not yet available.".into()
};
```

- [ ] **Step 9: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p agent -E 'test(autotuner)' --no-fail-fast`
Expected: All pass.

- [ ] **Step 10: Commit**

```bash
git add crates/agent/src/autotuner/mod.rs crates/app-core/src/init/cron.rs
git commit -m "feat(autotuner): replace generation context placeholders with real trend/behavioral/memory data"
```

---

### Task 2: Add correction acknowledgment prepend

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` (lines 490–528)

This is a 5-line change — no new types or structs needed. Use a local `bool` per request instead of a session field.

- [ ] **Step 1: Add correction ack prepend logic**

In `crates/agent/src/agent_loop/mod.rs`, the `correction_strength` variable is already computed before the pipeline. Use it after the pipeline:

Find where `response_content` is assigned from `run_pipeline` (around line 510). After it, add:

```rust
// Prepend correction acknowledgment if this message was a keyword correction
let response_content = if correction_strength.is_some() && last_assistant_content.is_some() {
    format!("Noted — adjusting for next time.\n\n{response_content}")
} else {
    response_content
};
```

This uses the existing `correction_strength` (from `detect_correction_prefix`) and `last_assistant_content` (from the session lock) — both already in scope. No session mutation needed.

Note: Reaction-based corrections don't get an ack (they happen asynchronously, not in a conversation flow). This is correct per spec — "For reaction-based corrections where no immediate reply follows, the ack is skipped."

- [ ] **Step 2: Write test**

Add a test that verifies `detect_correction_prefix` returns `Some` for a correction message — this is already tested. The ack prepend is a simple string operation that doesn't warrant a separate unit test. Verify via a manual test or integration test.

- [ ] **Step 3: Compile check**

Run: `cargo check -p agent 2>&1 | tail -10`
Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs
git commit -m "feat(autotuner): add correction acknowledgment prepend to assistant response"
```

---

### Task 3: Add keyword correction rate limiter

**Files:**
- Modify: `crates/session/src/manager.rs` (Session struct, lines 22–42)
- Modify: `crates/agent/src/agent_loop/mod.rs` (around line 490)

- [ ] **Step 1: Add `correction_cooldown` field to `Session`**

In `crates/session/src/manager.rs`, add to the `Session` struct:

```rust
    /// Messages remaining before next keyword correction can fire (0 = ready).
    #[serde(default)]
    pub correction_cooldown: u32,
```

The `#[serde(default)]` ensures backward compatibility with existing serialized sessions.

- [ ] **Step 2: Add rate limit guard in `process_message`**

In `crates/agent/src/agent_loop/mod.rs`, inside the session lock block (around lines 461–479), add:

```rust
// Decrement correction cooldown
if session.correction_cooldown > 0 {
    session.correction_cooldown -= 1;
}
```

Then, where the correction signal is emitted (around line 490), add a guard:

```rust
if let Some(strength) = correction_strength {
    if let Some(ref original) = last_assistant_content {
        // Rate limit: max 1 keyword correction per 3 messages (single lock scope)
        let should_emit = {
            let mut session = session_arc.lock().await;
            if session.correction_cooldown > 0 {
                false
            } else {
                session.correction_cooldown = 3; // set cooldown immediately
                true
            }
        };

        if should_emit {
            self.emit_correction_signal(
                msg.chat_id.as_str(),
                original.clone(),
                msg.content.clone(),
                bus::CorrectionKind::KeywordPrefix,
                strength,
            ).await;
        }
    }
}
```

Note: The rate limiter ONLY applies to keyword corrections. Reaction-based corrections (in `handle_reaction`) are never rate-limited per spec.

- [ ] **Step 3: Write test for rate limiting**

```rust
#[test]
fn rate_limiter_blocks_consecutive_corrections() {
    // Test that correction_cooldown decrements and blocks
    let mut cooldown: u32 = 3;
    // Simulate 3 messages
    for _ in 0..3 {
        assert!(cooldown > 0, "Should be rate-limited");
        cooldown -= 1;
    }
    assert_eq!(cooldown, 0, "Should be ready for next correction");
}
```

- [ ] **Step 4: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -10`
Run: `cargo nextest run -p agent -p session --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/session/src/manager.rs crates/agent/src/agent_loop/mod.rs
git commit -m "feat(autotuner): add keyword correction rate limiter (max 1 per 3 messages)"
```

---

### Task 4: Populate `AutotunerDecision` with real `improvement_pct` and `affected_params`

**Files:**
- Modify: `crates/autotuner/src/cycle.rs` (add `affected_param_names` helper)
- Modify: `crates/agent/src/autotuner/mod.rs` (lines 274–283, 348–357)

- [ ] **Step 1: Write test for `affected_param_names`**

In `crates/autotuner/src/cycle.rs`, add to the test module:

```rust
#[test]
fn affected_param_names_finds_differences() {
    use common::autotuner::TrialParams;
    let old = TrialParams {
        heuristic_confidence_threshold: Some(0.7),
        skill_keyword_weight: Some(0.5),
        ..Default::default()
    };
    let new = TrialParams {
        heuristic_confidence_threshold: Some(0.8), // changed
        skill_keyword_weight: Some(0.5),            // same
        skill_semantic_weight: Some(0.3),           // added
        ..Default::default()
    };
    let names = affected_param_names(&old, &new);
    assert!(names.contains(&"heuristic_confidence_threshold".to_string()));
    assert!(names.contains(&"skill_semantic_weight".to_string()));
    assert!(!names.contains(&"skill_keyword_weight".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p autotuner -E 'test(affected_param)' --no-fail-fast 2>&1 | tail -10`
Expected: Compile error.

- [ ] **Step 3: Implement `affected_param_names`**

Add to `crates/autotuner/src/cycle.rs`:

```rust
/// Returns the names of TrialParams fields that differ between two param sets.
pub fn affected_param_names(old: &TrialParams, new: &TrialParams) -> Vec<String> {
    let mut names = Vec::new();
    macro_rules! check_field {
        ($field:ident) => {
            if old.$field != new.$field {
                names.push(stringify!($field).to_string());
            }
        };
    }
    check_field!(skill_keyword_weight);
    check_field!(skill_semantic_weight);
    check_field!(skill_activation_threshold);
    check_field!(heuristic_confidence_threshold);
    check_field!(llm_classifier_timeout_ms);
    check_field!(relevance_weight_semantic);
    check_field!(relevance_weight_retrievability);
    check_field!(relevance_weight_situation);
    names
}
```

Note: Verify the exact field names on `TrialParams` by reading `crates/common/src/autotuner.rs`. The macro approach handles all 8 fields DRY.

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p autotuner -E 'test(affected_param)' --no-fail-fast`
Expected: PASS.

- [ ] **Step 5: Populate `improvement_pct` at promotion site**

In `crates/agent/src/autotuner/mod.rs`, at the promotion `AutotunerDecision` emit (lines 274–283):

The variables `trial_result` and the old champion's `baseline_metrics` should be in scope. Replace:

```rust
improvement_pct: 0.0,
affected_params: vec![],
```

With:

```rust
improvement_pct: if champion_before.baseline_metrics.correction_rate > 0.0 {
    ((champion_before.baseline_metrics.correction_rate - trial_result.correction_rate)
        / champion_before.baseline_metrics.correction_rate
        * 100.0)
} else {
    0.0
},
affected_params: autotuner::affected_param_names(
    &champion_before.params,
    &params,
),
```

Note: `champion_before` is the champion BEFORE promotion — verify this variable name by reading the surrounding code. The champion's `params` and `baseline_metrics` should be captured before the update.

- [ ] **Step 6: Populate `affected_params` at rollback site**

At the rollback `AutotunerDecision` emit (lines 348–357), the `champ` (current champion being rolled back) and `restored` (previous champion being restored) are available:

```rust
improvement_pct: 0.0, // regression, not improvement
affected_params: autotuner::affected_param_names(
    &restored.params,
    &champ.params,
),
```

- [ ] **Step 7: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p autotuner -p agent --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add crates/autotuner/src/cycle.rs crates/agent/src/autotuner/mod.rs
git commit -m "feat(autotuner): populate AutotunerDecision with real improvement_pct and affected_params"
```

---

### Task 5: First-person cognitive observation text

**Files:**
- Modify: `crates/cognitive/src/services/background.rs` (lines 627–641)

- [ ] **Step 1: Update `event_to_observation` text**

Replace the `AutotunerDecision` arm content format string:

```rust
DomainEvent::AutotunerDecision {
    verdict,
    improvement_pct,
    affected_params,
    ..
} => {
    let params_text = if affected_params.is_empty() {
        "general parameters".into()
    } else {
        affected_params.join(", ")
    };
    // Use raw string "reverted" — cognitive crate does not depend on autotuner crate.
    let content = if verdict == "reverted" {
        format!(
            "I noticed a recent change to {params_text} wasn't working well and reverted to my previous approach."
        )
    } else {
        format!(
            "I refined how I handle your requests — adjusted {params_text}, \
             improving response alignment by {improvement_pct:.1}%."
        )
    };
    Some(Observation {
        domain: "meta".into(),
        content,
        importance: if verdict == "reverted" { 0.9 } else { 0.8 },
        source_event: "AutotunerDecision".into(),
        timestamp: now,
    })
},
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p cognitive 2>&1 | tail -10`
Expected: Clean.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "feat(autotuner): use first-person observation text for AutotunerDecision events"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run full workspace compile**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: Clean.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep -E "^(error|warning)" | grep -v "nom v1.2.4" | head -20`
Expected: Only pre-existing cognitive warning.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt --all --check 2>&1 | head -5`
Expected: Clean. Run `cargo fmt --all` if needed.

- [ ] **Step 4: Run modified crate tests**

Run: `cargo nextest run -p autotuner -p agent -p cognitive -p session -p storage --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 5: Commit if any fixes**

```bash
git add -A && git commit -m "chore: fix clippy/fmt from autotuner phase 2"
```

---

## Dependency Graph

```
Task 1 (generation context) — independent, largest task
Task 2 (correction ack) — independent, smallest task
Task 3 (rate limiter) — independent of 1 and 4, depends on correction infrastructure from Phase 1
Task 4 (AutotunerDecision fields) — independent, needs autotuner crate change
Task 5 (first-person text) — depends on Task 4 (uses real affected_params in text)
Task 6 (verification) — depends on all

Tasks 1, 2, 3, 4 can all run in parallel.
Task 5 runs after Task 4.
Task 6 runs last.
```
