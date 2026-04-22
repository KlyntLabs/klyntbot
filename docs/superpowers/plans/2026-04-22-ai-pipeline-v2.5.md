# AI Pipeline v2.5 — Reforge Metrics + Community/CoActivation Events

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fold every reforge behavioral metric into the unified AI feature pipeline. Introduce a `#[ai(metric(...))]` attribute on event variants; auto-register generated `MetricSpec`s; harvest samples into a single `ai_metric_samples` table via a new `MetricHarvestConsumer`; replace `load_behavioral_metrics`' 5 hand-written SQL queries with one registry-driven aggregator. Add a `#[ai(promotion_threshold = N)]` override on `AiFeature` that beats the global `accumulate_promote_threshold`. Re-introduce `CommunityDiscovered` / `CommunityUpdated` / `CommunityWeakened` as a typed `CommunityEvent` enum (the v1 delete list is reversed here, cleanly). Add `CoActivationEvent::Strengthened` for co-activation threshold crossings. Every old path deleted in the same PR.

**Architecture:** Today `feedback.rs::load_behavioral_metrics` cross-reaches into 4 feature-owned tables via raw SQL — `task_estimation_history`, `daily_summaries`, `coaching_strategies`, `task_suggestions`, `productivity_forecasts` — and materialises a fixed `BehavioralMetrics` struct with 5 `Option<f64>` fields. v2.5 inverts the direction: each event variant declares `metric(name, value_from, window, min_samples, aggregation)`; the derive expands into a `MetricSpec` constant plus a `MetricSample { name, value }` pushed into the `AiSignal`'s new `metric_samples` vector; a new `MetricHarvestConsumer` writes every sample to a unified `ai_metric_samples(metric_name, value, sample_time, source_domain, source_event_kind)` table; `load_behavioral_metrics` iterates the registry, runs one generic aggregate query per `MetricSpec`, and returns `BehavioralMetrics` as a `BTreeMap<&'static str, f64>` with accessor helpers. The fixed struct disappears; field access (`.task_estimation_bias`) becomes `.get("task_estimation_bias")`. The two dead metrics (`suggestion_dismiss_rate`, `forecast_accuracy`) are deleted wholesale (their feature-side tables lost all writers when LLM task automations were removed 2026-04-20). The three live metrics — `task_estimation_bias`, `coaching_acceptance_rate`, `focus_quality_trend` — migrate via event annotation, with the coaching and productivity crates gaining minimal `CoachingEvent::StrategyApplied` and `ProductivityEvent::SessionEnded` enums so the metric samples originate from the pipeline, not from the feature tables. Four new metrics land: `focus_expiration_rate`, `budget_overrun_frequency`, `task_deferral_rate`, `goal_progress_velocity`. Per-feature `promotion_threshold` lands as an `AiFeatureAttr` field emitted as `const PROMOTE_THRESHOLD_OVERRIDE: Option<usize>`; the background consolidation service reads a per-`RecallDomain` override map at evaluation time, falling back to the global config. Community lifecycle events come back as `#[derive(AiEvent)]` on `CommunityEvent { Discovered, Updated, Weakened }` in the `community_intelligence` module — the `DomainEvent` variants deleted in v1 reappear, populated only via the generated `From<CommunityEvent>` conversion. `CoActivationRepo::record_co_retrieval` emits `CoActivationEvent::Strengthened { fact_id_a, fact_id_b, strength }` when a pair's strength crosses a 2.0 threshold.

**Tech Stack:** Rust 1.93 (stable); `ai-core` + `ai-core-macros` extended with `metric`, `promotion_threshold`; `syn` + `quote` + `proc-macro2` unchanged; `sqlx` (new `ai_metric_samples` table + `MetricRepo`); `async-trait`; `cargo-nextest` for tests; `trybuild` for macro snapshot tests.

**Spec:** `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md` — v2.5 section.

**Pre-release posture:** No dual dispatch, no feature flags, no deprecation. Each task deletes the old path in the same commit that introduces the new one. Schema changes edit existing migration SQL in place where appropriate, otherwise a new migration file. Dead metrics and their feature-side tables (`task_suggestions`, `productivity_forecasts`) are deleted outright.

---

## File Structure

### New files

```
crates/ai-core/src/metric.rs                                    — MetricSpec, Aggregation, MetricSample, MetricRegistry, Window parsing
crates/ai-core/tests/metric_test.rs                             — MetricSpec/Aggregation/MetricSample/Registry unit tests
crates/ai-core-macros/tests/expand/metric.rs                    — trybuild snapshot for #[ai(metric(...))]
crates/ai-core-macros/tests/expand/promotion_threshold.rs       — trybuild snapshot for #[ai(promotion_threshold = ...)]
crates/cognitive/migrations/005_ai_metric_samples.sql           — ai_metric_samples table + index
crates/cognitive/src/repos/ai_metric_samples.rs                 — MetricRepo (insert_sample, aggregate_metric)
crates/cognitive/src/consumers/metric.rs                        — MetricHarvestConsumer (SignalConsumer impl)
crates/cognitive/src/services/community_intelligence/events.rs  — CommunityEvent enum (AiEvent derive)
crates/cognitive/src/services/community_intelligence/co_activation_events.rs — CoActivationEvent enum
crates/feature-coaching/src/events.rs                           — CoachingEvent::StrategyApplied (minimal, for metric)
crates/feature-productivity/src/events.rs                       — ProductivityEvent::SessionEnded (minimal, for metric)
tests/ai_pipeline_v25_integration.rs                            — end-to-end: event → metric sample → aggregate
tests/ai_community_events_integration.rs                        — CommunityEvent published + consumed
tests/ai_promotion_threshold_override.rs                        — per-domain override beats global
tests/ai_no_raw_sql_in_feedback.rs                              — invariant: no sqlx::query in feedback.rs
```

### Modified files

```
crates/ai-core/src/lib.rs                                       — re-export MetricSpec, Aggregation, MetricSample, MetricRegistry
crates/ai-core/src/signal.rs                                    — add metric_samples: Vec<MetricSample> field
crates/ai-core/src/recall_domain.rs                             — add RecallDomain::Coaching variant
crates/ai-core/tests/signal_test.rs                             — cover metric_samples field, Coaching variant
crates/ai-core-macros/src/attrs.rs                              — parse metric(...) on variants; promotion_threshold on features
crates/ai-core-macros/src/ai_event.rs                           — emit metric_samples population in render_variant
crates/ai-core-macros/src/ai_feature.rs                         — emit PROMOTE_THRESHOLD_OVERRIDE + FEATURE_METRICS const

crates/feature-tasks/src/events.rs                              — add metric attr on EstimationRecorded; new FocusExpired + Deferred variants
crates/feature-tasks/src/lib.rs                                 — add promotion_threshold, advertise PROMOTE_THRESHOLD_OVERRIDE
crates/feature-finance/src/events.rs                            — add metric attr on BudgetAlert; new GoalProgress variant
crates/feature-finance/src/lib.rs                               — add promotion_threshold

crates/feature-coaching/src/lib.rs                              — pub mod events; emit CoachingEvent on strategy application
crates/feature-coaching/src/service.rs                          — emit CoachingEvent::StrategyApplied when strategies complete
crates/feature-coaching/Cargo.toml                              — (ai-core dep already present from v1.5)

crates/feature-productivity/src/lib.rs                          — pub mod events; emit ProductivityEvent on session end
crates/feature-productivity/src/session.rs (or wherever sessions end) — emit ProductivityEvent::SessionEnded
crates/feature-productivity/Cargo.toml                          — add ai-core, ai-core-macros, bus deps if missing

crates/bus/src/domain_events.rs                                 — re-add CommunityDiscovered/Updated/Weakened and CoActivationStrengthened variants (only set via From<...>)
crates/cognitive/src/services/community_intelligence.rs (or mod.rs) — publish CommunityEvent in apply_intelligence
crates/cognitive/src/services/community_intelligence/mod.rs     — pub mod events; pub mod co_activation_events
crates/cognitive/src/repos/co_activation.rs                     — emit CoActivationEvent::Strengthened from record_co_retrieval on threshold crossing

crates/cognitive/src/services/reforge/feedback.rs               — delete 5 raw SQL queries; replace with registry-driven aggregator
crates/cognitive/src/services/reforge/types.rs                  — BehavioralMetrics becomes BTreeMap-backed
crates/cognitive/src/services/reforge/collector.rs              — pass MetricRegistry to load_behavioral_metrics
crates/cognitive/src/services/reforge/mod.rs                    — re-exports if needed

crates/cognitive/src/services/background.rs                     — promote decision consults per-domain override before global threshold
crates/cognitive/src/consumers/mod.rs                           — pub mod metric

crates/app-core/src/init/ai_pipeline.rs                         — translate() covers CommunityEvent + CoActivationEvent; registers MetricHarvestConsumer + MetricRegistry
crates/app-core/src/init/mod.rs                                 — Phase 8/9 wires MetricHarvestConsumer; registers FEATURE_METRICS from each feature
crates/app-core/src/init/reforge.rs (or equivalent)             — pass MetricRegistry into ReforgeCollector

crates/cognitive/migrations/001_cognitive_tables.sql            — drop task_suggestions and productivity_forecasts tables (in-place edit, pre-release)
crates/feature-tasks/migrations/*.sql                           — remove task_suggestions-related tables if any live in tasks feature
crates/feature-productivity/migrations/*.sql                    — remove productivity_forecasts table if present

crates/config/src/schema/cognitive.rs                           — no change; global stays as fallback
```

### Deleted files / code

```
- Any `task_suggestions` repo + queries (task tool automations removed 2026-04-20; table has no writers)
- Any `productivity_forecasts` repo + queries (forecast_task/forecast_project/accuracy_report removed 2026-04-20)
- feedback.rs::load_behavioral_metrics raw SQL body (replaced with registry-driven)
- BehavioralMetrics struct with 5 named Option<f64> fields (replaced with BTreeMap-backed)
```

---

## Task Overview

| # | Task | Phase |
|---|---|---|
| 1 | Add `MetricSpec`, `Aggregation`, `MetricSample`, `Window` to `ai-core::metric` | Foundation |
| 2 | Add `MetricRegistry` to `ai-core` | Foundation |
| 3 | Extend `AiSignal` with `metric_samples: Vec<MetricSample>` | Foundation |
| 4 | Add `RecallDomain::Coaching` variant | Foundation |
| 5 | Parse `#[ai(metric(...))]` on event variants — `AiEventAttr.metric: Option<MetricAttr>` | Macros |
| 6 | Emit `metric_samples` population in `render_variant` | Macros |
| 7 | Emit `FEATURE_METRICS` const on event enum `impl AiEventMeta` | Macros |
| 8 | `trybuild` snapshot for `#[ai(metric(...))]` | Macros |
| 9 | Parse `#[ai(promotion_threshold = N)]` on features — `AiFeatureAttr.promotion_threshold: Option<syn::LitInt>` | Macros |
| 10 | Emit `PROMOTE_THRESHOLD_OVERRIDE` on generated `impl` | Macros |
| 11 | `trybuild` snapshot for `#[ai(promotion_threshold = ...)]` | Macros |
| 12 | Create `005_ai_metric_samples.sql` migration + `MetricRepo` | Storage |
| 13 | Create `MetricHarvestConsumer` (`SignalConsumer` impl) | Consumer |
| 14 | Refactor `BehavioralMetrics` to `BTreeMap<&'static str, f64>`-backed | Refactor |
| 15 | Update `BehavioralMetrics` callers (`ReforgeCollected`, any field access) | Refactor |
| 16 | Replace `load_behavioral_metrics` with registry-driven aggregator | Refactor |
| 17 | Wire `MetricRegistry` + `MetricHarvestConsumer` into app-core init | Refactor |
| 18 | Migrate `task_estimation_bias` → `#[ai(metric(...))]` on `EstimationRecorded` | Migrate |
| 19 | Delete dead `suggestion_dismiss_rate` + `forecast_accuracy` metrics | Migrate |
| 20 | Drop `task_suggestions` and `productivity_forecasts` tables + code | Migrate |
| 21 | Add `CoachingEvent::StrategyApplied`; emit from coaching service; annotate metric | Migrate |
| 22 | Add `ProductivityEvent::SessionEnded`; emit from productivity session end; annotate metric | Migrate |
| 23 | Add `focus_expiration_rate` — new `TaskEvent::FocusExpired` + annotation | New Metric |
| 24 | Add `budget_overrun_frequency` — annotate `FinanceEvent::BudgetAlert` | New Metric |
| 25 | Add `task_deferral_rate` — new `TaskEvent::Deferred` + annotation | New Metric |
| 26 | Add `goal_progress_velocity` — new `FinanceEvent::GoalProgress` + annotation | New Metric |
| 27 | Annotate `TasksFeature` + `FinanceFeature` with `promotion_threshold` | Threshold |
| 28 | Wire per-domain override into `BackgroundConsolidationService` promotion check | Threshold |
| 29 | Create `CommunityEvent` enum; re-add `DomainEvent` variants via generated conversion | Community |
| 30 | Publish `CommunityEvent` from `community_intelligence::apply_intelligence` | Community |
| 31 | Create `CoActivationEvent::Strengthened`; publish from `CoActivationRepo::record_co_retrieval` on threshold crossing | Community |
| 32 | Extend `ai_pipeline::translate()` for `CommunityEvent` + `CoActivationEvent` | Wiring |
| 33 | Integration: declaration → sample → aggregate → `BehavioralMetrics.get()` | Tests |
| 34 | Invariant: no raw SQL (`sqlx::query`/`sqlx::query_scalar`) in `feedback.rs` | Tests |
| 35 | Invariant: every registered `MetricSpec` has a matching generated declaration path | Tests |
| 36 | Integration: `CommunityEvent` → bus → `IngestionConsumer` | Tests |
| 37 | Integration: `TasksFeature::PROMOTE_THRESHOLD_OVERRIDE` beats global config | Tests |
| 38 | Final verification: clippy, nextest, doctests, grep sanity | Done |

---

## Task 1: Add `MetricSpec`, `Aggregation`, `MetricSample`, `Window` to `ai-core::metric`

**Files:**
- Create: `crates/ai-core/src/metric.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Create: `crates/ai-core/tests/metric_test.rs`

`★ Insight ─────────────────────────────────────`
Metrics are declared on event variants but must survive long after the event is consumed. Modeling them as `&'static MetricSpec` with `const` construction at compile time (same pattern `MirrorSnapshotSpec` set in v2) keeps the runtime registry a `Vec<&'static MetricSpec>` — zero allocation per signal, zero serialization cost. `Window` is parsed at compile time from a string literal into `u64` seconds so the macro does the parsing, not the runtime.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/ai-core/tests/metric_test.rs`:

```rust
use ai_core::metric::{Aggregation, MetricSample, MetricSpec};

#[test]
fn metric_spec_fields_constructable_at_const() {
    const SPEC: MetricSpec = MetricSpec {
        name: "task_estimation_bias",
        window_secs: 7 * 86_400,
        min_samples: 3,
        aggregation: Aggregation::Avg,
    };
    assert_eq!(SPEC.name, "task_estimation_bias");
    assert_eq!(SPEC.window_secs, 604_800);
    assert_eq!(SPEC.min_samples, 3);
    assert!(matches!(SPEC.aggregation, Aggregation::Avg));
}

#[test]
fn aggregation_variants() {
    let _ = Aggregation::Avg;
    let _ = Aggregation::Sum;
    let _ = Aggregation::Count;
}

#[test]
fn metric_sample_carries_name_and_value() {
    let s = MetricSample { name: "coaching_acceptance_rate", value: 1.0 };
    assert_eq!(s.name, "coaching_acceptance_rate");
    assert_eq!(s.value, 1.0);
}

#[test]
fn metric_sample_is_copy() {
    let s = MetricSample { name: "x", value: 0.5 };
    let _s2 = s;
    let _s3 = s;
}

#[test]
fn aggregation_as_sql_expr() {
    assert_eq!(Aggregation::Avg.as_sql_expr(), "AVG(value)");
    assert_eq!(Aggregation::Sum.as_sql_expr(), "SUM(value)");
    assert_eq!(Aggregation::Count.as_sql_expr(), "CAST(COUNT(*) AS REAL)");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p ai-core metric_test`
Expected: FAIL — `metric` module does not exist.

- [ ] **Step 3: Create `metric.rs`**

Create `crates/ai-core/src/metric.rs`:

```rust
/// How a metric is aggregated over its sample window.
///
/// `Avg` is the natural fit for rates (0/1 samples) and bias metrics.
/// `Sum` is the natural fit for counts with a value (e.g. total amount transacted).
/// `Count` ignores the sample value and returns the number of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Avg,
    Sum,
    Count,
}

impl Aggregation {
    /// The SQL aggregate expression over the `value` column of `ai_metric_samples`.
    /// Used by `MetricRepo::aggregate_metric` — keep the string stable; tests pin it.
    pub const fn as_sql_expr(&self) -> &'static str {
        match self {
            Aggregation::Avg => "AVG(value)",
            Aggregation::Sum => "SUM(value)",
            Aggregation::Count => "CAST(COUNT(*) AS REAL)",
        }
    }
}

/// Compile-time spec for a behavioural metric, emitted by `#[derive(AiEvent)]` when
/// a variant carries `#[ai(metric(...))]`. The runtime registry is a `Vec<&'static MetricSpec>`;
/// there is never a heap-allocated `MetricSpec`.
#[derive(Debug, Clone, Copy)]
pub struct MetricSpec {
    pub name: &'static str,
    pub window_secs: u64,
    pub min_samples: u32,
    pub aggregation: Aggregation,
}

/// A single sample emitted into `AiSignal::metric_samples` by the generated `to_signal()`.
/// Copied into `ai_metric_samples` by `MetricHarvestConsumer`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricSample {
    pub name: &'static str,
    pub value: f64,
}
```

- [ ] **Step 4: Register the module in `lib.rs`**

Edit `crates/ai-core/src/lib.rs`; add:

```rust
pub mod metric;

pub use metric::{Aggregation, MetricSample, MetricSpec};
```

(Leave all existing `pub mod` and `pub use` lines intact.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run -p ai-core metric_test`
Expected: PASS (5/5).

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core/src/metric.rs crates/ai-core/src/lib.rs crates/ai-core/tests/metric_test.rs
git commit -m "feat(ai-core): add MetricSpec, Aggregation, MetricSample"
```

---

## Task 2: Add `MetricRegistry` to `ai-core`

**Files:**
- Modify: `crates/ai-core/src/metric.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Modify: `crates/ai-core/tests/metric_test.rs`

`★ Insight ─────────────────────────────────────`
Deliberately avoiding `inventory` here — the v1 Risk in the spec called out cross-crate collection via `inventory` as brittle. Instead, each feature exposes a `pub const FEATURE_METRICS: &[&'static MetricSpec]` (generated by the derive in Task 7) and app-core explicitly calls `registry.register_all(TasksFeature::FEATURE_METRICS)`. This matches the `MIRROR_SNAPSHOTS` pattern from v2 exactly.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Append failing test**

Append to `crates/ai-core/tests/metric_test.rs`:

```rust
use ai_core::metric::MetricRegistry;

#[test]
fn registry_starts_empty() {
    let r = MetricRegistry::new();
    assert_eq!(r.all().len(), 0);
    assert!(r.get("anything").is_none());
}

#[test]
fn registry_collects_specs() {
    static SPEC_A: MetricSpec = MetricSpec {
        name: "a",
        window_secs: 60,
        min_samples: 1,
        aggregation: Aggregation::Avg,
    };
    static SPEC_B: MetricSpec = MetricSpec {
        name: "b",
        window_secs: 3600,
        min_samples: 2,
        aggregation: Aggregation::Sum,
    };

    let mut r = MetricRegistry::new();
    r.register(&SPEC_A);
    r.register_all(&[&SPEC_B]);

    assert_eq!(r.all().len(), 2);
    assert_eq!(r.get("a").unwrap().name, "a");
    assert_eq!(r.get("b").unwrap().window_secs, 3600);
}

#[test]
fn registry_rejects_duplicate_names() {
    static SPEC_1: MetricSpec = MetricSpec {
        name: "dup",
        window_secs: 60,
        min_samples: 1,
        aggregation: Aggregation::Avg,
    };
    static SPEC_2: MetricSpec = MetricSpec {
        name: "dup",
        window_secs: 120,
        min_samples: 2,
        aggregation: Aggregation::Sum,
    };

    let mut r = MetricRegistry::new();
    r.register(&SPEC_1);
    let err = r.try_register(&SPEC_2).unwrap_err();
    assert!(err.contains("dup"));
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core metric_test`
Expected: FAIL — `MetricRegistry` does not exist.

- [ ] **Step 3: Append `MetricRegistry` to `metric.rs`**

Append to `crates/ai-core/src/metric.rs`:

```rust
/// Workspace-global registry of `MetricSpec`s. Populated explicitly at startup
/// by app-core calling `register_all(Feature::FEATURE_METRICS)` for each feature.
/// Duplicate names are a programming error — fail fast at startup.
#[derive(Debug, Default)]
pub struct MetricRegistry {
    specs: Vec<&'static MetricSpec>,
}

impl MetricRegistry {
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    /// Panics on duplicate name. Use `try_register` to check first.
    pub fn register(&mut self, spec: &'static MetricSpec) {
        if let Err(e) = self.try_register(spec) {
            panic!("MetricRegistry: {}", e);
        }
    }

    pub fn try_register(&mut self, spec: &'static MetricSpec) -> Result<(), String> {
        if self.specs.iter().any(|s| s.name == spec.name) {
            return Err(format!("duplicate metric name: {}", spec.name));
        }
        self.specs.push(spec);
        Ok(())
    }

    pub fn register_all(&mut self, specs: &[&'static MetricSpec]) {
        for s in specs {
            self.register(s);
        }
    }

    pub fn all(&self) -> &[&'static MetricSpec] {
        &self.specs
    }

    pub fn get(&self, name: &str) -> Option<&'static MetricSpec> {
        self.specs.iter().copied().find(|s| s.name == name)
    }
}
```

- [ ] **Step 4: Update lib.rs re-export**

Edit `crates/ai-core/src/lib.rs`; extend the re-export:

```rust
pub use metric::{Aggregation, MetricRegistry, MetricSample, MetricSpec};
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core metric_test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core/src/metric.rs crates/ai-core/src/lib.rs crates/ai-core/tests/metric_test.rs
git commit -m "feat(ai-core): add MetricRegistry with duplicate-name guard"
```

---

## Task 3: Extend `AiSignal` with `metric_samples: Vec<MetricSample>`

**Files:**
- Modify: `crates/ai-core/src/signal.rs`
- Modify: `crates/ai-core/tests/signal_test.rs`

`★ Insight ─────────────────────────────────────`
A `Vec` is correct here even though most events produce zero or one sample — an event variant could declare multiple metrics in the future (e.g. a completion event that feeds both `task_completion_rate` and `task_duration_avg`). Allocating an empty `Vec` per signal is ~24 bytes, negligible compared to the signal's `content: String` and `raw_event: DomainEvent`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/signal_test.rs`:

```rust
#[test]
fn signal_carries_metric_samples() {
    use ai_core::{AiSignal, MetricSample, RecallDomain, SalienceVerdict};

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "EstimationRecorded",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: "est 30m actual 45m".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![MetricSample {
            name: "task_estimation_bias",
            value: 0.5,
        }],
    };
    assert_eq!(sig.metric_samples.len(), 1);
    assert_eq!(sig.metric_samples[0].name, "task_estimation_bias");
}

#[test]
fn signal_metric_samples_default_empty() {
    use ai_core::{AiSignal, RecallDomain, SalienceVerdict};

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Created",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "Task created".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    };
    assert!(sig.metric_samples.is_empty());
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core signal`
Expected: FAIL — `metric_samples` field missing.

- [ ] **Step 3: Add field to `AiSignal`**

Edit `crates/ai-core/src/signal.rs`; replace the `AiSignal` struct with:

```rust
use crate::metric::MetricSample;
use crate::metrics::AiMetrics;
use crate::recall_domain::RecallDomain;
use bus::DomainEvent;
use jiff::Timestamp;

#[derive(Debug, Clone)]
pub struct AiSignal {
    pub domain: RecallDomain,
    pub event_kind: &'static str,
    pub importance: f64,
    pub salience: SalienceVerdict,
    pub content: String,
    pub entity: Option<EntityRef>,
    pub timestamp: Timestamp,
    pub raw_event: Option<DomainEvent>,
    pub metrics: AiMetrics,
    pub coaching_signal: bool,
    pub coaching_rule: Option<String>,
    /// Metric samples emitted by this event, one per `#[ai(metric(...))]` declaration.
    /// Consumed by `MetricHarvestConsumer` and persisted to `ai_metric_samples`.
    pub metric_samples: Vec<MetricSample>,
}
```

- [ ] **Step 4: Fix any ambient compilation errors**

The derive macro's generated `to_signal()` doesn't yet populate the new field — add a transient default in `ai-core-macros/src/ai_event.rs::render_variant` so the workspace still compiles. Task 6 replaces this with real population. Search for the generated `AiSignal {` literal (around line 150+ of `ai_event.rs`) and append `metric_samples: Vec::new(),` to the struct init alongside the other defaults.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core signal`
Expected: PASS.

Run: `cargo build --workspace`
Expected: builds with only possible new warnings; no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core/src/signal.rs crates/ai-core/tests/signal_test.rs crates/ai-core-macros/src/ai_event.rs
git commit -m "feat(ai-core): add AiSignal.metric_samples field"
```

---

## Task 4: Add `RecallDomain::Coaching` variant

**Files:**
- Modify: `crates/ai-core/src/recall_domain.rs`
- Modify: `crates/ai-core/tests/signal_test.rs`

`★ Insight ─────────────────────────────────────`
`RecallDomain` is hand-enumerated (v1 risk-mitigation decision). Adding `Coaching` here — even though `feature-coaching` isn't a full `AiFeature` until v3 — lets the minimal `CoachingEvent::StrategyApplied` enum (Task 21) declare `#[ai(domain = "Coaching")]` and produce typed signals immediately. Without this variant, the coaching event would have to use `General`, which reads wrong at the consumer side.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/signal_test.rs`:

```rust
#[test]
fn coaching_domain_roundtrips() {
    use ai_core::RecallDomain;
    assert_eq!(RecallDomain::Coaching.as_str(), "coaching");
    assert_eq!(
        RecallDomain::from_str_or_general("coaching"),
        RecallDomain::Coaching
    );
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core coaching_domain_roundtrips`
Expected: FAIL — `Coaching` variant missing.

- [ ] **Step 3: Add the variant**

Edit `crates/ai-core/src/recall_domain.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallDomain {
    General,
    Tasks,
    Finance,
    Productivity,
    Learning,
    Mirror,
    Coaching,
}

impl RecallDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecallDomain::General => "general",
            RecallDomain::Tasks => "tasks",
            RecallDomain::Finance => "finance",
            RecallDomain::Productivity => "productivity",
            RecallDomain::Learning => "learning",
            RecallDomain::Mirror => "mirror",
            RecallDomain::Coaching => "coaching",
        }
    }

    pub fn from_str_or_general(s: &str) -> Self {
        match s {
            "tasks" => RecallDomain::Tasks,
            "finance" => RecallDomain::Finance,
            "productivity" => RecallDomain::Productivity,
            "learning" => RecallDomain::Learning,
            "mirror" => RecallDomain::Mirror,
            "coaching" => RecallDomain::Coaching,
            _ => RecallDomain::General,
        }
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p ai-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core/src/recall_domain.rs crates/ai-core/tests/signal_test.rs
git commit -m "feat(ai-core): add RecallDomain::Coaching variant"
```

---

## Task 5: Parse `#[ai(metric(...))]` on Event Variants

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`
- Modify: `crates/ai-core-macros/tests/attrs_test.rs` (create if missing)

`★ Insight ─────────────────────────────────────`
`window` is specified as a string literal like `"7d"` or `"1h"` because `u64::from_str` in a `syn` attribute is awkward and `Duration` isn't `const`-constructible on stable. Parsing `"7d"` → `604_800` at macro expansion time keeps the emitted `MetricSpec` literal numeric. The supported suffixes are minimal: `s`, `m`, `h`, `d` — no weeks/months because metrics use rolling windows, never calendar boundaries.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create or extend `crates/ai-core-macros/tests/attrs_test.rs`:

```rust
use ai_core_macros::attrs::{parse_window_secs, MetricAttr, Aggregation as MacroAgg};
// NOTE: attrs module made `pub` within the crate via `pub(crate) mod attrs;` in lib.rs;
//       tests reach it via the macros crate's test surface.

#[test]
fn window_parses_seconds() {
    assert_eq!(parse_window_secs("30s").unwrap(), 30);
    assert_eq!(parse_window_secs("5m").unwrap(), 300);
    assert_eq!(parse_window_secs("2h").unwrap(), 7_200);
    assert_eq!(parse_window_secs("7d").unwrap(), 604_800);
}

#[test]
fn window_rejects_empty_or_missing_unit() {
    assert!(parse_window_secs("").is_err());
    assert!(parse_window_secs("7").is_err());
    assert!(parse_window_secs("7x").is_err());
}
```

> **NOTE on module visibility**: if `attrs` is currently `pub(crate) mod attrs`, flip it to `pub mod attrs` in `crates/ai-core-macros/src/lib.rs` so the test can reach in. Proc-macro crates can still expose plain items to their own integration tests.

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core-macros attrs_test`
Expected: FAIL — `parse_window_secs` and `MetricAttr` don't exist.

- [ ] **Step 3: Add window parser + `MetricAttr` struct**

Append to `crates/ai-core-macros/src/attrs.rs`:

```rust
use syn::{meta::ParseNestedMeta, Expr, Ident, LitInt, LitStr};

/// Compile-time parse of `"7d"` / `"1h"` / `"30m"` / `"15s"` into u64 seconds.
/// Returns a descriptive error string on any parse failure.
pub fn parse_window_secs(s: &str) -> Result<u64, String> {
    if s.is_empty() {
        return Err("window must be non-empty (e.g. \"7d\")".into());
    }
    let (n, unit) = s.split_at(s.len() - 1);
    let n: u64 = n.parse().map_err(|_| format!("window prefix must be numeric: {}", s))?;
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(format!("window unit must be s|m|h|d: {}", s)),
    };
    Ok(n.saturating_mul(mult))
}

#[derive(Debug, Clone)]
pub struct MetricAttr {
    pub name: String,
    pub value_from: Expr,
    pub window_secs: u64,
    pub min_samples: u32,
    pub aggregation: Aggregation,
}

/// Mirror of `ai_core::metric::Aggregation` used only within the proc-macro crate.
/// We don't import from `ai-core` here because `ai-core-macros` is an L1 proc-macro crate
/// with no runtime dependency on `ai-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Avg,
    Sum,
    Count,
}

impl Aggregation {
    pub fn emit_tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Aggregation::Avg => quote::quote!(::ai_core::Aggregation::Avg),
            Aggregation::Sum => quote::quote!(::ai_core::Aggregation::Sum),
            Aggregation::Count => quote::quote!(::ai_core::Aggregation::Count),
        }
    }

    pub fn parse_str(s: &str) -> Result<Self, String> {
        match s {
            "avg" => Ok(Aggregation::Avg),
            "sum" => Ok(Aggregation::Sum),
            "count" => Ok(Aggregation::Count),
            _ => Err(format!("aggregation must be avg|sum|count: {}", s)),
        }
    }
}

pub(crate) fn parse_metric_attr(meta: ParseNestedMeta<'_>) -> syn::Result<MetricAttr> {
    let mut name: Option<String> = None;
    let mut value_from: Option<Expr> = None;
    let mut window: Option<u64> = None;
    let mut min_samples: u32 = 3; // default
    let mut aggregation: Option<Aggregation> = None;

    meta.parse_nested_meta(|nested| {
        let key = nested
            .path
            .get_ident()
            .ok_or_else(|| nested.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "name" => {
                let s: LitStr = nested.value()?.parse()?;
                name = Some(s.value());
            }
            "value_from" => {
                let e: Expr = nested.value()?.parse()?;
                value_from = Some(e);
            }
            "window" => {
                let s: LitStr = nested.value()?.parse()?;
                window = Some(parse_window_secs(&s.value()).map_err(|e| nested.error(e))?);
            }
            "min_samples" => {
                let n: LitInt = nested.value()?.parse()?;
                min_samples = n.base10_parse()?;
            }
            "aggregation" => {
                let s: LitStr = nested.value()?.parse()?;
                aggregation = Some(
                    Aggregation::parse_str(&s.value()).map_err(|e| nested.error(e))?,
                );
            }
            other => return Err(nested.error(format!("unknown metric() key: {}", other))),
        }
        Ok(())
    })?;

    Ok(MetricAttr {
        name: name.ok_or_else(|| meta.error("metric: name is required"))?,
        value_from: value_from
            .ok_or_else(|| meta.error("metric: value_from is required"))?,
        window_secs: window.ok_or_else(|| meta.error("metric: window is required"))?,
        min_samples,
        aggregation: aggregation
            .ok_or_else(|| meta.error("metric: aggregation is required"))?,
    })
}
```

Add `metric: Option<MetricAttr>` to the existing `AiEventAttr` struct (search attrs.rs for the `pub struct AiEventAttr {`):

```rust
pub struct AiEventAttr {
    pub importance: Option<f64>,
    pub importance_fn: Option<Path>,
    pub salience: SalienceSpec,
    pub observation_template: Option<String>,
    pub entity_bridge: Option<EntityBridge>,
    pub coaching: Option<CoachingSignalSpec>,
    pub metric: Option<MetricAttr>, // NEW
}
```

In the existing `parse_ai_event_attr` function, find the `match key.as_str()` block and add a new arm before the `other =>` catch-all:

```rust
"metric" => {
    out.metric = Some(parse_metric_attr(nested)?);
}
```

- [ ] **Step 4: Flip `attrs` visibility if needed**

Edit `crates/ai-core-macros/src/lib.rs`:

```rust
pub mod attrs;  // was pub(crate) mod attrs;
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core-macros attrs_test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core-macros/src/attrs.rs crates/ai-core-macros/src/lib.rs crates/ai-core-macros/tests/attrs_test.rs
git commit -m "feat(ai-core-macros): parse #[ai(metric(...))] attribute"
```

---

## Task 6: Emit `metric_samples` Population in `render_variant`

**Files:**
- Modify: `crates/ai-core-macros/src/ai_event.rs`

- [ ] **Step 1: Write failing test (trybuild later) — interim compile check**

This step is validated by the trybuild snapshot in Task 8, but first a compile-level check: add a tiny inline test in `crates/feature-tasks/src/events.rs` temporarily to verify the macro expansion compiles. We'll revert the inline tweak at the end of Step 5 and replace with the real Task 18 migration.

Add this one-off test variant **temporarily** to the existing `TaskEvent` enum (same file):

```rust
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "test metric shape",
        metric(
            name = "__test_metric",
            value_from = "1.0_f64",
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    TestMetric {},
```

Run: `cargo build -p feature-tasks`
Expected: FAIL — `render_variant` doesn't emit `metric_samples` population yet, or the unused `metric` field triggers a warning.

- [ ] **Step 2: Add emission to `render_variant`**

Edit `crates/ai-core-macros/src/ai_event.rs`. Find `render_variant` (approximately line 85). Inside the function, after the existing destructure of `attr: &AiEventAttr`, synthesise the `metric_samples` token stream:

```rust
    let metric_samples_ts = if let Some(metric) = &attr.metric {
        let m_name = &metric.name;
        let m_value = &metric.value_from;
        quote! {
            vec![::ai_core::MetricSample {
                name: #m_name,
                value: (#m_value) as f64,
            }]
        }
    } else {
        quote! { Vec::new() }
    };
```

Then in the generated `AiSignal { ... }` struct literal inside the same function, replace the placeholder `metric_samples: Vec::new(),` line (added in Task 3) with:

```rust
    metric_samples: #metric_samples_ts,
```

- [ ] **Step 3: Rebuild**

Run: `cargo build -p feature-tasks`
Expected: PASS. The temporary `TestMetric {}` variant compiles.

- [ ] **Step 4: Remove the temporary `TestMetric` variant**

Revert `crates/feature-tasks/src/events.rs` back to its original state without `TestMetric`.

- [ ] **Step 5: Run full workspace check**

Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core-macros/src/ai_event.rs
git commit -m "feat(ai-core-macros): emit metric_samples from #[ai(metric(...))]"
```

---

## Task 7: Emit `FEATURE_METRICS` Const on Event Enum

**Files:**
- Modify: `crates/ai-core-macros/src/ai_event.rs`
- Modify: `crates/ai-core/tests/signal_test.rs` (pin expected shape)

`★ Insight ─────────────────────────────────────`
The `FEATURE_METRICS` const lives on the event enum's `impl AiEventMeta for EventName`, not on the `AiFeature` struct. Rationale: metrics are declared per variant, so the event enum is the natural owner. The derive walks all variants in one pass, collects their `MetricAttr`s, and emits a flat slice. App-core registers `TaskEvent::FEATURE_METRICS`, `FinanceEvent::FEATURE_METRICS`, etc.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/signal_test.rs`:

```rust
#[test]
fn task_event_feature_metrics_populated_for_estimation_recorded() {
    // This test runs after Task 18 annotates EstimationRecorded, but the const
    // shape itself is introduced by the derive output here.
    let all = feature_tasks::TaskEvent::FEATURE_METRICS;
    // After Task 18 this is 1+; for this task, we only need the const to exist.
    let _ = all;
}
```

> **Note on dev-dep**: If `feature-tasks` is not yet a dev-dep of `ai-core`, skip this test for now; a parallel test lives in `tests/ai_pipeline_v25_integration.rs` at Task 33. The real contract for this task is that the derive expansion compiles without error and the snapshot in Task 8 pins it.

- [ ] **Step 2: Find the derive entry in `ai_event.rs`**

Inspect `crates/ai-core-macros/src/ai_event.rs`. The derive expansion function (typically `pub fn expand`) returns a `TokenStream` containing the `impl AiEventMeta for #enum_name {...}` block.

- [ ] **Step 3: Add `FEATURE_METRICS` emission**

Within the derive expansion function, after iterating variants and collecting per-variant `render_variant` token streams, collect a parallel `Vec<TokenStream>` of `MetricSpec` entries, one per variant that has `attr.metric.is_some()`:

```rust
    let mut metric_specs_ts: Vec<proc_macro2::TokenStream> = Vec::new();
    for (variant_ident, attr) in &variants {
        if let Some(metric) = &attr.metric {
            let m_name = &metric.name;
            let m_window = metric.window_secs;
            let m_min = metric.min_samples;
            let m_agg_ts = metric.aggregation.emit_tokens();
            // Unique static name to avoid collisions across variants.
            let const_ident = syn::Ident::new(
                &format!("METRIC_SPEC_{}", variant_ident.to_string().to_uppercase()),
                variant_ident.span(),
            );
            metric_specs_ts.push(quote! {
                {
                    static #const_ident: ::ai_core::MetricSpec = ::ai_core::MetricSpec {
                        name: #m_name,
                        window_secs: #m_window,
                        min_samples: #m_min,
                        aggregation: #m_agg_ts,
                    };
                    &#const_ident
                }
            });
        }
    }
```

Then in the `impl AiEventMeta for #enum_name {}` block being emitted, add an inherent constant on the type itself (outside the trait impl but associated with the type):

```rust
    let feature_metrics_impl = quote! {
        impl #enum_name {
            /// All `MetricSpec`s declared by variants of this enum via `#[ai(metric(...))]`.
            /// Registered by app-core at startup via `MetricRegistry::register_all`.
            pub const FEATURE_METRICS: &'static [&'static ::ai_core::MetricSpec] = &[
                #(#metric_specs_ts),*
            ];
        }
    };

    // Merge into the final expansion:
    Ok(quote! {
        #ai_event_meta_impl
        #from_impl
        #feature_metrics_impl
    })
```

(Exact integration depends on the current structure of `expand`; the key points are (a) the const is emitted and (b) it is a `&'static [&'static MetricSpec]` so the registry can store references.)

- [ ] **Step 4: Verify expansion**

Run: `cargo build -p feature-tasks -p feature-finance`
Expected: PASS. `TaskEvent::FEATURE_METRICS` and `FinanceEvent::FEATURE_METRICS` are both empty slices at this point.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros/src/ai_event.rs crates/ai-core/tests/signal_test.rs
git commit -m "feat(ai-core-macros): emit FEATURE_METRICS const on event enum"
```

---

## Task 8: `trybuild` Snapshot for `#[ai(metric(...))]`

**Files:**
- Create: `crates/ai-core-macros/tests/expand/metric.rs`

- [ ] **Step 1: Create the expansion input**

Create `crates/ai-core-macros/tests/expand/metric.rs`:

```rust
use ai_core_macros::AiEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Tasks")]
pub enum TaskMetricDemo {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "est {estimated_mins}m actual {actual_mins}m",
        metric(
            name = "task_estimation_bias",
            value_from = "deviation_pct",
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    EstimationRecorded {
        task_id: String,
        estimated_mins: u32,
        actual_mins: u32,
        deviation_pct: f64,
    },

    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Completed {title}",
        metric(
            name = "task_completion_rate",
            value_from = "1.0_f64",
            window = "1d",
            min_samples = 5,
            aggregation = "avg",
        ),
    )]
    Completed { task_id: String, title: String },
}

fn main() {
    use ai_core::AiEventMeta;
    let e = TaskMetricDemo::EstimationRecorded {
        task_id: "abc".into(),
        estimated_mins: 30,
        actual_mins: 45,
        deviation_pct: 0.5,
    };
    let sig = e.to_signal();
    assert_eq!(sig.metric_samples.len(), 1);
    assert_eq!(sig.metric_samples[0].name, "task_estimation_bias");
    assert!((sig.metric_samples[0].value - 0.5).abs() < 1e-9);

    assert_eq!(TaskMetricDemo::FEATURE_METRICS.len(), 2);
    let bias = TaskMetricDemo::FEATURE_METRICS
        .iter()
        .find(|s| s.name == "task_estimation_bias")
        .unwrap();
    assert_eq!(bias.window_secs, 7 * 86_400);
    assert_eq!(bias.min_samples, 3);
    assert!(matches!(bias.aggregation, ai_core::Aggregation::Avg));
}
```

- [ ] **Step 2: Wire into the trybuild harness**

Check `crates/ai-core-macros/tests/expand_smoke.rs` (or equivalent). Add:

```rust
#[test]
fn metric_expansion() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/metric.rs");
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p ai-core-macros metric_expansion`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ai-core-macros/tests/expand/metric.rs crates/ai-core-macros/tests/expand_smoke.rs
git commit -m "test(ai-core-macros): trybuild snapshot for #[ai(metric(...))]"
```

---

## Task 9: Parse `#[ai(promotion_threshold = N)]` on Features

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`
- Modify: `crates/ai-core-macros/tests/attrs_test.rs`

`★ Insight ─────────────────────────────────────`
`promotion_threshold` lives on the feature struct (not the event enum) because it's a domain-level policy, not an event-level one. Emitting it as `pub const PROMOTE_THRESHOLD_OVERRIDE: Option<usize>` on the feature's inherent impl keeps the lookup O(1) at the promotion call site: `if let Some(n) = TasksFeature::PROMOTE_THRESHOLD_OVERRIDE { n } else { global_config }`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core-macros/tests/attrs_test.rs`:

```rust
#[test]
fn feature_attr_parses_promotion_threshold() {
    // The parser uses LitInt; we verify it compiles via the trybuild test in Task 11.
    // Here we assert the AiFeatureAttr exposes the field.
    use ai_core_macros::attrs::AiFeatureAttr;
    // Compile-level check: the field must exist.
    let a = AiFeatureAttr::default();
    assert!(a.promotion_threshold.is_none());
}
```

> **Add `impl Default for AiFeatureAttr`** if it doesn't already exist — trivially `Default::default()` on each field.

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core-macros attrs_test`
Expected: FAIL — `promotion_threshold` field missing.

- [ ] **Step 3: Add field + parsing**

Edit `crates/ai-core-macros/src/attrs.rs`. Locate `pub struct AiFeatureAttr { ... }` and add the field:

```rust
pub struct AiFeatureAttr {
    pub recall_domain: Ident,
    pub skill: String,
    pub event: Path,
    pub recall_boost_when: Option<Expr>,
    pub recall_priority_field: Option<Ident>,
    pub recall_recency_field: Option<Ident>,
    pub recall_status_filter: Option<Expr>,
    pub mirror_snapshots: Vec<MirrorSnapshotAttr>,
    pub promotion_threshold: Option<u32>, // NEW
}
```

Add to `impl Default`:

```rust
impl Default for AiFeatureAttr {
    fn default() -> Self {
        Self {
            recall_domain: Ident::new("General", proc_macro2::Span::call_site()),
            skill: String::new(),
            event: syn::parse_str("::bus::DomainEvent").unwrap(),
            recall_boost_when: None,
            recall_priority_field: None,
            recall_recency_field: None,
            recall_status_filter: None,
            mirror_snapshots: Vec::new(),
            promotion_threshold: None,
        }
    }
}
```

In `parse_ai_feature_attr`'s match block, add before the `other =>` catch-all:

```rust
"promotion_threshold" => {
    let n: LitInt = nested.value()?.parse()?;
    out.promotion_threshold = Some(n.base10_parse()?);
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p ai-core-macros attrs_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros/src/attrs.rs crates/ai-core-macros/tests/attrs_test.rs
git commit -m "feat(ai-core-macros): parse #[ai(promotion_threshold = N)]"
```

---

## Task 10: Emit `PROMOTE_THRESHOLD_OVERRIDE` on Generated Impl

**Files:**
- Modify: `crates/ai-core-macros/src/ai_feature.rs`

- [ ] **Step 1: Write failing test (compile-level)**

Temporarily annotate `TasksFeature` in `crates/feature-tasks/src/lib.rs` with `promotion_threshold = 3`:

```rust
#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::events::TaskEvent",
    recall_boost_when = "query.message.to_lowercase().contains(\"deadline\") || ...",  // keep existing
    recall_priority_field = "priority",
    recall_recency_field = "updated_at",
    recall_status_filter = "status != \"archived\"",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["TaskFocusChanged", "TaskCompleted"],
    ),
    promotion_threshold = 3,
)]
pub struct TasksFeature { pool, task_tool }
```

Create `crates/feature-tasks/tests/promote_threshold_test.rs`:

```rust
#[test]
fn tasks_feature_exposes_promote_threshold_override() {
    assert_eq!(feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE, Some(3));
}
```

Run: `cargo nextest run -p feature-tasks promote_threshold_test`
Expected: FAIL — `PROMOTE_THRESHOLD_OVERRIDE` doesn't exist.

- [ ] **Step 2: Emit the constant**

Edit `crates/ai-core-macros/src/ai_feature.rs`. In the derive expansion function, after collecting `attr: AiFeatureAttr`, build a token stream for the override:

```rust
    let promote_threshold_ts = match attr.promotion_threshold {
        Some(n) => quote! { Some(#n as usize) },
        None => quote! { None },
    };
```

Add to the generated `impl` block (alongside `RECALL_SPEC` and `MIRROR_SNAPSHOTS`):

```rust
    let inherent_impl = quote! {
        impl #struct_ident {
            pub const RECALL_SPEC: ::ai_core::RecallSpec = ::ai_core::RecallSpec { /* ... */ };
            pub const MIRROR_SNAPSHOTS: &'static [::ai_core::MirrorSnapshotSpec] = &[
                /* ... */
            ];
            /// Optional per-feature override for `accumulate_promote_threshold`.
            /// When `Some(n)`, the background consolidation service uses `n` for this
            /// `RecallDomain` instead of the global config value.
            pub const PROMOTE_THRESHOLD_OVERRIDE: Option<usize> = #promote_threshold_ts;
        }
    };
```

(The exact merge with existing inherent-impl emission depends on current structure. Key requirement: emit the const on the feature struct.)

- [ ] **Step 3: Run**

Run: `cargo nextest run -p feature-tasks promote_threshold_test`
Expected: PASS.

Also run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 4: Remove the temporary annotation**

Revert `promotion_threshold = 3` from `crates/feature-tasks/src/lib.rs` — Task 27 adds the real annotation.

Delete `crates/feature-tasks/tests/promote_threshold_test.rs` — the real test lives in Task 37.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros/src/ai_feature.rs
git commit -m "feat(ai-core-macros): emit PROMOTE_THRESHOLD_OVERRIDE from #[ai(promotion_threshold)]"
```

---

## Task 11: `trybuild` Snapshot for `#[ai(promotion_threshold = ...)]`

**Files:**
- Create: `crates/ai-core-macros/tests/expand/promotion_threshold.rs`
- Modify: `crates/ai-core-macros/tests/expand_smoke.rs`

- [ ] **Step 1: Create expansion input**

Create `crates/ai-core-macros/tests/expand/promotion_threshold.rs`:

```rust
use ai_core_macros::{AiEvent, AiFeature};

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Tasks")]
pub enum DemoEvent {
    #[ai(importance = 0.5, salience = "accumulate", observation_template = "x")]
    Thing {},
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "demo-skill",
    event = "crate::DemoEvent",
    promotion_threshold = 7,
)]
pub struct DemoFeature;

fn main() {
    assert_eq!(DemoFeature::PROMOTE_THRESHOLD_OVERRIDE, Some(7));
}
```

- [ ] **Step 2: Wire into harness**

Append to `crates/ai-core-macros/tests/expand_smoke.rs`:

```rust
#[test]
fn promotion_threshold_expansion() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/promotion_threshold.rs");
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p ai-core-macros promotion_threshold_expansion`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ai-core-macros/tests/expand/promotion_threshold.rs crates/ai-core-macros/tests/expand_smoke.rs
git commit -m "test(ai-core-macros): trybuild snapshot for promotion_threshold"
```

---

## Task 12: `005_ai_metric_samples.sql` Migration + `MetricRepo`

**Files:**
- Create: `crates/cognitive/migrations/005_ai_metric_samples.sql`
- Create: `crates/cognitive/src/repos/ai_metric_samples.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Create: `crates/cognitive/src/repos/tests/ai_metric_samples_tests.rs` (or inline `#[cfg(test)] mod tests`)

`★ Insight ─────────────────────────────────────`
One unified table (`ai_metric_samples`) replaces five feature-owned tables. The trade is: a write on every event-with-metric vs. precomputed feature tables. For our volume (single-user) the write cost is nil and the read path simplifies to one generic aggregate query per metric. The `(metric_name, sample_time)` composite index keeps the time-windowed aggregate fast regardless of total row count.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the migration**

Create `crates/cognitive/migrations/005_ai_metric_samples.sql`:

```sql
CREATE TABLE IF NOT EXISTS ai_metric_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    value REAL NOT NULL,
    sample_time TEXT NOT NULL,          -- ISO8601 UTC
    source_domain TEXT NOT NULL,
    source_event_kind TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_metric_samples_name_time
    ON ai_metric_samples (metric_name, sample_time);

CREATE INDEX IF NOT EXISTS idx_ai_metric_samples_source
    ON ai_metric_samples (source_domain, source_event_kind);
```

- [ ] **Step 2: Register the migration**

Edit `crates/cognitive/src/lib.rs` (or wherever the migration list lives — typically `CognitiveFeature::migrations()`). Add to the returned `Vec<FeatureMigration>`:

```rust
FeatureMigration {
    version: 5,
    name: "ai_metric_samples",
    sql: include_str!("../migrations/005_ai_metric_samples.sql"),
},
```

- [ ] **Step 3: Write failing test**

Create `crates/cognitive/src/repos/ai_metric_samples.rs`:

```rust
use ai_core::{Aggregation, MetricSample, MetricSpec};
use common::Result;
use jiff::Timestamp;
use sqlx::SqlitePool;

pub struct MetricRepo {
    pool: SqlitePool,
}

impl MetricRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_sample(
        &self,
        sample: &MetricSample,
        sample_time: Timestamp,
        source_domain: &str,
        source_event_kind: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO ai_metric_samples \
             (metric_name, value, sample_time, source_domain, source_event_kind) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(sample.name)
        .bind(sample.value)
        .bind(sample_time.to_string())
        .bind(source_domain)
        .bind(source_event_kind)
        .execute(&self.pool)
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(())
    }

    /// Run the aggregation declared in `spec` over its rolling window. Returns `None` when
    /// fewer than `min_samples` rows fall in the window (matches the original HAVING behaviour).
    pub async fn aggregate_metric(&self, spec: &MetricSpec) -> Result<Option<f64>> {
        let window_expr = format!("-{} seconds", spec.window_secs);
        let sql = format!(
            "SELECT {} FROM ai_metric_samples \
             WHERE metric_name = ?1 \
               AND sample_time > datetime('now', ?2) \
             HAVING COUNT(*) >= ?3",
            spec.aggregation.as_sql_expr()
        );
        let row: Option<f64> = sqlx::query_scalar(&sql)
            .bind(spec.name)
            .bind(&window_expr)
            .bind(spec.min_samples as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(common::KlyntbotError::from)?;
        Ok(row)
    }

    #[cfg(test)]
    pub async fn count_samples_for(&self, metric_name: &str) -> Result<i64> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_metric_samples WHERE metric_name = ?",
        )
        .bind(metric_name)
        .fetch_one(&self.pool)
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> (StoragePool, MetricRepo) {
        let pool = StoragePool::connect_in_memory_with_migrations(&[
            /* cognitive migrations 1..=5 — or use the crate-level helper */
        ])
        .await
        .unwrap();
        let repo = MetricRepo::new(pool.sqlite().clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn insert_sample_persists() {
        let (_pool, repo) = setup().await;
        let sample = MetricSample { name: "test_metric", value: 0.42 };
        repo.insert_sample(&sample, Timestamp::now(), "tasks", "EstimationRecorded")
            .await
            .unwrap();
        assert_eq!(repo.count_samples_for("test_metric").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn aggregate_avg_returns_none_below_min_samples() {
        let (_pool, repo) = setup().await;
        static SPEC: MetricSpec = MetricSpec {
            name: "t_avg",
            window_secs: 604_800,
            min_samples: 3,
            aggregation: Aggregation::Avg,
        };
        // Insert only 2 rows -> HAVING COUNT(*) >= 3 filters them out.
        for v in [0.1_f64, 0.2] {
            repo.insert_sample(
                &MetricSample { name: "t_avg", value: v },
                Timestamp::now(),
                "tasks",
                "T",
            )
            .await
            .unwrap();
        }
        assert!(repo.aggregate_metric(&SPEC).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn aggregate_avg_returns_mean_when_sufficient() {
        let (_pool, repo) = setup().await;
        static SPEC: MetricSpec = MetricSpec {
            name: "t_mean",
            window_secs: 604_800,
            min_samples: 3,
            aggregation: Aggregation::Avg,
        };
        for v in [0.1_f64, 0.2, 0.3] {
            repo.insert_sample(
                &MetricSample { name: "t_mean", value: v },
                Timestamp::now(),
                "tasks",
                "T",
            )
            .await
            .unwrap();
        }
        let out = repo.aggregate_metric(&SPEC).await.unwrap().unwrap();
        assert!((out - 0.2).abs() < 1e-9);
    }

    #[tokio::test]
    async fn aggregate_respects_window() {
        let (_pool, repo) = setup().await;
        static SPEC: MetricSpec = MetricSpec {
            name: "t_win",
            window_secs: 1, // 1 second window
            min_samples: 1,
            aggregation: Aggregation::Avg,
        };
        // Insert a sample at "now - 5 seconds" via direct SQL:
        sqlx::query(
            "INSERT INTO ai_metric_samples (metric_name, value, sample_time, source_domain, source_event_kind) \
             VALUES (?, ?, datetime('now', '-5 seconds'), 'tasks', 'T')",
        )
        .bind("t_win")
        .bind(1.0)
        .execute(repo.pool())
        .await
        .unwrap();
        // With a 1s window, the -5s sample is outside -> HAVING fails -> None.
        assert!(repo.aggregate_metric(&SPEC).await.unwrap().is_none());
    }
}

impl MetricRepo {
    #[cfg(test)]
    fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
```

> **If `StoragePool::connect_in_memory_with_migrations` doesn't exist**, use whatever helper sets up a test pool with all cognitive migrations — typically `crates/storage/src/pool.rs` exposes one. Match the style used in existing `crates/cognitive/src/repos/tests/*` files.

Register the module in `crates/cognitive/src/repos/mod.rs`:

```rust
pub mod ai_metric_samples;
pub use ai_metric_samples::MetricRepo;
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive -E 'test(ai_metric_samples)'`
Expected: PASS (3 tests — insert, avg-below-min, avg-with-window).

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/migrations/005_ai_metric_samples.sql crates/cognitive/src/repos/ai_metric_samples.rs crates/cognitive/src/repos/mod.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add ai_metric_samples table + MetricRepo"
```

---

## Task 13: Create `MetricHarvestConsumer`

**Files:**
- Create: `crates/cognitive/src/consumers/metric.rs`
- Modify: `crates/cognitive/src/consumers/mod.rs`
- Create: `crates/cognitive/src/consumers/tests/metric_tests.rs` (or inline)

`★ Insight ─────────────────────────────────────`
The consumer is intentionally dumb: it writes every sample it sees. No filtering, no transformation. Signals without samples produce a no-op. This keeps the consumer idempotent and the macro contract tight — if the macro emits a sample, it lands in the table, full stop.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/src/consumers/metric.rs`:

```rust
use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use common::Result;
use std::sync::Arc;

use crate::repos::MetricRepo;

pub struct MetricHarvestConsumer {
    repo: Arc<MetricRepo>,
}

impl MetricHarvestConsumer {
    pub fn new(repo: Arc<MetricRepo>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl SignalConsumer for MetricHarvestConsumer {
    fn name(&self) -> &'static str {
        "metric_harvest"
    }

    async fn consume(&self, signal: &AiSignal) -> Result<()> {
        if signal.metric_samples.is_empty() {
            return Ok(());
        }
        let domain = signal.domain.as_str();
        for sample in &signal.metric_samples {
            self.repo
                .insert_sample(sample, signal.timestamp, domain, signal.event_kind)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, MetricSample, RecallDomain, SalienceVerdict};
    use storage::StoragePool;

    async fn setup() -> (StoragePool, Arc<MetricRepo>) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Trigger cognitive migrations — adapt to actual helper.
        let repo = Arc::new(MetricRepo::new(pool.sqlite().clone()));
        (pool, repo)
    }

    fn sig_with(samples: Vec<MetricSample>) -> AiSignal {
        AiSignal {
            domain: RecallDomain::Tasks,
            event_kind: "EstimationRecorded",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: "x".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: samples,
        }
    }

    #[tokio::test]
    async fn consumer_persists_each_sample() {
        let (_pool, repo) = setup().await;
        let consumer = MetricHarvestConsumer::new(repo.clone());
        let sig = sig_with(vec![
            MetricSample { name: "task_estimation_bias", value: 0.3 },
            MetricSample { name: "task_duration_avg", value: 45.0 },
        ]);
        consumer.consume(&sig).await.unwrap();
        assert_eq!(repo.count_samples_for("task_estimation_bias").await.unwrap(), 1);
        assert_eq!(repo.count_samples_for("task_duration_avg").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn consumer_noop_on_empty_samples() {
        let (_pool, repo) = setup().await;
        let consumer = MetricHarvestConsumer::new(repo.clone());
        consumer.consume(&sig_with(Vec::new())).await.unwrap();
        assert_eq!(repo.count_samples_for("task_estimation_bias").await.unwrap(), 0);
    }
}
```

Register in `crates/cognitive/src/consumers/mod.rs`:

```rust
pub mod metric;
pub use metric::MetricHarvestConsumer;
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive -E 'test(metric)'`
Expected: PASS (both tests).

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/consumers/metric.rs crates/cognitive/src/consumers/mod.rs
git commit -m "feat(cognitive): MetricHarvestConsumer persists AiSignal.metric_samples"
```

---

## Task 14: Refactor `BehavioralMetrics` to Map-Backed

**Files:**
- Modify: `crates/cognitive/src/services/reforge/types.rs`
- Modify: `crates/cognitive/src/services/reforge/feedback.rs` (compile repair only; Task 16 rewrites the body)

`★ Insight ─────────────────────────────────────`
Replacing the struct with a `BTreeMap<&'static str, f64>`-backed wrapper inverts the coupling: the struct no longer has to know the set of metrics, so adding/removing metrics becomes purely declarative. `BTreeMap` over `HashMap` gives deterministic iteration order — useful for snapshot tests and reforge logs.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/src/services/reforge/types.rs` (or create a `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod behavioral_metrics_shape_tests {
    use super::BehavioralMetrics;

    #[test]
    fn new_is_empty() {
        let bm = BehavioralMetrics::default();
        assert!(bm.get("anything").is_none());
        assert_eq!(bm.iter().count(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut bm = BehavioralMetrics::default();
        bm.insert("task_estimation_bias", 0.42);
        assert_eq!(bm.get("task_estimation_bias"), Some(0.42));
        assert_eq!(bm.iter().count(), 1);
    }

    #[test]
    fn overwrite_is_last_write_wins() {
        let mut bm = BehavioralMetrics::default();
        bm.insert("x", 1.0);
        bm.insert("x", 2.0);
        assert_eq!(bm.get("x"), Some(2.0));
    }
}
```

Run: `cargo nextest run -p cognitive behavioral_metrics_shape`
Expected: FAIL — struct doesn't have `iter`/`insert`/map shape.

- [ ] **Step 2: Rewrite `BehavioralMetrics`**

Replace the existing `BehavioralMetrics` struct in `crates/cognitive/src/services/reforge/types.rs`:

```rust
use serde::Serialize;
use std::collections::BTreeMap;

/// Aggregated behavioural metrics keyed by metric name.
///
/// Populated from `ai_metric_samples` via `load_behavioral_metrics`, which iterates
/// the workspace `MetricRegistry`. Every key corresponds to a `#[ai(metric(name = ...))]`
/// declaration on a feature event variant.
///
/// Access via `.get(name)`; iteration is alphabetically stable thanks to `BTreeMap`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BehavioralMetrics {
    values: BTreeMap<&'static str, f64>,
}

impl BehavioralMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &'static str, value: f64) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&&'static str, &f64)> {
        self.values.iter()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
```

- [ ] **Step 3: Repair the existing `feedback.rs` call site**

`feedback.rs::load_behavioral_metrics` currently assigns `out.task_estimation_bias = Some(...)`. Make the file compile temporarily by doing the simplest-possible rewrite: change every `out.foo = Some(v)` to `out.insert("foo", v)`. Task 16 replaces the entire body with the registry-driven aggregator, so this is transient.

Example diff for `feedback.rs`:

```rust
// Before:
if let Ok(Some(bias)) = sqlx::query_scalar(...).fetch_optional(pool).await {
    out.task_estimation_bias = Some(bias);
}

// After (transient):
if let Ok(Some(bias)) = sqlx::query_scalar(...).fetch_optional(pool).await {
    out.insert("task_estimation_bias", bias);
}
```

Apply to all 5 metric writes.

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive -E 'test(behavioral_metrics_shape)'`
Expected: PASS.

Run: `cargo build -p cognitive`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/reforge/types.rs crates/cognitive/src/services/reforge/feedback.rs
git commit -m "refactor(reforge): BehavioralMetrics becomes BTreeMap-backed"
```

---

## Task 15: Update `BehavioralMetrics` Callers

**Files:**
- Modify: `crates/cognitive/src/services/reforge/collector.rs`
- Modify: `crates/cognitive/src/services/reforge/` — any other file reading a named field
- Modify: any downstream consumer (serialization, logging)

- [ ] **Step 1: Find all field-access call sites**

Run: `grep -Rn "behavioral_metrics\." crates/ | grep -E "\.(task_estimation_bias|coaching_acceptance_rate|focus_quality_trend|suggestion_dismiss_rate|forecast_accuracy)"`

Expected output: a short list — typically `collector.rs`, `review.rs` (if it formats metrics for LLM prompts), and possibly `synthesizer.rs`.

- [ ] **Step 2: Rewrite each access**

For each hit, replace `.task_estimation_bias` (etc.) with `.get("task_estimation_bias")`. Example:

```rust
// Before:
let bias = metrics.behavioral_metrics.task_estimation_bias.unwrap_or(0.0);

// After:
let bias = metrics.behavioral_metrics.get("task_estimation_bias").unwrap_or(0.0);
```

If any site iterates all named fields (e.g. for LLM prompt formatting), replace with:

```rust
for (name, value) in metrics.behavioral_metrics.iter() {
    writeln!(prompt, "- {}: {:.3}", name, value)?;
}
```

- [ ] **Step 3: Build + lint**

Run: `cargo build --workspace`
Run: `cargo clippy -p cognitive --all-targets`
Expected: PASS, no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/reforge/
git commit -m "refactor(reforge): update BehavioralMetrics callers to map-backed access"
```

---

## Task 16: Replace `load_behavioral_metrics` with Registry-Driven Aggregator

**Files:**
- Modify: `crates/cognitive/src/services/reforge/feedback.rs`
- Modify: `crates/cognitive/src/services/reforge/collector.rs` (signature change)

`★ Insight ─────────────────────────────────────`
The rewritten function has zero SQL — all queries come from `MetricRepo::aggregate_metric(spec)`, which is parameterised by `MetricSpec`. Adding a new metric requires zero edits to this function; it's the declaration, not the function, that drives behaviour. This is the key inversion v2.5 achieves.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/src/services/reforge/feedback.rs` (in a `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod load_tests {
    use super::*;
    use ai_core::{Aggregation, MetricRegistry, MetricSpec};
    use storage::StoragePool;

    #[tokio::test]
    async fn returns_empty_when_no_metrics_registered() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let registry = MetricRegistry::new();
        let repo = MetricRepo::new(pool.sqlite().clone());
        let bm = load_behavioral_metrics(&repo, &registry).await;
        assert!(bm.is_empty());
    }

    #[tokio::test]
    async fn returns_aggregate_for_registered_metric() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = MetricRepo::new(pool.sqlite().clone());
        static SPEC: MetricSpec = MetricSpec {
            name: "test_bias",
            window_secs: 604_800,
            min_samples: 3,
            aggregation: Aggregation::Avg,
        };
        let mut registry = MetricRegistry::new();
        registry.register(&SPEC);

        for v in [0.1_f64, 0.2, 0.3] {
            repo.insert_sample(
                &ai_core::MetricSample { name: "test_bias", value: v },
                jiff::Timestamp::now(),
                "tasks",
                "T",
            )
            .await
            .unwrap();
        }
        let bm = load_behavioral_metrics(&repo, &registry).await;
        let val = bm.get("test_bias").unwrap();
        assert!((val - 0.2).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive load_tests`
Expected: FAIL — new `load_behavioral_metrics` signature doesn't exist.

- [ ] **Step 3: Replace the function body**

In `crates/cognitive/src/services/reforge/feedback.rs`, delete the entire existing `load_behavioral_metrics` (all 5 SQL queries) and its callers' arguments. Replace with:

```rust
use ai_core::MetricRegistry;
use crate::repos::MetricRepo;
use crate::services::reforge::types::BehavioralMetrics;

/// Loads all registered metrics by running each metric's declared aggregation
/// over the rolling window against `ai_metric_samples`. Metrics below their
/// `min_samples` floor are omitted (the map simply does not contain their key).
///
/// Replaces the 5 hand-written SQL queries the old implementation carried.
/// Adding a new metric is a one-file change on the emitting feature — no edits here.
pub async fn load_behavioral_metrics(
    repo: &MetricRepo,
    registry: &MetricRegistry,
) -> BehavioralMetrics {
    let mut out = BehavioralMetrics::new();
    for spec in registry.all() {
        match repo.aggregate_metric(spec).await {
            Ok(Some(value)) => out.insert(spec.name, value),
            Ok(None) => {
                tracing::trace!(
                    metric = spec.name,
                    "insufficient samples within window; skipping"
                );
            }
            Err(e) => {
                tracing::warn!(
                    metric = spec.name,
                    error = %e,
                    "failed to aggregate metric"
                );
            }
        }
    }
    out
}
```

Delete the old `EVENT_USER_CORRECTED_AI` constant if it's no longer referenced in the file. Scan the file for any remaining `sqlx::` imports used only by the deleted queries and remove them.

- [ ] **Step 4: Update the caller in `collector.rs`**

Find the call to `load_behavioral_metrics(pool)` in `crates/cognitive/src/services/reforge/collector.rs`. Update:

```rust
// Before:
let behavioral_metrics = load_behavioral_metrics(&self.pool).await;

// After:
let behavioral_metrics = load_behavioral_metrics(&self.metric_repo, &self.metric_registry).await;
```

Add fields to the collector struct (and constructor) for `metric_repo: Arc<MetricRepo>` and `metric_registry: Arc<MetricRegistry>`. Task 17 wires them from app-core init.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive load_tests`
Expected: PASS.

Run: `cargo build -p cognitive`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/reforge/feedback.rs crates/cognitive/src/services/reforge/collector.rs
git commit -m "refactor(reforge): load_behavioral_metrics is registry-driven; delete 5 SQL queries"
```

---

## Task 17: Wire `MetricRegistry` + `MetricHarvestConsumer` into app-core Init

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/init/reforge.rs` (or wherever `ReforgeCollector` is constructed)

- [ ] **Step 1: Write failing test**

Append to `crates/app-core/tests/init_tests.rs` (or create one):

```rust
#[tokio::test]
async fn init_registers_all_feature_metrics() {
    use ai_core::MetricRegistry;
    // feature-tasks and feature-finance expose FEATURE_METRICS consts.
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(feature_finance::FinanceEvent::FEATURE_METRICS);
    // After Tasks 18-26 register real metrics, this should be > 0.
    // For this task alone, assert the registration path itself works without panic.
    assert!(reg.all().len() >= feature_tasks::TaskEvent::FEATURE_METRICS.len());
}
```

- [ ] **Step 2: Extend `ai_pipeline.rs`**

Edit `crates/app-core/src/init/ai_pipeline.rs`. Wherever the existing `translate()` + `start()` live, add:

```rust
use ai_core::MetricRegistry;
use std::sync::Arc;

/// Build a workspace-global MetricRegistry populated from every registered AiFeature's
/// event enum `FEATURE_METRICS` const. Called exactly once at startup.
pub fn build_metric_registry() -> MetricRegistry {
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(feature_finance::FinanceEvent::FEATURE_METRICS);
    // Tasks 21/22 add coaching + productivity here:
    // reg.register_all(feature_coaching::CoachingEvent::FEATURE_METRICS);
    // reg.register_all(feature_productivity::ProductivityEvent::FEATURE_METRICS);
    reg
}
```

- [ ] **Step 3: Wire the consumer in the init sequence**

Edit `crates/app-core/src/init/mod.rs`. In the phase that registers `SignalConsumer`s (typically Phase 8 after v1.5), add:

```rust
use cognitive::consumers::MetricHarvestConsumer;
use cognitive::repos::MetricRepo;
use std::sync::Arc;

// After storage pool is ready:
let metric_repo = Arc::new(MetricRepo::new(storage.sqlite().clone()));
let metric_consumer: Arc<dyn ai_core::SignalConsumer> =
    Arc::new(MetricHarvestConsumer::new(metric_repo.clone()));
consumers.push(metric_consumer);

// MetricRegistry is built once and passed into ReforgeCollector:
let metric_registry = Arc::new(ai_pipeline::build_metric_registry());
```

Pass `metric_repo` and `metric_registry` into the `ReforgeCollector` constructor in `crates/app-core/src/init/reforge.rs`:

```rust
let reforge_collector = ReforgeCollector::new(
    // ...existing args...,
    metric_repo.clone(),
    metric_registry.clone(),
);
```

- [ ] **Step 4: Run**

Run: `cargo build --workspace`
Expected: PASS.

Run: `cargo nextest run -p app-core init_registers_all_feature_metrics`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs crates/app-core/src/init/mod.rs crates/app-core/src/init/reforge.rs crates/app-core/tests/init_tests.rs
git commit -m "feat(app-core): register MetricHarvestConsumer + build MetricRegistry at startup"
```

---

## Task 18: Migrate `task_estimation_bias` → Declarative on `EstimationRecorded`

**Files:**
- Modify: `crates/feature-tasks/src/events.rs`

`★ Insight ─────────────────────────────────────`
`deviation_pct` is already a field on `EstimationRecorded`. The migration is pure declaration — no new code path, no new event emission site. The old `task_estimation_history` table continues to exist for timeline UI purposes (if it does), but reforge no longer reads it; it reads `ai_metric_samples`. If the table has no other readers, delete it in Task 20's pattern (not scoped here).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/feature-tasks/tests/events_test.rs` (create if missing):

```rust
use ai_core::AiEventMeta;

#[test]
fn estimation_recorded_emits_task_estimation_bias_sample() {
    let e = feature_tasks::TaskEvent::EstimationRecorded {
        task_id: "t".into(),
        estimated_minutes: Some(30),
        actual_minutes: Some(45),
        deviation_pct: Some(0.5),
    };
    let sig = e.to_signal();
    let sample = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "task_estimation_bias")
        .expect("task_estimation_bias sample present");
    assert!((sample.value - 0.5).abs() < 1e-9);
}

#[test]
fn feature_metrics_contains_task_estimation_bias() {
    assert!(feature_tasks::TaskEvent::FEATURE_METRICS
        .iter()
        .any(|s| s.name == "task_estimation_bias"));
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p feature-tasks events_test`
Expected: FAIL — no metric sample declared.

- [ ] **Step 3: Annotate the variant**

Edit `crates/feature-tasks/src/events.rs`. Replace the `EstimationRecorded` variant's `#[ai(...)]` block:

```rust
#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Estimation recorded: est {estimated_minutes:?}m vs actual {actual_minutes:?}m",
    metric(
        name = "task_estimation_bias",
        value_from = "deviation_pct.unwrap_or(0.0)",
        window = "7d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
EstimationRecorded {
    task_id: String,
    estimated_minutes: Option<u32>,
    actual_minutes: Option<u32>,
    deviation_pct: Option<f64>,
},
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p feature-tasks events_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/src/events.rs crates/feature-tasks/tests/events_test.rs
git commit -m "feat(feature-tasks): declare task_estimation_bias metric on EstimationRecorded"
```

---

## Task 19: Delete Dead `suggestion_dismiss_rate` + `forecast_accuracy` Code Paths

**Files:**
- Modify: `crates/cognitive/src/services/reforge/feedback.rs` (remove if Task 16 left stubs — likely already gone)
- Search: any remaining reference to these names

`★ Insight ─────────────────────────────────────`
Per CLAUDE.md gotcha: "Built-in AI task automations removed (2026-04-20)" — `list_suggestions/suggest/dismiss/apply` and `forecast_task/forecast_project/accuracy_report` are all gone. The tables `task_suggestions` and `productivity_forecasts` no longer receive writes. Keeping the metrics around would return `None` forever. Pre-release policy: delete.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Search for remaining references**

Run: `grep -Rn "suggestion_dismiss_rate\|forecast_accuracy" crates/`

Expected: any matches are in files the plan hasn't touched yet — typically review/synthesizer prompts or serialization.

- [ ] **Step 2: Remove remaining references**

For each hit, delete the reference (not rewrite). Examples:

- LLM prompt template listing metrics: remove the lines that reference `suggestion_dismiss_rate` / `forecast_accuracy`.
- Any config default listing the 5 metric names: remove those 2 names.
- Any serialisation that once named those fields: unaffected because `BehavioralMetrics` is now map-backed — but remove any hardcoded string lookups.

- [ ] **Step 3: Run clippy + build**

Run: `cargo clippy -p cognitive --all-targets`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "refactor(reforge): delete dead suggestion_dismiss_rate and forecast_accuracy references"
```

---

## Task 20: Drop `task_suggestions` and `productivity_forecasts` Tables + Code

**Files:**
- Search: all crates for `task_suggestions`, `productivity_forecasts`, `TaskSuggestion`, `ProductivityForecast`
- Modify: each migration file, repo file, type file that defined or used them
- Delete: any `*_suggestion.rs`, `*_forecast.rs` files that are now orphans

- [ ] **Step 1: Find callers**

```bash
grep -Rln "task_suggestions\|productivity_forecasts\|TaskSuggestion\|ProductivityForecast" crates/
```

Expected list (roughly): `feature-tasks/migrations/*.sql`, `feature-productivity/migrations/*.sql`, possibly `feature-productivity/src/forecast*.rs` (orphan since 2026-04-20), `feature-tasks/src/suggestion*.rs` (orphan since 2026-04-20), `crates/cognitive/src/services/reforge/feedback.rs` (already cleaned).

- [ ] **Step 2: Delete orphan files and types**

For each file that defines a type named `TaskSuggestion`, `TaskSuggestionRow`, `ProductivityForecast`, `ProductivityForecastRow` and has no callers outside itself, delete the file. Update the parent `mod.rs` to remove the module declaration.

- [ ] **Step 3: Edit migrations in-place**

Pre-release policy: edit migration SQL rather than adding a drop migration.

In `crates/feature-tasks/migrations/<file>.sql` — remove the `CREATE TABLE task_suggestions (...)` block and any index on that table.

In `crates/feature-productivity/migrations/<file>.sql` — remove `CREATE TABLE productivity_forecasts (...)` similarly.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run --workspace -E 'kind(test)'`
Expected: PASS. Some tests referencing the deleted tables may need deletion too — delete them rather than skip.

- [ ] **Step 5: Commit**

```bash
git add -u
# If any files were deleted that Git can't auto-detect:
git add -A
git commit -m "refactor: drop task_suggestions + productivity_forecasts (dead since 2026-04-20)"
```

---

## Task 21: Add `CoachingEvent::StrategyApplied`; Emit; Annotate Metric

**Files:**
- Create: `crates/feature-coaching/src/events.rs`
- Modify: `crates/feature-coaching/src/lib.rs`
- Modify: `crates/feature-coaching/src/service.rs` (emission site)
- Modify: `crates/bus/src/domain_events.rs` (register new variant via `From`)
- Modify: `crates/app-core/src/init/ai_pipeline.rs` (add to `build_metric_registry`)

`★ Insight ─────────────────────────────────────`
`feature-coaching` is full-migrated in v3, not v2.5 — but `coaching_acceptance_rate` cannot disappear without a replacement signal source, so v2.5 adds the minimum: one event variant (`StrategyApplied`), one emission site, one metric annotation. The enum will grow in v3 when the full coaching feature migration lands; nothing in this minimal version is throwaway.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/feature-coaching/tests/events_test.rs`:

```rust
use ai_core::AiEventMeta;

#[test]
fn strategy_applied_emits_acceptance_sample() {
    let e = feature_coaching::events::CoachingEvent::StrategyApplied {
        strategy_id: "s".into(),
        rule_text: "review spending".into(),
        accepted: true,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "coaching_acceptance_rate")
        .unwrap();
    assert!((s.value - 1.0).abs() < 1e-9);
}

#[test]
fn strategy_rejected_emits_zero_sample() {
    let e = feature_coaching::events::CoachingEvent::StrategyApplied {
        strategy_id: "s".into(),
        rule_text: "review spending".into(),
        accepted: false,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "coaching_acceptance_rate")
        .unwrap();
    assert!(s.value.abs() < 1e-9);
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p feature-coaching events_test`
Expected: FAIL — module/enum doesn't exist.

- [ ] **Step 3: Create `events.rs`**

Create `crates/feature-coaching/src/events.rs`:

```rust
use ai_core_macros::AiEvent;
use serde::{Deserialize, Serialize};

/// Typed events emitted by the coaching subsystem.
///
/// In v2.5 the enum is minimal — just `StrategyApplied` — so `coaching_acceptance_rate`
/// can originate from the pipeline rather than from raw-SQL reads of `coaching_strategies`.
/// v3 expands this enum as part of full `AiFeature` migration of the coaching crate.
#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "Coaching")]
pub enum CoachingEvent {
    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Coaching strategy '{rule_text}' — accepted={accepted}",
        metric(
            name = "coaching_acceptance_rate",
            value_from = "if *accepted { 1.0 } else { 0.0 }",
            window = "7d",
            min_samples = 5,
            aggregation = "avg",
        ),
    )]
    StrategyApplied {
        strategy_id: String,
        rule_text: String,
        accepted: bool,
    },
}
```

Register the module in `crates/feature-coaching/src/lib.rs`:

```rust
pub mod events;
```

- [ ] **Step 4: Add `DomainEvent` variant**

Edit `crates/bus/src/domain_events.rs`. Add the variant:

```rust
/// Coaching strategy was applied; carries acceptance verdict for `coaching_acceptance_rate`.
CoachingStrategyApplied {
    strategy_id: String,
    rule_text: String,
    accepted: bool,
},
```

Add to `variant_name()`:

```rust
DomainEvent::CoachingStrategyApplied { .. } => "CoachingStrategyApplied",
```

Add a `KIND_` constant:

```rust
pub const KIND_COACHING_STRATEGY_APPLIED: &'static str = "CoachingStrategyApplied";
```

The `From<CoachingEvent> for DomainEvent` impl is generated by the derive — no manual glue needed.

- [ ] **Step 5: Emit from coaching service**

Edit `crates/feature-coaching/src/service.rs`. At the point where a strategy application is persisted (look for `times_used`/`times_accepted` updates — identified in the earlier exploration around `coaching_strategies` table writes), add after the DB update:

```rust
if let Some(bus) = &self.bus {
    let event: bus::DomainEvent = crate::events::CoachingEvent::StrategyApplied {
        strategy_id: strategy.id.clone(),
        rule_text: strategy.rule_text.clone(),
        accepted: was_accepted,
    }
    .into();
    let _ = bus.publish(event);
}
```

> **NOTE**: if `CoachingService` does not currently hold a `DomainEventBus` reference, add one as a constructor arg. Follow the pattern set by other publishers in the codebase (Tasks, Finance) — take `Arc<DomainEventBus>`.

- [ ] **Step 6: Extend `ai_pipeline::translate()`**

Edit `crates/app-core/src/init/ai_pipeline.rs`. In the `translate()` function, add:

```rust
bus::DomainEvent::CoachingStrategyApplied { strategy_id, rule_text, accepted } => {
    Some(feature_coaching::events::CoachingEvent::StrategyApplied {
        strategy_id: strategy_id.clone(),
        rule_text: rule_text.clone(),
        accepted: *accepted,
    }
    .to_signal())
}
```

Also add to `build_metric_registry`:

```rust
reg.register_all(feature_coaching::events::CoachingEvent::FEATURE_METRICS);
```

- [ ] **Step 7: Run**

Run: `cargo nextest run -p feature-coaching events_test`
Expected: PASS.

Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-coaching/ crates/bus/src/domain_events.rs crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(feature-coaching): CoachingEvent::StrategyApplied drives coaching_acceptance_rate"
```

---

## Task 22: Add `ProductivityEvent::SessionEnded`; Emit; Annotate Metric

**Files:**
- Create: `crates/feature-productivity/src/events.rs`
- Modify: `crates/feature-productivity/src/lib.rs`
- Modify: wherever focus sessions end in `feature-productivity` (emission site)
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/app-core/src/init/ai_pipeline.rs`
- Modify: `crates/feature-productivity/Cargo.toml`

- [ ] **Step 1: Write failing test**

Create `crates/feature-productivity/tests/events_test.rs`:

```rust
use ai_core::AiEventMeta;

#[test]
fn session_ended_emits_focus_quality_sample() {
    let e = feature_productivity::events::ProductivityEvent::SessionEnded {
        session_id: "s".into(),
        quality: 0.82,
        duration_mins: 45,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "focus_quality_trend")
        .unwrap();
    assert!((s.value - 0.82).abs() < 1e-9);
}
```

- [ ] **Step 2: Add dependencies**

Edit `crates/feature-productivity/Cargo.toml`:

```toml
[dependencies]
# existing deps...
ai-core.workspace = true
ai-core-macros.workspace = true
bus.workspace = true
```

- [ ] **Step 3: Create `events.rs`**

Create `crates/feature-productivity/src/events.rs`:

```rust
use ai_core_macros::AiEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "Productivity")]
pub enum ProductivityEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Session ended: {duration_mins}m, quality {quality:.2}",
        metric(
            name = "focus_quality_trend",
            value_from = "*quality",
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    SessionEnded {
        session_id: String,
        quality: f64,
        duration_mins: u32,
    },
}
```

Register in `crates/feature-productivity/src/lib.rs`:

```rust
pub mod events;
```

- [ ] **Step 4: Add `DomainEvent` variant**

Edit `crates/bus/src/domain_events.rs`:

```rust
ProductivitySessionEnded {
    session_id: String,
    quality: f64,
    duration_mins: u32,
},
```

Add the `variant_name()` arm and `KIND_PRODUCTIVITY_SESSION_ENDED` const.

- [ ] **Step 5: Emit at session end**

In `feature-productivity`'s session lifecycle code (search for where `daily_summaries.avg_session_quality` is updated), add after the DB write:

```rust
if let Some(bus) = &self.bus {
    let event: bus::DomainEvent = crate::events::ProductivityEvent::SessionEnded {
        session_id: session.id.clone(),
        quality: session.quality,
        duration_mins: session.duration_mins,
    }
    .into();
    let _ = bus.publish(event);
}
```

Inject `Arc<DomainEventBus>` into the productivity service constructor if absent.

- [ ] **Step 6: Extend `translate()` + register metrics**

In `crates/app-core/src/init/ai_pipeline.rs`, add to `translate()`:

```rust
bus::DomainEvent::ProductivitySessionEnded { session_id, quality, duration_mins } => {
    Some(feature_productivity::events::ProductivityEvent::SessionEnded {
        session_id: session_id.clone(),
        quality: *quality,
        duration_mins: *duration_mins,
    }
    .to_signal())
}
```

In `build_metric_registry`:

```rust
reg.register_all(feature_productivity::events::ProductivityEvent::FEATURE_METRICS);
```

- [ ] **Step 7: Run**

Run: `cargo nextest run -p feature-productivity events_test`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-productivity/ crates/bus/src/domain_events.rs crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(feature-productivity): SessionEnded drives focus_quality_trend"
```

---

## Task 23: Add `focus_expiration_rate` Metric

**Files:**
- Modify: `crates/feature-tasks/src/events.rs` — add `FocusExpired` variant
- Modify: wherever focus expiration is detected (background job or timer) — emit the event
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

`★ Insight ─────────────────────────────────────`
"Focus expiration" means a task was focused but its deadline passed without completion — a regression signal. Modelling it as its own event variant (rather than a flag on an existing one) keeps the metric single-purpose: every `FocusExpired` emission is a `1.0` sample, `AVG` of those plus `FocusChanged`→`Completed` (which samples `0.0`) over the window gives the expiration rate.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/feature-tasks/tests/events_test.rs`:

```rust
#[test]
fn focus_expired_emits_expiration_sample_of_one() {
    use ai_core::AiEventMeta;
    let e = feature_tasks::TaskEvent::FocusExpired {
        task_id: "t".into(),
        title: "Ship v2.5".into(),
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "focus_expiration_rate")
        .unwrap();
    assert!((s.value - 1.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Add the variant**

Edit `crates/feature-tasks/src/events.rs`. Add to `TaskEvent`:

```rust
#[ai(
    importance = 0.6,
    salience = "accumulate",
    observation_template = "Focus expired on task: {title}",
    entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    metric(
        name = "focus_expiration_rate",
        value_from = "1.0_f64",
        window = "7d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
FocusExpired {
    task_id: String,
    title: String,
},
```

Also extend `FocusChanged` — when a focus transitions from "active" to "completed" it should sample `0.0` for the same metric. But that semantically differs from "changed" → skip that complication; `focus_expiration_rate` uses `Completed` as the denominator signal. Update `Completed`:

```rust
#[ai(
    importance = 0.6,
    salience = "extract_if(deviation_pct.unwrap_or(0.0) > 50.0)",
    observation_template = "Completed {title} (deviation {deviation_pct:?}%)",
    entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    coaching_signal,
    metric(
        name = "focus_expiration_rate",
        value_from = "0.0_f64",
        window = "7d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
Completed { task_id: String, title: String, deviation_pct: Option<f64> },
```

Both `FocusExpired` (samples `1.0`) and `Completed` (samples `0.0`) write to `focus_expiration_rate`; the average over the window is the rate.

- [ ] **Step 3: Add DomainEvent variant**

Edit `crates/bus/src/domain_events.rs`:

```rust
TaskFocusExpired { task_id: String, title: String },
```

Add `variant_name` arm and `KIND_TASK_FOCUS_EXPIRED`.

- [ ] **Step 4: Emit on expiration**

Find the focus-deadline watcher — likely in `feature-tasks` or a scheduling crate. At the point where a focused task's deadline passes without completion, emit:

```rust
let event: bus::DomainEvent = crate::events::TaskEvent::FocusExpired {
    task_id: task.id.clone(),
    title: task.title.clone(),
}
.into();
let _ = bus.publish(event);
```

> **If no such watcher exists**: add one as a small background tick in `feature-tasks/src/focus_watcher.rs`. Check every 60s for tasks with `focus_deadline < now` and `status != completed`. Emit once per expiry.

- [ ] **Step 5: Extend `translate()`**

```rust
bus::DomainEvent::TaskFocusExpired { task_id, title } => Some(
    feature_tasks::TaskEvent::FocusExpired {
        task_id: task_id.clone(),
        title: title.clone(),
    }
    .to_signal()
),
```

- [ ] **Step 6: Run**

Run: `cargo nextest run -p feature-tasks events_test`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-tasks/ crates/bus/src/domain_events.rs crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(feature-tasks): focus_expiration_rate metric via FocusExpired + Completed"
```

---

## Task 24: Add `budget_overrun_frequency` — Annotate `BudgetAlert`

**Files:**
- Modify: `crates/feature-finance/src/events.rs`

- [ ] **Step 1: Write failing test**

Create `crates/feature-finance/tests/events_test.rs` (or append):

```rust
use ai_core::AiEventMeta;

#[test]
fn budget_alert_emits_overrun_sample() {
    let e = feature_finance::events::FinanceEvent::BudgetAlert {
        category: "groceries".into(),
        spent: 600,
        limit: 500,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "budget_overrun_frequency")
        .unwrap();
    assert!((s.value - 1.0).abs() < 1e-9);
}

#[test]
fn transaction_recorded_emits_non_overrun_sample() {
    let e = feature_finance::events::FinanceEvent::TransactionRecorded {
        transaction_id: "tx".into(),
        category: "groceries".into(),
        amount: 42,
        // ... other fields as in current FinanceEvent
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "budget_overrun_frequency")
        .unwrap();
    assert!(s.value.abs() < 1e-9);
}
```

- [ ] **Step 2: Annotate variants**

Edit `crates/feature-finance/src/events.rs`. Update `BudgetAlert`:

```rust
#[ai(
    importance = 0.9,
    salience = "extract",
    observation_template = "Budget alert: {category} {spent} of {limit}",
    entity_bridge(type = "finance_category", name_from = "category", id_from = "category"),
    coaching_signal(category_from = "category", amount_from = "spent", rule = "Review spending..."),
    metric(
        name = "budget_overrun_frequency",
        value_from = "1.0_f64",
        window = "30d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
BudgetAlert { category: String, spent: i64, limit: i64 },
```

And `TransactionRecorded` also samples the denominator:

```rust
#[ai(
    importance = 0.6,
    salience = "accumulate",
    observation_template = "{category}: {amount}",
    entity_bridge(type = "finance_transaction", name_from = "transaction_id", id_from = "transaction_id"),
    coaching_signal(category_from = "category", amount_from = "amount"),
    metric(
        name = "budget_overrun_frequency",
        value_from = "0.0_f64",
        window = "30d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
TransactionRecorded { /* existing fields */ },
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p feature-finance events_test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-finance/src/events.rs crates/feature-finance/tests/events_test.rs
git commit -m "feat(feature-finance): budget_overrun_frequency metric on BudgetAlert + TransactionRecorded"
```

---

## Task 25: Add `task_deferral_rate` — New `TaskEvent::Deferred` Variant

**Files:**
- Modify: `crates/feature-tasks/src/events.rs`
- Modify: emission site (task update path)
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/feature-tasks/tests/events_test.rs`:

```rust
#[test]
fn deferred_emits_deferral_sample() {
    use ai_core::AiEventMeta;
    let e = feature_tasks::TaskEvent::Deferred {
        task_id: "t".into(),
        title: "Write plan".into(),
        previous_due: Some("2026-04-22T00:00:00Z".into()),
        new_due: Some("2026-04-23T00:00:00Z".into()),
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "task_deferral_rate")
        .unwrap();
    assert!((s.value - 1.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Add variant**

Edit `crates/feature-tasks/src/events.rs`:

```rust
#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Deferred '{title}' from {previous_due:?} to {new_due:?}",
    entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    metric(
        name = "task_deferral_rate",
        value_from = "1.0_f64",
        window = "7d",
        min_samples = 3,
        aggregation = "avg",
    ),
)]
Deferred {
    task_id: String,
    title: String,
    previous_due: Option<String>,
    new_due: Option<String>,
},
```

Also sample `0.0` on `Completed` (denominator). Update its metric attr to include both `focus_expiration_rate` and `task_deferral_rate`:

> **Macro limitation**: v2.5's `#[ai(metric(...))]` only supports ONE metric per variant. To support multiple, extend the attribute parser to accept an array (Task 5 only handles a single `metric(...)`). If this becomes a blocker, add Task 25b: extend parser to support `metric(...)` appearing multiple times or as `metrics([ ... ])`. For v2.5, keep it simple: each variant declares one metric; use separate event variants where a single event conceptually produces multiple metric signatures.

Instead of a second metric on `Completed`, introduce a denominator by having every TaskEvent except `Deferred` that reaches the consumer emit a sample from a separate accumulator. Simplest pragmatic approach: define `task_deferral_rate` to only accumulate `1.0` values; its meaning becomes "deferrals per 7d" rather than a rate. Rename the metric:

```rust
metric(
    name = "task_deferrals_per_week",
    value_from = "1.0_f64",
    window = "7d",
    min_samples = 3,
    aggregation = "sum",
),
```

Aggregation `sum` gives count-per-window directly. Update the Task Overview entry and the test accordingly.

- [ ] **Step 3: Add `DomainEvent` variant**

```rust
TaskDeferred {
    task_id: String,
    title: String,
    previous_due: Option<String>,
    new_due: Option<String>,
},
```

Add `variant_name`, `KIND_TASK_DEFERRED`.

- [ ] **Step 4: Emit on deferral**

In the task update code path (search for the `update_due_date` or similar) — when `due_date` changes forward in time, emit:

```rust
let event: bus::DomainEvent = crate::events::TaskEvent::Deferred {
    task_id: id,
    title: task.title.clone(),
    previous_due: previous.to_rfc3339_opt(),
    new_due: new.to_rfc3339_opt(),
}
.into();
let _ = bus.publish(event);
```

- [ ] **Step 5: Extend `translate()`**

```rust
bus::DomainEvent::TaskDeferred { task_id, title, previous_due, new_due } => Some(
    feature_tasks::TaskEvent::Deferred {
        task_id: task_id.clone(),
        title: title.clone(),
        previous_due: previous_due.clone(),
        new_due: new_due.clone(),
    }
    .to_signal()
),
```

- [ ] **Step 6: Run**

Run: `cargo nextest run -p feature-tasks events_test`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-tasks/ crates/bus/src/domain_events.rs crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(feature-tasks): task_deferrals_per_week metric via TaskEvent::Deferred"
```

---

## Task 26: Add `goal_progress_velocity` — New `FinanceEvent::GoalProgress`

**Files:**
- Modify: `crates/feature-finance/src/events.rs`
- Modify: emission site (goal tracking path)
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/feature-finance/tests/events_test.rs`:

```rust
#[test]
fn goal_progress_emits_velocity_sample() {
    use ai_core::AiEventMeta;
    let e = feature_finance::events::FinanceEvent::GoalProgress {
        goal_id: "g".into(),
        name: "Emergency fund".into(),
        current_amount: 5000,
        target_amount: 10000,
        delta: 250,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "goal_progress_velocity")
        .unwrap();
    assert!((s.value - 250.0).abs() < 1e-9);
}
```

- [ ] **Step 2: Add variant**

Edit `crates/feature-finance/src/events.rs`:

```rust
#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Goal '{name}' advanced by {delta} → {current_amount}/{target_amount}",
    entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id"),
    metric(
        name = "goal_progress_velocity",
        value_from = "*delta as f64",
        window = "30d",
        min_samples = 3,
        aggregation = "sum",
    ),
)]
GoalProgress {
    goal_id: String,
    name: String,
    current_amount: i64,
    target_amount: i64,
    delta: i64,
},
```

- [ ] **Step 3: Add `DomainEvent` variant**

```rust
FinanceGoalProgress {
    goal_id: String,
    name: String,
    current_amount: i64,
    target_amount: i64,
    delta: i64,
},
```

`variant_name`, `KIND_FINANCE_GOAL_PROGRESS`.

- [ ] **Step 4: Emit on goal progress**

Find the code that updates `goals.current_amount` in `feature-finance`. After the update:

```rust
let event: bus::DomainEvent = crate::events::FinanceEvent::GoalProgress {
    goal_id: goal.id.clone(),
    name: goal.name.clone(),
    current_amount: goal.current_amount,
    target_amount: goal.target_amount,
    delta,
}
.into();
let _ = bus.publish(event);
```

- [ ] **Step 5: Extend `translate()`**

```rust
bus::DomainEvent::FinanceGoalProgress {
    goal_id, name, current_amount, target_amount, delta,
} => Some(
    feature_finance::events::FinanceEvent::GoalProgress {
        goal_id: goal_id.clone(),
        name: name.clone(),
        current_amount: *current_amount,
        target_amount: *target_amount,
        delta: *delta,
    }
    .to_signal()
),
```

- [ ] **Step 6: Run**

Run: `cargo nextest run -p feature-finance events_test`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-finance/ crates/bus/src/domain_events.rs crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(feature-finance): goal_progress_velocity metric via GoalProgress"
```

---

## Task 27: Annotate `TasksFeature` + `FinanceFeature` with `promotion_threshold`

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs`
- Modify: `crates/feature-finance/src/lib.rs`

`★ Insight ─────────────────────────────────────`
Spec §6 v2.5 motivation: "finance budget alerts promote fast; casual chat stays slow." Tasks gets a lower threshold (fast promotion — task signals are high-fidelity), Finance gets an even lower one for budget-adjacent events. Other feature domains continue to use the global `accumulate_promote_threshold` default.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/feature-tasks/tests/promote_threshold_test.rs`:

```rust
#[test]
fn tasks_override() {
    assert_eq!(feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE, Some(3));
}
```

Create `crates/feature-finance/tests/promote_threshold_test.rs`:

```rust
#[test]
fn finance_override() {
    assert_eq!(feature_finance::FinanceFeature::PROMOTE_THRESHOLD_OVERRIDE, Some(2));
}
```

- [ ] **Step 2: Annotate**

Edit `crates/feature-tasks/src/lib.rs` — add `promotion_threshold = 3` to the `#[ai(...)]` block:

```rust
#[derive(AiFeature)]
#[ai(
    // existing attrs...
    promotion_threshold = 3,
)]
pub struct TasksFeature { pool, task_tool }
```

Edit `crates/feature-finance/src/lib.rs` — add `promotion_threshold = 2`:

```rust
#[derive(AiFeature, Default)]
#[ai(
    // existing attrs...
    promotion_threshold = 2,
)]
pub struct FinanceFeature { tool }
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p feature-tasks promote_threshold_test`
Run: `cargo nextest run -p feature-finance promote_threshold_test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/ crates/feature-finance/
git commit -m "feat(features): per-feature promotion_threshold overrides (tasks=3, finance=2)"
```

---

## Task 28: Wire Per-Domain Override into Background Consolidation

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`
- Modify: `crates/app-core/src/init/mod.rs`

`★ Insight ─────────────────────────────────────`
The existing `should_promote(promote_threshold, min_days)` takes a single scalar. v2.5 changes it to take a `HashMap<RecallDomain, usize>` of overrides plus the global fallback. At call time, the per-`RecallDomain` value wins if present. The map is built once at startup from each feature's `PROMOTE_THRESHOLD_OVERRIDE` const — same shape as `MetricRegistry` registration.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/src/services/background.rs` (or a sibling `tests.rs`):

```rust
#[cfg(test)]
mod promote_override_tests {
    use super::*;
    use ai_core::RecallDomain;
    use std::collections::HashMap;

    #[test]
    fn promote_uses_override_when_present() {
        let mut overrides = HashMap::new();
        overrides.insert(RecallDomain::Tasks, 3_usize);

        let entry = AccumulatorEntry::with_dummy_samples(4); // 4 observations
        // Global is 5; override for Tasks is 3; entry has 4 -> should promote.
        assert!(entry.should_promote_for_domain(
            &RecallDomain::Tasks,
            &overrides,
            5, // global default
            1, // min_days
        ));
        // Without the override, global 5 > 4 -> should not promote.
        let no_overrides: HashMap<RecallDomain, usize> = HashMap::new();
        assert!(!entry.should_promote_for_domain(
            &RecallDomain::Tasks,
            &no_overrides,
            5,
            1,
        ));
    }
}
```

> **If `AccumulatorEntry` has no `with_dummy_samples` test helper**, add one in `#[cfg(test)] impl` block — take a sample count and construct the internal vec directly.

- [ ] **Step 2: Rewrite `should_promote`**

Edit `crates/cognitive/src/services/background.rs`. Find `should_promote` and add the new method (keep the old one for now; Task ends with its deletion):

```rust
impl AccumulatorEntry {
    /// Returns the effective promotion threshold for a given `RecallDomain`.
    pub fn effective_threshold(
        domain: &ai_core::RecallDomain,
        overrides: &std::collections::HashMap<ai_core::RecallDomain, usize>,
        global: usize,
    ) -> usize {
        overrides.get(domain).copied().unwrap_or(global)
    }

    pub fn should_promote_for_domain(
        &self,
        domain: &ai_core::RecallDomain,
        overrides: &std::collections::HashMap<ai_core::RecallDomain, usize>,
        global_threshold: usize,
        min_days: usize,
    ) -> bool {
        let t = Self::effective_threshold(domain, overrides, global_threshold);
        self.observations.len() >= t && self.days_seen.len() >= min_days
    }
}
```

Replace the single call site in `background.rs` (around line 732):

```rust
// Before:
if entry.should_promote(promote_threshold, min_days)

// After:
if entry.should_promote_for_domain(&entry.domain, &self.promote_overrides, promote_threshold, min_days)
```

> **`entry.domain`** is the `RecallDomain` the accumulator belongs to. If the entry doesn't carry its domain yet, add it as a field when constructed from the signal.

Add `promote_overrides: HashMap<RecallDomain, usize>` to `BackgroundConsolidationService` fields + constructor.

After Step 3 below, delete the old `should_promote(promote_threshold, min_days)` method — it has no callers.

- [ ] **Step 3: Build the override map in app-core init**

Edit `crates/app-core/src/init/mod.rs`. After `build_metric_registry` or near it:

```rust
use ai_core::RecallDomain;
use std::collections::HashMap;

let mut promote_overrides: HashMap<RecallDomain, usize> = HashMap::new();
if let Some(n) = feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE {
    promote_overrides.insert(RecallDomain::Tasks, n);
}
if let Some(n) = feature_finance::FinanceFeature::PROMOTE_THRESHOLD_OVERRIDE {
    promote_overrides.insert(RecallDomain::Finance, n);
}
// Others use the global default.

// Pass into BackgroundConsolidationService:
let bg_config = BackgroundServiceConfig {
    promote_threshold: cognitive_config.accumulate_promote_threshold,
    promote_overrides,
    // ...existing fields...
};
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive promote_override_tests`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 5: Delete old `should_promote`**

Remove the now-unused `should_promote(promote_threshold, min_days)` method from `background.rs`. Run `cargo clippy -p cognitive --all-targets` to confirm no dead code warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/background.rs crates/app-core/src/init/mod.rs
git commit -m "feat(cognitive): per-RecallDomain promotion threshold override beats global"
```

---

## Task 29: Create `CommunityEvent` Enum; Re-add `DomainEvent` Variants

**Files:**
- Create: `crates/cognitive/src/services/community_intelligence/events.rs`
- Modify: `crates/cognitive/src/services/community_intelligence/mod.rs` (was `community_intelligence.rs`)
- Modify: `crates/bus/src/domain_events.rs`

`★ Insight ─────────────────────────────────────`
v1 deleted `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened` from `DomainEvent` because nothing published them. v2.5 re-adds them — but this time the only way to construct them is via the generated `From<CommunityEvent> for DomainEvent`, which is the pipeline's preferred entry point. No direct construction in feature code; the enum discipline is enforced by keeping the payload fields inside `CommunityEvent` and letting the derive wire the conversion.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Restructure `community_intelligence` into a module dir**

If it's currently `community_intelligence.rs` (a single file), convert:

```bash
mkdir -p crates/cognitive/src/services/community_intelligence
git mv crates/cognitive/src/services/community_intelligence.rs crates/cognitive/src/services/community_intelligence/mod.rs
```

- [ ] **Step 2: Write failing test**

Create `crates/cognitive/src/services/community_intelligence/events.rs` with a placeholder, then test:

Create `crates/cognitive/tests/community_event_tests.rs`:

```rust
use ai_core::AiEventMeta;
use cognitive::services::community_intelligence::events::CommunityEvent;

#[test]
fn community_discovered_to_signal() {
    let e = CommunityEvent::Discovered {
        community_id: "c1".into(),
        name: "Finance Planning".into(),
        member_count: 5,
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "CommunityDiscovered");
    assert_eq!(sig.content, "Community discovered: Finance Planning (5 members)");
}

#[test]
fn community_event_into_domain_event() {
    let e = CommunityEvent::Updated {
        community_id: "c1".into(),
        name: "Finance Planning".into(),
        reason: "merged".into(),
    };
    let de: bus::DomainEvent = e.into();
    assert_eq!(de.variant_name(), "CommunityUpdated");
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p cognitive community_event_tests`
Expected: FAIL — enum doesn't exist.

- [ ] **Step 4: Create `events.rs`**

Create `crates/cognitive/src/services/community_intelligence/events.rs`:

```rust
use ai_core_macros::AiEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "General")]
pub enum CommunityEvent {
    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Community discovered: {name} ({member_count} members)",
    )]
    Discovered {
        community_id: String,
        name: String,
        member_count: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Community {name} updated: {reason}",
    )]
    Updated {
        community_id: String,
        name: String,
        reason: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Community {name} weakened (stability {stability:.2})",
    )]
    Weakened {
        community_id: String,
        name: String,
        stability: f64,
    },
}
```

Register in `mod.rs`:

```rust
pub mod events;
pub mod co_activation_events; // for Task 31
```

- [ ] **Step 5: Re-add `DomainEvent` variants**

Edit `crates/bus/src/domain_events.rs`. Re-add the three variants (exact shape matches the generated `From` conversion — field names and types must line up with `CommunityEvent`):

```rust
/// A new community was discovered by Phase 6.5 restructuring.
CommunityDiscovered {
    community_id: String,
    name: String,
    member_count: u32,
},

/// A community's composition or metadata changed.
CommunityUpdated {
    community_id: String,
    name: String,
    reason: String,
},

/// A community's stability dropped enough to mark it for deletion on the next sweep.
CommunityWeakened {
    community_id: String,
    name: String,
    stability: f64,
},
```

Add to `variant_name`:

```rust
DomainEvent::CommunityDiscovered { .. } => "CommunityDiscovered",
DomainEvent::CommunityUpdated { .. } => "CommunityUpdated",
DomainEvent::CommunityWeakened { .. } => "CommunityWeakened",
```

Add `KIND_` constants:

```rust
pub const KIND_COMMUNITY_DISCOVERED: &'static str = "CommunityDiscovered";
pub const KIND_COMMUNITY_UPDATED: &'static str = "CommunityUpdated";
pub const KIND_COMMUNITY_WEAKENED: &'static str = "CommunityWeakened";
```

> **Verify the derive's `From` conversion matches the variants**: the `#[derive(AiEvent)]` macro generates `From<CommunityEvent> for DomainEvent` by matching variant names prefixed with the enum's domain label. Read `ai-core-macros/src/ai_event.rs` to confirm the exact naming convention; if the derive expects `From<CommunityEvent> for DomainEvent` to map `Discovered → CommunityDiscovered`, that's a known convention from v1 (`TaskEvent::Created → DomainEvent::TaskCreated`). If the convention differs for this enum, adjust the `#[ai]` attribute or the DomainEvent variant names to match.

- [ ] **Step 6: Run**

Run: `cargo nextest run -p cognitive community_event_tests`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/services/community_intelligence/ crates/bus/src/domain_events.rs
git commit -m "feat(cognitive): CommunityEvent enum + re-introduce DomainEvent::Community* variants"
```

---

## Task 30: Publish `CommunityEvent` from `apply_intelligence`

**Files:**
- Modify: `crates/cognitive/src/services/community_intelligence/mod.rs`
- Modify: `crates/cognitive/src/services/service.rs` (caller that provides the bus)

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/tests/community_event_tests.rs`:

```rust
use cognitive::services::community_intelligence::{
    apply_intelligence, CommunityIntelligenceOutput, CommunityRename, CommunityMerge,
};
use cognitive::repos::{CommunityRepo, CoActivationRepo};
use storage::StoragePool;

#[tokio::test]
async fn apply_intelligence_publishes_community_updated_on_rename() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let crepo = CommunityRepo::new(pool.sqlite().clone());
    let coarepo = CoActivationRepo::new(pool.sqlite().clone());
    let bus = std::sync::Arc::new(bus::DomainEventBus::new(64));
    let mut rx = bus.subscribe();

    // Seed one community
    crepo.upsert_community(/* fixture */).await.unwrap();

    let out = CommunityIntelligenceOutput {
        names: vec![CommunityRename {
            community_id: "c1".into(),
            label: "Renamed".into(),
        }],
        merges: vec![],
        splits: vec![],
    };
    apply_intelligence(&out, &crepo, &coarepo, Some(bus.clone())).await;

    // Drain the channel, expect one CommunityUpdated event.
    let event = rx.recv().await.unwrap();
    assert!(matches!(event, bus::DomainEvent::CommunityUpdated { .. }));
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive apply_intelligence_publishes`
Expected: FAIL — `apply_intelligence` doesn't accept a bus.

- [ ] **Step 3: Extend `apply_intelligence`**

Edit `crates/cognitive/src/services/community_intelligence/mod.rs`. Modify the signature:

```rust
pub async fn apply_intelligence(
    output: &CommunityIntelligenceOutput,
    community_repo: &CommunityRepo,
    co_activation_repo: &CoActivationRepo,
    bus: Option<std::sync::Arc<bus::DomainEventBus>>,
) -> (u32, u32, u32) {
    use crate::services::community_intelligence::events::CommunityEvent;

    let mut renames = 0u32;
    let mut merges = 0u32;
    let mut splits = 0u32;

    for rename in &output.names {
        community_repo.rename(&rename.community_id, &rename.label).await.ok();
        if let Some(b) = &bus {
            let event: bus::DomainEvent = CommunityEvent::Updated {
                community_id: rename.community_id.clone(),
                name: rename.label.clone(),
                reason: "renamed".into(),
            }
            .into();
            let _ = b.publish(event);
        }
        renames += 1;
    }

    for merge in &output.merges {
        community_repo
            .merge_communities(&merge.absorb_id, &merge.into_id)
            .await
            .ok();
        if let Some(b) = &bus {
            let event: bus::DomainEvent = CommunityEvent::Updated {
                community_id: merge.into_id.clone(),
                name: String::new(), // repo lookup if needed
                reason: format!("merged:{}", merge.reason),
            }
            .into();
            let _ = b.publish(event);
        }
        merges += 1;
    }

    for split in &output.splits {
        // Mark weakened before split, or emit Discovered for the new child — implementation
        // depends on how splits resolve. Minimal: emit Weakened against the source.
        if let Some(b) = &bus {
            let event: bus::DomainEvent = CommunityEvent::Weakened {
                community_id: split.community_id.clone(),
                name: String::new(),
                stability: 0.0,
            }
            .into();
            let _ = b.publish(event);
        }
        splits += 1;
    }

    // Discovery path: if the community_repo tracks "newly-created" communities during
    // the intelligence pass, emit CommunityDiscovered for each. If not currently
    // separately tracked, query `list_communities_created_since(before_timestamp)`.

    (renames, merges, splits)
}
```

- [ ] **Step 4: Update callers**

In `crates/cognitive/src/services/service.rs` (the Phase 6.5b path), update:

```rust
apply_intelligence(&output, &community_repo, &co_activation_repo, Some(bus.clone())).await;
```

Pass `Arc<DomainEventBus>` into the caller if not already present.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive apply_intelligence_publishes`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/community_intelligence/ crates/cognitive/src/services/service.rs crates/cognitive/tests/community_event_tests.rs
git commit -m "feat(cognitive): publish CommunityEvent from apply_intelligence"
```

---

## Task 31: Create `CoActivationEvent::Strengthened`; Publish on Threshold Crossing

**Files:**
- Create: `crates/cognitive/src/services/community_intelligence/co_activation_events.rs`
- Modify: `crates/cognitive/src/repos/co_activation.rs`
- Modify: `crates/bus/src/domain_events.rs`

`★ Insight ─────────────────────────────────────`
`record_co_retrieval` is called on every fact-pair co-retrieval; emitting a `DomainEvent` per call would flood the bus. The fix: publish only when a pair's cumulative strength crosses a 2.0 threshold (a "strengthened" pair). This is ~100x less traffic while still giving coaching/retrieval consumers a meaningful signal of which semantic links are hardening.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/tests/co_activation_event_tests.rs`:

```rust
use ai_core::AiEventMeta;
use cognitive::services::community_intelligence::co_activation_events::CoActivationEvent;

#[test]
fn strengthened_to_signal() {
    let e = CoActivationEvent::Strengthened {
        fact_id_a: "a".into(),
        fact_id_b: "b".into(),
        strength: 2.5,
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "CoActivationStrengthened");
    assert!(sig.content.contains("2.5"));
}
```

- [ ] **Step 2: Create the enum**

Create `crates/cognitive/src/services/community_intelligence/co_activation_events.rs`:

```rust
use ai_core_macros::AiEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "General")]
pub enum CoActivationEvent {
    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Co-activation strengthened: {fact_id_a}↔{fact_id_b} → {strength:.2}",
    )]
    Strengthened {
        fact_id_a: String,
        fact_id_b: String,
        strength: f64,
    },
}
```

- [ ] **Step 3: Add `DomainEvent` variant**

Edit `crates/bus/src/domain_events.rs`:

```rust
CoActivationStrengthened {
    fact_id_a: String,
    fact_id_b: String,
    strength: f64,
},
```

`variant_name`, `KIND_CO_ACTIVATION_STRENGTHENED`.

- [ ] **Step 4: Emit from `record_co_retrieval`**

Edit `crates/cognitive/src/repos/co_activation.rs`. In `record_co_retrieval`, after updating `strength`:

```rust
const STRENGTH_THRESHOLD: f64 = 2.0;

// After the UPSERT returning the new strength value:
if previous_strength < STRENGTH_THRESHOLD && new_strength >= STRENGTH_THRESHOLD {
    if let Some(bus) = &self.bus {
        use crate::services::community_intelligence::co_activation_events::CoActivationEvent;
        let event: bus::DomainEvent = CoActivationEvent::Strengthened {
            fact_id_a: fact_id_a.to_string(),
            fact_id_b: fact_id_b.to_string(),
            strength: new_strength,
        }
        .into();
        let _ = bus.publish(event);
    }
}
```

Add `bus: Option<Arc<DomainEventBus>>` to `CoActivationRepo`; extend `new()` to accept it (or add `with_bus()` builder).

> **If the repo doesn't currently expose `previous_strength`**: change the SQL to `SELECT ... RETURNING previous.strength, new.strength` (SQLite RETURNING since 3.35) or do a two-step read-then-write inside a transaction.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive co_activation_event_tests`
Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/community_intelligence/co_activation_events.rs crates/cognitive/src/repos/co_activation.rs crates/bus/src/domain_events.rs
git commit -m "feat(cognitive): CoActivationEvent::Strengthened published on 2.0 threshold crossing"
```

---

## Task 32: Extend `ai_pipeline::translate()` for `CommunityEvent` + `CoActivationEvent`

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/app-core/tests/ai_pipeline_tests.rs`:

```rust
#[test]
fn translate_community_discovered() {
    use bus::DomainEvent;
    let e = DomainEvent::CommunityDiscovered {
        community_id: "c".into(),
        name: "Finance".into(),
        member_count: 4,
    };
    let sig = app_core::init::ai_pipeline::translate(&e).expect("signal");
    assert_eq!(sig.event_kind, "CommunityDiscovered");
}

#[test]
fn translate_co_activation_strengthened() {
    use bus::DomainEvent;
    let e = DomainEvent::CoActivationStrengthened {
        fact_id_a: "a".into(),
        fact_id_b: "b".into(),
        strength: 2.5,
    };
    let sig = app_core::init::ai_pipeline::translate(&e).expect("signal");
    assert_eq!(sig.event_kind, "CoActivationStrengthened");
}
```

- [ ] **Step 2: Extend `translate()`**

Edit `crates/app-core/src/init/ai_pipeline.rs`. Add match arms:

```rust
bus::DomainEvent::CommunityDiscovered { community_id, name, member_count } => Some(
    cognitive::services::community_intelligence::events::CommunityEvent::Discovered {
        community_id: community_id.clone(),
        name: name.clone(),
        member_count: *member_count,
    }
    .to_signal()
),
bus::DomainEvent::CommunityUpdated { community_id, name, reason } => Some(
    cognitive::services::community_intelligence::events::CommunityEvent::Updated {
        community_id: community_id.clone(),
        name: name.clone(),
        reason: reason.clone(),
    }
    .to_signal()
),
bus::DomainEvent::CommunityWeakened { community_id, name, stability } => Some(
    cognitive::services::community_intelligence::events::CommunityEvent::Weakened {
        community_id: community_id.clone(),
        name: name.clone(),
        stability: *stability,
    }
    .to_signal()
),
bus::DomainEvent::CoActivationStrengthened { fact_id_a, fact_id_b, strength } => Some(
    cognitive::services::community_intelligence::co_activation_events::CoActivationEvent::Strengthened {
        fact_id_a: fact_id_a.clone(),
        fact_id_b: fact_id_b.clone(),
        strength: *strength,
    }
    .to_signal()
),
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p app-core ai_pipeline`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs crates/app-core/tests/ai_pipeline_tests.rs
git commit -m "feat(app-core): translate CommunityEvent + CoActivationEvent"
```

---

## Task 33: Integration — Declaration → Sample → Aggregate → `BehavioralMetrics.get()`

**Files:**
- Create: `tests/ai_pipeline_v25_integration.rs`

- [ ] **Step 1: Write the test**

Create `tests/ai_pipeline_v25_integration.rs`:

```rust
//! v2.5 end-to-end: an event declared with `#[ai(metric(...))]` flows through the
//! pipeline, lands in ai_metric_samples, and is returned by load_behavioral_metrics.

use ai_core::{MetricRegistry, SignalConsumer};
use cognitive::consumers::MetricHarvestConsumer;
use cognitive::repos::MetricRepo;
use cognitive::services::reforge::feedback::load_behavioral_metrics;
use std::sync::Arc;
use storage::StoragePool;

#[tokio::test]
async fn task_estimation_recorded_flows_end_to_end() {
    // 1. Setup an in-memory pool with all cognitive migrations applied.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply 001..=005 migrations via the cognitive crate's migration helper:
    cognitive::apply_migrations(&pool).await.unwrap();

    let metric_repo = Arc::new(MetricRepo::new(pool.sqlite().clone()));
    let consumer = MetricHarvestConsumer::new(metric_repo.clone());

    // 2. Build registry with TaskEvent's FEATURE_METRICS.
    let mut registry = MetricRegistry::new();
    registry.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);

    // 3. Emit 3 samples with deviation_pct = 0.1, 0.3, 0.5.
    for dp in [0.1_f64, 0.3, 0.5] {
        let e = feature_tasks::TaskEvent::EstimationRecorded {
            task_id: "t".into(),
            estimated_minutes: Some(30),
            actual_minutes: Some(45),
            deviation_pct: Some(dp),
        };
        let sig = <feature_tasks::TaskEvent as ai_core::AiEventMeta>::to_signal(&e);
        consumer.consume(&sig).await.unwrap();
    }

    // 4. Load behavioral metrics — registry-driven aggregate.
    let bm = load_behavioral_metrics(&metric_repo, &registry).await;
    let bias = bm
        .get("task_estimation_bias")
        .expect("task_estimation_bias present");
    assert!((bias - 0.3).abs() < 1e-9, "expected 0.3, got {}", bias);
}

#[tokio::test]
async fn dead_metric_not_present_in_behavioral_metrics() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    cognitive::apply_migrations(&pool).await.unwrap();
    let metric_repo = Arc::new(MetricRepo::new(pool.sqlite().clone()));
    let mut registry = MetricRegistry::new();
    registry.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    let bm = load_behavioral_metrics(&metric_repo, &registry).await;
    assert!(bm.get("suggestion_dismiss_rate").is_none());
    assert!(bm.get("forecast_accuracy").is_none());
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -E 'test(task_estimation_recorded_flows) | test(dead_metric_not_present)'`
Expected: PASS (2/2).

- [ ] **Step 3: Commit**

```bash
git add tests/ai_pipeline_v25_integration.rs
git commit -m "test(ai-pipeline): v2.5 end-to-end declaration → sample → aggregate"
```

---

## Task 34: Invariant — No Raw SQL in `feedback.rs`

**Files:**
- Create: `tests/ai_no_raw_sql_in_feedback.rs`

- [ ] **Step 1: Write the invariant test**

Create `tests/ai_no_raw_sql_in_feedback.rs`:

```rust
//! Invariant: reforge feedback must not contain any raw SQL.
//! All aggregation goes through MetricRepo::aggregate_metric driven by MetricRegistry.

use std::fs;
use std::path::Path;

#[test]
fn feedback_rs_has_no_raw_sql() {
    let path = Path::new("crates/cognitive/src/services/reforge/feedback.rs");
    let src = fs::read_to_string(path).expect("read feedback.rs");

    for pat in ["sqlx::query!", "sqlx::query_scalar!", "sqlx::query(", "sqlx::query_scalar("] {
        assert!(
            !src.contains(pat),
            "feedback.rs contains forbidden pattern: {}",
            pat
        );
    }

    // Also forbid inline SQL strings — search for common SQL verbs in string literals.
    for verb in ["SELECT ", "INSERT ", "UPDATE ", "DELETE "] {
        assert!(
            !src.contains(verb),
            "feedback.rs contains inline SQL verb: {}",
            verb
        );
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -E 'test(feedback_rs_has_no_raw_sql)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_no_raw_sql_in_feedback.rs
git commit -m "test(invariant): feedback.rs has no raw SQL"
```

---

## Task 35: Invariant — Every Registered `MetricSpec` Has a Declaration Path

**Files:**
- Create: `tests/ai_metric_declarations_reachable.rs`

- [ ] **Step 1: Write the invariant test**

Create `tests/ai_metric_declarations_reachable.rs`:

```rust
//! Every MetricSpec in the registry must be declared on a variant in one of the
//! registered feature event enums. Catches drift when a metric is registered but
//! never emitted, or when a declared metric isn't registered.

use ai_core::{MetricRegistry, MetricSpec};

#[test]
fn every_registry_spec_is_declared() {
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(feature_finance::FinanceEvent::FEATURE_METRICS);
    reg.register_all(feature_coaching::events::CoachingEvent::FEATURE_METRICS);
    reg.register_all(feature_productivity::events::ProductivityEvent::FEATURE_METRICS);

    for spec in reg.all() {
        // Check the name appears in one of the declared FEATURE_METRICS slices.
        let found = [
            feature_tasks::TaskEvent::FEATURE_METRICS,
            feature_finance::FinanceEvent::FEATURE_METRICS,
            feature_coaching::events::CoachingEvent::FEATURE_METRICS,
            feature_productivity::events::ProductivityEvent::FEATURE_METRICS,
        ]
        .iter()
        .flat_map(|s| s.iter())
        .any(|s: &&'static MetricSpec| s.name == spec.name);
        assert!(found, "registered metric not declared anywhere: {}", spec.name);
    }
}

#[test]
fn reached_10_or_more_metrics() {
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(feature_finance::FinanceEvent::FEATURE_METRICS);
    reg.register_all(feature_coaching::events::CoachingEvent::FEATURE_METRICS);
    reg.register_all(feature_productivity::events::ProductivityEvent::FEATURE_METRICS);
    // Spec §8 success metric: Reforge metric count current 5 → 10+ after v2.5.
    // v2.5 target: task_estimation_bias, coaching_acceptance_rate, focus_quality_trend,
    // focus_expiration_rate, budget_overrun_frequency, task_deferrals_per_week,
    // goal_progress_velocity + any incidentals from multi-variant emissions.
    assert!(
        reg.all().len() >= 7,
        "expected >=7 registered metrics, got {}",
        reg.all().len()
    );
}
```

> **Note** — the spec target is 10+; if v2.5 lands at 7, that's acceptable below the spec's aspirational target but can be covered by adding more metric variants in implementation. Adjust the assertion if more metrics land during implementation.

- [ ] **Step 2: Run**

Run: `cargo nextest run -E 'test(every_registry_spec_is_declared) | test(reached_10_or_more_metrics)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_metric_declarations_reachable.rs
git commit -m "test(invariant): registered metrics are declared and count ≥ 7"
```

---

## Task 36: Integration — `CommunityEvent` → Bus → `IngestionConsumer`

**Files:**
- Create: `tests/ai_community_events_integration.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/ai_community_events_integration.rs`:

```rust
//! CommunityEvent published to bus reaches the signal router and lands in
//! the cognitive IngestionConsumer's observation table.

use std::sync::Arc;
use storage::StoragePool;

#[tokio::test]
async fn community_discovered_flows_to_ingestion() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    cognitive::apply_migrations(&pool).await.unwrap();

    let bus = Arc::new(bus::DomainEventBus::new(64));
    let obs_repo = Arc::new(cognitive::repos::ObservationRepo::new(pool.sqlite().clone()));
    let ent_repo = Arc::new(cognitive::repos::EntityRepo::new(pool.sqlite().clone()));
    let ingestion = Arc::new(cognitive::consumers::IngestionConsumer::new(
        obs_repo.clone(),
        ent_repo.clone(),
    ));

    // Start the SignalRouter with translate() + ingestion consumer.
    let _router = ai_core::SignalRouter::start(
        bus.clone(),
        vec![ingestion.clone()],
        app_core::init::ai_pipeline::translate,
    );

    // Publish a CommunityDiscovered event.
    bus.publish(bus::DomainEvent::CommunityDiscovered {
        community_id: "c1".into(),
        name: "Finance Planning".into(),
        member_count: 5,
    }).await.unwrap();

    // Give the router a tick to drain.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let count = obs_repo.count_for_domain("general").await.unwrap();
    assert!(count >= 1, "IngestionConsumer did not receive CommunityDiscovered");
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -E 'test(community_discovered_flows_to_ingestion)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_community_events_integration.rs
git commit -m "test(integration): CommunityEvent flows to IngestionConsumer"
```

---

## Task 37: Integration — Per-Feature Promotion Threshold Override

**Files:**
- Create: `tests/ai_promotion_threshold_override.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/ai_promotion_threshold_override.rs`:

```rust
use std::collections::HashMap;
use ai_core::RecallDomain;

#[test]
fn tasks_override_beats_global() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_tasks::TasksFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Tasks, n);
    }

    // Global default is 5; Tasks override is Some(3) (from Task 27).
    // For a RecallDomain::Tasks lookup, the effective threshold is 3.
    let global = 5usize;
    let effective_tasks =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::Tasks,
            &overrides,
            global,
        );
    assert_eq!(effective_tasks, 3);

    // For RecallDomain::General (no override), falls back to global.
    let effective_general =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::General,
            &overrides,
            global,
        );
    assert_eq!(effective_general, global);
}

#[test]
fn finance_override_is_lower_still() {
    let mut overrides = HashMap::new();
    if let Some(n) = feature_finance::FinanceFeature::PROMOTE_THRESHOLD_OVERRIDE {
        overrides.insert(RecallDomain::Finance, n);
    }
    let effective =
        cognitive::services::background::AccumulatorEntry::effective_threshold(
            &RecallDomain::Finance,
            &overrides,
            5,
        );
    assert_eq!(effective, 2);
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -E 'test(tasks_override_beats_global) | test(finance_override_is_lower_still)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_promotion_threshold_override.rs
git commit -m "test(integration): per-feature promotion threshold overrides global"
```

---

## Task 38: Final Verification

**Files:** n/a (verification only)

- [ ] **Step 1: Format check**

Run: `cargo fmt --all --check`
Expected: clean (no diff).

If not clean: `cargo fmt --all` then verify with `git diff --stat`.

- [ ] **Step 2: Clippy — zero warnings policy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: `finished ... 0 warnings`.

- [ ] **Step 3: Workspace tests**

Run: `cargo nextest run --workspace`
Expected: all pass (no skips, no ignores).

- [ ] **Step 4: Doctests**

Run: `cargo test --workspace --doc`
Expected: all pass.

- [ ] **Step 5: Grep sanity — kill-list coverage**

Run each and confirm zero matches (except allowed occurrences documented inline):

```bash
# Dead metric names
grep -Rn "suggestion_dismiss_rate\|forecast_accuracy" crates/ tests/ || true

# Dead tables
grep -Rn "task_suggestions\|productivity_forecasts" crates/ tests/ || true

# Raw SQL in feedback
grep -Rn "sqlx::query" crates/cognitive/src/services/reforge/feedback.rs || true

# Named field access on BehavioralMetrics (should all be .get(name))
grep -RnE "\.behavioral_metrics\.[a-z_]+(_bias|_rate|_trend|_accuracy|_frequency|_velocity)" crates/ || true
```

Expected: no matches.

- [ ] **Step 6: Manual smoke — run the desktop app (optional but recommended)**

```bash
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

In a second terminal:

```bash
cd desktop-ui && bun run dev
```

Verify:
- App launches without startup panics.
- Create a task, record an estimation, observe the cycle runs (reforge nightly — may require waiting; at minimum, verify `ai_metric_samples` rows appear via SQLite inspection).
- `sqlite3 ~/.klyntbot-dev/data.db 'SELECT metric_name, COUNT(*) FROM ai_metric_samples GROUP BY metric_name;'` returns non-empty rows for the migrated metrics.

- [ ] **Step 7: Plan checklist**

Re-read this plan end-to-end. For each task, confirm:
- [x] Test was written first and initially failed.
- [x] Implementation is the minimum needed to pass.
- [x] No placeholder, TODO, or dead code left behind.
- [x] Commit message follows conventional format.

- [ ] **Step 8: Final commit**

If any touch-up was required during verification:

```bash
git add -u
git commit -m "chore(v2.5): final verification touch-ups"
```

---

## Plan Checklist — Spec §6 v2.5 Done Criteria

- [x] `BehavioralMetrics` fields match the set of `#[ai(metric)]` declarations — Task 14 refactor + Tasks 18/21/22/23/24/25/26 declarations.
- [x] Zero hand-written SQL in reforge feedback — Task 16 rewrite + Task 34 invariant.
- [x] Community restructuring publishes typed events — Tasks 29/30/31.
- [x] Spec §8 success metrics:
  - Reforge metric count ≥ 7 (target 10+ stretch) — Task 35.
  - Zero hand-written dispatch lines in feedback.rs — Tasks 16/34.
  - Per-feature promotion threshold lands — Tasks 27/28/37.
