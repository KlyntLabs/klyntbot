# Reforge Phase 6 — Autotuner Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire autotuner evaluation into Reforge Phase 6 and merge trial generation into the Phase 3 Review LLM call, with 5 guardrails against LLM-generated trial risks.

**Architecture:** The orchestrator owns evaluation math and champion state; Reforge Phase 3 generates trial suggestions alongside skill edits (no 4th LLM call). Phase 6 runs evaluation, handles promotion/rollback, then validates and creates trials from Phase 3's suggestions.

**Tech Stack:** Rust, autotuner crate (NightlyCycle, ConstraintEvaluator, TrialRepo), agent crate (AutoTunerOrchestrator), common crate (TrialParams)

---

## File Structure

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/src/services/reforge/types.rs` | Add TrialSuggestion, TrialHistoryEntry, MetricsSnapshot, AutotunerContext; extend ReviewOutput + ReforgeCollected |
| `crates/cognitive/src/services/reforge/service.rs` | Implement Phase 6 (evaluate + create trials); add guardrail functions |
| `crates/cognitive/src/services/reforge/collector.rs` | Load autotuner context (champion, trial history, metrics) |
| `crates/agent/src/adapters/reforge_handlers.rs` | Extend Review prompt with autotuner context; add trial_suggestions to output |
| `crates/app-core/src/init/cron.rs` | Pass orchestrator + autotuner deps into Reforge handler |

---

### Task 1: Extend types for autotuner integration

**Files:**
- Modify: `crates/cognitive/src/services/reforge/types.rs`

- [ ] **Step 1: Add autotuner types**

Add these types to `types.rs`:

```rust
// ── Autotuner types (Phase 6) ─────────────────────────────────

/// A trial suggestion from the Review LLM call.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrialSuggestion {
    pub hypothesis: String,
    #[serde(default = "default_pace")]
    pub pace: String,
    #[serde(default)]
    pub param_overrides: std::collections::HashMap<String, f64>,
}

fn default_pace() -> String {
    "balanced".to_string()
}

/// Summary of a past trial outcome for experiment history context.
#[derive(Debug, Clone, Serialize)]
pub struct TrialOutcome {
    pub params_summary: String,
    pub result: String,
    pub constraint_failures: Vec<String>,
    pub improvement: Option<f64>,
}

/// Summary of a past experiment for LLM context.
#[derive(Debug, Clone, Serialize)]
pub struct TrialHistoryEntry {
    pub experiment_id: String,
    pub days_ago: u32,
    pub trials: Vec<TrialOutcome>,
}

/// Snapshot of key performance metrics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct MetricsSnapshot {
    pub correction_rate: f64,
    pub retrieval_precision: f64,
    pub avg_response_time_ms: f64,
    pub avg_tokens_per_message: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
}

/// Autotuner context collected in Phase 1 for Phase 3 + Phase 6.
#[derive(Debug, Clone, Default)]
pub struct AutotunerContext {
    pub champion_summary: String,
    pub trial_history: Vec<TrialHistoryEntry>,
    pub metrics_24h: MetricsSnapshot,
    pub metrics_7d: MetricsSnapshot,
    pub active_trial_count: u32,
}
```

- [ ] **Step 2: Extend ReforgeCollected**

Add field to `ReforgeCollected`:

```rust
pub autotuner_ctx: Option<AutotunerContext>,
```

- [ ] **Step 3: Extend ReviewOutput**

Add field to `ReviewOutput`:

```rust
#[serde(default)]
pub trial_suggestions: Vec<TrialSuggestion>,
```

- [ ] **Step 4: Extend ReviewInput**

Add field to `ReviewInput`:

```rust
pub autotuner_context: Option<String>,
```

- [ ] **Step 5: Extend ReforgeResult**

Add fields to `ReforgeResult`:

```rust
pub trials_created: u32,
pub champion_promoted: bool,
pub regression_detected: bool,
```

- [ ] **Step 6: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles (warnings about unused fields OK).

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add autotuner types to Reforge"
```

---

### Task 2: Extend collector to load autotuner context

**Files:**
- Modify: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: Add autotuner context loading function**

Read the current `collector.rs` to understand the existing function signature and pattern. Then add a new public function:

```rust
use super::types::{AutotunerContext, MetricsSnapshot, TrialHistoryEntry, TrialOutcome};

/// Load autotuner context for Phase 3 prompt and Phase 6 evaluation.
/// Returns None if orchestrator is not available.
pub async fn load_autotuner_context(
    trial_repo: &autotuner::TrialRepo,
    metric_source: &dyn autotuner::MetricSource,
    champion: &common::TrialParams,
) -> AutotunerContext {
    let now = chrono::Utc::now();
    let since_24h = now - chrono::Duration::hours(24);
    let since_7d = now - chrono::Duration::days(7);

    // Current metrics
    let metrics_24h = metric_source
        .collect_metrics(since_24h, None)
        .await
        .map(|s| MetricsSnapshot {
            correction_rate: s.correction_rate,
            retrieval_precision: s.retrieval_precision,
            avg_response_time_ms: s.avg_response_time_ms,
            avg_tokens_per_message: s.avg_tokens_per_message,
            routing_stability: s.routing_stability,
            memory_relevance: s.memory_relevance,
        })
        .unwrap_or_default();

    let metrics_7d = metric_source
        .collect_metrics(since_7d, None)
        .await
        .map(|s| MetricsSnapshot {
            correction_rate: s.correction_rate,
            retrieval_precision: s.retrieval_precision,
            avg_response_time_ms: s.avg_response_time_ms,
            avg_tokens_per_message: s.avg_tokens_per_message,
            routing_stability: s.routing_stability,
            memory_relevance: s.memory_relevance,
        })
        .unwrap_or_default();

    // Active trial count
    let active_trial_count = trial_repo
        .count_active()
        .await
        .unwrap_or(0) as u32;

    // Recent experiment history (last 5 completed trials)
    let trial_history = load_trial_history(trial_repo).await;

    // Format champion params
    let champion_summary = format_champion_params(champion);

    AutotunerContext {
        champion_summary,
        trial_history,
        metrics_24h,
        metrics_7d,
        active_trial_count,
    }
}
```

The `format_champion_params` and `load_trial_history` are helper functions. The implementer should read `TrialRepo` to find methods like `list_completed(limit)` or `list_recent()` to load trial history. Format the champion params grouped by category (routing, retrieval, memory, etc.). Use the param names from the TrialParams struct.

If `TrialRepo` doesn't have a `count_active()` method, add one (simple `SELECT COUNT(*) FROM trials WHERE status = 'active'`).

- [ ] **Step 2: Wire into collect() output**

In the existing `collect()` function, after loading all other data, set `autotuner_ctx: None`. The actual context loading happens in the cron handler (which has access to the orchestrator) and gets passed separately.

- [ ] **Step 3: Verify**

Run: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/ crates/autotuner/
git commit -m "feat(cognitive): add autotuner context loading to Reforge collector"
```

---

### Task 3: Extend Review prompt with autotuner context

**Files:**
- Modify: `crates/agent/src/adapters/reforge_handlers.rs`

- [ ] **Step 1: Read the current Review prompt and format_review_input**

Read `reforge_handlers.rs` fully to understand the current REVIEW_PROMPT constant and `format_review_input()` function.

- [ ] **Step 2: Extend REVIEW_PROMPT**

Add to the REVIEW_PROMPT constant (before the JSON schema):

```
If autotuner context is provided, also suggest up to 3 parameter experiments.
Each experiment should have a hypothesis, a pace (conservative/balanced/bold), and param_overrides (only params you want to change — the rest inherit from champion).
Build on past experiment results: don't repeat configurations that failed. Focus on parameters related to the issues you identified.
Only suggest experiments when you have evidence (routing problems, retrieval issues, correction patterns). If everything is working well, return an empty trial_suggestions array.
```

Add `trial_suggestions` to the JSON response schema in the prompt:

```json
"trial_suggestions": [{"hypothesis":"...","pace":"conservative"|"balanced"|"bold","param_overrides":{"param_name":value,...}}]
```

- [ ] **Step 3: Extend format_review_input**

Add a section to `format_review_input()` that appends autotuner context when present:

```rust
if let Some(ref ctx) = input.autotuner_context {
    out.push_str(&format!("\n## Autotuner Context\n{ctx}\n"));
}
```

- [ ] **Step 4: Build autotuner context string in service.rs**

In the service's `run_review()` method (which builds `ReviewInput`), format the autotuner context as a string for the prompt. Read `ReforgeCollected.autotuner_ctx` and format:

```
### Current Champion Parameters
[grouped params]

### Performance Metrics
Metrics (last 24h): correction_rate=X, retrieval_precision=Y, ...
Metrics (7-day avg): correction_rate=X, retrieval_precision=Y, ...

### Recent Experiment History
[formatted history entries]

### Active Trials: N/6 (cap)
```

- [ ] **Step 5: Verify**

Run: `cargo build -p agent`

- [ ] **Step 6: Commit**

```bash
git add crates/agent/ crates/cognitive/
git commit -m "feat(agent): extend Review prompt with autotuner context and trial suggestions"
```

---

### Task 4: Implement Phase 6 guardrails

**Files:**
- Modify: `crates/cognitive/src/services/reforge/service.rs`

- [ ] **Step 1: Add param validation function**

Read `crates/autotuner/src/generator.rs` to find the param range definitions. Then implement:

```rust
/// Validate and clamp trial param overrides against defined ranges.
/// Returns None if more than half the overrides are invalid.
fn validate_param_overrides(
    overrides: &HashMap<String, f64>,
) -> Option<HashMap<String, f64>> {
    // Define param ranges (from generator.rs)
    let ranges: HashMap<&str, (f64, f64)> = HashMap::from([
        ("skill_keyword_weight", (0.0, 1.0)),
        ("skill_semantic_weight", (0.0, 1.0)),
        ("skill_activation_threshold", (0.40, 0.95)),
        ("heuristic_confidence_threshold", (0.50, 0.95)),
        ("relevance_weight_semantic", (0.10, 0.60)),
        ("relevance_weight_retrievability", (0.05, 0.40)),
        ("relevance_weight_situation", (0.05, 0.40)),
        ("relevance_weight_importance", (0.05, 0.40)),
        ("relevance_weight_frequency", (0.02, 0.30)),
        ("relevance_weight_temporal", (0.01, 0.20)),
        ("relevance_weight_hierarchy", (0.0, 0.25)),
        ("relevance_weight_path_coherence", (0.0, 0.20)),
        ("relevance_weight_community", (0.0, 0.30)),
        ("relevance_weight_cross_note", (0.0, 0.20)),
        ("fsrs_desired_retention", (0.70, 0.99)),
        ("vector_top_k", (10.0, 100.0)),
        ("min_similarity", (0.30, 0.80)),
        ("rewrite_confidence_threshold", (0.30, 0.95)),
    ]);

    let mut validated = HashMap::new();
    let mut invalid_count = 0;

    for (key, &value) in overrides {
        if let Some(&(min, max)) = ranges.get(key.as_str()) {
            let clamped = value.clamp(min, max);
            if (clamped - value).abs() > f64::EPSILON {
                tracing::warn!("Reforge: clamped param {key} from {value} to {clamped}");
                invalid_count += 1;
            }
            validated.insert(key.clone(), clamped);
        } else {
            tracing::warn!("Reforge: unknown param {key}, skipping");
            invalid_count += 1;
        }
    }

    if !overrides.is_empty() && invalid_count * 2 > overrides.len() {
        tracing::warn!("Reforge: rejecting trial — >50% params invalid ({invalid_count}/{})", overrides.len());
        return None;
    }

    Some(validated)
}
```

- [ ] **Step 2: Add diversity gate function**

```rust
/// Check that trials are sufficiently diverse from each other and from champion.
/// Returns true if diversity is sufficient.
fn check_diversity(
    suggestions: &[&HashMap<String, f64>],
    champion_params: &HashMap<String, f64>,
) -> bool {
    let param_keys: Vec<&str> = vec![
        "skill_keyword_weight", "skill_semantic_weight", "skill_activation_threshold",
        "relevance_weight_semantic", "relevance_weight_retrievability",
        "relevance_weight_situation", "fsrs_desired_retention",
        "vector_top_k", "min_similarity",
    ];

    let to_vec = |params: &HashMap<String, f64>| -> Vec<f64> {
        param_keys.iter().map(|k| *params.get(*k).unwrap_or(&0.0)).collect()
    };

    let euclidean = |a: &[f64], b: &[f64]| -> f64 {
        a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt()
    };

    let champion_vec = to_vec(champion_params);
    let max_distance = (param_keys.len() as f64).sqrt(); // theoretical max

    // Check each pair of suggestions
    let vecs: Vec<Vec<f64>> = suggestions.iter().map(|s| to_vec(s)).collect();
    for i in 0..vecs.len() {
        for j in (i + 1)..vecs.len() {
            let dist = euclidean(&vecs[i], &vecs[j]) / max_distance;
            if dist < 0.05 {
                tracing::warn!("Reforge: trial pair {i}/{j} too similar (distance {dist:.3})");
                return false;
            }
        }
    }

    // Check all against champion
    let all_close = vecs.iter().all(|v| {
        euclidean(v, &champion_vec) / max_distance < 0.10
    });
    if all_close {
        tracing::warn!("Reforge: all trials too close to champion");
        return false;
    }

    true
}
```

- [ ] **Step 3: Write tests for guardrails**

```rust
#[cfg(test)]
mod guardrail_tests {
    use super::*;

    #[test]
    fn test_validate_valid_params() {
        let overrides = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.35),
            ("min_similarity".to_string(), 0.60),
        ]);
        let result = validate_param_overrides(&overrides);
        assert!(result.is_some());
        let v = result.unwrap();
        assert!((v["relevance_weight_semantic"] - 0.35).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_clamps_out_of_range() {
        let overrides = HashMap::from([
            ("relevance_weight_semantic".to_string(), 0.90), // max is 0.60
        ]);
        let result = validate_param_overrides(&overrides).unwrap();
        assert!((result["relevance_weight_semantic"] - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_rejects_mostly_invalid() {
        let overrides = HashMap::from([
            ("unknown_param_1".to_string(), 0.5),
            ("unknown_param_2".to_string(), 0.5),
            ("relevance_weight_semantic".to_string(), 0.35),
        ]);
        // 2/3 invalid → rejected
        assert!(validate_param_overrides(&overrides).is_none());
    }

    #[test]
    fn test_diversity_gate_passes_diverse() {
        let a = HashMap::from([("relevance_weight_semantic".to_string(), 0.20)]);
        let b = HashMap::from([("relevance_weight_semantic".to_string(), 0.50)]);
        let champion = HashMap::from([("relevance_weight_semantic".to_string(), 0.30)]);
        assert!(check_diversity(&[&a, &b], &champion));
    }

    #[test]
    fn test_diversity_gate_rejects_identical() {
        let a = HashMap::from([("relevance_weight_semantic".to_string(), 0.35)]);
        let b = HashMap::from([("relevance_weight_semantic".to_string(), 0.35)]);
        let champion = HashMap::from([("relevance_weight_semantic".to_string(), 0.30)]);
        assert!(!check_diversity(&[&a, &b], &champion));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(guardrail) | test(validate_param) | test(diversity)'`
Expected: 5 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add Phase 6 guardrails — param validation + diversity gate"
```

---

### Task 5: Implement Phase 6 in ReforgeService

**Files:**
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`

- [ ] **Step 1: Define the AutotunerBridge trait**

In `crates/cognitive/src/services/reforge/mod.rs`, add a trait that abstracts autotuner operations. This keeps the cognitive crate independent of the autotuner crate:

```rust
/// Bridge trait for autotuner operations. Implemented in the agent crate
/// to avoid circular dependencies.
#[async_trait]
pub trait AutotunerBridge: Send + Sync {
    /// Run trial evaluation and promotion. Returns a summary for logging.
    async fn run_evaluation(&self) -> common::Result<Phase6Result>;

    /// Create new pending trials from validated param overrides.
    async fn create_trials(&self, suggestions: Vec<ValidatedTrial>) -> common::Result<u32>;

    /// Get the current champion params as a flat key-value map.
    fn champion_params_map(&self) -> HashMap<String, f64>;

    /// Get the count of currently active trials.
    async fn active_trial_count(&self) -> u32;
}

/// Result of Phase 6 evaluation.
#[derive(Debug, Clone, Default)]
pub struct Phase6Result {
    pub promoted: bool,
    pub promotion_summary: Option<String>,
    pub regression: bool,
    pub evaluated_count: usize,
    pub failed_constraints: Vec<String>,
}

/// A validated trial suggestion ready for creation.
#[derive(Debug, Clone)]
pub struct ValidatedTrial {
    pub hypothesis: String,
    pub pace: String,
    pub params: HashMap<String, f64>,
}
```

- [ ] **Step 2: Implement Phase 6 in the run_reforge function**

Replace the Phase 6 placeholder in `service.rs` with:

```rust
// Phase 6: Optimize
info!("Reforge Phase 6: Autotuner evaluation + trial creation");
if let Some(ref bridge) = autotuner_bridge {
    // Step 1: Evaluate existing trials
    match bridge.run_evaluation().await {
        Ok(eval) => {
            result.champion_promoted = eval.promoted;
            result.regression_detected = eval.regression;
            if let Some(ref summary) = eval.promotion_summary {
                info!("Reforge Phase 6: {summary}");
            }
            if eval.regression {
                warn!("Reforge Phase 6: champion regression detected");
            }
        }
        Err(e) => {
            warn!("Reforge Phase 6 evaluation failed: {e}");
            result.phase_errors.push(format!("Optimize/evaluate: {e}"));
        }
    }

    // Step 2: Create new trials from Phase 3 suggestions
    if let Some(ref review) = review_output {
        let created = create_trials_from_suggestions(
            &review.trial_suggestions,
            bridge.as_ref(),
        )
        .await;
        result.trials_created = created;
    }
} else {
    debug!("Reforge Phase 6: skipped (no autotuner bridge)");
}
```

- [ ] **Step 3: Implement create_trials_from_suggestions**

```rust
/// Validate, deduplicate, and create trials from LLM suggestions.
async fn create_trials_from_suggestions(
    suggestions: &[TrialSuggestion],
    bridge: &dyn AutotunerBridge,
) -> u32 {
    if suggestions.is_empty() {
        return 0;
    }

    // Guardrail: active trial cap (max 6)
    let active = bridge.active_trial_count().await;
    if active >= 6 {
        info!("Reforge Phase 6: skipping trial creation — {active} active trials (cap: 6)");
        return 0;
    }

    let champion_map = bridge.champion_params_map();

    // Validate each suggestion's params
    let mut validated: Vec<ValidatedTrial> = Vec::new();
    for suggestion in suggestions {
        if let Some(params) = validate_param_overrides(&suggestion.param_overrides) {
            validated.push(ValidatedTrial {
                hypothesis: suggestion.hypothesis.clone(),
                pace: suggestion.pace.clone(),
                params,
            });
        }
    }

    if validated.is_empty() {
        warn!("Reforge Phase 6: all trial suggestions rejected by param validation");
        return 0;
    }

    // Diversity gate
    let param_refs: Vec<&HashMap<String, f64>> = validated.iter().map(|v| &v.params).collect();
    if !check_diversity(&param_refs, &champion_map) {
        warn!("Reforge Phase 6: trial suggestions rejected by diversity gate");
        return 0;
    }

    // Create trials via bridge
    match bridge.create_trials(validated).await {
        Ok(count) => {
            info!("Reforge Phase 6: created {count} new trial(s)");
            count
        }
        Err(e) => {
            warn!("Reforge Phase 6: trial creation failed: {e}");
            0
        }
    }
}
```

- [ ] **Step 4: Add autotuner_bridge parameter to run_reforge**

Read the current `run_reforge` function signature and add `autotuner_bridge: Option<&dyn AutotunerBridge>`. Update all callers (cron handler, integration test) to pass `None` for now.

- [ ] **Step 5: Verify**

Run: `cargo build -p cognitive`
Run: `cargo nextest run -E 'test(reforge)'`

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): implement Reforge Phase 6 — autotuner evaluation + trial creation"
```

---

### Task 6: Implement AutotunerBridge in agent crate

**Files:**
- Create: `crates/agent/src/adapters/autotuner_bridge.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Read orchestrator and NightlyCycle APIs**

Read `crates/agent/src/autotuner/mod.rs` and `crates/autotuner/src/cycle.rs` to understand:
- `AutoTunerOrchestrator` methods: `run_evaluation()`, `update_champion()`, `try_current_champion_params()`
- `NightlyCycle::new()` and `run_evaluation_and_promotion()`
- `TrialRepo` methods for creating trials

- [ ] **Step 2: Implement the bridge**

Create `crates/agent/src/adapters/autotuner_bridge.rs`:

```rust
//! AutotunerBridge implementation that delegates to AutoTunerOrchestrator.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cognitive::services::reforge::{AutotunerBridge, Phase6Result, ValidatedTrial};

use crate::autotuner::AutoTunerOrchestrator;

pub struct AgentAutotunerBridge {
    orchestrator: Arc<AutoTunerOrchestrator>,
    nightly_cycle: autotuner::NightlyCycle,
}

impl AgentAutotunerBridge {
    pub fn new(
        orchestrator: Arc<AutoTunerOrchestrator>,
        nightly_cycle: autotuner::NightlyCycle,
    ) -> Self {
        Self {
            orchestrator,
            nightly_cycle,
        }
    }
}

#[async_trait]
impl AutotunerBridge for AgentAutotunerBridge {
    async fn run_evaluation(&self) -> common::Result<Phase6Result> {
        let champion = self.orchestrator.current_champion().await;
        let cycle_result = self.nightly_cycle.run_evaluation_and_promotion(&champion).await?;

        let mut result = Phase6Result::default();
        result.evaluated_count = cycle_result.evaluated_trials.len();

        // Handle promotion
        if let Some((trial_id, trial_result, trial_params)) = cycle_result.promotion {
            // Build new champion and update orchestrator
            let new_champion = autotuner::trial::Champion {
                trial_id: Some(trial_id),
                params: trial_params,
                promoted_at: chrono::Utc::now(),
                baseline_metrics: trial_result.clone(),
                reason_for_promotion: format!(
                    "correction_rate improved to {:.2}%",
                    trial_result.correction_rate * 100.0
                ),
                impact_summary: String::new(),
                consecutive_regression_days: 0,
            };
            self.orchestrator.update_champion(new_champion).await;
            result.promoted = true;
            result.promotion_summary = Some(format!(
                "Trial {trial_id} promoted — correction_rate: {:.1}%",
                trial_result.correction_rate * 100.0
            ));
        }

        result.regression = cycle_result.regression;
        result.failed_constraints = cycle_result
            .failed_constraints
            .iter()
            .map(|(id, failures)| format!("{id}: {}", failures.join(", ")))
            .collect();

        Ok(result)
    }

    async fn create_trials(&self, suggestions: Vec<ValidatedTrial>) -> common::Result<u32> {
        let mut count = 0u32;
        let champion_params = self.orchestrator.try_current_champion_params();

        for suggestion in &suggestions {
            // Merge overrides with champion defaults
            let mut params = champion_params.clone().unwrap_or_default();
            // Apply overrides — implementer must map HashMap<String, f64> onto TrialParams fields
            apply_overrides_to_params(&mut params, &suggestion.params);

            // Create trial via orchestrator's trial_repo
            // The exact method depends on TrialRepo API — may be insert_pending or create_trial
            // Read TrialRepo to find the right method
            match self.orchestrator.create_pending_trial(
                params,
                &suggestion.hypothesis,
                &suggestion.pace,
            ).await {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!("Failed to create trial: {e}");
                }
            }
        }

        Ok(count)
    }

    fn champion_params_map(&self) -> HashMap<String, f64> {
        let params = self.orchestrator.try_current_champion_params();
        match params {
            Some(p) => params_to_map(&p),
            None => HashMap::new(),
        }
    }

    async fn active_trial_count(&self) -> u32 {
        self.orchestrator.active_trial_count().await.unwrap_or(0)
    }
}

/// Convert TrialParams struct to a flat HashMap for guardrail comparison.
fn params_to_map(params: &common::TrialParams) -> HashMap<String, f64> {
    // Read TrialParams struct and map each field to its string key.
    // The implementer should read the struct definition and list all fields.
    let mut map = HashMap::new();
    // Example: map.insert("relevance_weight_semantic".to_string(), params.relevance_weight_semantic.unwrap_or(0.30));
    // ... for all 27 params
    map
}

/// Apply HashMap overrides onto a TrialParams struct.
fn apply_overrides_to_params(params: &mut common::TrialParams, overrides: &HashMap<String, f64>) {
    // The implementer should map each string key to the corresponding TrialParams field.
    // Example: if let Some(&v) = overrides.get("relevance_weight_semantic") { params.relevance_weight_semantic = Some(v); }
    // ... for all supported params
}
```

Note: The `params_to_map` and `apply_overrides_to_params` functions need to map between string keys and TrialParams struct fields. The implementer should read `common::TrialParams` and implement the mapping for all 27 params. This is mechanical but must be complete.

- [ ] **Step 3: Export module**

In `crates/agent/src/adapters/mod.rs`, add:
```rust
pub mod autotuner_bridge;
```

- [ ] **Step 4: Verify**

Run: `cargo build -p agent`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): implement AutotunerBridge for Reforge Phase 6"
```

---

### Task 7: Wire AutotunerBridge into Reforge cron handler

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Read current Reforge cron handler**

Read the `JOB_REFORGE_NIGHTLY` handler in `cron.rs` to understand what's captured in the closure and how `run_reforge` is called.

- [ ] **Step 2: Capture orchestrator in the closure**

The `Arc<AutoTunerOrchestrator>` is already constructed during app init (search for `orchestrator` in the file). Clone it into the Reforge closure:

```rust
let orchestrator_for_reforge = orchestrator.clone(); // Arc<AutoTunerOrchestrator>
```

- [ ] **Step 3: Construct AutotunerBridge inside the handler**

Inside the `JOB_REFORGE_NIGHTLY` handler async block:

```rust
// Build autotuner bridge
let autotuner_bridge: Option<Box<dyn cognitive::services::reforge::AutotunerBridge>> =
    if let Some(ref orch) = orchestrator_for_reforge {
        let trial_repo = autotuner::TrialRepo::new(pool.clone());
        let metric_source: Arc<dyn autotuner::MetricSource> = Arc::new(
            agent::autotuner::metric_collector::AgentMetricCollector::new(/* repos */),
        );
        let nightly_cycle = autotuner::NightlyCycle::new(
            config.autotuner.clone(),
            trial_repo,
            metric_source,
        );
        Some(Box::new(
            agent::adapters::autotuner_bridge::AgentAutotunerBridge::new(
                Arc::clone(orch),
                nightly_cycle,
            ),
        ))
    } else {
        None
    };
```

The implementer should read the commented-out autotuner code to see exactly how `trial_repo`, `metric_source`, and `AgentMetricCollector` were constructed previously.

- [ ] **Step 4: Pass bridge to run_reforge**

```rust
let bridge_ref = autotuner_bridge.as_deref();
// Pass to run_reforge as the new autotuner_bridge parameter
```

- [ ] **Step 5: Also load autotuner context for Phase 3**

Before calling `run_reforge`, load the autotuner context:

```rust
let autotuner_ctx = if let Some(ref orch) = orchestrator_for_reforge {
    let champion_params = orch.try_current_champion_params().unwrap_or_default();
    Some(cognitive::services::reforge::collector::load_autotuner_context(
        &trial_repo, metric_source.as_ref(), &champion_params,
    ).await)
} else {
    None
};
```

Pass this to `run_reforge` or set it on the collected data.

- [ ] **Step 6: Verify full workspace builds**

Run: `cargo build --workspace`

- [ ] **Step 7: Run all tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/
git commit -m "feat(app-core): wire AutotunerBridge into Reforge cron handler"
```

---

### Task 8: Integration test — Phase 6 with mock bridge

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Create MockAutotunerBridge**

```rust
struct MockAutotunerBridge {
    promoted: bool,
}

impl MockAutotunerBridge {
    fn new(promoted: bool) -> Self {
        Self { promoted }
    }
}

#[async_trait::async_trait]
impl cognitive::services::reforge::AutotunerBridge for MockAutotunerBridge {
    async fn run_evaluation(&self) -> common::Result<cognitive::services::reforge::Phase6Result> {
        Ok(cognitive::services::reforge::Phase6Result {
            promoted: self.promoted,
            promotion_summary: if self.promoted {
                Some("Mock trial promoted".into())
            } else {
                None
            },
            regression: false,
            evaluated_count: 2,
            failed_constraints: vec![],
        })
    }

    async fn create_trials(
        &self,
        suggestions: Vec<cognitive::services::reforge::ValidatedTrial>,
    ) -> common::Result<u32> {
        Ok(suggestions.len() as u32)
    }

    fn champion_params_map(&self) -> std::collections::HashMap<String, f64> {
        let mut map = std::collections::HashMap::new();
        map.insert("relevance_weight_semantic".to_string(), 0.30);
        map.insert("min_similarity".to_string(), 0.55);
        map
    }

    async fn active_trial_count(&self) -> u32 {
        2
    }
}
```

- [ ] **Step 2: Write test**

```rust
#[tokio::test]
async fn test_reforge_phase6_with_autotuner_bridge() {
    // Setup: same as existing reforge test but pass MockAutotunerBridge
    // ...
    let bridge = MockAutotunerBridge::new(true);
    // Call run_reforge with Some(&bridge)
    // Assert: result.champion_promoted == true
    // Assert: result.trials_created >= 0
    // Assert: phase_errors is empty
}
```

- [ ] **Step 3: Run test**

Run: `cargo nextest run -E 'test(reforge_phase6)'`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: add Reforge Phase 6 integration test with mock autotuner bridge"
```

---

## Summary of Tasks

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | Extend types | 1 | compile check |
| 2 | Collector loads autotuner context | 1-2 | compile check |
| 3 | Extend Review prompt | 2 | compile check |
| 4 | Phase 6 guardrails | 1 | 5 unit tests |
| 5 | Phase 6 implementation | 2 | build + existing tests |
| 6 | AutotunerBridge impl | 2 | compile check |
| 7 | Cron wiring | 1 | workspace build + tests |
| 8 | Integration test | 1 | 1 integration test |

---

### Known Gap: Stale Trial Expiry

The spec mentions auto-expiring trials older than 7 days with < 20 messages. This should be added to the `AutotunerBridge::run_evaluation()` implementation in Task 6 — before evaluating, deactivate stale trials. The implementer should check `TrialRepo` for an `expire_stale(max_age_days, min_messages)` method or add one.

---

**Total: ~10 files modified, 5 unit tests, 1 integration test, 8 commits**
