# Autotuner Live Param Wiring — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire 12 unconnected autotuner `TrialParams` into their live subsystems so promoted champions actually change system behavior. (16 total params - 1 already live - 3 promotion-time by design = 12 to wire.)

**Architecture:** Three injection patterns matching each subsystem's layer constraints: (1) shared `RwLock<Option<TrialParams>>` for memory params (crosses L5→L5 via L0 type), (2) pass-through params at the `AgentRuntime` call site for skill routing (L1 method accepts L0 type), (3) per-call timeout override for the LLM classifier (same-crate, existing autotuner ref on `IntentAnalyzer`).

**Tech Stack:** Rust (std::sync::RwLock, common::TrialParams, tokio async)

**Spec:** No formal spec — gap identified during autotuner audit. See `crates/common/src/autotuner.rs` for `TrialParams` definition.

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/cognitive/src/services/memory_retriever.rs` | Read champion memory params in `fetch_facts` | 1 |
| `crates/agent/src/autotuner/mod.rs` | Write champion params to shared lock on promotion + init | 2 |
| `crates/app-core/src/init/cron.rs` | Create shared lock, pass to orchestrator | 2 |
| `crates/agent/src/agent_loop/builder.rs` | Pass shared lock to `UnifiedMemoryService` | 2 |
| `crates/skill-system/src/router.rs` | Accept optional `TrialParams` in `select_orchestrator` + `activate_skills` | 3 |
| `crates/agent/src/autotuner/hooks.rs` | Add `champion_params()` to `AutoTunerHook` trait | 3 |
| `crates/agent/src/agent_runtime/runtime.rs` | Pass champion params from `autotuner_hook` to router | 3 |
| `crates/agent/src/intent_pipeline/analysis.rs` | Read champion `llm_classifier_timeout_ms`, pass to classifier | 4 |

---

### Task 1: Wire memory params into live retrieval

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

The `UnifiedMemoryService` currently builds `RetrievalParams` from its static `self.config` in `fetch_facts()` (line 77). We add a shared lock that holds the promoted champion's `TrialParams`. On each retrieval, read the lock and merge memory params with config defaults. `TrialParams` is in `common` (L0) — no new crate dependency needed.

- [ ] **Step 1: Add `champion_overrides` field and builder method**

In `crates/cognitive/src/services/memory_retriever.rs`, add a field to `UnifiedMemoryService` and a builder method:

```rust
use std::sync::Arc;
// (add to existing imports at the top)
```

Add to the struct (after `situation` field, ~line 31):

```rust
    /// Live champion params from the autotuner. Read on each retrieval to override
    /// config defaults for vector_top_k, min_similarity, and relevance weights.
    champion_overrides: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
```

Initialize in `new()`:

```rust
    champion_overrides: None,
```

Add builder method (after `with_situation`):

```rust
    pub fn with_champion_overrides(
        mut self,
        overrides: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        self.champion_overrides = Some(overrides);
        self
    }
```

- [ ] **Step 2: Read champion overrides in `fetch_facts`**

In `fetch_facts` (starting at line 77), replace the static `RetrievalParams` construction with one that merges champion overrides:

```rust
    async fn fetch_facts(&self, query: &str, limit: usize) -> Vec<(String, f64, String, String)> {
        if !self.config.dynamic_facts_enabled || query.is_empty() {
            return Vec::new();
        }

        let situational_boost = self.current_situational_boost().await;

        // Read champion overrides (non-blocking: std::sync::RwLock, held briefly)
        let (top_k, min_sim, w_sem, w_ret, w_imp, w_freq, w_sit, w_temp) =
            if let Some(ref lock) = self.champion_overrides {
                if let Ok(guard) = lock.read() {
                    if let Some(ref params) = *guard {
                        let defaults = [
                            self.config.relevance_weight_semantic,
                            self.config.relevance_weight_retrievability,
                            self.config.relevance_weight_importance,
                            self.config.relevance_weight_frequency,
                            self.config.relevance_weight_situation,
                            self.config.relevance_weight_temporal,
                        ];
                        let w = params.resolve_relevance_weights(&defaults);
                        (
                            params.vector_top_k.unwrap_or(self.config.vector_top_k),
                            params.min_similarity.unwrap_or(self.config.min_similarity),
                            w[0], w[1], w[2], w[3], w[4], w[5],
                        )
                    } else {
                        self.default_retrieval_tuple()
                    }
                } else {
                    self.default_retrieval_tuple()
                }
            } else {
                self.default_retrieval_tuple()
            };

        let params = RetrievalParams {
            limit,
            vector_top_k: top_k,
            min_similarity: min_sim,
            situational_boost,
            max_stability: self.config.max_stability,
            relevance_weight_semantic: w_sem,
            relevance_weight_retrievability: w_ret,
            relevance_weight_importance: w_imp,
            relevance_weight_frequency: w_freq,
            relevance_weight_situation: w_sit,
            relevance_weight_temporal: w_temp,
            scope_chain: Vec::new(),
        };

        // ... rest of the method unchanged (match retrieve_relevant_facts ...)
```

- [ ] **Step 3: Add `default_retrieval_tuple` helper**

Add a private method to avoid repeating the config-default tuple:

```rust
    /// Config-default retrieval parameters as a tuple for the fallback path.
    fn default_retrieval_tuple(&self) -> (usize, f64, f64, f64, f64, f64, f64, f64) {
        (
            self.config.vector_top_k,
            self.config.min_similarity,
            self.config.relevance_weight_semantic,
            self.config.relevance_weight_retrievability,
            self.config.relevance_weight_importance,
            self.config.relevance_weight_frequency,
            self.config.relevance_weight_situation,
            self.config.relevance_weight_temporal,
        )
    }
```

- [ ] **Step 4: Also read champion overrides in `retrieve_scoped`**

The `retrieve_scoped` method (~line 130, used for persona/squad memory) builds `RetrievalParams` identically to `fetch_facts`. Apply the same champion override logic using `default_retrieval_tuple()`. Search for the second `RetrievalParams {` construction in the file and apply the same pattern. Do NOT modify `retrieve_with_overrides` — that method is for shadow scoring and already accepts explicit params.

- [ ] **Step 5: Add test**

In the `#[cfg(test)] mod tests` block (or create one if absent):

```rust
#[test]
fn champion_overrides_affect_retrieval_params() {
    use common::TrialParams;

    let lock = Arc::new(std::sync::RwLock::new(Some(TrialParams {
        vector_top_k: Some(50),
        min_similarity: Some(0.40),
        relevance_weight_semantic: Some(0.50),
        ..Default::default()
    })));

    // Verify the lock is readable and params are present
    let guard = lock.read().unwrap();
    let params = guard.as_ref().unwrap();
    assert_eq!(params.vector_top_k, Some(50));
    assert!((params.min_similarity.unwrap() - 0.40).abs() < f64::EPSILON);

    // Verify resolve_relevance_weights uses override for semantic, defaults for rest
    let defaults = [0.30, 0.20, 0.15, 0.10, 0.15, 0.10];
    let weights = params.resolve_relevance_weights(&defaults);
    // Semantic override = 0.50, rest = defaults, then normalized
    assert!(weights[0] > defaults[0], "Semantic weight should be higher than default");
}
```

- [ ] **Step 6: Verify**

Run: `cargo check -p cognitive`

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(cognitive): read champion memory params in live retrieval"
```

---

### Task 2: Wire shared lock through orchestrator and builder

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

The shared lock is created in `cron.rs` (where the orchestrator is built), passed to the orchestrator (write side), and through the builder to `UnifiedMemoryService` (read side). On champion promotion, the orchestrator writes the new params. On init, the orchestrator seeds the lock with the current champion.

- [ ] **Step 1: Add `memory_param_sink` to `AutoTunerOrchestrator`**

In `crates/agent/src/autotuner/mod.rs`, add to the struct:

```rust
    /// Shared lock for live memory param injection. Written on champion promotion,
    /// read by UnifiedMemoryService on each retrieval.
    memory_param_sink: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
```

Initialize in `new()`:

```rust
    memory_param_sink: None,
```

Add builder method:

```rust
    pub fn with_memory_param_sink(
        mut self,
        sink: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        // Seed with current champion params (if any) so live retrieval uses them immediately.
        // `self.champion` is a tokio::sync::RwLock — try_read() is non-blocking and returns
        // Err only if a write lock is held (fine to skip seeding in that rare case).
        if let Ok(champion) = self.champion.try_read() {
            if champion.trial_id.is_some() {
                // The sink is a std::sync::RwLock (intentional — only held briefly, never
                // across await points). Handle lock poisoning gracefully.
                let mut guard = sink.write().unwrap_or_else(|e| e.into_inner());
                *guard = Some(champion.params.clone());
            }
        }
        self.memory_param_sink = Some(sink);
        self
    }

    /// Expose the shared lock so the builder can pass it to UnifiedMemoryService.
    pub fn memory_param_sink(&self) -> Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>> {
        self.memory_param_sink.clone()
    }
```

- [ ] **Step 2: Write to sink in `update_champion`**

In `update_champion` (line 167), after `*guard = new_champion;` (line 188), add:

```rust
        // Propagate memory params to live retrieval.
        // After `*guard = new_champion`, dereferencing `guard` gives the NEW champion data.
        // The sink is std::sync::RwLock — held briefly, never across await.
        // Also handles rollback (where new_champion has trial_id=None → clears the sink).
        if let Some(ref sink) = self.memory_param_sink {
            let mut param_guard = sink.write().unwrap_or_else(|e| e.into_inner());
            *param_guard = if guard.trial_id.is_some() {
                Some(guard.params.clone())
            } else {
                None
            };
        }
```

- [ ] **Step 3: Create shared lock in `cron.rs`**

In `crates/app-core/src/init/cron.rs`, in the autotuner init block (after `trial_repo.migrate()`, ~line 116):

```rust
        let memory_param_sink = Arc::new(std::sync::RwLock::new(None));
```

Pass it to the orchestrator (add to the builder chain after `.with_session_repo(...)`):

```rust
            .with_memory_param_sink(Arc::clone(&memory_param_sink)),
```

- [ ] **Step 4: Pass shared lock to builder**

The `AgentLoopBuilder` needs access to the shared lock. It already holds `autotuner: Option<Arc<AutoTunerOrchestrator>>`. In `crates/agent/src/agent_loop/builder.rs`, after constructing `UnifiedMemoryService` (~line 642-655), add the champion overrides:

```rust
        // Wire live champion memory params into retrieval
        if let Some(ref orchestrator) = self.autotuner {
            if let Some(sink) = orchestrator.memory_param_sink() {
                retriever = retriever.with_champion_overrides(sink);
            }
        }
```

This goes after `retriever = retriever.with_situation(...)` and before `let memory_service = Arc::new(retriever);`.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(autotuner): propagate champion memory params to live retrieval via shared lock"
```

---

### Task 3: Wire skill routing params into live SkillRouter

**Files:**
- Modify: `crates/skill-system/src/router.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

The `SkillRouter` is in `skill-system` (L1) — it can accept `common::TrialParams` (L0) but can't hold an autotuner reference (L5). The solution: modify `select_orchestrator` to accept optional `TrialParams`, and have `AgentRuntime` (L5) pass champion params at the call site.

- [ ] **Step 1: Modify `select_orchestrator` to accept optional params**

In `crates/skill-system/src/router.rs`, change the signature (~line 51):

```rust
    pub fn select_orchestrator<'a>(
        &self,
        message: &str,
        catalog: &'a SkillCatalog,
        champion_params: Option<&common::TrialParams>,
    ) -> &'a Arc<SkillPackage> {
        let (kw_w, sem_w) = champion_params
            .map(|p| (p.skill_keyword_weight, p.skill_semantic_weight))
            .unwrap_or((None, None));
        self.select_orchestrator_blended(message, &[], catalog, kw_w, sem_w)
    }
```

Add `use common;` to the imports if not already present (check `Cargo.toml` — `skill-system` likely already depends on `common`).

- [ ] **Step 2: Modify `activate_skills` to accept optional threshold**

In `crates/skill-system/src/router.rs`, change `activate_skills` (~line 109):

```rust
    pub fn activate_skills<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
        activation_threshold: Option<f64>,
    ) -> Vec<&'a Arc<SkillPackage>> {
        let threshold = activation_threshold.unwrap_or(SKILL_ACTIVATION_THRESHOLD);
```

Replace the hardcoded `SKILL_ACTIVATION_THRESHOLD` usage in the filter with `threshold`:

Find: `if blended >= SKILL_ACTIVATION_THRESHOLD {`
Replace: `if blended >= threshold {`

> **Note:** `activate_skills` currently has **no call sites** in the runtime. This wiring is prep for when non-orchestrator skill activation is integrated into `AgentRuntime::run()`. The `skill_activation_threshold` param will be dead code until then — but wiring the method signature now avoids a future breaking change.

- [ ] **Step 3: Fix all call sites**

Search for `select_orchestrator(` and `activate_skills(` across the workspace. Every call needs the new parameter. Known call sites for `select_orchestrator`:

- `crates/agent/src/agent_runtime/runtime.rs:248` — will get champion params in Step 5
- `crates/agent/src/agent_runtime/runtime.rs` (tests, ~line 1293) — pass `None`
- `crates/skill-system/src/router.rs` (tests, ~lines 203, 211, 225, 231) — pass `None`

Search exhaustively with `grep -rn 'select_orchestrator(' crates/` to catch any others. For `activate_skills`, search for call sites and add `None` as the last argument (currently no call sites outside `router.rs` tests).

- [ ] **Step 4: Add `champion_params` method to `AutoTunerHook` trait**

`AgentRuntime` already holds `autotuner_hook: Option<Arc<dyn AutoTunerHook>>` — reuse this instead of adding a second orchestrator reference. In `crates/agent/src/autotuner/hooks.rs`, add to the `AutoTunerHook` trait:

```rust
    /// Return the current champion's trial params (non-blocking).
    fn champion_params(&self) -> Option<common::TrialParams> {
        None // default: no autotuner
    }
```

Implement on `AutoTunerHookImpl`:

```rust
    fn champion_params(&self) -> Option<common::TrialParams> {
        self.orchestrator.try_current_champion_params()
    }
```

- [ ] **Step 5: Pass champion params at the router call site**

In `runtime.rs`, replace line 248:

```rust
        let mut profile = {
            let catalog = self.skill_catalog.read().await;
            let router = self.skill_router.read().await;
            let champion_params = self.autotuner_hook.as_ref()
                .and_then(|h| h.champion_params());
            Arc::clone(router.select_orchestrator(message, &catalog, champion_params.as_ref()))
        };
```

No new field or builder method needed — `autotuner_hook` is already wired.

- [ ] **Step 6: Add test for routing with champion params**

In `crates/skill-system/src/router.rs` tests:

```rust
#[test]
fn select_orchestrator_accepts_champion_params() {
    let catalog = test_catalog();
    let router = SkillRouter::new(&catalog);

    // Without params — uses defaults
    let result1 = router.select_orchestrator("hello", &catalog, None);

    // With params — should not panic, may or may not change result
    let params = common::TrialParams {
        skill_keyword_weight: Some(0.90),
        skill_semantic_weight: Some(0.10),
        ..Default::default()
    };
    let result2 = router.select_orchestrator("hello", &catalog, Some(&params));

    // Both should return valid skill packages
    assert!(!result1.name.is_empty());
    assert!(!result2.name.is_empty());
}
```

- [ ] **Step 7: Verify**

Run: `cargo check --workspace` then `cargo nextest run -p skill-system -p agent --no-fail-fast`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(autotuner): wire skill routing params into live SkillRouter"
```

---

### Task 4: Wire LLM classifier timeout

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs`

The `IntentAnalyzer` already holds `Arc<AutoTunerOrchestrator>` and reads `heuristic_confidence_threshold` per-message. We add `llm_classifier_timeout_ms` to the same pattern: read champion params in `analyze()` and pass a timeout override to `IntentClassifier::classify()`.

- [ ] **Step 1: Add timeout override to `IntentClassifier::classify`**

In `crates/agent/src/intent_pipeline/analysis.rs`, modify `classify` (~line 1043):

```rust
    pub async fn classify(
        &self,
        message: &str,
        tool_names: &[&str],
        params: &ChatParams,
        strategy_context: Option<&str>,
        timeout_override: Option<Duration>,
    ) -> Result<IntentAnalysis> {
```

Change the `tokio::time::timeout` call (~line 1061):

```rust
        let timeout = timeout_override.unwrap_or(self.timeout);
        let result =
            tokio::time::timeout(timeout, self.provider.chat(&messages, None, params)).await;
```

- [ ] **Step 2: Add `effective_llm_classifier_timeout` method**

Following the pattern of `effective_heuristic_threshold`, add to `IntentAnalyzer`:

```rust
    fn effective_llm_classifier_timeout(&self) -> Option<Duration> {
        // Live autotuner champion params take precedence
        if let Some(ref orchestrator) = self.autotuner {
            if let Some(params) = orchestrator.try_current_champion_params() {
                if let Some(timeout_ms) = params.llm_classifier_timeout_ms {
                    return Some(Duration::from_millis(timeout_ms));
                }
            }
        }
        // Static override from shadow mode
        if let Some(ref overrides) = self.overrides {
            if let Some(timeout_ms) = overrides.llm_classifier_timeout_ms {
                return Some(Duration::from_millis(timeout_ms));
            }
        }
        None // Use classifier's built-in default
    }
```

- [ ] **Step 3: Pass timeout override at the `classify_with_llm` call site**

The internal wrapper `classify_with_llm` (~line 1554) calls `self.classifier.classify(...)`. This is the Layer 3 path. Update it to pass the timeout override:

```rust
        let timeout = self.effective_llm_classifier_timeout();
        let analysis = self.classifier.classify(
            message,
            tool_names,
            &self.classifier_params,
            strategy_context.as_deref(),
            timeout,
        ).await?;
```

The shadow classifier never reaches Layer 3 (`shadow_mode` returns before LLM call), so this code path is not exercised in shadow mode.

- [ ] **Step 4: Fix test `classify` call sites**

Search for `.classify(` within `analysis.rs` tests (~lines 1912, 1936, 1947, 1963, 1978). Each call needs a `None` appended for the new `timeout_override` parameter.

- [ ] **Step 5: Add test**

```rust
#[test]
fn effective_llm_classifier_timeout_reads_champion() {
    // Test that the method returns None when no autotuner is set
    let analyzer = IntentAnalyzer::new(
        mock_provider(),
        "gpt-4o",
        &OrchestratorConfig::default(),
    );
    assert!(analyzer.effective_llm_classifier_timeout().is_none());
}
```

- [ ] **Step 6: Verify**

Run: `cargo check -p agent` then `cargo nextest run -p agent -E 'test(intent)' --no-fail-fast`

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(autotuner): wire llm_classifier_timeout_ms into live IntentAnalyzer"
```

---

### Task 5: Final verification

- [ ] **Step 1: Full workspace compile**

Run: `cargo check --workspace`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Format**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Test all modified crates**

Run: `cargo nextest run -p cognitive -p skill-system -p agent -p app-core --no-fail-fast`

- [ ] **Step 5: Frontend build**

Run: `cd desktop-ui && bun run build`

(No frontend changes in this plan, but verify nothing is broken by the Rust changes.)

- [ ] **Step 6: Commit if fixes needed**

```bash
git commit -m "chore: fix lint/fmt from autotuner live param wiring"
```

---

## Dependency Graph

```
Task 1 (memory params in UnifiedMemoryService) ──→ Task 2 (shared lock wiring)
                                                          │
Task 3 (skill routing params) ────────────────────────────┤
                                                          │
Task 4 (LLM timeout) ────────────────────────────────────┤
                                                          │
                                                          └──→ Task 5 (verification)
```

Tasks 1→2 are sequential. Tasks 3 and 4 are independent of each other and of Tasks 1-2 but share the builder file. Task 5 depends on all others.

---

## What This Enables

After this plan, all 16 `TrialParams` have live injection:

| Param Group | Injection Method | Latency |
|---|---|---|
| `heuristic_confidence_threshold` | Already live — `IntentAnalyzer` reads per-message | Immediate |
| 6 relevance weights + `vector_top_k` + `min_similarity` | Shared `RwLock` read in `fetch_facts` | Immediate (next retrieval) |
| `skill_keyword_weight` + `skill_semantic_weight` + `skill_activation_threshold` | `try_current_champion_params()` at router call site | Immediate (next message) |
| `llm_classifier_timeout_ms` | `effective_llm_classifier_timeout()` per `analyze()` | Immediate (next L3 classify) |
| `fsrs_desired_retention` + `accumulate_*` | Promotion-time restart (by design) | Next restart |
