# Layer 4: Autotuner Crate

> `crates/autotuner/` -- Self-optimizing experiment loop that shadow-scores routing parameter variants against live traffic, evaluates them via multi-metric constraints, and promotes winners using LLM-guided reasoning.

## Overview

The `autotuner` crate implements the core logic for Klyntbot's self-optimization system. It enables the agent to continuously improve how it routes and classifies messages by running controlled experiments on routing parameters (SkillRouter weights, IntentAnalyzer thresholds, cognitive retrieval relevance weights).

**Key design principle:** The autotuner crate is pure logic — evaluation, constraint checking, variant generation, and cycle orchestration. It defines traits (`ShadowClassifier`, `MetricSource`) that the `agent` crate implements with access to the live pipeline. This follows the project's dependency inversion pattern.

**Architecture:** `TrialParams` lives in `common` (L0) as a pure value object. `AutoTunerConfig` lives in `config` (L1). Storage via `TrialRepo` in `storage` (L2). The autotuner crate (L4) holds experiment logic. Thin orchestrator in `agent/autotuner/` (L5) wires to the runtime. Frontend components in `desktop-ui/src/features/autotuner/`.

## Dependencies

| Dependency | Purpose |
|---|---|
| `common` | `TrialParams`, `Result<T>`, `KlyntbotError` |
| `config` | `AutoTunerConfig` (constraint thresholds, schedule, pace) |
| `storage` | `TrialRepo` (trial/experiment persistence), `LearningStateRepo` (champion state) |
| `bus` | `DomainEventBus` (for metric collection) |
| `async-trait` | Async trait definitions |
| `chrono` | Date/time for trial timestamps |
| `uuid` | Trial and experiment identifiers |
| `serde`, `serde_json` | Serialization of params, results, events |
| `tracing` | Structured logging |

---

## Module Structure

```
autotuner/src/
  lib.rs            -- Pub exports
  trial.rs          -- Trial, TrialResult, TrialStatus, Experiment, Champion
  evaluator.rs      -- ConstraintEvaluator (multi-metric constraint check, diversity bonus)
  metrics.rs        -- MetricSnapshot, MetricAggregator (pure computation)
  generator.rs      -- VariantGenerator (LLM-guided variant generation)
  cycle.rs          -- NightlyCycle (evaluate → promote → generate → activate)
  events.rs         -- AutoTunerEvent enum (Report, Promotion, Rollback)
  traits.rs         -- ShadowClassifier, MetricSource traits
```

---

## Core Types

### Trial

Represents one experimental variant with a set of parameter overrides.

```rust
pub struct Trial {
    pub id: Uuid,
    pub experiment_id: Uuid,
    pub params: TrialParams,           // from common crate (L0)
    pub generation_reasoning: String,  // LLM's hypothesis for this variant
    pub status: TrialStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<TrialResult>,
}

pub enum TrialStatus {
    Pending,     // generated but not yet tested
    Active,      // currently being shadow-scored
    Completed,   // shadow period ended, metrics collected
    Promoted,    // became the new champion
    Reverted,    // auto-reverted after regression
}
```

### TrialResult

Aggregated metrics for a completed trial.

```rust
pub struct TrialResult {
    pub trial_id: Uuid,
    pub messages_scored: u32,
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
}
```

### Champion

The current best configuration, persisted in `LearningStateRepo`.

```rust
pub struct Champion {
    pub trial_id: Option<Uuid>,        // None = using Config defaults
    pub params: TrialParams,
    pub promoted_at: DateTime<Utc>,
    pub baseline_metrics: TrialResult,
    pub reason_for_promotion: String,
    pub impact_summary: String,
    pub consecutive_regression_days: u8,
}
```

Champion state is persisted via `LearningStateRepo` under keys `"autotuner_champion"` and `"autotuner_previous_champion"`.

### Experiment

Groups related trials generated in a single nightly cycle.

```rust
pub struct Experiment {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub hypothesis: String,
    pub trend_analysis: String,
    pub recommendation_for_next: String,
    pub trial_ids: Vec<Uuid>,
}
```

---

## Traits (defined here, implemented in `agent`)

### ShadowClassifier

Runs classification-only shadow scoring using Layer 1-2 of the IntentAnalyzer cascade (heuristics + embedding). Never invokes the Layer 3 LLM classifier for shadow paths.

```rust
#[async_trait]
pub trait ShadowClassifier: Send + Sync {
    async fn classify_shadow(
        &self,
        message: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> Result<ShadowPrediction>;
}
```

### MetricSource

Collects ground truth metrics from the live pipeline.

```rust
#[async_trait]
pub trait MetricSource: Send + Sync {
    async fn collect_metrics(
        &self,
        since: DateTime<Utc>,
        trial_id: Option<Uuid>,
    ) -> Result<MetricSnapshot>;
}
```

### Supporting Types

```rust
pub struct ShadowContext {
    pub chat_id: String,
    pub session_key: String,
}

pub struct ShadowPrediction {
    pub predicted_orchestrator: String,
    pub predicted_mode: String,       // "direct" or "reactive"
    pub confidence: f32,
    pub predicted_iteration_budget: u32,
    pub deferred_to_llm: bool,        // true if Layer 1-2 returned None
}

pub struct MetricSnapshot {
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
    pub total_messages: u32,
}
```

---

## ConstraintEvaluator

Checks whether a trial meets all promotion constraints relative to the baseline (Champion's metrics).

```rust
pub struct ConstraintEvaluator {
    min_correction_improvement: f64,       // default 5%
    max_token_cost_increase: f64,          // default 8%
    max_response_time_increase: f64,       // default 15%
    max_routing_stability_decrease: f64,   // default 10%
    max_memory_relevance_decrease: f64,    // default 5%
}
```

### Promotion Rules

| Metric | Rule | Rationale |
|---|---|---|
| Correction rate | Must improve by >= 5% | Primary optimization goal |
| Token cost | Must not increase by > 8% | Prevent brute-force Reactive mode |
| Response time | Must not increase by > 15% | Keep the agent snappy |
| Routing stability | Must not decrease by > 10% | Avoid erratic behavior |
| Memory relevance | Must not drop by > 5% | Protect Phase 2 foundation |

If multiple trials pass all constraints, the one with the best correction rate improvement wins, with a diversity bonus (preference for variants that differ most from the current Champion by parameter distance).

---

## NightlyCycle

Orchestrates the nightly experiment cycle. Registered as a `CronJob` (default: `0 2 * * *`).

```
1. EVALUATE  — Score yesterday's Active trials (aggregate shadow metrics)
      ↓
2. PROMOTE   — Check multi-metric constraints, crown winner (if any)
      ↓
3. GENERATE  — LLM suggests next 3 variants (conservative / moderate / bold)
      ↓
4. ACTIVATE  — New trials enter shadow scoring
      ↓
5. REPORT    — Emit AutoTunerEvent::Report for Transparency Panel
```

**Minimum sample size:** 50 messages scored before a trial is eligible for promotion.

**Three diversity tiers:** Conservative (near-champion tweak), moderate (shift 2-3 params), bold (large shift based on behavioral patterns). Controlled by the `experiment_pace` config field.

---

## Regression Detection & Auto-Rollback

Runs daily after evaluation:

1. Compare today's Champion performance against `baseline_metrics`.
2. If correction rate has worsened: increment `consecutive_regression_days`.
3. If `consecutive_regression_days >= 3`: revert to previous Champion, emit `AutoTunerEvent::Rollback`.
4. If correction rate is stable or improved: reset counter to 0.

---

## Events

Events emitted by the autotuner crate. The L5 orchestrator (`agent/autotuner/`) maps these to `AgentEvent` variants for the frontend Transparency Panel.

```rust
pub enum AutoTunerEvent {
    Report(AutoTunerReport),
    Promotion(AutoTunerPromotion),
    Rollback(AutoTunerRollback),
}
```

| Event | When | Content |
|---|---|---|
| `Report` | After nightly cycle completes | Champion summary, active experiment, completed trials, trend |
| `Promotion` | Trial promoted to Champion | Trial ID, reason, impact, params changed |
| `Rollback` | Auto-rollback triggered | Reverted trial ID, reason, reverted-to Champion |

---

## Shadow Execution Flow (per-message)

On every inbound message:

1. **Control path** — Pipeline runs with Champion params (or Config defaults). Drives the actual response.
2. **Shadow path** — For each Active trial, run Layer 1-2 only (Aho-Corasick heuristics + embedding cosine) of IntentAnalyzer + SkillRouter with trial's `TrialParams`. Log predicted routing decision, confidence, and orchestrator.
3. **Ground truth** — After response delivery, record user corrections, satisfaction, token usage, and response time against both control and shadow predictions.

**Cost:** Shadow scoring at Layer 1-2 only adds <1ms per trial per message. With 3 active trials, overhead is <3ms — near-zero.

---

## How Champion Params Reach the Live Pipeline

1. `AutoTunerOrchestrator` (in `agent/autotuner/`) maintains the in-memory `Champion`.
2. On every message, `AgentRuntime` calls `orchestrator.current_champion_params()`.
3. If `Some(params)`, attaches to `RoutingContext.champion_params: Option<TrialParams>`.
4. `IntentAnalyzer` reads overrides from `self.overrides` field (set at construction). `SkillRouter` reads optional weight params passed to `select_orchestrator_blended()`.
5. If no Champion exists (fresh start), everything falls through to Config defaults — zero behavior change.

---

## Integration Points

| System | Integration | Direction |
|---|---|---|
| `common` (L0) | `TrialParams` struct (pure value object) | Referenced by RoutingContext, autotuner, agent |
| `config` (L1) | `AutoTunerConfig` on `Config` struct | Schedule, constraints, pace |
| `RoutingContext` (L1) | `champion_params: Option<TrialParams>` field | Agent reads Champion params per-request |
| `storage` (L2) | `TrialRepo` (trials + experiments + shadow log tables) | Trial persistence |
| `storage` (L2) | `LearningStateRepo` | Champion state persistence |
| `skill-system` (L3) | `select_orchestrator_blended()` weight overrides | Shadow classifier calls with trial weights |
| `agent` (L5) | `IntentAnalyzer.overrides` field | Shadow classifier creates per-trial instances |
| `DomainEventBus` (L1) | `MetricCollector` subscribes to `UserCorrectedAI`, `CoachingFeedback` | Ground truth collection |
| `StrategyRepo` (L2) | Execution mode accuracy (Direct/Reactive) | Evaluation metrics |
| `CronService` (L3) | Nightly cycle registered as `CronJob` | Triggers experiment cycle |
| `AgentEvent` (L5) | Report, Promotion, Rollback variants | Transparency Panel |
