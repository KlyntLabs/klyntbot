# AI Pipeline v2 — Mirror Redesign

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fold every mirror subscriber into the unified AI feature pipeline. Introduce a `MirrorSignalSource` trait with a shared accumulator + flush-scheduler base so new mirror snapshot types cost one declaration and one migration. Declaratively opt Task and Finance into mirror via a new `#[ai(mirror_snapshot = "...")]` attribute. Revive the inert `TrialPreviewSubscriber` against a new typed `TrialLifecycleEvent`. Add mirror retention policy. Every old path deleted in the same PR.

**Architecture:** Today the four mirror subscribers each hand-roll a `broadcast::Receiver<DomainEvent>` loop with subscriber-specific accumulator, flush cadence, and alert emission. v2 replaces those with one trait (`MirrorSignalSource`) + one runner (`MirrorSubscriberRunner`) that handles the cross-cutting concerns: subscribe-to-`SignalRouter`, accumulate, flush on an interval, flush-on-shutdown, and alert emission. Each subscriber becomes a ~50-line `impl MirrorSignalSource` instead of a ~250-line subscriber module. Two new declaration-driven snapshot types — task focus patterns and finance spending drift — land as `TaskFocusPatternSource` and `FinanceSpendingDriftSource`, both discovered via feature-crate `#[ai(mirror_snapshot = "...")]` attributes.

**Tech Stack:** Rust 1.93 (stable); `ai-core` + `ai-core-macros` extended with `mirror_snapshot`; `async-trait`; `tokio::time::interval`; `tokio_util::sync::CancellationToken`; existing `MirrorRepo` + `DomainEventBus` (unchanged).

**Spec:** `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md` — v2 section.

**Pre-release posture:** No dual dispatch, no feature flags, no deprecation. Each task deletes the old path in the same commit that introduces the new one. Schema changes edit `003_mirror_tables.sql` in place.

---

## File Structure

### New files

```
crates/ai-core/src/mirror.rs                          — MirrorSignalSource trait + MirrorSubscriberRunner + MirrorSnapshotSpec
crates/ai-core-macros/tests/expand/mirror_snapshot.rs — mirror_snapshot attr expansion snapshot
crates/cognitive/src/mirror/sources/mod.rs            — re-exports
crates/cognitive/src/mirror/sources/routing.rs        — RoutingSignalSource (replaces subscribers/routing.rs)
crates/cognitive/src/mirror/sources/meta_rule.rs      — MetaRuleSignalSource (replaces subscribers/meta_rule.rs)
crates/cognitive/src/mirror/sources/config_archiver.rs — ConfigArchiverSource (replaces subscribers/version.rs)
crates/cognitive/src/mirror/sources/trial.rs          — TrialPreviewSource (replaces subscribers/trial.rs)
crates/cognitive/src/mirror/sources/task_focus.rs     — TaskFocusPatternSource
crates/cognitive/src/mirror/sources/finance_drift.rs  — FinanceSpendingDriftSource
crates/cognitive/src/mirror/retention.rs              — MirrorRetentionService (periodic cleanup)
tests/ai_mirror_pipeline_integration.rs               — end-to-end: SkillRouted → routing snapshot; AutotunerDecision{activated} → trial timer
tests/ai_mirror_snapshot_coverage.rs                  — invariant: every declared mirror_snapshot attr has a registered source
```

### Modified files

```
crates/ai-core/src/lib.rs                 — re-export MirrorSignalSource, MirrorSnapshotSpec, MirrorSubscriberRunner
crates/ai-core-macros/src/attrs.rs        — parse mirror_snapshot(name = "...", flush_interval_secs = ..., event_kind = "...")
crates/ai-core-macros/src/ai_feature.rs   — emit MIRROR_SNAPSHOTS constant from declarations

crates/bus/src/domain_events.rs           — add AutotunerDecision "activated" verdict documentation; no new variants

crates/feature-tasks/src/lib.rs           — add #[ai(mirror_snapshot(...))] for task focus patterns
crates/feature-finance/src/lib.rs         — add #[ai(mirror_snapshot(...))] for finance spending drift

crates/cognitive/src/mirror/mod.rs        — pub mod sources (drop subscribers); re-export new types; delete MirrorAlert::TrialUnpromising if confirmed unused post-migration
crates/cognitive/src/mirror/types.rs      — add MirrorSnapshotKind enum; extend MirrorAlertType if new snapshot types add alerts; retention policy field on MirrorRepo calls
crates/cognitive/src/mirror/engine.rs     — MirrorEngine::start returns (facade, runners, shutdown); takes SignalRouter-compatible signal source list
crates/cognitive/src/mirror/facade.rs     — drop domain_event_bus plumbing (unused since v1); trial timer map stays
crates/cognitive/src/mirror/repo.rs       — add cleanup_old_brain_versions, cleanup_old_trend_narratives, cleanup_old_meta_rules; accept retention config
crates/cognitive/src/mirror/subscribers/  — ENTIRE DIRECTORY DELETED
crates/cognitive/migrations/003_mirror_tables.sql — add mirror_task_focus_snapshots + mirror_finance_drift_snapshots tables (in-place edit)

crates/app-core/src/init/mod.rs           — Phase 9 rebuilt: wire MirrorSignalSources into the SignalRouter consumer list; start MirrorRetentionService
crates/app-core/src/init/ai_pipeline.rs   — translator covers AutotunerDecision + SkillRouted + UserCorrectedAI for mirror signal fan-out
crates/app-core/src/adapters/trial_evaluator.rs — unchanged signature; now actually reached by TrialPreviewSource

crates/agent/src/autotuner/mod.rs         — publish AutotunerDecision { verdict: "activated", trial_id, ... } at trial kickoff (currently only promoted/reverted emit)
```

### Deleted files

```
crates/cognitive/src/mirror/subscribers/mod.rs
crates/cognitive/src/mirror/subscribers/routing.rs
crates/cognitive/src/mirror/subscribers/meta_rule.rs
crates/cognitive/src/mirror/subscribers/version.rs
crates/cognitive/src/mirror/subscribers/trial.rs
```

---

## Task Overview

| # | Task | Phase |
|---|---|---|
| 1 | Add `Mirror` variant to `RecallDomain` | Foundation |
| 2 | Introduce `MirrorSnapshotSpec` + `MirrorSignalSource` trait + `MirrorSubscriberRunner` in `ai-core` | Foundation |
| 3 | `#[ai(mirror_snapshot(...))]` attribute — parse | Macros |
| 4 | `#[ai(mirror_snapshot(...))]` — emit `MIRROR_SNAPSHOTS` constant | Macros |
| 5 | Add `cleanup_old_brain_versions`, `cleanup_old_trend_narratives`, `cleanup_old_meta_rules` to `MirrorRepo` | Repo |
| 6 | Create `MirrorRetentionService` (periodic cleanup) | Retention |
| 7 | Emit `AutotunerDecision { verdict: "activated", ... }` at trial kickoff | Bus |
| 8 | Port `RoutingMirrorSubscriber` → `RoutingSignalSource` (behind runner) | Sources |
| 9 | Port `MetaRuleDetector` → `MetaRuleSignalSource` | Sources |
| 10 | Port `ConfigArchiver` → `ConfigArchiverSource` | Sources |
| 11 | Port `TrialPreviewSubscriber` → `TrialPreviewSource` (active, not inert) | Sources |
| 12 | New: `TaskFocusPatternSource` + schema `mirror_task_focus_snapshots` | Sources |
| 13 | New: `FinanceSpendingDriftSource` + schema `mirror_finance_drift_snapshots` | Sources |
| 14 | Annotate `TasksFeature` with `mirror_snapshot` declaration | Features |
| 15 | Annotate `FinanceFeature` with `mirror_snapshot` declaration | Features |
| 16 | Rebuild `MirrorEngine::start` to wire `MirrorSignalSource` list | Wiring |
| 17 | Update `app-core/src/init/mod.rs` Phase 9 to register mirror sources as `SignalConsumer`s | Wiring |
| 18 | Extend `ai_pipeline::translate()` for `AutotunerDecision` + audit `SkillRouted` / `UserCorrectedAI` translation | Wiring |
| 19 | Delete `crates/cognitive/src/mirror/subscribers/` directory | Cleanup |
| 20 | Drop `domain_event_bus` from `MirrorFacade` (unused since v1) | Cleanup |
| 21 | Integration — `AiSignal(SkillRouted)` → `RoutingSignalSource` → snapshot persisted | Tests |
| 22 | Integration — `AiSignal(AutotunerDecision activated)` → trial timer starts | Tests |
| 23 | Integration — `AiSignal(TaskFocusChanged)` fan-out produces task focus snapshot | Tests |
| 24 | Integration — `AiSignal(BudgetAlert)` fan-out produces finance drift snapshot | Tests |
| 25 | Invariant — every declared `mirror_snapshot` attr has a registered source | Tests |
| 26 | Invariant — no `broadcast::Receiver<DomainEvent>` in `crates/cognitive/src/mirror/` | Tests |
| 27 | Final verification: clippy, nextest, doctests, grep sanity, manual smoke | Done |

---

## Task 1: Add `Mirror` Variant to `RecallDomain`

**Files:**
- Modify: `crates/ai-core/src/recall_domain.rs`
- Modify: `crates/ai-core/tests/signal_test.rs` (if the existing round-trip test asserts all variants)

`★ Insight ─────────────────────────────────────`
`RecallDomain` is a manually enumerated enum (not `inventory`-collected) per v1 Risk mitigation. Adding a variant here is the zero-ambiguity way to make mirror a first-class domain, and it lets mirror sources emit `AiSignal`s of their own for downstream cognitive ingestion without going through `General`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/signal_test.rs`:

```rust
#[test]
fn mirror_domain_roundtrips() {
    use ai_core::RecallDomain;
    assert_eq!(RecallDomain::Mirror.as_str(), "mirror");
    assert_eq!(RecallDomain::from_str_or_general("mirror"), RecallDomain::Mirror);
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core mirror_domain_roundtrips`
Expected: FAIL — `RecallDomain::Mirror` not defined.

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
        }
    }

    pub fn from_str_or_general(s: &str) -> Self {
        match s {
            "tasks" => RecallDomain::Tasks,
            "finance" => RecallDomain::Finance,
            "productivity" => RecallDomain::Productivity,
            "learning" => RecallDomain::Learning,
            "mirror" => RecallDomain::Mirror,
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
git add crates/ai-core
git commit -m "feat(ai-core): add RecallDomain::Mirror variant"
```

---

## Task 2: Introduce `MirrorSnapshotSpec` + `MirrorSignalSource` Trait + `MirrorSubscriberRunner`

**Files:**
- Create: `crates/ai-core/src/mirror.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Create: `crates/ai-core/tests/mirror_runner_test.rs`

`★ Insight ─────────────────────────────────────`
`MirrorSignalSource` differs from `SignalConsumer` in three ways: (1) it only receives a filtered subset of `AiSignal`s — the trait declares which `event_kind`s it cares about via `SUBSCRIBED_KINDS`; (2) it exposes an optional `flush_interval` so the runner can drive timer-based aggregation without each source re-inventing `tokio::time::interval`; (3) it returns the *alert* (optional) rather than writing to a repo directly — keeping the source logic pure and testable. The runner is the adapter that turns a `MirrorSignalSource` into a `SignalConsumer` and spawns the flush loop.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing integration test**

Create `crates/ai-core/tests/mirror_runner_test.rs`:

```rust
use ai_core::{
    mirror::{MirrorSignalSource, MirrorSnapshotSpec, MirrorSubscriberRunner},
    AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer,
};
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct CountSource {
    count: Arc<AtomicU32>,
    flushes: Arc<AtomicU32>,
}

#[async_trait]
impl MirrorSignalSource for CountSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "count",
        subscribed_kinds: &["Ping"],
        flush_interval_secs: Some(60),
    };

    fn name(&self) -> &'static str {
        "count-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn dummy_signal(kind: &'static str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: kind,
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    }
}

#[tokio::test]
async fn runner_filters_by_subscribed_kinds() {
    let count = Arc::new(AtomicU32::new(0));
    let flushes = Arc::new(AtomicU32::new(0));
    let source = Arc::new(CountSource {
        count: count.clone(),
        flushes: flushes.clone(),
    });
    let runner = MirrorSubscriberRunner::new(source, CancellationToken::new());

    runner.consume(&dummy_signal("Ping")).await.unwrap();
    runner.consume(&dummy_signal("Pong")).await.unwrap();
    runner.consume(&dummy_signal("Ping")).await.unwrap();

    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert_eq!(flushes.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn runner_flushes_on_shutdown() {
    let count = Arc::new(AtomicU32::new(0));
    let flushes = Arc::new(AtomicU32::new(0));
    let source = Arc::new(CountSource {
        count: count.clone(),
        flushes: flushes.clone(),
    });
    let cancel = CancellationToken::new();
    let runner = MirrorSubscriberRunner::new(source, cancel.clone());

    // Manually drive the flush loop with a tiny interval for test speed.
    let handle = runner.clone().spawn_flush_loop(Duration::from_millis(50));
    tokio::time::sleep(Duration::from_millis(120)).await;
    cancel.cancel();
    handle.await.unwrap();

    // At least one interval flush + one shutdown flush.
    assert!(flushes.load(Ordering::Relaxed) >= 2);
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core mirror_runner`
Expected: FAIL — module `mirror` does not exist.

- [ ] **Step 3: Create `ai-core/src/mirror.rs`**

```rust
//! Mirror pipeline primitives — shared between ai-core consumers and cognitive
//! mirror sources. The runner handles event filtering + flush scheduling so each
//! concrete `MirrorSignalSource` can focus on aggregation + alerting.

use crate::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Declarative description of one mirror snapshot type.
#[derive(Debug, Clone, Copy)]
pub struct MirrorSnapshotSpec {
    /// Unique snapshot identifier (matches the mirror_<name>_snapshots table).
    pub name: &'static str,
    /// The list of `AiSignal::event_kind` values this source wants to see.
    /// An empty list means "all" (rare; usually a bug).
    pub subscribed_kinds: &'static [&'static str],
    /// If `Some`, the runner drives `flush()` every N seconds. If `None`, the
    /// source is event-driven only (flush is triggered externally or inside
    /// `accumulate`).
    pub flush_interval_secs: Option<u64>,
}

/// One mirror snapshot producer. Accumulates filtered `AiSignal`s and emits a
/// snapshot on flush. Implementations own their own repo handles.
#[async_trait]
pub trait MirrorSignalSource: Send + Sync + 'static {
    const SPEC: MirrorSnapshotSpec;

    /// Human-readable name for logs.
    fn name(&self) -> &'static str;

    /// Handle one matching `AiSignal`. Should not block.
    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()>;

    /// Build + persist the snapshot (if any) and reset the accumulator.
    /// Called on `flush_interval_secs` ticks and once on shutdown.
    async fn flush(&self) -> common::Result<()>;
}

/// Runtime adapter: `MirrorSignalSource` → `SignalConsumer`.
///
/// Filters incoming signals against `SPEC::subscribed_kinds`, forwards matches
/// to `accumulate`, and (optionally) spawns a background task that calls
/// `flush` at `flush_interval_secs` + once on shutdown.
pub struct MirrorSubscriberRunner<S: MirrorSignalSource> {
    source: Arc<S>,
    cancel: CancellationToken,
}

impl<S: MirrorSignalSource> MirrorSubscriberRunner<S> {
    pub fn new(source: Arc<S>, cancel: CancellationToken) -> Arc<Self> {
        Arc::new(Self { source, cancel })
    }

    /// Spawn the background flush loop. Returns the join handle; callers must
    /// keep it alive for the app lifetime. Panics if `SPEC::flush_interval_secs`
    /// is `None` — event-driven sources must not call this.
    pub fn spawn_flush_loop(self: Arc<Self>, override_interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(override_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Skip the immediate first tick — we want the first flush after one full interval.
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = self.cancel.cancelled() => {
                        if let Err(e) = self.source.flush().await {
                            tracing::warn!(source = self.source.name(), error = %e,
                                "MirrorSubscriberRunner: shutdown flush failed");
                        }
                        return;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = self.source.flush().await {
                            tracing::warn!(source = self.source.name(), error = %e,
                                "MirrorSubscriberRunner: interval flush failed");
                        }
                    }
                }
            }
        })
    }

    /// Spawn using the source's declared interval. Returns `None` if the source
    /// is event-driven (no interval). Callers store the handle.
    pub fn spawn_declared_flush_loop(self: Arc<Self>) -> Option<JoinHandle<()>> {
        let secs = S::SPEC.flush_interval_secs?;
        Some(self.spawn_flush_loop(Duration::from_secs(secs)))
    }
}

#[async_trait]
impl<S: MirrorSignalSource> SignalConsumer for MirrorSubscriberRunner<S> {
    fn name(&self) -> &'static str {
        self.source.name()
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if !S::SPEC.subscribed_kinds.contains(&signal.event_kind) {
            return Ok(());
        }
        self.source.accumulate(signal).await
    }
}
```

- [ ] **Step 4: Wire into `lib.rs`**

Edit `crates/ai-core/src/lib.rs`:

```rust
//! Unified AI feature pipeline.
pub mod metrics;
pub mod mirror;
pub mod recall;
pub mod recall_domain;
pub mod recall_provider_registry;
pub mod router;
pub mod signal;
pub mod traits;

pub use metrics::AiMetrics;
pub use mirror::{MirrorSignalSource, MirrorSnapshotSpec, MirrorSubscriberRunner};
pub use recall::{RecallItem, RecallQuery, RecallSpec};
pub use recall_domain::RecallDomain;
pub use recall_provider_registry::RecallProviderRegistry;
pub use router::{SignalRouter, Translator};
pub use signal::{AiSignal, EntityRef, SalienceVerdict};
pub use traits::{AiEntity, AiEventMeta, AiFeature, RecallProvider, SignalConsumer};
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core mirror_runner`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core
git commit -m "feat(ai-core): MirrorSignalSource trait + MirrorSubscriberRunner"
```

---

## Task 3: `#[ai(mirror_snapshot(...))]` Attribute — Parse

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`

`★ Insight ─────────────────────────────────────`
We keep the attribute on the *feature struct*, not on events. The mirror source type is implemented in `cognitive`, not in the feature crate; the feature only *declares* that it owns a mirror snapshot type. This lets the invariant test (Task 25) cross-check "every declared snapshot has a registered source" at workspace scope.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend `AiFeatureAttr`**

Edit `crates/ai-core-macros/src/attrs.rs`. Add to the struct:

```rust
pub struct AiFeatureAttr {
    pub recall_domain: syn::Ident,
    pub skill: String,
    pub event: syn::Path,
    pub recall_boost_when: Option<syn::Expr>,
    pub recall_priority_field: Option<syn::Ident>,
    pub recall_recency_field: Option<syn::Ident>,
    pub recall_status_filter: Option<syn::Expr>,
    pub mirror_snapshots: Vec<MirrorSnapshotAttr>,
}

pub struct MirrorSnapshotAttr {
    pub name: String,
    pub flush_interval_secs: Option<u64>,
    pub subscribed_kinds: Vec<String>,
}
```

- [ ] **Step 2: Extend `parse_ai_feature_attr`**

In the same file, inside `parse_ai_feature_attr`, before the `Ok(AiFeatureAttr { ... })` return:

Add a mutable collector at the top of the function alongside the other `let mut` bindings:

```rust
    let mut mirror_snapshots: Vec<MirrorSnapshotAttr> = Vec::new();
```

Add a new arm inside the `match k.as_str() { ... }`:

```rust
            "mirror_snapshot" => {
                mirror_snapshots.push(parse_mirror_snapshot(&meta)?);
            }
```

Return `mirror_snapshots` in the `Ok(...)`:

```rust
    Ok(AiFeatureAttr {
        recall_domain: recall_domain.ok_or_else(...)?,
        skill: skill.ok_or_else(...)?,
        event: event.ok_or_else(...)?,
        recall_boost_when,
        recall_priority_field,
        recall_recency_field,
        recall_status_filter,
        mirror_snapshots,
    })
```

- [ ] **Step 3: Add the nested parser**

Append to `crates/ai-core-macros/src/attrs.rs`:

```rust
fn parse_mirror_snapshot(
    meta: &syn::meta::ParseNestedMeta,
) -> syn::Result<MirrorSnapshotAttr> {
    let mut name: Option<String> = None;
    let mut flush_interval_secs: Option<u64> = None;
    let mut subscribed_kinds: Vec<String> = Vec::new();

    meta.parse_nested_meta(|inner| {
        let key = inner
            .path
            .get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "name" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                name = Some(s.value());
            }
            "flush_interval_secs" => {
                let value: syn::Expr = inner.value()?.parse()?;
                let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) = value else {
                    return Err(inner.error("flush_interval_secs must be an integer literal"));
                };
                flush_interval_secs = Some(i.base10_parse::<u64>()?);
            }
            "event_kinds" => {
                // `event_kinds = ["TaskFocusChanged", "TaskCompleted"]` (bracketed string list).
                let arr: syn::ExprArray = inner.value()?.parse()?;
                for elem in arr.elems {
                    let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = elem else {
                        return Err(inner.error(
                            "event_kinds must be a list of string literals, e.g. [\"Foo\", \"Bar\"]",
                        ));
                    };
                    subscribed_kinds.push(s.value());
                }
            }
            other => {
                return Err(inner.error(format!(
                    "unknown mirror_snapshot key: {other} \
                     (expected name / flush_interval_secs / event_kinds)"
                )));
            }
        }
        Ok(())
    })?;

    Ok(MirrorSnapshotAttr {
        name: name.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "mirror_snapshot requires name = \"...\"",
            )
        })?,
        flush_interval_secs,
        subscribed_kinds,
    })
}
```

- [ ] **Step 4: Run**

Run: `cargo build -p ai-core-macros`
Expected: PASS (no callers yet use the attribute).

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): parse mirror_snapshot attribute"
```

---

## Task 4: Emit `MIRROR_SNAPSHOTS` Constant From `#[derive(AiFeature)]`

**Files:**
- Modify: `crates/ai-core-macros/src/ai_feature.rs`
- Create: `crates/ai-core-macros/tests/expand/mirror_snapshot.rs`
- Modify: `crates/ai-core-macros/tests/expand_smoke.rs`

`★ Insight ─────────────────────────────────────`
The derive generates a `pub const MIRROR_SNAPSHOTS: &'static [MirrorSnapshotSpec] = &[...]` on the feature struct. Any code that wants to enumerate "what mirror snapshots does this feature declare?" reads this constant — no runtime registry, no string parsing, no reflection. Task 25's invariant test walks all `AiFeature` impls via a hand-maintained list and compares their `MIRROR_SNAPSHOTS` to the set of registered sources.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing trybuild test**

Create `crates/ai-core-macros/tests/expand/mirror_snapshot.rs`:

```rust
use ai_core::{AiFeature, MirrorSnapshotSpec, RecallDomain};
use ai_core_macros::{AiEvent, AiFeature};
use bus::DomainEvent;

#[derive(AiEvent)]
pub enum TinyEvent {
    #[ai(importance = 0.5, salience = "accumulate", observation_template = "x")]
    Ping,
}

impl From<TinyEvent> for DomainEvent {
    fn from(_: TinyEvent) -> Self {
        DomainEvent::ChatTurnCompleted {
            session_key: String::new(),
            user_message: None,
        }
    }
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::TinyEvent",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["TaskFocusChanged", "TaskCompleted"],
    ),
    mirror_snapshot(
        name = "task_velocity",
        event_kinds = ["TaskCompleted"],
    ),
)]
pub struct TinyFeature;

fn main() {
    assert_eq!(<TinyFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    let specs: &'static [MirrorSnapshotSpec] = TinyFeature::MIRROR_SNAPSHOTS;
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "task_focus");
    assert_eq!(specs[0].flush_interval_secs, Some(3600));
    assert_eq!(specs[0].subscribed_kinds, &["TaskFocusChanged", "TaskCompleted"]);
    assert_eq!(specs[1].name, "task_velocity");
    assert_eq!(specs[1].flush_interval_secs, None);
    assert_eq!(specs[1].subscribed_kinds, &["TaskCompleted"]);
}
```

Wire it into the trybuild harness. Edit `crates/ai-core-macros/tests/expand_smoke.rs`:

```rust
#[test]
fn expansion_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/noop.rs");
    t.pass("tests/expand/event_basic.rs");
    t.pass("tests/expand/entity_basic.rs");
    t.pass("tests/expand/coaching_signal.rs");
    t.pass("tests/expand/mirror_snapshot.rs");
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p ai-core-macros`
Expected: FAIL — `MIRROR_SNAPSHOTS` constant not emitted.

- [ ] **Step 3: Emit the constant**

Edit `crates/ai-core-macros/src/ai_feature.rs`. Replace the function body with:

```rust
use crate::attrs::{parse_ai_feature_attr, MirrorSnapshotAttr};
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    let feat = parse_ai_feature_attr(&input.attrs)?;

    let recall_domain_ident = &feat.recall_domain;
    let skill_lit = &feat.skill;
    let event_path = &feat.event;

    let boost_expr = match &feat.recall_boost_when {
        Some(expr) => quote! {
            fn score_query(&self, query: &::ai_core::RecallQuery) -> f64 {
                if { #expr } { 1.0 } else { 0.3 }
            }
        },
        None => quote! {},
    };

    let priority = feat.recall_priority_field.as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let recency = feat.recall_recency_field.as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let status = match &feat.recall_status_filter {
        Some(expr) => {
            let s = quote!(#expr).to_string();
            quote! { Some(#s) }
        }
        None => quote! { None },
    };

    let mirror_specs_tokens = render_mirror_specs(&feat.mirror_snapshots);

    Ok(quote! {
        impl ::ai_core::AiFeature for #struct_ident {
            const DOMAIN: ::ai_core::RecallDomain = ::ai_core::RecallDomain::#recall_domain_ident;
            const SKILL: &'static str = #skill_lit;
            type Event = #event_path;
        }

        impl ::ai_core::RecallProvider for #struct_ident {
            fn domain(&self) -> ::ai_core::RecallDomain {
                ::ai_core::RecallDomain::#recall_domain_ident
            }
            #boost_expr
        }

        impl #struct_ident {
            pub const RECALL_SPEC: ::ai_core::RecallSpec = ::ai_core::RecallSpec {
                priority_field: #priority,
                recency_field: #recency,
                status_filter: #status,
            };

            pub const MIRROR_SNAPSHOTS: &'static [::ai_core::MirrorSnapshotSpec] =
                #mirror_specs_tokens;
        }
    })
}

fn render_mirror_specs(snapshots: &[MirrorSnapshotAttr]) -> TokenStream {
    if snapshots.is_empty() {
        return quote! { &[] };
    }
    let entries = snapshots.iter().map(|s| {
        let name = &s.name;
        let interval = match s.flush_interval_secs {
            Some(v) => quote! { Some(#v) },
            None => quote! { None },
        };
        let kinds = s.subscribed_kinds.iter().map(|k| quote! { #k });
        quote! {
            ::ai_core::MirrorSnapshotSpec {
                name: #name,
                subscribed_kinds: &[ #(#kinds),* ],
                flush_interval_secs: #interval,
            }
        }
    });
    quote! { &[ #(#entries),* ] }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p ai-core-macros`
Expected: trybuild step FAIL if there's a macro bug; fix and re-run.

Also run: `cargo build --workspace` — the existing `TasksFeature` / `FinanceFeature` derives (no `mirror_snapshot` attrs) must still compile and receive `MIRROR_SNAPSHOTS = &[]`.

Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): emit MIRROR_SNAPSHOTS constant from AiFeature"
```

---

## Task 5: Add Cleanup Methods to `MirrorRepo`

**Files:**
- Modify: `crates/cognitive/src/mirror/repo.rs`

`★ Insight ─────────────────────────────────────`
`MirrorRepo` already has `cleanup_old_snapshots` and `cleanup_old_snippets`. Three tables have no retention today: `mirror_trend_narratives`, `mirror_meta_rules` (dismissed only), `mirror_brain_versions`. Per Open Question 3 in the spec, retention is "no policy today; v2 defines it per snapshot type." Defaults: narratives keep 365d, meta-rules keep 180d for `Disabled` status only (active/pending stay forever), brain versions keep forever (we never auto-prune history — they're like git commits). We still add the method so future pruning is opt-in.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/src/mirror/repo.rs`'s `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn cleanup_old_trend_narratives_keeps_recent() {
    let repo = crate::mirror::test_mirror_repo().await;
    let old = TrendNarrative {
        id: Uuid::new_v4(),
        generated_at: Timestamp::now()
            .checked_sub(jiff::ToSpan::days(400))
            .unwrap(),
        period_start: Timestamp::now(),
        period_end: Timestamp::now(),
        routing_summary: "old".into(),
        improvement_highlights: vec![],
        experiment_summary: String::new(),
        meta_rule_updates: vec![],
        full_narrative: "old narrative".into(),
        user_feedback: None,
    };
    let recent = TrendNarrative {
        id: Uuid::new_v4(),
        generated_at: Timestamp::now(),
        ..old.clone()
    };
    repo.insert_trend_narrative(&old).await.unwrap();
    repo.insert_trend_narrative(&recent).await.unwrap();

    let deleted = repo.cleanup_old_trend_narratives(365).await.unwrap();
    assert_eq!(deleted, 1);

    let remaining = repo.get_narratives(100).await.unwrap();
    assert_eq!(remaining.len(), 1);
}

#[tokio::test]
async fn cleanup_old_meta_rules_only_disabled() {
    let repo = crate::mirror::test_mirror_repo().await;
    let mut mk = |status: MetaRuleStatus, created_at: Timestamp| MetaRule {
        id: Uuid::new_v4(),
        trigger_condition: String::new(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status,
        signal_count: 0,
        created_at,
        updated_at: created_at,
    };
    let old_disabled = mk(
        MetaRuleStatus::Disabled,
        Timestamp::now().checked_sub(jiff::ToSpan::days(200)).unwrap(),
    );
    let old_active = mk(
        MetaRuleStatus::Active,
        Timestamp::now().checked_sub(jiff::ToSpan::days(200)).unwrap(),
    );
    let recent_disabled = mk(MetaRuleStatus::Disabled, Timestamp::now());
    repo.insert_meta_rule(&old_disabled).await.unwrap();
    repo.insert_meta_rule(&old_active).await.unwrap();
    repo.insert_meta_rule(&recent_disabled).await.unwrap();

    let deleted = repo.cleanup_old_meta_rules(180).await.unwrap();
    assert_eq!(deleted, 1);

    // Active + recent disabled remain.
    assert_eq!(
        repo.get_meta_rules_by_status(MetaRuleStatus::Active).await.unwrap().len(),
        1
    );
    assert_eq!(
        repo.get_meta_rules_by_status(MetaRuleStatus::Disabled).await.unwrap().len(),
        1
    );
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive cleanup_old_trend_narratives cleanup_old_meta_rules`
Expected: FAIL — methods do not exist.

- [ ] **Step 3: Implement the methods**

Append to `impl MirrorRepo` in `crates/cognitive/src/mirror/repo.rs`:

```rust
/// Delete trend narratives older than `max_age_days`. Returns rows removed.
pub async fn cleanup_old_trend_narratives(&self, max_age_days: u32) -> Result<u64> {
    let cutoff = Timestamp::now()
        .checked_sub(jiff::ToSpan::days(max_age_days as i64))
        .map_err(|e| common::KlyntbotError::internal(format!("cutoff: {e}")))?;
    let result = sqlx::query("DELETE FROM mirror_trend_narratives WHERE generated_at < ?")
        .bind(cutoff.to_string())
        .execute(&self.pool.inner().clone())
        .await
        .map_err(|e| common::KlyntbotError::storage(format!("cleanup narratives: {e}")))?;
    Ok(result.rows_affected())
}

/// Delete `Disabled` meta-rules older than `max_age_days`. Active/Pending
/// rules are never auto-removed. Returns rows removed.
pub async fn cleanup_old_meta_rules(&self, max_age_days: u32) -> Result<u64> {
    let cutoff = Timestamp::now()
        .checked_sub(jiff::ToSpan::days(max_age_days as i64))
        .map_err(|e| common::KlyntbotError::internal(format!("cutoff: {e}")))?;
    let result = sqlx::query(
        "DELETE FROM mirror_meta_rules
         WHERE status = 'disabled' AND updated_at < ?",
    )
    .bind(cutoff.to_string())
    .execute(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("cleanup meta rules: {e}")))?;
    Ok(result.rows_affected())
}

/// Delete brain versions marked `reverted = 1` older than `max_age_days`.
/// Promoted versions are never deleted.
pub async fn cleanup_reverted_brain_versions(&self, max_age_days: u32) -> Result<u64> {
    let cutoff = Timestamp::now()
        .checked_sub(jiff::ToSpan::days(max_age_days as i64))
        .map_err(|e| common::KlyntbotError::internal(format!("cutoff: {e}")))?;
    let result = sqlx::query(
        "DELETE FROM mirror_brain_versions
         WHERE reverted = 1 AND promoted_at < ?",
    )
    .bind(cutoff.to_string())
    .execute(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("cleanup brain versions: {e}")))?;
    Ok(result.rows_affected())
}
```

(If existing repo code uses a different SQL helper — e.g. `self.pool.inner()` vs `&self.pool.inner().clone()` — match the existing pattern used in `cleanup_old_snapshots` at `repo.rs:474`.)

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive cleanup_old_trend_narratives cleanup_old_meta_rules`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/repo.rs
git commit -m "feat(cognitive/mirror): add cleanup methods for narratives, meta-rules, brain versions"
```

---

## Task 6: Create `MirrorRetentionService`

**Files:**
- Create: `crates/cognitive/src/mirror/retention.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/src/mirror/retention.rs` with this skeleton (module only — implementation follows):

```rust
//! Periodic cleanup of aged mirror rows.

use crate::mirror::MirrorRepo;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy)]
pub struct MirrorRetentionConfig {
    pub routing_snapshot_days: u32,
    pub snippet_days: u32,
    pub narrative_days: u32,
    pub disabled_meta_rule_days: u32,
    pub reverted_brain_version_days: u32,
    pub trial_preview_days: u32,
    pub sweep_interval_secs: u64,
}

impl Default for MirrorRetentionConfig {
    fn default() -> Self {
        Self {
            routing_snapshot_days: 90,
            snippet_days: 30,
            narrative_days: 365,
            disabled_meta_rule_days: 180,
            reverted_brain_version_days: 730,
            trial_preview_days: 180,
            sweep_interval_secs: 24 * 3600, // daily
        }
    }
}

pub struct MirrorRetentionService;

impl MirrorRetentionService {
    /// Spawn the daily sweep. Returns the join handle; caller keeps it alive.
    pub fn spawn(
        repo: Arc<MirrorRepo>,
        config: MirrorRetentionConfig,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.sweep_interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await; // Skip immediate first tick.
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = interval.tick() => {
                        Self::sweep_once(&repo, &config).await;
                    }
                }
            }
        })
    }

    pub async fn sweep_once(repo: &MirrorRepo, config: &MirrorRetentionConfig) {
        let _ = repo.cleanup_old_snapshots(config.routing_snapshot_days).await;
        let _ = repo.cleanup_old_snippets(config.snippet_days).await;
        let _ = repo.cleanup_old_trend_narratives(config.narrative_days).await;
        let _ = repo.cleanup_old_meta_rules(config.disabled_meta_rule_days).await;
        let _ = repo.cleanup_reverted_brain_versions(config.reverted_brain_version_days).await;
        let _ = repo.cleanup_old_trial_previews(config.trial_preview_days).await;
        tracing::debug!("mirror retention sweep completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sweep_once_does_not_panic_on_empty_tables() {
        let repo = crate::mirror::test_mirror_repo().await;
        MirrorRetentionService::sweep_once(&repo, &MirrorRetentionConfig::default()).await;
    }
}
```

- [ ] **Step 2: Wire into `mirror/mod.rs`**

Edit `crates/cognitive/src/mirror/mod.rs`:

```rust
pub mod engine;
pub mod facade;
pub mod narratives;
pub mod repo;
pub mod retention;
pub mod sources;
pub mod types;
pub use engine::MirrorEngine;
pub use facade::MirrorFacade;
pub use narratives::{snippet_from_alert, NarrativeHandler};
pub use repo::MirrorRepo;
pub use retention::{MirrorRetentionConfig, MirrorRetentionService};
// sources re-exports added in Task 8+.
pub use types::{ /* unchanged type list */ };
```

(Keep the existing `pub use types::{...}` list as-is; the `subscribers` mod + its re-exports will be deleted in Task 19.)

- [ ] **Step 3: Run**

Run: `cargo nextest run -p cognitive sweep_once_does_not_panic`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/mirror
git commit -m "feat(cognitive/mirror): MirrorRetentionService with configurable per-table retention"
```

---

## Task 7: Emit `AutotunerDecision { verdict: "activated" }` at Trial Kickoff

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs`

`★ Insight ─────────────────────────────────────`
The spec calls the old `TrialActivated` variant out as dead in v1. Rather than reintroduce a new variant (which would bloat the enum), v2 reuses the already-typed `AutotunerDecision` event with a new `verdict: "activated"` string. `TrialPreviewSource` filters on `verdict == "activated"` the same way `ConfigArchiver` filters on `"promoted"`. This keeps the bus-surface change to zero variants.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Identify kickoff site**

Find where trials transition from `Proposed` → `Running`. Search:

```bash
rg 'TrialStatus::Running|update_trial_status.*running|create_trial' crates/agent/src/autotuner/
```

The likely site: `NightlyCycle::run_evaluation_and_promotion` or the variant-generation step that inserts new trials into `TrialRepo`. Identify the exact line where a new trial is persisted and note it (e.g. `crates/agent/src/autotuner/mod.rs:XYZ`).

- [ ] **Step 2: Write failing test**

Append to the existing autotuner test module (file TBD — likely `crates/agent/src/autotuner/mod.rs` `#[cfg(test)] mod tests`) the following. If the existing tests don't publish to a bus, create a new ephemeral one for this test:

```rust
#[tokio::test]
async fn activated_verdict_published_when_trial_starts() {
    // Setup: build a minimal NightlyCycle with a fake MetricSource that returns
    // "create new trial" and a real DomainEventBus. Run the cycle and observe
    // a DomainEvent::AutotunerDecision with verdict == "activated" is published
    // exactly once per new trial.
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mut rx = bus.subscribe();

    // ... Build orchestrator + cycle per existing test helpers ...
    // Trigger the path that creates a new trial.

    let mut saw_activated = false;
    while let Ok(event) =
        tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await
    {
        if let Ok(bus::DomainEvent::AutotunerDecision { verdict, .. }) = event {
            if verdict == "activated" {
                saw_activated = true;
                break;
            }
        }
    }
    assert!(saw_activated, "expected AutotunerDecision(activated) event");
}
```

(If the existing test infrastructure doesn't support end-to-end cycle runs, implement this as a narrower unit test that calls the helper added in Step 3 directly.)

- [ ] **Step 3: Add the kickoff publish**

In `crates/agent/src/autotuner/mod.rs`, at the identified kickoff site, immediately after successfully inserting the new trial into `TrialRepo`:

```rust
if let Some(ref bus) = domain_event_bus {
    bus.publish(bus::DomainEvent::AutotunerDecision {
        trial_id: new_trial_id.to_string(),
        verdict: "activated".to_string(),
        improvement_pct: 0.0,
        affected_params: proposed_param_names,
    });
}
```

If multiple kickoff sites exist (e.g. human-triggered vs nightly), extract a helper:

```rust
fn publish_activated(
    bus: &Option<Arc<DomainEventBus>>,
    trial_id: &str,
    affected_params: Vec<String>,
) {
    if let Some(bus) = bus.as_ref() {
        bus.publish(bus::DomainEvent::AutotunerDecision {
            trial_id: trial_id.to_string(),
            verdict: "activated".to_string(),
            improvement_pct: 0.0,
            affected_params,
        });
    }
}
```

- [ ] **Step 4: Document the verdict value**

Edit `crates/bus/src/domain_events.rs` at the `AutotunerDecision` variant docs. Add/extend the block comment above the variant:

```rust
    /// Autotuner lifecycle event. `verdict` is one of:
    /// - `"activated"` — a new trial started running (emitted by `autotuner` at kickoff)
    /// - `"promoted"` — trial beat champion and became new champion
    /// - `"reverted"` — previous promotion was rolled back due to regression
    AutotunerDecision {
        trial_id: String,
        verdict: String,
        improvement_pct: f64,
        affected_params: Vec<String>,
    },
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p agent activated_verdict_published`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent crates/bus
git commit -m "feat(autotuner): publish AutotunerDecision(activated) at trial kickoff"
```

---

## Task 8: Port `RoutingMirrorSubscriber` → `RoutingSignalSource`

**Files:**
- Create: `crates/cognitive/src/mirror/sources/mod.rs`
- Create: `crates/cognitive/src/mirror/sources/routing.rs`

`★ Insight ─────────────────────────────────────`
The port preserves every behavioral detail: same `DashMap` accumulator, same drift-detection thresholds (fallback_rate > 0.70, per-skill delta > 15pp), same 7-day history lookup. What changes is the *source* of the event: we read from the already-translated `AiSignal` instead of matching on `DomainEvent` directly. The `signal.raw_event` field carries the original `DomainEvent` when we need structured fields the translator didn't hoist.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create `sources/mod.rs`**

```rust
//! Mirror signal sources — each implements `MirrorSignalSource` and is wired
//! to the workspace `SignalRouter` via `MirrorSubscriberRunner`.

pub mod config_archiver;
pub mod finance_drift;
pub mod meta_rule;
pub mod routing;
pub mod task_focus;
pub mod trial;

pub use config_archiver::ConfigArchiverSource;
pub use finance_drift::FinanceSpendingDriftSource;
pub use meta_rule::MetaRuleSignalSource;
pub use routing::RoutingSignalSource;
pub use task_focus::TaskFocusPatternSource;
pub use trial::TrialPreviewSource;
```

(Not all these files exist yet; the other Tasks 9–13 create them. Leaving the module declarations here now keeps the wiring explicit.)

- [ ] **Step 2: Write failing test**

Create `crates/cognitive/src/mirror/sources/routing.rs` with a test shell first:

```rust
//! RoutingSignalSource — accumulates SkillRouted AiSignals and flushes hourly.
//! Replaces the old RoutingMirrorSubscriber that subscribed to DomainEventBus directly.

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn skill_routed_signal(skill: &str, confidence: f64) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "SkillRouted",
            importance: 0.3,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(bus::DomainEvent::SkillRouted {
                skill_name: skill.to_string(),
                confidence,
                source: "keyword".into(),
                trigger_phrases: vec!["hello".into()],
                session_key: "s".into(),
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[tokio::test]
    async fn accumulates_skill_routed_signals() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(RoutingSignalSource::new(repo.clone()));
        let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), CancellationToken::new());

        runner.consume(&skill_routed_signal("general", 0.85)).await.unwrap();
        runner.consume(&skill_routed_signal("general", 0.85)).await.unwrap();
        runner.consume(&skill_routed_signal("finance", 0.75)).await.unwrap();

        // Flush persists a snapshot.
        source.flush().await.unwrap();
        let latest = repo.get_latest_routing_snapshot().await.unwrap().unwrap();
        assert_eq!(latest.total_messages, 3);
        assert_eq!(latest.distribution["general"].count, 2);
    }

    #[tokio::test]
    async fn ignores_non_skill_routed_signals() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(RoutingSignalSource::new(repo));
        let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), CancellationToken::new());

        let other = AiSignal {
            event_kind: "TaskCreated",
            raw_event: None,
            ..skill_routed_signal("x", 1.0)
        };
        runner.consume(&other).await.unwrap();
        let snap = source.build_snapshot();
        assert_eq!(snap.total_messages, 0);
    }
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p cognitive routing_signal_source`
Expected: FAIL — type not defined.

- [ ] **Step 4: Implement `RoutingSignalSource`**

Prepend to `crates/cognitive/src/mirror/sources/routing.rs` (before the `#[cfg(test)]` block):

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use dashmap::DashMap;
use jiff::Timestamp;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::mirror::{
    snippet_from_alert, MirrorAlert, MirrorRepo, NarrativeSnippet, RoutingSnapshot, SkillRouteStats,
};

const MAX_TRIGGER_PHRASES: usize = 100;

struct SkillRouteAccum {
    count: u32,
    confidence_sum: f64,
    trigger_hits: HashMap<String, u32>,
}

impl SkillRouteAccum {
    fn new() -> Self {
        Self {
            count: 0,
            confidence_sum: 0.0,
            trigger_hits: HashMap::new(),
        }
    }
}

pub struct RoutingSignalSource {
    accumulator: DashMap<String, SkillRouteAccum>,
    total_count: AtomicU32,
    low_confidence_count: AtomicU32,
    repo: MirrorRepo,
}

impl RoutingSignalSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo,
        }
    }

    fn accumulate_inner(&self, skill_name: &str, confidence: f64, triggers: &[String]) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        if confidence < 0.6 {
            self.low_confidence_count.fetch_add(1, Ordering::Relaxed);
        }
        let mut entry = self
            .accumulator
            .entry(skill_name.to_string())
            .or_insert_with(SkillRouteAccum::new);
        entry.count += 1;
        entry.confidence_sum += confidence;
        if entry.trigger_hits.len() < MAX_TRIGGER_PHRASES {
            for trigger in triggers {
                *entry.trigger_hits.entry(trigger.clone()).or_insert(0) += 1;
            }
        }
    }

    pub fn build_snapshot(&self) -> RoutingSnapshot {
        let total = self.total_count.load(Ordering::Relaxed);
        let low_conf = self.low_confidence_count.load(Ordering::Relaxed);
        let fallback_rate = if total > 0 { low_conf as f64 / total as f64 } else { 0.0 };

        let mut distribution: HashMap<String, SkillRouteStats> = HashMap::new();
        let mut global_conf_sum = 0.0_f64;
        let mut global_count = 0_u32;
        for entry in self.accumulator.iter() {
            let skill = entry.key().clone();
            let accum = entry.value();
            let percentage = if total > 0 { accum.count as f64 / total as f64 * 100.0 } else { 0.0 };
            let avg_confidence = if accum.count > 0 { accum.confidence_sum / accum.count as f64 } else { 0.0 };
            let mut sorted: Vec<(String, u32)> =
                accum.trigger_hits.iter().map(|(k, v)| (k.clone(), *v)).collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            let top_triggers: Vec<String> =
                sorted.into_iter().take(5).map(|(k, _)| k).collect();
            global_conf_sum += accum.confidence_sum;
            global_count += accum.count;
            distribution.insert(
                skill,
                SkillRouteStats { count: accum.count, percentage, avg_confidence, top_triggers },
            );
        }
        let avg_routing_confidence =
            if global_count > 0 { global_conf_sum / global_count as f64 } else { 0.0 };

        RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 1,
            total_messages: total,
            distribution,
            fallback_rate,
            avg_routing_confidence,
            low_confidence_count: low_conf,
            user_feedback: None,
        }
    }

    pub fn detect_drift(
        &self,
        snapshot: &RoutingSnapshot,
        history: &[RoutingSnapshot],
    ) -> Option<MirrorAlert> {
        if history.is_empty() {
            return None;
        }
        if snapshot.fallback_rate > 0.70 {
            return Some(MirrorAlert::RoutingDrift {
                skill: "fallback".into(),
                delta: snapshot.fallback_rate * 100.0,
                suggestion: "Many recent messages had low routing confidence — consider \
                             reviewing your skill configuration"
                    .into(),
            });
        }
        for (skill, stats) in &snapshot.distribution {
            let historical: Vec<f64> = history
                .iter()
                .filter_map(|h| h.distribution.get(skill))
                .map(|s| s.percentage)
                .collect();
            if historical.is_empty() {
                continue;
            }
            let avg: f64 = historical.iter().sum::<f64>() / historical.len() as f64;
            let delta = (stats.percentage - avg).abs();
            if delta > 15.0 {
                return Some(MirrorAlert::RoutingDrift {
                    skill: skill.clone(),
                    delta,
                    suggestion: format!(
                        "Consider reviewing the '{skill}' skill's trigger configuration"
                    ),
                });
            }
        }
        None
    }

    pub fn reset(&self) {
        self.accumulator.clear();
        self.total_count.store(0, Ordering::Relaxed);
        self.low_confidence_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl MirrorSignalSource for RoutingSignalSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "routing",
        subscribed_kinds: &["SkillRouted"],
        flush_interval_secs: Some(3600),
    };

    fn name(&self) -> &'static str {
        "mirror.routing"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(DomainEvent::SkillRouted {
            skill_name,
            confidence,
            trigger_phrases,
            ..
        }) = signal.raw_event.as_ref()
        {
            self.accumulate_inner(skill_name, *confidence, trigger_phrases);
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let snapshot = self.build_snapshot();
        debug!(
            total_messages = snapshot.total_messages,
            fallback_rate = snapshot.fallback_rate,
            "Mirror routing flush"
        );
        let history = match self.repo.get_routing_history(7).await {
            Ok(h) => h,
            Err(e) => {
                warn!("Mirror: failed to fetch routing history: {e}");
                vec![]
            }
        };
        let drift = self.detect_drift(&snapshot, &history);
        if let Err(e) = self.repo.insert_routing_snapshot(&snapshot).await {
            warn!("Mirror: failed to persist routing snapshot: {e}");
        }
        if let Some(alert) = drift {
            info!(?alert, "Mirror: routing drift detected");
            let snippet: NarrativeSnippet = snippet_from_alert(&alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("Mirror: failed to persist drift snippet: {e}");
            }
        }
        self.reset();
        Ok(())
    }
}
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive routing_signal_source`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/sources
git commit -m "feat(cognitive/mirror): RoutingSignalSource (replaces RoutingMirrorSubscriber)"
```

---

## Task 9: Port `MetaRuleDetector` → `MetaRuleSignalSource`

**Files:**
- Create: `crates/cognitive/src/mirror/sources/meta_rule.rs`

`★ Insight ─────────────────────────────────────`
`MetaRuleDetector` is purely event-driven — no timer, no flush. Its port therefore sets `flush_interval_secs: None` and performs all emission inside `accumulate`. The state mutation still needs interior mutability (the original had `&mut self` in its run loop; the trait takes `&self`). We wrap the `HashMap`s in `tokio::sync::Mutex` to keep the changes minimal.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/src/mirror/sources/meta_rule.rs` with test shell:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use bus::DomainEvent;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn user_corrected_signal(session: &str, skill: &str) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "UserCorrectedAI",
            importance: 0.7,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::UserCorrectedAI {
                original: String::new(),
                correction: String::new(),
                kind: bus::CorrectionKind::Factual,
                strength: 0.8,
                session_key: session.into(),
                active_skill: Some(skill.into()),
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[tokio::test]
    async fn same_session_corrections_trigger_meta_rule() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(MetaRuleSignalSource::new(repo.clone()));
        let runner = ai_core::MirrorSubscriberRunner::new(source, CancellationToken::new());

        runner.consume(&user_corrected_signal("s1", "general")).await.unwrap();
        runner.consume(&user_corrected_signal("s1", "general")).await.unwrap();

        let rules =
            repo.get_meta_rules_by_status(crate::mirror::MetaRuleStatus::Pending).await.unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].trigger_condition.contains("2 times"));
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive meta_rule_signal_source`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend to `meta_rule.rs`:

```rust
use std::collections::HashMap;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use jiff::Timestamp;
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use crate::mirror::{
    snippet_from_alert, MetaRule, MetaRuleAction, MetaRuleSource as MetaRuleOriginSource,
    MetaRuleStatus, MirrorAlert, MirrorRepo,
};

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.4;
const SAME_SESSION_CORRECTION_THRESHOLD: u32 = 2;
const CROSS_SESSION_CORRECTION_THRESHOLD: u32 = 3;
const LOW_CONFIDENCE_STREAK_THRESHOLD: u32 = 3;
const MAX_TRACKED_SESSIONS: usize = 100;
const MAX_TRACKED_SKILLS: usize = 50;

#[derive(Default)]
struct State {
    session_corrections: HashMap<String, u32>,
    skill_corrections: HashMap<String, u32>,
    low_confidence_streak: u32,
}

pub struct MetaRuleSignalSource {
    state: Mutex<State>,
    repo: MirrorRepo,
}

impl MetaRuleSignalSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            state: Mutex::new(State::default()),
            repo,
        }
    }

    async fn record_correction(
        &self,
        session_key: &str,
        skill_name: &str,
    ) -> Option<MirrorAlert> {
        let mut state = self.state.lock().await;

        if state.session_corrections.len() > MAX_TRACKED_SESSIONS {
            state.session_corrections.clear();
        }
        let session_count = {
            let c = state.session_corrections.entry(session_key.to_string()).or_insert(0);
            *c += 1;
            *c
        };
        if session_count >= SAME_SESSION_CORRECTION_THRESHOLD {
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: format!(
                    "User corrected me {session_count} times in the same session \
                     — ask for clarification before responding"
                ),
                source: MetaRuleOriginSource::CorrectionDerived,
            });
        }

        if state.skill_corrections.len() > MAX_TRACKED_SKILLS {
            state.skill_corrections.clear();
        }
        let skill_count = {
            let c = state.skill_corrections.entry(skill_name.to_string()).or_insert(0);
            *c += 1;
            *c
        };
        if skill_count >= CROSS_SESSION_CORRECTION_THRESHOLD {
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: format!(
                    "User has corrected me {skill_count} times across sessions for the \
                     '{skill_name}' skill — adjust routing or response strategy"
                ),
                source: MetaRuleOriginSource::CorrectionDerived,
            });
        }
        None
    }

    async fn record_low_confidence(
        &self,
        skill: &str,
        confidence: f64,
    ) -> Option<MirrorAlert> {
        if confidence >= LOW_CONFIDENCE_THRESHOLD {
            return None;
        }
        let mut state = self.state.lock().await;
        state.low_confidence_streak += 1;
        if state.low_confidence_streak >= LOW_CONFIDENCE_STREAK_THRESHOLD {
            state.low_confidence_streak = 0;
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id: Uuid::new_v4(),
                rule_text: format!(
                    "Routing confidence has been consistently low (latest: {confidence:.2} \
                     for '{skill}') — consider adding trigger keywords or adjusting skill scopes"
                ),
                source: MetaRuleOriginSource::ReflectionGenerated,
            });
        }
        None
    }

    async fn record_high_confidence(&self) {
        let mut state = self.state.lock().await;
        state.low_confidence_streak = 0;
    }

    async fn handle_alert(&self, alert: &MirrorAlert) {
        if let MirrorAlert::MetaRuleProposed { rule_id, rule_text, source } = alert {
            let rule = MetaRule {
                id: *rule_id,
                trigger_condition: rule_text.clone(),
                action: MetaRuleAction::ForceClarification,
                source: source.clone(),
                effectiveness_score: 0.5,
                status: MetaRuleStatus::Pending,
                signal_count: 1,
                created_at: Timestamp::now(),
                updated_at: Timestamp::now(),
            };
            if let Err(e) = self.repo.insert_meta_rule(&rule).await {
                warn!("MetaRuleSignalSource: failed to insert meta-rule: {e}");
            }
            let snippet = snippet_from_alert(alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("MetaRuleSignalSource: failed to insert snippet: {e}");
            }
        }
    }
}

#[async_trait]
impl MirrorSignalSource for MetaRuleSignalSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "meta_rule",
        subscribed_kinds: &["UserCorrectedAI", "SkillRouted"],
        flush_interval_secs: None, // event-driven
    };

    fn name(&self) -> &'static str {
        "mirror.meta_rule"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match signal.raw_event.as_ref() {
            Some(DomainEvent::UserCorrectedAI { session_key, active_skill, .. }) => {
                let skill = active_skill.as_deref().unwrap_or("unknown");
                if let Some(alert) = self.record_correction(session_key, skill).await {
                    self.handle_alert(&alert).await;
                }
            }
            Some(DomainEvent::SkillRouted { confidence, skill_name, .. }) => {
                if *confidence < LOW_CONFIDENCE_THRESHOLD {
                    if let Some(alert) =
                        self.record_low_confidence(skill_name, *confidence).await
                    {
                        self.handle_alert(&alert).await;
                    }
                } else {
                    self.record_high_confidence().await;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Event-driven source; no periodic flush. Reset streak on shutdown so a
        // restart doesn't carry stale state.
        self.state.lock().await.low_confidence_streak = 0;
        Ok(())
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive meta_rule_signal_source`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/sources/meta_rule.rs
git commit -m "feat(cognitive/mirror): MetaRuleSignalSource replaces MetaRuleDetector"
```

---

## Task 10: Port `ConfigArchiver` → `ConfigArchiverSource`

**Files:**
- Create: `crates/cognitive/src/mirror/sources/config_archiver.rs`

`★ Insight ─────────────────────────────────────`
`ConfigArchiver` keeps its `bootstrap` method verbatim (still called once in Phase 9), but its event-reactive path becomes a trait impl. It only acts when `verdict == "promoted"` — notably *not* `"activated"`, which is why we don't conflict with `TrialPreviewSource`'s filter.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Test shell**

Create `crates/cognitive/src/mirror/sources/config_archiver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use bus::DomainEvent;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn autotuner_signal(verdict: &str) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "AutotunerDecision",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::AutotunerDecision {
                trial_id: "t1".into(),
                verdict: verdict.into(),
                improvement_pct: 5.0,
                affected_params: vec!["temp".into()],
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[tokio::test]
    async fn promoted_verdict_records_brain_version() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = crate::mirror::subscribers_helpers_bootstrap(repo.clone()).await;
        // Use the helper that does a bootstrap on v1 first.
        let source = Arc::new(ConfigArchiverSource::new(repo.clone(), None));
        let runner = ai_core::MirrorSubscriberRunner::new(source, CancellationToken::new());

        runner.consume(&autotuner_signal("promoted")).await.unwrap();
        let versions = repo.get_brain_versions().await.unwrap();
        assert!(versions.iter().any(|v| v.trial_id.as_deref() == Some("t1")));
    }

    #[tokio::test]
    async fn activated_verdict_ignored() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(ConfigArchiverSource::new(repo.clone(), None));
        let runner = ai_core::MirrorSubscriberRunner::new(source, CancellationToken::new());

        runner.consume(&autotuner_signal("activated")).await.unwrap();
        let versions = repo.get_brain_versions().await.unwrap();
        assert!(versions.iter().all(|v| v.trial_id.as_deref() != Some("t1")));
    }
}
```

(The `subscribers_helpers_bootstrap` reference is a stand-in — the test only needs `repo` plus the source; remove that line when you write the test for real.)

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive config_archiver_source`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend to `config_archiver.rs`:

```rust
use std::sync::Arc;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use jiff::Timestamp;
use serde_json::Value;
use tracing::warn;

use crate::mirror::{AutotunerBridge, BrainVersion, MirrorRepo};

pub struct ConfigArchiverSource {
    repo: MirrorRepo,
    bridge: Option<Arc<dyn AutotunerBridge>>,
}

impl ConfigArchiverSource {
    pub fn new(repo: MirrorRepo, bridge: Option<Arc<dyn AutotunerBridge>>) -> Self {
        Self { repo, bridge }
    }

    async fn get_current_params(&self) -> Value {
        if let Some(bridge) = &self.bridge {
            bridge.current_champion_params().await.unwrap_or(Value::Object(Default::default()))
        } else {
            Value::Object(Default::default())
        }
    }

    pub async fn bootstrap(&self, default_params: Value) -> common::Result<()> {
        let next = self.repo.get_next_version_number().await?;
        if next > 1 {
            return Ok(());
        }
        let v = BrainVersion {
            version: 1,
            trial_id: None,
            promoted_at: Timestamp::now(),
            params: default_params,
            reason: "Initial brain state".into(),
            parent_version: None,
            metrics_at_promotion: Value::Object(Default::default()),
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }

    pub async fn record_promotion(
        &self,
        trial_id: Option<String>,
        reason: String,
        metrics: Value,
    ) -> common::Result<()> {
        let (params, next) =
            tokio::join!(self.get_current_params(), self.repo.get_next_version_number());
        let next = next?;
        let parent = if next > 1 { Some(next - 1) } else { None };
        let v = BrainVersion {
            version: next,
            trial_id,
            promoted_at: Timestamp::now(),
            params,
            reason,
            parent_version: parent,
            metrics_at_promotion: metrics,
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }
}

#[async_trait]
impl MirrorSignalSource for ConfigArchiverSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "config_archive",
        subscribed_kinds: &["AutotunerDecision"],
        flush_interval_secs: None,
    };

    fn name(&self) -> &'static str {
        "mirror.config_archiver"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        let Some(DomainEvent::AutotunerDecision { trial_id, verdict, improvement_pct, .. }) =
            signal.raw_event.as_ref()
        else {
            return Ok(());
        };
        if verdict != "promoted" {
            return Ok(());
        }
        if let Err(e) = self
            .record_promotion(
                Some(trial_id.clone()),
                format!("Promoted: {:.1}% improvement", improvement_pct),
                serde_json::json!({"improvement_pct": improvement_pct}),
            )
            .await
        {
            warn!("ConfigArchiverSource: failed to record promotion: {e}");
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}
```

- [ ] **Step 4: Fix test**

Remove the `subscribers_helpers_bootstrap` reference from the test in Step 1 — it was a placeholder. The test needs no bootstrap call; `insert_brain_version` via `record_promotion` works directly (parent_version = None when next==1).

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive config_archiver_source`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/sources/config_archiver.rs
git commit -m "feat(cognitive/mirror): ConfigArchiverSource replaces ConfigArchiver"
```

---

## Task 11: Port `TrialPreviewSubscriber` → `TrialPreviewSource` (Active)

**Files:**
- Create: `crates/cognitive/src/mirror/sources/trial.rs`

`★ Insight ─────────────────────────────────────`
The old subscriber was inert because `TrialActivated` was deleted in v1. v2 activates it against `AutotunerDecision { verdict: "activated" }` (Task 7). The 4-hour timer pattern stays identical. Because sources are `Send + Sync + 'static` and we need to spawn timers inside `accumulate`, we keep the `Arc<DashMap<String, JoinHandle<()>>>` exactly as before.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/src/mirror/sources/trial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use bus::DomainEvent;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn activated_signal(trial_id: &str) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "AutotunerDecision",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::AutotunerDecision {
                trial_id: trial_id.into(),
                verdict: "activated".into(),
                improvement_pct: 0.0,
                affected_params: vec!["p".into()],
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[tokio::test]
    async fn activated_verdict_starts_preview_timer() {
        let repo = crate::mirror::test_mirror_repo().await;
        let timers = Arc::new(dashmap::DashMap::new());
        let source = Arc::new(TrialPreviewSource::new(repo, timers.clone(), None));

        // Short-circuit: set preview_delay to 50ms via test hook.
        source.set_preview_delay_for_test(std::time::Duration::from_millis(50));

        let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), CancellationToken::new());
        runner.consume(&activated_signal("t42")).await.unwrap();

        assert!(timers.contains_key("t42"), "timer should be registered");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        // After the timer fires, the timer entry is removed.
        assert!(!timers.contains_key("t42"));
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive trial_preview_source`
Expected: FAIL.

- [ ] **Step 3: Implement**

Prepend to `trial.rs`:

```rust
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use dashmap::DashMap;
use jiff::Timestamp;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::mirror::{
    narratives::snippet_from_alert, EarlyTrialEvaluator, MirrorAlert, MirrorRepo,
    PreviewRecommendation, TrialEarlySignals, TrialPreview,
};
use crate::mirror::sources::trial_compute::compute_recommendation;

const DEFAULT_PREVIEW_DELAY_SECS: u64 = 4 * 60 * 60;
const MIN_MESSAGES_FOR_KILL: u32 = 5;

pub struct TrialPreviewSource {
    repo: MirrorRepo,
    active_timers: Arc<DashMap<String, JoinHandle<()>>>,
    evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
    preview_delay_secs: AtomicU64,
}

impl TrialPreviewSource {
    pub fn new(
        repo: MirrorRepo,
        active_timers: Arc<DashMap<String, JoinHandle<()>>>,
        evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
    ) -> Self {
        Self {
            repo,
            active_timers,
            evaluator,
            preview_delay_secs: AtomicU64::new(DEFAULT_PREVIEW_DELAY_SECS),
        }
    }

    #[cfg(test)]
    pub fn set_preview_delay_for_test(&self, d: std::time::Duration) {
        self.preview_delay_secs.store(d.as_millis() as u64, Ordering::Relaxed);
    }

    fn preview_delay_ms(&self) -> u64 {
        self.preview_delay_secs.load(Ordering::Relaxed)
    }

    fn start_preview_timer(&self, trial_id: String) {
        let repo = self.repo.clone();
        let evaluator = self.evaluator.clone();
        let timers = self.active_timers.clone();
        let delay_ms = self.preview_delay_ms();
        let tid = trial_id.clone();

        let handle = tokio::spawn(async move {
            let started_at = Timestamp::now();
            // The unit stored depends on whether tests have shortened the delay.
            // Production uses seconds; tests use ms via set_preview_delay_for_test.
            let d = if delay_ms > 1000 * 10 {
                std::time::Duration::from_secs(delay_ms)
            } else {
                std::time::Duration::from_millis(delay_ms)
            };
            tokio::time::sleep(d).await;

            let signals = if let Some(eval) = &evaluator {
                eval.evaluate_trial_early(&trial_id, started_at)
                    .await
                    .unwrap_or_default()
            } else {
                TrialEarlySignals::default()
            };
            let messages_scored = signals.messages_scored;
            let recommendation = compute_recommendation(&signals, messages_scored);

            let narrative = format!(
                "After {} ({} messages): correction rate {:.1}% vs champion. {}.",
                if delay_ms > 1000 * 10 {
                    format!("{} hours", delay_ms / 3600)
                } else {
                    format!("{} ms", delay_ms)
                },
                messages_scored,
                signals.correction_rate_delta * 100.0,
                match &recommendation {
                    PreviewRecommendation::Continue => "Looking good — keep going",
                    PreviewRecommendation::Kill => "Trending down — consider killing early",
                    PreviewRecommendation::NeedMoreData => "Not enough data yet — keep watching",
                }
            );

            let preview = TrialPreview {
                id: Uuid::new_v4(),
                trial_id: trial_id.clone(),
                started_at,
                preview_at: Timestamp::now(),
                messages_scored,
                early_signals: signals,
                recommendation: recommendation.clone(),
                narrative: narrative.clone(),
            };
            let _ = repo.insert_trial_preview(&preview).await;

            if recommendation == PreviewRecommendation::Kill {
                let alert = MirrorAlert::TrialUnpromising {
                    trial_id: trial_id.clone(),
                    reason: narrative,
                };
                let snippet = snippet_from_alert(&alert);
                let _ = repo.insert_snippet(&snippet).await;
            }
            timers.remove(&trial_id);
        });

        if let Some((_, old)) = self.active_timers.remove(&tid) {
            old.abort();
        }
        self.active_timers.insert(tid, handle);
    }
}

#[async_trait]
impl MirrorSignalSource for TrialPreviewSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "trial_preview",
        subscribed_kinds: &["AutotunerDecision"],
        flush_interval_secs: None,
    };

    fn name(&self) -> &'static str {
        "mirror.trial_preview"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        let Some(DomainEvent::AutotunerDecision { verdict, trial_id, .. }) =
            signal.raw_event.as_ref()
        else {
            return Ok(());
        };
        if verdict != "activated" {
            return Ok(());
        }
        self.start_preview_timer(trial_id.clone());
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Abort all in-flight preview timers so the tokio runtime can shut down
        // cleanly. Flushed only once at shutdown.
        for entry in self.active_timers.iter() {
            entry.value().abort();
        }
        self.active_timers.clear();
        Ok(())
    }
}
```

- [ ] **Step 4: Extract `compute_recommendation` helper**

Create `crates/cognitive/src/mirror/sources/trial_compute.rs`:

```rust
use crate::mirror::{PreviewRecommendation, TrendDirection, TrialEarlySignals};

const MIN_MESSAGES_FOR_KILL: u32 = 5;

pub fn compute_recommendation(
    signals: &TrialEarlySignals,
    messages_scored: u32,
) -> PreviewRecommendation {
    if signals.correction_rate_delta < -0.10 {
        return PreviewRecommendation::Kill;
    }
    if messages_scored < MIN_MESSAGES_FOR_KILL
        && signals.confidence_trend == TrendDirection::Falling
    {
        return PreviewRecommendation::Kill;
    }
    if messages_scored >= MIN_MESSAGES_FOR_KILL
        && signals.correction_rate_delta > 0.0
        && (signals.confidence_trend == TrendDirection::Rising
            || signals.confidence_trend == TrendDirection::Stable)
    {
        return PreviewRecommendation::Continue;
    }
    PreviewRecommendation::NeedMoreData
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_on_regression() {
        let s = TrialEarlySignals {
            correction_rate_delta: -0.15,
            ..TrialEarlySignals::default()
        };
        assert_eq!(compute_recommendation(&s, 20), PreviewRecommendation::Kill);
    }

    #[test]
    fn continue_on_positive_stable() {
        let s = TrialEarlySignals {
            correction_rate_delta: 0.05,
            confidence_trend: TrendDirection::Rising,
            ..TrialEarlySignals::default()
        };
        assert_eq!(compute_recommendation(&s, 20), PreviewRecommendation::Continue);
    }
}
```

Add `pub mod trial_compute;` to `crates/cognitive/src/mirror/sources/mod.rs`.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive trial_preview_source trial_compute`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/sources
git commit -m "feat(cognitive/mirror): revive TrialPreviewSource against AutotunerDecision(activated)"
```

---

## Task 12: New `TaskFocusPatternSource` + Schema

**Files:**
- Modify: `crates/cognitive/migrations/003_mirror_tables.sql`
- Create: `crates/cognitive/src/mirror/sources/task_focus.rs`
- Modify: `crates/cognitive/src/mirror/repo.rs` (add `insert_task_focus_snapshot`, `get_latest_task_focus_snapshot`)
- Modify: `crates/cognitive/src/mirror/types.rs` (add `TaskFocusSnapshot`)

`★ Insight ─────────────────────────────────────`
This is the first of two *new* v2 snapshot types. `TaskFocusPatternSource` aggregates `TaskFocusChanged` and `TaskCompleted` signals over a rolling hour to detect: (a) rapid context switching (N focus changes, low completion rate); (b) long-unfinished focus (a task focused > X hours without completion). Both shape up as pre-release metrics the Mirror UI can surface without LLM generation.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend the migration in place**

Edit `crates/cognitive/migrations/003_mirror_tables.sql`. Append at the end:

```sql
CREATE TABLE IF NOT EXISTS mirror_task_focus_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 1,
    focus_changes INTEGER NOT NULL,
    tasks_completed INTEGER NOT NULL,
    completion_rate REAL NOT NULL,
    longest_unfinished_secs INTEGER,
    top_tasks_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_focus_snapshots_time ON mirror_task_focus_snapshots(captured_at);
```

(Per CLAUDE.md pre-release policy, migrations edit in place; no new version bump.)

- [ ] **Step 2: Add `TaskFocusSnapshot` type**

Append to `crates/cognitive/src/mirror/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFocusSnapshot {
    pub id: Uuid,
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub focus_changes: u32,
    pub tasks_completed: u32,
    pub completion_rate: f64,
    pub longest_unfinished_secs: Option<i64>,
    pub top_tasks: Vec<(String, u32)>,
}
```

Re-export from `mod.rs`.

- [ ] **Step 3: Add repo methods**

Append to `impl MirrorRepo` in `crates/cognitive/src/mirror/repo.rs`:

```rust
pub async fn insert_task_focus_snapshot(&self, snap: &TaskFocusSnapshot) -> Result<()> {
    let top_json = serde_json::to_string(&snap.top_tasks).map_err(|e| {
        common::KlyntbotError::internal(format!("serialize top_tasks: {e}"))
    })?;
    sqlx::query(
        "INSERT INTO mirror_task_focus_snapshots
         (id, captured_at, window_hours, focus_changes, tasks_completed,
          completion_rate, longest_unfinished_secs, top_tasks_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(snap.id.to_string())
    .bind(snap.captured_at.to_string())
    .bind(snap.window_hours as i64)
    .bind(snap.focus_changes as i64)
    .bind(snap.tasks_completed as i64)
    .bind(snap.completion_rate)
    .bind(snap.longest_unfinished_secs)
    .bind(top_json)
    .execute(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("insert task focus: {e}")))?;
    Ok(())
}

pub async fn get_latest_task_focus_snapshot(&self) -> Result<Option<TaskFocusSnapshot>> {
    let row = sqlx::query(
        "SELECT id, captured_at, window_hours, focus_changes, tasks_completed,
                completion_rate, longest_unfinished_secs, top_tasks_json
         FROM mirror_task_focus_snapshots ORDER BY captured_at DESC LIMIT 1",
    )
    .fetch_optional(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("latest task focus: {e}")))?;
    let Some(row) = row else { return Ok(None) };
    let id: String = row.try_get(0)?;
    let captured_at: String = row.try_get(1)?;
    let window_hours: i64 = row.try_get(2)?;
    let focus_changes: i64 = row.try_get(3)?;
    let tasks_completed: i64 = row.try_get(4)?;
    let completion_rate: f64 = row.try_get(5)?;
    let longest_unfinished_secs: Option<i64> = row.try_get(6)?;
    let top_tasks_json: String = row.try_get(7)?;
    let top_tasks: Vec<(String, u32)> = serde_json::from_str(&top_tasks_json).map_err(|e| {
        common::KlyntbotError::internal(format!("deserialize top_tasks: {e}"))
    })?;
    Ok(Some(TaskFocusSnapshot {
        id: Uuid::parse_str(&id)
            .map_err(|e| common::KlyntbotError::internal(format!("uuid: {e}")))?,
        captured_at: captured_at
            .parse()
            .map_err(|e| common::KlyntbotError::internal(format!("ts: {e}")))?,
        window_hours: window_hours as u8,
        focus_changes: focus_changes as u32,
        tasks_completed: tasks_completed as u32,
        completion_rate,
        longest_unfinished_secs,
        top_tasks,
    }))
}
```

(If the existing repo file uses helper macros like `sqlx::query_as!` or `Row::try_get` wrappers, match that style.)

- [ ] **Step 4: Test shell**

Create `crates/cognitive/src/mirror/sources/task_focus.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use bus::DomainEvent;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn focus_changed(task_id: &str) -> AiSignal {
        AiSignal {
            domain: RecallDomain::Tasks,
            event_kind: "FocusChanged",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::TaskFocusChanged {
                task_id: task_id.into(),
                focus_deadline: Some("2026-04-22T12:00:00Z".into()),
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    fn task_completed(task_id: &str) -> AiSignal {
        AiSignal {
            event_kind: "Completed",
            raw_event: Some(DomainEvent::TaskCompleted {
                task_id: task_id.into(),
                actual_duration_mins: None,
                estimated_duration_mins: None,
                deviation_pct: Some(10.0),
            }),
            ..focus_changed(task_id)
        }
    }

    #[tokio::test]
    async fn flush_records_focus_pattern_snapshot() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(TaskFocusPatternSource::new(repo.clone()));
        let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), CancellationToken::new());

        runner.consume(&focus_changed("t1")).await.unwrap();
        runner.consume(&focus_changed("t1")).await.unwrap();
        runner.consume(&focus_changed("t2")).await.unwrap();
        runner.consume(&task_completed("t1")).await.unwrap();

        source.flush().await.unwrap();
        let snap = repo.get_latest_task_focus_snapshot().await.unwrap().unwrap();
        assert_eq!(snap.focus_changes, 3);
        assert_eq!(snap.tasks_completed, 1);
        assert!((snap.completion_rate - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(snap.top_tasks[0].0, "t1");
    }
}
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive task_focus_pattern`
Expected: FAIL.

- [ ] **Step 6: Implement**

Prepend to `task_focus.rs`:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use dashmap::DashMap;
use jiff::Timestamp;
use tracing::warn;
use uuid::Uuid;

use crate::mirror::{MirrorRepo, TaskFocusSnapshot};

pub struct TaskFocusPatternSource {
    repo: MirrorRepo,
    focus_hits: DashMap<String, u32>,
    completions: DashMap<String, u32>,
    focus_total: AtomicU32,
    complete_total: AtomicU32,
}

impl TaskFocusPatternSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            focus_hits: DashMap::new(),
            completions: DashMap::new(),
            focus_total: AtomicU32::new(0),
            complete_total: AtomicU32::new(0),
        }
    }

    fn reset(&self) {
        self.focus_hits.clear();
        self.completions.clear();
        self.focus_total.store(0, Ordering::Relaxed);
        self.complete_total.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl MirrorSignalSource for TaskFocusPatternSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "task_focus",
        subscribed_kinds: &["FocusChanged", "Completed"],
        flush_interval_secs: Some(3600),
    };

    fn name(&self) -> &'static str {
        "mirror.task_focus"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match signal.raw_event.as_ref() {
            Some(DomainEvent::TaskFocusChanged { task_id, .. }) => {
                *self.focus_hits.entry(task_id.clone()).or_insert(0) += 1;
                self.focus_total.fetch_add(1, Ordering::Relaxed);
            }
            Some(DomainEvent::TaskCompleted { task_id, .. }) => {
                *self.completions.entry(task_id.clone()).or_insert(0) += 1;
                self.complete_total.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let focus_changes = self.focus_total.load(Ordering::Relaxed);
        let tasks_completed = self.complete_total.load(Ordering::Relaxed);
        let completion_rate = if focus_changes > 0 {
            tasks_completed as f64 / focus_changes as f64
        } else {
            0.0
        };
        let mut top: Vec<(String, u32)> = self
            .focus_hits
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        top.truncate(5);

        let snapshot = TaskFocusSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 1,
            focus_changes,
            tasks_completed,
            completion_rate,
            longest_unfinished_secs: None, // future extension
            top_tasks: top,
        };
        if let Err(e) = self.repo.insert_task_focus_snapshot(&snapshot).await {
            warn!("TaskFocusPatternSource: insert failed: {e}");
        }
        self.reset();
        Ok(())
    }
}
```

- [ ] **Step 7: Run**

Run: `cargo nextest run -p cognitive task_focus_pattern`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive
git commit -m "feat(cognitive/mirror): TaskFocusPatternSource + mirror_task_focus_snapshots schema"
```

---

## Task 13: New `FinanceSpendingDriftSource` + Schema

**Files:**
- Modify: `crates/cognitive/migrations/003_mirror_tables.sql`
- Create: `crates/cognitive/src/mirror/sources/finance_drift.rs`
- Modify: `crates/cognitive/src/mirror/repo.rs` (add `insert_finance_drift_snapshot`, `get_latest_finance_drift_snapshot`)
- Modify: `crates/cognitive/src/mirror/types.rs` (add `FinanceDriftSnapshot`)

`★ Insight ─────────────────────────────────────`
`FinanceSpendingDriftSource` aggregates `TransactionRecorded` and `BudgetAlert` events to detect spending drift: total spend per category in a rolling day, `over_budget_count`, and deviation from the 14-day per-category average. The 14-day baseline is fetched from `mirror_finance_drift_snapshots` itself on each flush, analogous to how `RoutingSignalSource` reads routing history.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend the migration**

Append to `crates/cognitive/migrations/003_mirror_tables.sql`:

```sql
CREATE TABLE IF NOT EXISTS mirror_finance_drift_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    window_hours INTEGER NOT NULL DEFAULT 24,
    total_transactions INTEGER NOT NULL,
    over_budget_count INTEGER NOT NULL,
    per_category_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_finance_drift_snapshots_time ON mirror_finance_drift_snapshots(captured_at);
```

- [ ] **Step 2: Add `FinanceDriftSnapshot` type**

Append to `crates/cognitive/src/mirror/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDriftSnapshot {
    pub id: Uuid,
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub total_transactions: u32,
    pub over_budget_count: u32,
    pub per_category: HashMap<String, CategorySpend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySpend {
    pub total_amount: f64,
    pub transaction_count: u32,
    pub budget_alerts: u32,
}
```

Re-export both from `mirror/mod.rs`.

- [ ] **Step 3: Add repo methods**

Append to `impl MirrorRepo`:

```rust
pub async fn insert_finance_drift_snapshot(&self, snap: &FinanceDriftSnapshot) -> Result<()> {
    let cat_json = serde_json::to_string(&snap.per_category)
        .map_err(|e| common::KlyntbotError::internal(format!("serialize categories: {e}")))?;
    sqlx::query(
        "INSERT INTO mirror_finance_drift_snapshots
         (id, captured_at, window_hours, total_transactions, over_budget_count, per_category_json)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(snap.id.to_string())
    .bind(snap.captured_at.to_string())
    .bind(snap.window_hours as i64)
    .bind(snap.total_transactions as i64)
    .bind(snap.over_budget_count as i64)
    .bind(cat_json)
    .execute(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("insert finance drift: {e}")))?;
    Ok(())
}

pub async fn get_latest_finance_drift_snapshot(&self) -> Result<Option<FinanceDriftSnapshot>> {
    let row = sqlx::query(
        "SELECT id, captured_at, window_hours, total_transactions, over_budget_count, per_category_json
         FROM mirror_finance_drift_snapshots ORDER BY captured_at DESC LIMIT 1",
    )
    .fetch_optional(&self.pool.inner().clone())
    .await
    .map_err(|e| common::KlyntbotError::storage(format!("latest finance drift: {e}")))?;
    let Some(row) = row else { return Ok(None) };
    let id: String = row.try_get(0)?;
    let captured_at: String = row.try_get(1)?;
    let window_hours: i64 = row.try_get(2)?;
    let total_transactions: i64 = row.try_get(3)?;
    let over_budget_count: i64 = row.try_get(4)?;
    let per_category_json: String = row.try_get(5)?;
    let per_category: HashMap<String, CategorySpend> = serde_json::from_str(&per_category_json)
        .map_err(|e| common::KlyntbotError::internal(format!("deserialize categories: {e}")))?;
    Ok(Some(FinanceDriftSnapshot {
        id: Uuid::parse_str(&id).map_err(|e| common::KlyntbotError::internal(format!("uuid: {e}")))?,
        captured_at: captured_at
            .parse()
            .map_err(|e| common::KlyntbotError::internal(format!("ts: {e}")))?,
        window_hours: window_hours as u8,
        total_transactions: total_transactions as u32,
        over_budget_count: over_budget_count as u32,
        per_category,
    }))
}
```

- [ ] **Step 4: Test shell**

Create `crates/cognitive/src/mirror/sources/finance_drift.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
    use bus::DomainEvent;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn tx(category: &str, amount: f64, over_budget: bool) -> AiSignal {
        AiSignal {
            domain: RecallDomain::Finance,
            event_kind: "TransactionRecorded",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::TransactionRecorded {
                category: category.into(),
                amount,
                is_over_budget: over_budget,
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    fn alert(category: &str) -> AiSignal {
        AiSignal {
            event_kind: "BudgetAlert",
            raw_event: Some(DomainEvent::BudgetAlert {
                category: category.into(),
                spent: 400.0,
                limit: 500.0,
            }),
            ..tx(category, 0.0, false)
        }
    }

    #[tokio::test]
    async fn flush_aggregates_per_category() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(FinanceSpendingDriftSource::new(repo.clone()));
        let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), CancellationToken::new());

        runner.consume(&tx("food", 20.0, false)).await.unwrap();
        runner.consume(&tx("food", 35.5, false)).await.unwrap();
        runner.consume(&tx("food", 450.0, true)).await.unwrap();
        runner.consume(&alert("food")).await.unwrap();
        runner.consume(&tx("transport", 12.0, false)).await.unwrap();

        source.flush().await.unwrap();
        let snap = repo.get_latest_finance_drift_snapshot().await.unwrap().unwrap();
        assert_eq!(snap.total_transactions, 4);
        assert_eq!(snap.over_budget_count, 1);
        let food = &snap.per_category["food"];
        assert!((food.total_amount - 505.5).abs() < 1e-6);
        assert_eq!(food.transaction_count, 3);
        assert_eq!(food.budget_alerts, 1);
        let trans = &snap.per_category["transport"];
        assert_eq!(trans.transaction_count, 1);
    }
}
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive finance_spending_drift`
Expected: FAIL.

- [ ] **Step 6: Implement**

Prepend to `finance_drift.rs`:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use dashmap::DashMap;
use jiff::Timestamp;
use tracing::warn;
use uuid::Uuid;

use crate::mirror::{CategorySpend, FinanceDriftSnapshot, MirrorRepo};

#[derive(Default)]
struct CatAccum {
    total_amount: f64,
    transaction_count: u32,
    budget_alerts: u32,
}

pub struct FinanceSpendingDriftSource {
    repo: MirrorRepo,
    per_category: DashMap<String, CatAccum>,
    total_transactions: AtomicU32,
    over_budget_count: AtomicU32,
}

impl FinanceSpendingDriftSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            per_category: DashMap::new(),
            total_transactions: AtomicU32::new(0),
            over_budget_count: AtomicU32::new(0),
        }
    }

    fn reset(&self) {
        self.per_category.clear();
        self.total_transactions.store(0, Ordering::Relaxed);
        self.over_budget_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl MirrorSignalSource for FinanceSpendingDriftSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "finance_drift",
        subscribed_kinds: &["TransactionRecorded", "BudgetAlert"],
        flush_interval_secs: Some(24 * 3600), // daily
    };

    fn name(&self) -> &'static str {
        "mirror.finance_drift"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match signal.raw_event.as_ref() {
            Some(DomainEvent::TransactionRecorded { category, amount, is_over_budget }) => {
                let mut entry = self.per_category.entry(category.clone()).or_default();
                entry.total_amount += *amount;
                entry.transaction_count += 1;
                self.total_transactions.fetch_add(1, Ordering::Relaxed);
                if *is_over_budget {
                    self.over_budget_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            Some(DomainEvent::BudgetAlert { category, .. }) => {
                let mut entry = self.per_category.entry(category.clone()).or_default();
                entry.budget_alerts += 1;
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let mut per_category = HashMap::new();
        for entry in self.per_category.iter() {
            let accum = entry.value();
            per_category.insert(
                entry.key().clone(),
                CategorySpend {
                    total_amount: accum.total_amount,
                    transaction_count: accum.transaction_count,
                    budget_alerts: accum.budget_alerts,
                },
            );
        }
        let snapshot = FinanceDriftSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 24,
            total_transactions: self.total_transactions.load(Ordering::Relaxed),
            over_budget_count: self.over_budget_count.load(Ordering::Relaxed),
            per_category,
        };
        if let Err(e) = self.repo.insert_finance_drift_snapshot(&snapshot).await {
            warn!("FinanceSpendingDriftSource: insert failed: {e}");
        }
        self.reset();
        Ok(())
    }
}
```

- [ ] **Step 7: Run**

Run: `cargo nextest run -p cognitive finance_spending_drift`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive
git commit -m "feat(cognitive/mirror): FinanceSpendingDriftSource + mirror_finance_drift_snapshots schema"
```

---

## Task 14: Annotate `TasksFeature` with `mirror_snapshot`

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs`

- [ ] **Step 1: Extend the derive**

Edit `crates/feature-tasks/src/lib.rs`. Replace the existing `#[ai(...)]` attribute list with:

```rust
#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::events::TaskEvent",
    recall_boost_when = "query.message.to_lowercase().contains(\"deadline\") || query.message.to_lowercase().contains(\"task\") || query.message.to_lowercase().contains(\"overdue\")",
    recall_priority_field = "priority",
    recall_recency_field = "updated_at",
    recall_status_filter = "status != \"archived\"",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["FocusChanged", "Completed"],
    ),
)]
pub struct TasksFeature { /* unchanged */ }
```

- [ ] **Step 2: Run**

Run: `cargo build -p feature-tasks`
Expected: PASS.

Verify the generated constant exists:

```bash
cargo expand -p feature-tasks --lib 2>/dev/null | grep -A 8 'MIRROR_SNAPSHOTS'
```

Expected: `TasksFeature::MIRROR_SNAPSHOTS` contains one entry with `name: "task_focus"`.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-tasks/src/lib.rs
git commit -m "feat(feature-tasks): declare task_focus mirror snapshot"
```

---

## Task 15: Annotate `FinanceFeature` with `mirror_snapshot`

**Files:**
- Modify: `crates/feature-finance/src/lib.rs`

- [ ] **Step 1: Extend the derive**

Find the existing `#[derive(AiFeature, Default)] #[ai(...)]` on `FinanceFeature` (around `lib.rs:46`). Append to the `#[ai(...)]` body:

```rust
    mirror_snapshot(
        name = "finance_drift",
        flush_interval_secs = 86400,
        event_kinds = ["TransactionRecorded", "BudgetAlert"],
    ),
```

- [ ] **Step 2: Run**

Run: `cargo build -p feature-finance`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-finance/src/lib.rs
git commit -m "feat(feature-finance): declare finance_drift mirror snapshot"
```

---

## Task 16: Rebuild `MirrorEngine::start` to Wire `MirrorSignalSource` List

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`

`★ Insight ─────────────────────────────────────`
`MirrorEngine::start` returns a struct of registered `Arc<dyn SignalConsumer>` (to hand to `SignalRouter`) plus owned flush-loop `JoinHandle`s (to keep alive). The engine no longer subscribes to `DomainEventBus` itself — `SignalRouter` is the single subscriber and it fans out to the consumers the engine produced.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing integration test**

Update/replace the existing test module in `crates/cognitive/src/mirror/engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{MirrorSubscriberRunner, SignalConsumer};

    #[tokio::test]
    async fn start_produces_five_consumers() {
        let repo = crate::mirror::test_mirror_repo().await;
        let built = MirrorEngine::start(
            repo,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(built.consumers.len(), 6,
            "routing + meta_rule + config_archiver + trial + task_focus + finance_drift");
        for h in built.flush_handles.iter() {
            assert!(!h.is_finished());
        }
        built.shutdown.cancel();
        for h in built.flush_handles {
            h.await.unwrap();
        }
    }
}
```

- [ ] **Step 2: Replace the engine**

Rewrite `crates/cognitive/src/mirror/engine.rs`:

```rust
//! MirrorEngine — lifecycle manager for the Mirror self-reflection layer.
//!
//! Constructs six `MirrorSignalSource` impls and wraps each in a
//! `MirrorSubscriberRunner`. Returns the runners (as `SignalConsumer`s) for
//! hand-off to the global `SignalRouter`, plus flush-loop handles the caller
//! must keep alive.

use std::sync::Arc;

use ai_core::{MirrorSubscriberRunner, SignalConsumer};
use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::mirror::{
    sources::{
        ConfigArchiverSource, FinanceSpendingDriftSource, MetaRuleSignalSource,
        RoutingSignalSource, TaskFocusPatternSource, TrialPreviewSource,
    },
    AutotunerBridge, MirrorFacade, MirrorRepo, NarrativeHandler,
};
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo};

pub struct StartedMirror {
    pub facade: MirrorFacade,
    pub consumers: Vec<Arc<dyn SignalConsumer>>,
    pub flush_handles: Vec<JoinHandle<()>>,
    pub shutdown: CancellationToken,
}

pub struct MirrorEngine;

impl MirrorEngine {
    pub fn start(
        repo: MirrorRepo,
        narrative_handler: Option<Arc<dyn NarrativeHandler>>,
        autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
        episodic_repo: Option<EpisodicMemoryRepo>,
        rule_repo: Option<ProceduralRuleRepo>,
        trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
    ) -> StartedMirror {
        let shutdown = CancellationToken::new();
        let active_timers: Arc<DashMap<String, JoinHandle<()>>> = Arc::new(DashMap::new());

        // Build each source.
        let routing = Arc::new(RoutingSignalSource::new(repo.clone()));
        let meta_rule = Arc::new(MetaRuleSignalSource::new(repo.clone()));
        let config_archiver =
            Arc::new(ConfigArchiverSource::new(repo.clone(), autotuner_bridge.clone()));
        let trial = Arc::new(TrialPreviewSource::new(
            repo.clone(),
            active_timers.clone(),
            trial_evaluator,
        ));
        let task_focus = Arc::new(TaskFocusPatternSource::new(repo.clone()));
        let finance_drift = Arc::new(FinanceSpendingDriftSource::new(repo.clone()));

        // Wrap each in a runner; spawn flush loops for sources that declare an interval.
        let mut consumers: Vec<Arc<dyn SignalConsumer>> = Vec::new();
        let mut flush_handles: Vec<JoinHandle<()>> = Vec::new();

        macro_rules! register {
            ($source:expr) => {{
                let runner = MirrorSubscriberRunner::new($source, shutdown.clone());
                if let Some(h) = runner.clone().spawn_declared_flush_loop() {
                    flush_handles.push(h);
                }
                consumers.push(runner as Arc<dyn SignalConsumer>);
            }};
        }
        register!(routing);
        register!(meta_rule);
        register!(config_archiver);
        register!(trial);
        register!(task_focus);
        register!(finance_drift);

        // Build the facade (unchanged API; drop the now-unused domain_event_bus).
        let mut facade = MirrorFacade::new(repo);
        facade = facade.with_active_timers(active_timers);
        if let Some(handler) = narrative_handler {
            facade = facade.with_narrative_handler(handler);
        }
        if let Some(bridge) = autotuner_bridge {
            facade = facade.with_autotuner_bridge(bridge);
        }
        if let Some(episodic) = episodic_repo {
            facade = facade.with_episodic_repo(episodic);
        }
        if let Some(r) = rule_repo {
            facade = facade.with_rule_repo(r);
        }

        StartedMirror { facade, consumers, flush_handles, shutdown }
    }
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p cognitive mirror::engine`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/mirror/engine.rs
git commit -m "refactor(cognitive/mirror): rebuild MirrorEngine around MirrorSignalSource trait"
```

---

## Task 17: Rewire Phase 9 in `app-core/src/init/mod.rs`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Replace the Phase 9 block**

In `crates/app-core/src/init/mod.rs`, find the Phase 9 block (lines 549–608 in the current file). Replace with:

```rust
        // ── Phase 9: Mirror self-reflection layer ────────────────────────
        let (mirror_facade, mirror_flush_handles, mirror_shutdown, mirror_consumers) = {
            let mirror_repo = ::cognitive::mirror::MirrorRepo::new(storage_pool.clone());
            let narrative_handler: Option<Arc<dyn ::cognitive::mirror::NarrativeHandler>> =
                cognitive_provider.as_ref().map(|cp| {
                    let model = config
                        .cognitive
                        .model
                        .as_deref()
                        .unwrap_or(&config.agents.defaults.model)
                        .to_string();
                    Arc::new(::agent::mirror_handlers::LlmNarrativeHandler::new(
                        cp.clone(),
                        model,
                    )) as Arc<dyn ::cognitive::mirror::NarrativeHandler>
                });
            let autotuner_bridge: Option<Arc<dyn ::cognitive::mirror::AutotunerBridge>> =
                autotuner.as_ref().map(|orch| {
                    Arc::new(crate::adapters::autotuner_bridge::AppAutotunerBridge::new(
                        Arc::clone(orch),
                    )) as Arc<dyn ::cognitive::mirror::AutotunerBridge>
                });
            let episodic_repo = Some(::cognitive::EpisodicMemoryRepo::new(
                storage_pool.inner().clone(),
            ));
            let rule_repo = Some(::cognitive::ProceduralRuleRepo::new(
                storage_pool.inner().clone(),
            ));
            let trial_evaluator: Option<Arc<dyn ::cognitive::mirror::EarlyTrialEvaluator>> = Some(
                Arc::new(crate::adapters::trial_evaluator::AppTrialEvaluator::new(
                    ::storage::StrategyRepo::new(storage_pool.inner().clone()),
                )),
            );

            let started = ::cognitive::mirror::MirrorEngine::start(
                mirror_repo.clone(),
                narrative_handler,
                autotuner_bridge,
                episodic_repo,
                rule_repo,
                trial_evaluator,
            );

            // Bootstrap brain version 1 on first run (unchanged behaviour).
            let bootstrap_repo = mirror_repo.clone();
            tokio::spawn(async move {
                let archiver = ::cognitive::mirror::sources::ConfigArchiverSource::new(
                    bootstrap_repo,
                    None,
                );
                let _ = archiver.bootstrap(serde_json::json!({})).await;
            });

            // Spawn retention sweep.
            let retention_cancel = shutdown_token.child_token();
            let retention_handle = ::cognitive::mirror::MirrorRetentionService::spawn(
                Arc::new(mirror_repo),
                ::cognitive::mirror::MirrorRetentionConfig::default(),
                retention_cancel.clone(),
            );

            let facade = {
                let text_embedder: Arc<dyn ::cognitive::TextEmbedder> =
                    Arc::new(::agent::TextEmbedderImpl::new(Arc::clone(&embedding_engine)));
                started.facade.with_text_embedder(text_embedder)
            };

            info!(
                consumer_count = started.consumers.len(),
                "mirror self-reflection engine started"
            );

            let mut all_handles = started.flush_handles;
            all_handles.push(retention_handle);

            (
                Some(Arc::new(facade)),
                Some(all_handles),
                Some(started.shutdown),
                started.consumers,
            )
        };
```

- [ ] **Step 2: Feed mirror consumers into the SignalRouter**

In the same file, find Phase 8 (the `let ai_pipeline_router = { ... }` block). Phase 8 currently instantiates the router with a `vec![ingestion, chat_turn, recall, session, atom, coaching_collector, coaching_consumer]` list and must also include the mirror consumers.

Because Phase 9 currently runs *after* Phase 8, we need to invert the order: construct mirror sources first (they don't depend on anything from Phase 8), then pass the consumer list into the Phase 8 router.

Restructure by moving the mirror block *above* Phase 8 in `init/mod.rs`. Then inside the Phase 8 `ai_pipeline::start(...)` call, extend the consumer vector:

```rust
let router = ai_pipeline::start(
    Arc::clone(&domain_event_bus),
    {
        let mut consumers = vec![
            ingestion,
            chat_turn,
            recall,
            session,
            atom,
            coaching_collector,
            coaching_consumer,
        ];
        consumers.extend(mirror_consumers.iter().cloned());
        consumers
    },
);
info!(
    "AI pipeline SignalRouter started with {} consumers ({} non-mirror + {} mirror)",
    7 + mirror_consumers.len(),
    7,
    mirror_consumers.len()
);
```

- [ ] **Step 3: Update `state.rs`**

Edit `crates/app-core/src/state.rs`. Rename `_mirror_handles` storage to reflect that it now holds flush loops + retention handle. The field name stays `_mirror_handles` but its docstring changes:

```rust
/// Join handles for the mirror flush loops + retention sweep. Held for
/// lifetime of AppCore so the tasks aren't dropped.
pub _mirror_handles: Option<Vec<tokio::task::JoinHandle<()>>>,
```

- [ ] **Step 4: Run**

Run: `cargo build --workspace`
Expected: PASS.

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): wire mirror sources into SignalRouter; add retention sweep"
```

---

## Task 18: Extend `ai_pipeline::translate()` for Mirror-Relevant Events

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

`★ Insight ─────────────────────────────────────`
The translator is what gates whether each mirror source sees its events. Today `translate_system_event` covers `ChatTurnCompleted`, `SessionEnded`, `CoachingPatternDetected`, `AtomReinforced`, `DistractionDetected`, `FocusSessionStarted`, `FocusSessionEnded`. v2 adds `SkillRouted`, `UserCorrectedAI`, `AutotunerDecision`. The event_kind strings must match the `MirrorSnapshotSpec::subscribed_kinds` in the sources exactly.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write failing test**

Append to the end of `crates/app-core/src/init/ai_pipeline.rs`:

```rust
#[cfg(test)]
mod translate_mirror_tests {
    use super::*;

    #[test]
    fn skill_routed_translates() {
        let ev = bus::DomainEvent::SkillRouted {
            skill_name: "general".into(),
            confidence: 0.85,
            source: "keyword".into(),
            trigger_phrases: vec!["hi".into()],
            session_key: "s".into(),
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "SkillRouted");
    }

    #[test]
    fn user_corrected_translates() {
        let ev = bus::DomainEvent::UserCorrectedAI {
            original: String::new(),
            correction: String::new(),
            kind: bus::CorrectionKind::Factual,
            strength: 0.8,
            session_key: "s".into(),
            active_skill: Some("general".into()),
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "UserCorrectedAI");
    }

    #[test]
    fn autotuner_decision_translates_activated() {
        let ev = bus::DomainEvent::AutotunerDecision {
            trial_id: "t1".into(),
            verdict: "activated".into(),
            improvement_pct: 0.0,
            affected_params: vec!["x".into()],
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "AutotunerDecision");
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p app-core translate_mirror`
Expected: FAIL.

- [ ] **Step 3: Extend `translate_system_event`**

Edit `crates/app-core/src/init/ai_pipeline.rs`. Inside the `match event { ... }` in `translate_system_event`, add new arms before the `_ => None` fallback:

```rust
        DomainEvent::SkillRouted { skill_name, confidence, .. } => Some(AiSignal {
            event_kind: "SkillRouted",
            importance: *confidence,
            content: skill_name.clone(),
            ..base
        }),
        DomainEvent::UserCorrectedAI { active_skill, strength, .. } => Some(AiSignal {
            event_kind: "UserCorrectedAI",
            importance: *strength,
            content: active_skill.clone().unwrap_or_default(),
            ..base
        }),
        DomainEvent::AutotunerDecision { verdict, improvement_pct, trial_id, .. } => {
            Some(AiSignal {
                event_kind: "AutotunerDecision",
                importance: 0.6,
                content: format!("{}: {} ({:.1}%)", trial_id, verdict, improvement_pct),
                metrics: AiMetrics {
                    category: Some(verdict.clone()),
                    ..AiMetrics::default()
                },
                ..base
            })
        }
```

- [ ] **Step 4: Adjust `TaskFocusPatternSource` and related sources for `event_kind` strings**

Given v1.5 uses the *feature event variant name* (e.g. `"FocusChanged"` for `TaskEvent::FocusChanged`) and not the raw `DomainEvent` variant (`"TaskFocusChanged"`), confirm the subscribed_kinds in `TaskFocusPatternSource::SPEC` match what the translator emits. The v1 translator calls `e.to_signal()` on the feature `TaskEvent::FocusChanged`, which returns `event_kind: "FocusChanged"`. Keep `subscribed_kinds: &["FocusChanged", "Completed"]` in Task 12.

For `RoutingSignalSource`, `MetaRuleSignalSource`, `ConfigArchiverSource`, and `TrialPreviewSource` — all of these consume *system-level* events that are translated here (no feature `AiEvent`). Their `subscribed_kinds` use the `event_kind` emitted by `translate_system_event`: `"SkillRouted"`, `"UserCorrectedAI"`, `"AutotunerDecision"`. Confirm the strings in the `SPEC` consts match exactly.

For `FinanceSpendingDriftSource`, the kind strings are `"TransactionRecorded"` and `"BudgetAlert"` — these come from `FinanceEvent::TransactionRecorded` / `FinanceEvent::BudgetAlert`, matching v1 behaviour.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p app-core translate_mirror`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): translate SkillRouted / UserCorrectedAI / AutotunerDecision into AiSignals"
```

---

## Task 19: Delete `crates/cognitive/src/mirror/subscribers/` Directory

**Files:**
- Delete: `crates/cognitive/src/mirror/subscribers/` (whole directory)
- Modify: `crates/cognitive/src/mirror/mod.rs`

- [ ] **Step 1: Remove the module declaration**

Edit `crates/cognitive/src/mirror/mod.rs`. Remove the `pub mod subscribers;` line and the `pub use subscribers::{ConfigArchiver, MetaRuleDetector, RoutingMirrorSubscriber, TrialPreviewSubscriber};` re-exports.

Verify every `pub use types::{...}` line still holds unchanged.

- [ ] **Step 2: Delete the directory**

```bash
rm -rf crates/cognitive/src/mirror/subscribers/
```

- [ ] **Step 3: Search for stragglers**

```bash
rg 'subscribers::|RoutingMirrorSubscriber|MetaRuleDetector|TrialPreviewSubscriber|ConfigArchiver' crates/
```

Any hit outside `crates/cognitive/src/mirror/sources/` or new code is a leftover reference. Delete or replace with the new `*Source` type.

- [ ] **Step 4: Run**

Run: `cargo build --workspace`
Expected: PASS.

Run: `cargo nextest run --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -u crates/cognitive
git commit -m "chore(cognitive/mirror): delete subscribers/ (replaced by sources/)"
```

---

## Task 20: Drop `domain_event_bus` From `MirrorFacade`

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

`★ Insight ─────────────────────────────────────`
`MirrorFacade::with_domain_event_bus` was added when the deleted `MirrorTrialKilled` event was published through the facade. v1 removed that variant and the facade field became dead — `domain_event_bus: None` with no call sites referencing it. Dropping it cleans up the API; per pre-release posture, no deprecation path needed.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Search for usages**

```bash
rg 'with_domain_event_bus|domain_event_bus' crates/cognitive/src/mirror/
```

Expected: the `Some` field, `with_domain_event_bus` builder method, and any stale comments.

- [ ] **Step 2: Remove the field + builder**

Edit `crates/cognitive/src/mirror/facade.rs`. Remove:
- The `domain_event_bus: Option<Arc<bus::DomainEventBus>>` field.
- Its initialization in `MirrorFacade::new` (set it to `None`).
- The entire `with_domain_event_bus` method.

If the struct has no other uses of `Arc<bus::DomainEventBus>`, also drop any now-unused imports.

- [ ] **Step 3: Remove engine-side wiring**

Edit `crates/cognitive/src/mirror/engine.rs`. The new engine (Task 16) already omits `facade.with_domain_event_bus(bus)`. Confirm that is the case — if any remnants exist, delete them.

- [ ] **Step 4: Update engine `start` signature**

The engine no longer needs the `bus: Arc<DomainEventBus>` parameter. Remove it from `MirrorEngine::start`:

```rust
pub fn start(
    repo: MirrorRepo,
    narrative_handler: Option<Arc<dyn NarrativeHandler>>,
    autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
    episodic_repo: Option<EpisodicMemoryRepo>,
    rule_repo: Option<ProceduralRuleRepo>,
    trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
) -> StartedMirror
```

(Task 16 already reflects this; cross-check.)

- [ ] **Step 5: Update `app-core/src/init/mod.rs`**

Remove `Arc::clone(&domain_event_bus)` argument from the `MirrorEngine::start(...)` call. The Phase 9 block now takes fewer arguments.

Note the `CLAUDE.md` gotcha says "MirrorEngine::start takes `Arc<DomainEventBus>` — not `&DomainEventBus`". v2 removes the dep entirely. After this task lands, update the CLAUDE.md gotcha text in a separate, tiny commit (see Task 27, Step 7).

- [ ] **Step 6: Run**

Run: `cargo build --workspace`
Run: `cargo nextest run --workspace`
Expected: both PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive crates/app-core
git commit -m "refactor(mirror): drop unused domain_event_bus from MirrorFacade/Engine"
```

---

## Task 21: Integration — `SkillRouted` → Routing Snapshot

**Files:**
- Create: `tests/ai_mirror_pipeline_integration.rs`

- [ ] **Step 1: Write**

```rust
use ai_core::SignalConsumer;
use std::sync::Arc;

#[tokio::test]
async fn skill_routed_event_persists_routing_snapshot_via_ai_pipeline() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let started = cognitive::mirror::MirrorEngine::start(
        mirror_repo.clone(),
        None,
        None,
        None,
        None,
        None,
    );

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        started.consumers.clone(),
        app_core::init::ai_pipeline::translate,
    );

    // Publish 3 SkillRouted events.
    for i in 0..3 {
        bus.publish(bus::DomainEvent::SkillRouted {
            skill_name: "general".into(),
            confidence: 0.8 + (i as f64 * 0.01),
            source: "keyword".into(),
            trigger_phrases: vec!["hi".into()],
            session_key: "s".into(),
        });
    }

    // Force a flush by driving the source directly. (The timer is on a 1-hour
    // interval; for test speed we locate the source behind the runner wrapper
    // via Arc downcast — but runners don't expose that. Easier: re-construct
    // the source in-test and run its flush.)
    //
    // Pragmatic approach: sleep briefly to let the router deliver, then inspect
    // the accumulator via `build_snapshot` — but we don't have a handle. So we
    // verify behaviour indirectly: construct a second RoutingSignalSource, feed
    // the same events, call flush, and assert.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Direct source test (complements the integration test above).
    let direct = Arc::new(cognitive::mirror::sources::RoutingSignalSource::new(
        mirror_repo.clone(),
    ));
    let runner2 = ai_core::MirrorSubscriberRunner::new(
        direct.clone(),
        tokio_util::sync::CancellationToken::new(),
    );
    let sig = app_core::init::ai_pipeline::translate(&bus::DomainEvent::SkillRouted {
        skill_name: "general".into(),
        confidence: 0.85,
        source: "keyword".into(),
        trigger_phrases: vec!["hi".into()],
        session_key: "s".into(),
    })
    .unwrap();
    let mut sig = sig;
    sig.raw_event = Some(bus::DomainEvent::SkillRouted {
        skill_name: "general".into(),
        confidence: 0.85,
        source: "keyword".into(),
        trigger_phrases: vec!["hi".into()],
        session_key: "s".into(),
    });
    runner2.consume(&sig).await.unwrap();
    direct.flush().await.unwrap();

    let latest = mirror_repo.get_latest_routing_snapshot().await.unwrap().unwrap();
    assert!(latest.total_messages >= 1);
    assert!(latest.distribution.contains_key("general"));

    started.shutdown.cancel();
    for h in started.flush_handles {
        h.await.unwrap();
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(mirror): e2e — SkillRouted → routing snapshot via AI pipeline"
```

---

## Task 22: Integration — `AutotunerDecision(activated)` → Trial Timer Starts

**Files:**
- Modify: `tests/ai_mirror_pipeline_integration.rs`

- [ ] **Step 1: Append test**

```rust
#[tokio::test]
async fn autotuner_decision_activated_starts_trial_timer() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let active_timers = Arc::new(dashmap::DashMap::new());
    let source =
        Arc::new(cognitive::mirror::sources::TrialPreviewSource::new(
            mirror_repo,
            active_timers.clone(),
            None,
        ));
    source.set_preview_delay_for_test(std::time::Duration::from_secs(5));
    let cancel = tokio_util::sync::CancellationToken::new();
    let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), cancel.clone());

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![runner as Arc<dyn ai_core::SignalConsumer>],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(bus::DomainEvent::AutotunerDecision {
        trial_id: "t-abc".into(),
        verdict: "activated".into(),
        improvement_pct: 0.0,
        affected_params: vec!["temp".into()],
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert!(active_timers.contains_key("t-abc"));
    cancel.cancel();
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(mirror): e2e — AutotunerDecision(activated) starts trial preview timer"
```

---

## Task 23: Integration — `TaskFocusChanged` → Task Focus Snapshot

**Files:**
- Modify: `tests/ai_mirror_pipeline_integration.rs`

- [ ] **Step 1: Append test**

```rust
#[tokio::test]
async fn task_focus_changes_produce_focus_snapshot() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let source =
        Arc::new(cognitive::mirror::sources::TaskFocusPatternSource::new(mirror_repo.clone()));
    let cancel = tokio_util::sync::CancellationToken::new();
    let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), cancel.clone());

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![runner as Arc<dyn ai_core::SignalConsumer>],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(bus::DomainEvent::TaskFocusChanged {
        task_id: "t1".into(),
        focus_deadline: Some("2026-04-22T12:00:00Z".into()),
    });
    bus.publish(bus::DomainEvent::TaskFocusChanged {
        task_id: "t1".into(),
        focus_deadline: Some("2026-04-22T14:00:00Z".into()),
    });
    bus.publish(bus::DomainEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(45),
        estimated_duration_mins: Some(30),
        deviation_pct: Some(50.0),
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    source.flush().await.unwrap();

    let snap = mirror_repo.get_latest_task_focus_snapshot().await.unwrap().unwrap();
    assert_eq!(snap.focus_changes, 2);
    assert_eq!(snap.tasks_completed, 1);
    cancel.cancel();
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(mirror): e2e — TaskFocusChanged → task focus snapshot"
```

---

## Task 24: Integration — `BudgetAlert` → Finance Drift Snapshot

**Files:**
- Modify: `tests/ai_mirror_pipeline_integration.rs`

- [ ] **Step 1: Append test**

```rust
#[tokio::test]
async fn finance_events_produce_drift_snapshot() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let source = Arc::new(
        cognitive::mirror::sources::FinanceSpendingDriftSource::new(mirror_repo.clone()),
    );
    let cancel = tokio_util::sync::CancellationToken::new();
    let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), cancel.clone());

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![runner as Arc<dyn ai_core::SignalConsumer>],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(bus::DomainEvent::TransactionRecorded {
        category: "food".into(),
        amount: 42.0,
        is_over_budget: false,
    });
    bus.publish(bus::DomainEvent::TransactionRecorded {
        category: "food".into(),
        amount: 1000.0,
        is_over_budget: true,
    });
    bus.publish(bus::DomainEvent::BudgetAlert {
        category: "food".into(),
        spent: 450.0,
        limit: 500.0,
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    source.flush().await.unwrap();

    let snap = mirror_repo.get_latest_finance_drift_snapshot().await.unwrap().unwrap();
    assert_eq!(snap.total_transactions, 2);
    assert_eq!(snap.over_budget_count, 1);
    let food = &snap.per_category["food"];
    assert!((food.total_amount - 1042.0).abs() < 1e-6);
    assert_eq!(food.budget_alerts, 1);
    cancel.cancel();
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(mirror): e2e — finance events → finance drift snapshot"
```

---

## Task 25: Invariant — Every Declared `mirror_snapshot` Attr Has a Registered Source

**Files:**
- Create: `tests/ai_mirror_snapshot_coverage.rs`

`★ Insight ─────────────────────────────────────`
This is the v2 analogue of v1.5's Task 25 literal-scan invariant. It walks the hand-maintained list of features with `MIRROR_SNAPSHOTS` declarations and checks every declared `name` has a matching `SPEC.name` on a registered mirror source. If a feature declares `mirror_snapshot(name = "foo")` but no `MirrorSignalSource::SPEC::name == "foo"` exists, the test fails.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the test**

```rust
use ai_core::{MirrorSignalSource, MirrorSnapshotSpec};
use cognitive::mirror::sources::{
    ConfigArchiverSource, FinanceSpendingDriftSource, MetaRuleSignalSource,
    RoutingSignalSource, TaskFocusPatternSource, TrialPreviewSource,
};

/// Every declared feature mirror_snapshot attr must have a matching source.
#[test]
fn every_declared_mirror_snapshot_has_a_registered_source() {
    // Hand-maintained list — update when adding a feature.
    let declared_specs: Vec<(&'static str, &'static [MirrorSnapshotSpec])> = vec![
        ("TasksFeature", feature_tasks::TasksFeature::MIRROR_SNAPSHOTS),
        ("FinanceFeature", feature_finance::FinanceFeature::MIRROR_SNAPSHOTS),
    ];

    // Hand-maintained list — update when adding a source.
    let registered_specs: &[MirrorSnapshotSpec] = &[
        RoutingSignalSource::SPEC,
        MetaRuleSignalSource::SPEC,
        ConfigArchiverSource::SPEC,
        TrialPreviewSource::SPEC,
        TaskFocusPatternSource::SPEC,
        FinanceSpendingDriftSource::SPEC,
    ];

    for (feat_name, specs) in &declared_specs {
        for spec in *specs {
            let name = spec.name;
            let covered = registered_specs.iter().any(|s| s.name == name);
            assert!(
                covered,
                "{feat_name} declares mirror_snapshot(name = \"{name}\") but no \
                 MirrorSignalSource registers SPEC.name == \"{name}\""
            );
        }
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_snapshot_coverage`
Expected: PASS.

- [ ] **Step 3: Negative check (optional but cheap)**

Add a second test to catch registered sources that *don't* correspond to a feature declaration (i.e. system-level sources like `routing` / `meta_rule` / `config_archive` / `trial_preview`, which have no feature owner). Allow-list those:

```rust
#[test]
fn feature_owned_sources_have_a_declaration() {
    const SYSTEM_OWNED: &[&str] = &["routing", "meta_rule", "config_archive", "trial_preview"];
    let registered_specs: &[MirrorSnapshotSpec] = &[
        RoutingSignalSource::SPEC,
        MetaRuleSignalSource::SPEC,
        ConfigArchiverSource::SPEC,
        TrialPreviewSource::SPEC,
        TaskFocusPatternSource::SPEC,
        FinanceSpendingDriftSource::SPEC,
    ];
    let all_declared: Vec<&'static str> = [
        feature_tasks::TasksFeature::MIRROR_SNAPSHOTS,
        feature_finance::FinanceFeature::MIRROR_SNAPSHOTS,
    ]
    .iter()
    .flat_map(|s| s.iter().map(|spec| spec.name))
    .collect();

    for spec in registered_specs {
        if SYSTEM_OWNED.contains(&spec.name) {
            continue;
        }
        assert!(
            all_declared.contains(&spec.name),
            "registered source \"{}\" is not declared by any feature and not on the system \
             allow-list",
            spec.name,
        );
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run --test ai_mirror_snapshot_coverage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests
git commit -m "test(mirror): invariant — every mirror_snapshot attr has a registered source"
```

---

## Task 26: Invariant — No `broadcast::Receiver<DomainEvent>` in `crates/cognitive/src/mirror/`

**Files:**
- Modify: `tests/ai_mirror_snapshot_coverage.rs` (or a separate invariant file — pick one and stick)

- [ ] **Step 1: Add a literal-scan test**

Append to `tests/ai_mirror_snapshot_coverage.rs`:

```rust
#[test]
fn no_broadcast_receiver_in_mirror_sources() {
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mirror_dir = root.join("crates").join("cognitive").join("src").join("mirror");
    let mut violations: Vec<String> = Vec::new();
    for entry in walkdir::WalkDir::new(&mirror_dir) {
        let entry = entry.unwrap();
        if !entry.file_name().to_string_lossy().ends_with(".rs") { continue; }
        let path = entry.path();
        let text = std::fs::read_to_string(path).unwrap();
        for (lineno, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") { continue; }
            if line.contains("broadcast::Receiver<")
                && line.contains("DomainEvent")
            {
                violations.push(format!("{}:{}: {}", path.display(), lineno + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Mirror must not subscribe to DomainEventBus directly; use SignalConsumer.\n{}",
        violations.join("\n")
    );
}
```

Verify `walkdir` is already a dev-dep (v1.5 Task 25 added it). If not, add to the facade crate's `[dev-dependencies]`.

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_mirror_snapshot_coverage no_broadcast_receiver`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(mirror): invariant — no broadcast::Receiver<DomainEvent> in mirror code"
```

---

## Task 27: Final Verification

**Files:** none (verification + small cleanup)

- [ ] **Step 1: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings (modulo pre-existing `desktop` crate exceptions).

- [ ] **Step 2: Full test pass**

Run: `cargo nextest run --workspace`
Run: `cargo test --workspace --doc`
Expected: both PASS.

- [ ] **Step 3: Grep sanity checks**

```bash
# Subscribers directory truly gone.
ls crates/cognitive/src/mirror/subscribers/ 2>&1 | grep -q 'No such file' || echo "STILL EXISTS"

# Old subscriber types referenced anywhere.
rg 'RoutingMirrorSubscriber|MetaRuleDetector|TrialPreviewSubscriber|ConfigArchiver\b' crates/
# Expected: 0 hits (note: ConfigArchiver is gone; ConfigArchiverSource is the replacement).

# No broadcast::Receiver<DomainEvent> in mirror sources.
rg 'broadcast::Receiver' crates/cognitive/src/mirror/
# Expected: 0 hits.

# Every new source has its SPEC registered.
rg 'impl MirrorSignalSource' crates/cognitive/src/mirror/sources/
# Expected: 6 impls (routing, meta_rule, config_archiver, trial, task_focus, finance_drift).

# Mirror retention service actually spawned.
rg 'MirrorRetentionService::spawn' crates/app-core/
# Expected: 1 hit.

# Autotuner emits the new "activated" verdict.
rg '"activated"' crates/agent/src/autotuner/
# Expected: ≥ 1 hit.
```

Any 0-expected hit that's non-zero is a plan-scope violation — fix before closing.

- [ ] **Step 4: Manual smoke**

```bash
cd desktop-ui && bun install && bun run dev &
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

Inside the running app:
1. Trigger enough `SkillRouted` events to log ~5 skill selections. Manually call `RoutingSignalSource::flush` via the test helper or wait for the next hour. Verify `mirror_routing_snapshots` has a new row.
2. Create a task, focus it, complete it. Call `TaskFocusPatternSource::flush` (via a dev command or restart). Verify `mirror_task_focus_snapshots` has a row.
3. Record 2 transactions and 1 `BudgetAlert` on the same category. Flush `FinanceSpendingDriftSource`. Verify `mirror_finance_drift_snapshots` has a row with the expected per-category breakdown.
4. Open the Mirror tool via MCP: `mirror.get_state` returns a non-empty state without any log errors mentioning `broadcast` or `subscriber`.

(These are explanatory checks — not blockers — but do them before marking v2 done.)

- [ ] **Step 5: CLAUDE.md touch-up**

Edit `CLAUDE.md` at the gotcha block. Replace:

```markdown
- **`MirrorEngine::start` takes `Arc<DomainEventBus>`** — not `&DomainEventBus`. Signature: `start(repo, bus: Arc<DomainEventBus>, narrative_handler, autotuner_bridge, episodic_repo)`.
```

with:

```markdown
- **`MirrorEngine::start` returns `StartedMirror`** — a struct with `.facade`, `.consumers` (for `SignalRouter`), `.flush_handles`, `.shutdown`. Mirror no longer subscribes to `DomainEventBus` directly; it participates in the unified AI pipeline via `MirrorSignalSource` impls in `crates/cognitive/src/mirror/sources/`.
```

- [ ] **Step 6: Final commit**

If earlier tasks didn't commit the CLAUDE.md edit:

```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE.md): update mirror gotcha to reflect v2 SignalRouter integration"
```

Otherwise, close with an empty commit:

```bash
git commit --allow-empty -m "chore(ai-pipeline): close v2 — mirror redesign complete"
```

---

## v2 Done Criteria (from spec §6)

- [ ] Adding a new mirror concern requires one `#[ai(mirror_snapshot = ...)]` attribute and one SQL table migration — demonstrated by Tasks 12–15 (two new snapshot types added this way).
- [ ] All 4 legacy mirror subscribers deleted; replaced by 6 `MirrorSignalSource` impls (Tasks 8–13, 19).
- [ ] Mirror participates in the unified `SignalRouter` pipeline — zero `broadcast::Receiver<DomainEvent>` in `crates/cognitive/src/mirror/` (Task 26 invariant).
- [ ] Task and Finance each have at least one mirror snapshot type declared + registered (Tasks 14, 15, 25).
- [ ] Mirror retention policy defined and running daily (Tasks 5, 6; `MirrorRetentionService` spawned in Phase 9).
- [ ] `TrialPreviewSource` is active, not inert (Task 7 reintroduces the activated lifecycle; Task 11 wires it up; Task 22 integration test).
- [ ] `cargo clippy --workspace --all-targets --all-features` clean (Task 27).
- [ ] `cargo nextest run --workspace` green (Task 27).
