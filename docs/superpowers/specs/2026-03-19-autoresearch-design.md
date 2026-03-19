# Autoresearch: Self-Optimizing Agent via LLM-Guided Experimentation

**Date:** 2026-03-19
**Status:** Design approved, pending implementation plan
**Inspired by:** Karpathy's autoresearch — automated experimentation loops for hyperparameter/prompt optimization

## Overview

Klyntbot becomes a self-optimizing personal AI that continuously improves how it understands and serves its user. An automated experiment loop generates parameter variants, shadow-scores them on live traffic, evaluates results against multi-metric constraints, and promotes winners — all guided by an LLM that reasons about *why* certain configurations work for this specific user.

This is not generic hyperparameter tuning. It is **personal autoresearch** — the AI reflecting on its own thinking patterns to better serve one person.

## Why Klyntbot Can Outperform Traditional Autoresearch

Karpathy's autoresearch runs offline on ML models using validation loss as the metric. Klyntbot has structural advantages:

- **Real user feedback loop** — `UserCorrectedAI`, `CoachingFeedback`, thumbs up/down provide signal far more meaningful than val loss.
- **DomainEventBus** + **StrategyRepo** + **LearningStateRepo** already collect evaluation data.
- **Cognitive memory + FSRS** have decay, salience filtering, and accumulation — effectively the "state" for autoresearch.
- **SkillRouter** + **IntentAnalyzer** + **ContextEngine** are high-leverage knobs where small changes cascade to every downstream system.
- **CronService** + **BackgroundConsolidationService** allow experiment loops to run asynchronously.

## Phased Rollout

### Phase 1: Routing Optimization (this spec)

Optimize the control surface of the entire agent: SkillRouter weights, IntentAnalyzer thresholds, ContextEngine budget allocation.

**Why routing first:** Routing is upstream of memory. If the SkillRouter picks the wrong orchestrator, or IntentAnalyzer misclassifies complexity, or the token budget is poorly allocated, even perfectly tuned FSRS retrieval will surface the wrong memories. Optimizing routing first creates a multiplier effect for everything downstream.

### Phase 2: Memory Optimization (future spec, after Phase 1 stabilizes)

Optimize FSRS weights, 6-factor relevance scoring, salience thresholds, promote/min_days. Reuses the same experiment infrastructure built in Phase 1. Begins after Phase 1 Champion has stabilized (no promotions in 3 consecutive cycles, minimum 2 weeks).

---

## Core Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Experiment mode | Shadow scoring on live traffic | Captures real distribution (not stale history). Control path always drives real response — zero user-visible risk. Bootstrapped with 24-48h historical replay for initial candidates. |
| Variant generation | LLM-guided search | The AI reasons about *why* configurations work for this user. Discovers non-obvious correlations. More expensive per iteration but higher quality than grid/random/Bayesian. |
| Runtime config | Per-request `TrialParams` on `RoutingContext` | No global mutable state. Experiment boundary is explicit per-request. Natural fit for shadow scoring (control path uses Config, experimental path uses TrialParams). |
| Safety guardrails | Multi-metric constraints + 3-day auto-rollback | Variants promoted only if they improve the primary metric without regressing secondary metrics. Automatic revert after 3 consecutive regression days. Emergency revert button in UI. |
| Architecture | `autotuner` crate at L4, thin orchestrator in `agent` at L5 | Follows existing dependency inversion pattern. Pure logic (evaluation, generation, constraints) is unit-testable without agent runtime. |

---

## Architecture

### Crate: `autotuner` (L4)

**Dependencies:** `common`, `config`, `storage`, `providers`

```
crates/autotuner/
  src/
    lib.rs                — pub exports
    config.rs             — AutoTunerConfig (bounds, constraints, schedule)
    trial.rs              — Trial, TrialParams, TrialResult, TrialStatus, Experiment
    champion.rs           — Champion, promotion logic, rollback state
    repo.rs               — TrialRepo (autotuner_trials + autotuner_experiments tables)
    evaluator.rs          — ConstraintEvaluator (multi-metric check, diversity bonus)
    generator.rs          — VariantGenerator (LLM-guided, takes Arc<dyn LlmProvider>)
    cycle.rs              — NightlyCycle (orchestrates evaluate → promote → generate → activate)
    metrics.rs            — MetricSnapshot, MetricAggregator (pure computation, no I/O)
    migration.rs          — FeatureMigration for autotuner tables
```

### Module: `agent/autotuner/` (L5 thin glue)

```
crates/agent/src/
  autotuner/
    mod.rs                — AutoTunerOrchestrator (wires everything)
    shadow_classifier.rs  — impl ShadowClassifier for IntentAnalyzer + SkillRouter
    metric_collector.rs   — impl MetricSource using StrategyRepo + DomainEventBus
    hooks.rs              — AutoTunerHook trait, on_message_received/completed
```

### Dependency flow

```
L4: autotuner (storage, config, common, providers)
L5: agent/autotuner/ (autotuner, agent internals, bus)
```

---

## Data Model

### TrialParams — per-request parameter override

```rust
/// Attached to RoutingContext for shadow scoring.
/// Each field is Option — None means "use Config default."
pub struct TrialParams {
    pub trial_id: Uuid,

    // Phase 1: SkillRouter knobs
    pub skill_keyword_weight: Option<f64>,           // default 0.7, bounds [0.30, 0.90]
    pub skill_semantic_weight: Option<f64>,           // default 0.3, bounds [0.10, 0.70]
    pub skill_activation_threshold: Option<f64>,      // default 0.4, bounds [0.20, 0.70]

    // Phase 1: IntentAnalyzer knobs
    pub heuristic_confidence_threshold: Option<f64>,  // default 0.85, bounds [0.60, 0.95]
    pub llm_classifier_timeout_ms: Option<u64>,       // default 2000, bounds [500, 5000]

    // Phase 1: ContextEngine knobs
    pub memory_retrieval_weight: Option<f64>,         // default 0.20, bounds [0.05, 0.50]
    pub semantic_weight: Option<f64>,                 // default 0.30, bounds [0.10, 0.60]
    pub situation_weight: Option<f64>,                // default 0.25, bounds [0.05, 0.50]
}
```

**Constraint:** `skill_keyword_weight + skill_semantic_weight = 1.0`. All 6 relevance weights must sum to 1.0.

### Trial — one experiment variant

```rust
pub struct Trial {
    pub id: Uuid,
    pub experiment_id: Uuid,
    pub params: TrialParams,
    pub generation_reasoning: String,   // LLM's hypothesis for this variant
    pub status: TrialStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum TrialStatus {
    Pending,     // generated but not yet tested
    Active,      // currently being shadow-scored
    Completed,   // shadow period ended, metrics collected
    Promoted,    // became the new champion
    Reverted,    // auto-reverted after regression
}
```

### TrialResult — metrics for a completed trial

```rust
pub struct TrialResult {
    pub trial_id: Uuid,
    pub messages_scored: u32,
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,        // % agreement with champion routing
    pub memory_relevance: f64,         // % of retrieved memories used in response
    pub user_satisfaction: Option<f64>,
}
```

### Experiment — groups related trials

```rust
pub struct Experiment {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub hypothesis: String,            // high-level theme from LLM
    pub trend_analysis: String,        // LLM's analysis of recent trends
    pub recommendation_for_next: String,
    pub trial_ids: Vec<Uuid>,
}
```

### Champion — current best configuration

```rust
pub struct Champion {
    pub trial_id: Option<Uuid>,        // None = using Config defaults
    pub params: TrialParams,
    pub promoted_at: DateTime<Utc>,
    pub baseline_metrics: TrialResult,
    pub reason_for_promotion: String,  // LLM justification
    pub impact_summary: String,        // e.g., "-15% corrections, -4% tokens"
    pub consecutive_regression_days: u8,
}
```

Champion state is persisted via `LearningStateRepo` (keys: `"autotuner_champion"`, `"autotuner_previous_champion"`).

### Storage — `autotuner_trials` table

Single table with JSON-serialized params and results. `TrialRepo` provides typed access. `autotuner_experiments` table for experiment grouping.

---

## Traits (defined in `autotuner`, implemented in `agent`)

```rust
/// Runs classification-only shadow scoring with trial parameters.
#[async_trait]
pub trait ShadowClassifier: Send + Sync {
    async fn classify_shadow(
        &self,
        message: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> Result<ShadowPrediction>;
}

/// Collects ground truth metrics from the live pipeline.
#[async_trait]
pub trait MetricSource: Send + Sync {
    async fn collect_metrics(
        &self,
        since: DateTime<Utc>,
        trial_id: Option<Uuid>,
    ) -> Result<MetricSnapshot>;
}
```

```rust
pub struct ShadowContext {
    pub chat_id: ChatId,
    pub session_key: SessionKey,
    pub recent_history: Vec<SessionMessage>,
}

pub struct ShadowPrediction {
    pub predicted_orchestrator: String,
    pub predicted_mode: ExecutionMode,
    pub confidence: f32,
    pub predicted_iteration_budget: u32,
}
```

---

## Experiment Lifecycle

### Phase 0: Bootstrap (first 24-48 hours)

1. **Collect baseline** — Run Config defaults for 24h, recording all metrics. This becomes the Champion's `baseline_metrics`.
2. **Historical replay** — Take the last 7 days of sessions. Re-run intent pipeline with 5-10 random parameter perturbations (within bounds). Score against ground truth from `StrategyRepo`.
3. **Seed generation** — Feed replay results to LLM generator to produce 3 promising initial variants.
4. **Transition** — Seeded variants become the first `Active` trials for live shadow scoring.

### Nightly Experiment Cycle (steady state)

Runs as a `CronJob` (default: `0 2 * * *` — 2am daily).

```
1. EVALUATE  — Score yesterday's Active trials (aggregate shadow metrics)
      ↓
2. PROMOTE   — Check multi-metric constraints, crown winner (if any)
      ↓
3. GENERATE  — LLM suggests next 3 variants (conservative / moderate / bold)
      ↓
4. ACTIVATE  — New trials enter shadow scoring
      ↓
5. REPORT    — Emit AgentEvent::AutoTunerReport for Transparency Panel
```

**Step 1 — EVALUATE:**
- Aggregate shadow metrics for each `Active` trial from the day's messages.
- Require minimum 50 messages scored before a trial is eligible for promotion (prevents noisy decisions from small samples).
- Compute `TrialResult`, mark trial as `Completed`.

**Step 2 — PROMOTE (multi-metric constraint check):**

| Metric | Promotion Rule | Rationale |
|---|---|---|
| Correction rate | Must improve by >= 5% | Primary goal |
| Token cost | Must not increase by > 8% | Prevent brute-force Reactive mode |
| Response time | Must not increase by > 15% | Keep the agent snappy |
| Routing stability | Must not decrease by > 10% | Avoid erratic behavior |
| Memory relevance | Must not drop by > 5% | Protect Phase 2 foundation |

- If multiple trials pass all constraints: prefer the one with the best correction rate improvement, with a diversity bonus (small preference for variants that differ most from current Champion by parameter distance).
- LLM generates `reason_for_promotion` and `impact_summary` for the winner.
- If no trial passes: Champion stays. The LLM is told why each trial failed, informing the next generation.

**Step 3 — GENERATE (LLM-guided):**
- Prompt includes: Champion params + metrics, last 5 completed trials + results, 7-day trend summary, user behavior patterns, user memory snapshot (from cognitive layer), parameter bounds, constraint rules.
- Three diversity tiers enforced: one conservative tweak, one moderate exploration, one bold hypothesis.
- Instruction to avoid repeating patterns from last 5 trials.
- Output includes `recommendation_for_next_cycle` for continuity between cycles.
- Grouped under a new `Experiment`.

**Step 4 — ACTIVATE:** Set 3 new trials to `Active`.

**Step 5 — REPORT:** Emit `AgentEvent::AutoTunerReport` with champion summary, active experiment, completed trials, and trend.

### Shadow Execution (per-message, real-time)

On every inbound message:

1. **Control path** — Pipeline runs with Champion params (or Config defaults). This drives the actual response.
2. **Shadow path** — For each `Active` trial, run **only the classification stage** (IntentAnalyzer + SkillRouter) with `TrialParams`. Log the predicted routing decision, confidence, and selected orchestrator. No full execution — classification-only is near-zero cost.
3. **Ground truth** — After response delivery, record user corrections, satisfaction feedback, token usage, and response time against both control and shadow predictions.

### Regression Detection & Auto-Rollback

Runs daily after evaluation:

1. Compare today's Champion performance against `baseline_metrics`.
2. If correction rate has worsened: increment `consecutive_regression_days`.
3. If `consecutive_regression_days >= 3`:
   - Revert to previous Champion (from `"autotuner_previous_champion"` in `LearningStateRepo`).
   - Mark current Champion's trial as `Reverted`.
   - Emit `AgentEvent::AutoTunerRollback`.
   - Reset regression counter.
4. If correction rate is stable or improved: reset counter to 0.

---

## How Champion Params Reach the Live Pipeline

When a trial is promoted:

1. `AutoTunerOrchestrator` updates its in-memory `Champion`.
2. On every message, `AgentRuntime` calls `orchestrator.current_champion_params()`.
3. If `Some(params)`, attaches to `RoutingContext.champion_params`.
4. `IntentAnalyzer` and `SkillRouter` check `ctx.champion_params` before falling back to `Config`.

No Config mutation. No global mutable state. If no Champion exists (fresh start), everything falls through to Config defaults — zero behavior change.

---

## LLM Generation Prompt

The prompt has five sections: role, context (champion + trials + trends + behavior + memory), constraints, parameter bounds, and task.

Key design principles:

- **User-specific reasoning:** The prompt includes a "User Behavior Patterns" section (e.g., "68% of corrections happened on research queries routed to the general orchestrator") and a "User Memory Snapshot" from the cognitive layer. This forces the LLM to reason about the *person*, not just the numbers.
- **Three diversity tiers:** Conservative (near-champion tweak), moderate (shift 2-3 params), bold (large shift based on behavioral patterns). Prevents clustering around local optima.
- **Chain of reasoning:** Output includes `recommendation_for_next_cycle`, fed back into the next night's prompt. Creates continuity across experiment cycles.
- **Constraint-aware:** The LLM must explain how each suggestion satisfies the promotion constraints, reducing wasted trials.

Output format is JSON with `variants[]` (each with hypothesis, params, constraint_reasoning, confidence, confidence_reasoning), `trend_analysis`, and `recommendation_for_next_cycle`.

Full prompt template is in the implementation plan.

---

## Transparency Panel UI

Three-layer progressive disclosure:

### Layer 1: Ambient Indicator (chat header)

Small, unobtrusive status visible only when a recent promotion occurred:

```
Getting to know you better +12% this week
```

Clicking opens the full panel. Hidden when no experiments are active.

### Layer 2: Summary Card (panel top)

```
AI Self-Improvement

Current config: Trial #42 (promoted 2 days ago)
"This change helps me understand your research style better,
so I route complex questions to the right expert faster."

Impact: -15% corrections, -4% tokens, +2% memory relevance

Testing now: Experiment #48 (3 variants)
47 messages scored so far today

[Revert to defaults]          [Pause experiments]
```

### Layer 3: Experiment History (scrollable timeline)

Each entry shows:
- Status icon: active, promoted, failed, reverted
- The LLM's hypothesis
- For completed: outcome + impact summary
- For reverted: why it failed + what the system learned

Entries grouped by weekly themes (e.g., "Week of Mar 10-17: Better at research routing").

### Micro-Confirmation on Promotion

For the first 3 promotions after enabling, show a non-blocking toast:
"I just improved how I understand you (Trial #42). Want to see what changed?"
One-tap "Show me" opens the panel.

### Data Contract — AgentEvent Variants

```rust
AutoTunerReport {
    champion: ChampionSummary,
    active_experiment: Option<ExperimentSummary>,
    completed_trials: Vec<TrialSummary>,
    trend: String,
}

AutoTunerPromotion {
    trial_id: Uuid,
    reason: String,
    impact: String,
    params_changed: Vec<ParamChange>,
}

AutoTunerRollback {
    reverted_trial_id: Uuid,
    reason: String,
    reverted_to: ChampionSummary,
}
```

### Tauri Commands

```rust
autotuner_status(state) -> AutoTunerStatus
autotuner_history(state, limit) -> Vec<ExperimentSummary>
autotuner_revert(state) -> ChampionSummary
autotuner_pause(state) -> ()
autotuner_resume(state) -> ()
```

### Frontend Components

```
desktop-ui/src/features/autotuner/
  components/
    AutoTunerPanel.tsx
    ChampionCard.tsx
    ExperimentTimeline.tsx
    AmbientIndicator.tsx
  hooks/
    useAutoTunerStatus.ts
    useAutoTunerHistory.ts
  types.ts
```

Uses existing `useQuery`/`useMutation` + `ipc()` pattern. Glass-panel styling.

---

## Phase 2 Sketch: Memory Optimization

Reuses 90% of Phase 1 infrastructure. Begins after Phase 1 Champion stabilizes (no promotions in 3 consecutive cycles, minimum 2 weeks).

### New TrialParams (Phase 2 additions)

```rust
pub fsrs_desired_retention: Option<f64>,          // default 0.9
pub accumulate_promote_threshold: Option<usize>,  // default 5
pub accumulate_min_days: Option<usize>,           // default 3
pub vector_top_k: Option<usize>,                  // default 30
pub min_similarity: Option<f64>,                  // default 0.55
pub relevance_weight_importance: Option<f64>,     // default 0.15
pub relevance_weight_frequency: Option<f64>,      // default 0.10
pub relevance_weight_temporal: Option<f64>,       // default 0.05
```

### New Trait: ShadowRetriever

```rust
#[async_trait]
pub trait ShadowRetriever: Send + Sync {
    async fn retrieve_shadow(
        &self,
        query: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> Result<Vec<RetrievedMemory>>;
}
```

### Phase 2 Metrics

| Metric | Measurement |
|---|---|
| Retrieval precision | % of retrieved memories appearing in final response |
| Retrieval recall | Inverse of "I already told you" corrections |
| Memory freshness | Average age of retrieved memories |
| Promotion accuracy | % of accumulated events correctly promoted to extraction |

### Phase 2 Constraints

| Metric | Rule |
|---|---|
| Retrieval precision | Must not drop by > 5% |
| Retrieval recall | Must improve by >= 5% |
| Correction rate | Must not increase by > 3% (protect Phase 1 gains) |

---

## Integration Points Summary

| System | Integration | Direction |
|---|---|---|
| `RoutingContext` | Add `champion_params: Option<TrialParams>` | Agent reads Champion params |
| `IntentAnalyzer` | `analyze()` accepts optional `TrialParams` | Shadow classifier calls with trial params |
| `SkillRouter` | `select_orchestrator_blended()` accepts optional weight overrides | Shadow classifier calls with trial weights |
| `DomainEventBus` | `MetricCollector` subscribes to `UserCorrectedAI`, `CoachingFeedback`, `TaskExecutionCompleted` | Ground truth collection |
| `StrategyRepo` | `MetricSource` reads classification accuracy history | Evaluation metrics |
| `CronService` | Nightly cycle registered as `CronJob` at startup | Triggers experiment cycle |
| `LearningStateRepo` | Stores Champion + previous Champion | Persistence across restarts |
| `AgentEvent` | New variants: `AutoTunerReport`, `AutoTunerPromotion`, `AutoTunerRollback` | Transparency Panel |

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Experiments degrade live UX | Shadow scoring only — control path always drives real response. Champion promoted only after passing all constraints. |
| Convergence on degenerate config | Multi-metric constraints prevent single-metric optimization. Diversity bonus in promotion. Bold tier in generation. |
| Regression after promotion | Automatic 3-day rollback + emergency revert button. |
| LLM cost of nightly generation | Uses cheap classifier model. One prompt per night (~1k tokens). Negligible vs daily usage. |
| Shadow scoring compute cost | Classification-only shadow (no full execution). Near-zero overhead per message. |
| Small sample noise | Minimum 50 messages before promotion eligibility. |
| Local optima | Three diversity tiers (conservative/moderate/bold). Explicit instruction to avoid repeating recent patterns. |

---

## Non-Goals

- **Multi-user A/B testing** — This is single-user personal optimization, not population-level experimentation.
- **Real-time experiment switching** — Experiments change nightly, not per-message.
- **Prompt evolution** — Phase 1-2 optimize numerical parameters only. Prompt text optimization is a future phase.
- **Structured observability** — Consistent with project non-goals. Transparency Panel is the observability layer.
