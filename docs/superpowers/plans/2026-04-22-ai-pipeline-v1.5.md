# AI Pipeline v1.5 — Coaching + Retrieval Boost + Collector Merge

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the *consumer* side of the AI pipeline — convert coaching signal ingestion and the five cognitive collectors to `SignalConsumer` impls reading `AiSignal`, replace string-typed retrieval domains with the `RecallDomain` enum, and port every hardcoded per-event/per-pattern map into declarative macro attributes (`coaching_signal`, `recall_boost_when`, `recall_priority_field`, `recall_recency_field`, `recall_status_filter`).

**Architecture:** v1 established the `SignalRouter` and migrated producers (Tasks, Finance). v1.5 finishes the migration by pointing every remaining AI subsystem at the router: (a) a new `CoachingSignalConsumer` replaces `CoachingService`'s direct `DomainEvent` subscription, letting the 14-arm `conversion.rs` match and 8-arm `coaching_collector::pattern_to_rule` both be deleted; (b) the five cognitive collectors (`ChatTurn`, `Recall`, `Session`, `Atom`, `Coaching`) each become a `SignalConsumer` that filters on `event_kind` and forwards the existing `CognitiveSignal` to the convergence stage, which stays unchanged; (c) `RetrievalFeedbackRepo` and `CognitiveContextSource` stop accepting magic strings — they speak `RecallDomain`, which per-feature `RecallProvider` impls are generated from `#[ai(recall_*)]` attributes on `TasksFeature` / `FinanceFeature`.

**Tech Stack:** Rust 1.93, `ai-core` + `ai-core-macros` (extended in this stage), `async-trait`, `tokio::broadcast` for the router (unchanged), `sqlx` (repo change is boundary-only — column stays TEXT).

**Spec:** `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md` — v1.5 section.

**Pre-release posture:** Every old path is deleted in the same PR that introduces the new one. No dual dispatch, no deprecation shims.

---

## File Structure

### New files

```
crates/ai-core-macros/tests/expand/coaching_signal.rs  — coaching_signal attr expansion snapshot
crates/ai-core-macros/tests/expand/recall_attrs.rs     — recall_* attrs expansion snapshot
crates/ai-core/src/metrics.rs                          — AiMetrics { app, amount, category }
crates/ai-core/src/recall_provider_registry.rs         — typed registry for RecallProvider impls
crates/feature-coaching/src/consumer.rs                — CoachingSignalConsumer (SignalConsumer impl)
crates/cognitive/src/pipeline/ai_signal_filter.rs      — shared helper: "does this AiSignal belong to kind X?"
tests/ai_pipeline_v15_integration.rs                   — end-to-end: event → AiSignal → all 5 collectors + coaching
tests/ai_no_domain_literals.rs                         — invariant: zero "general"/"tasks"/"finance" string literals in AI crates
```

### Modified files

```
crates/ai-core/src/signal.rs                — add `metrics: AiMetrics`, `coaching_signal: bool`, `coaching_rule: Option<String>`
crates/ai-core/src/lib.rs                   — re-export AiMetrics, RecallProviderRegistry
crates/ai-core/src/traits.rs                — extend RecallProvider with recall_*_spec methods
crates/ai-core-macros/src/attrs.rs          — parse coaching_signal(...), recall_boost_when, recall_priority_field, recall_recency_field, recall_status_filter
crates/ai-core-macros/src/ai_event.rs       — emit metrics / coaching_signal / coaching_rule population
crates/ai-core-macros/src/ai_feature.rs     — emit RecallProvider impl from recall_* attrs

crates/feature-tasks/src/events.rs          — add #[ai(coaching_signal(...))] where relevant
crates/feature-tasks/src/lib.rs             — add #[ai(recall_boost_when=..., recall_priority_field=..., ...)] on TasksFeature
crates/feature-finance/src/events.rs        — same treatment for Finance events
crates/feature-finance/src/lib.rs           — recall_* attrs on FinanceFeature

crates/bus/src/domain_events.rs             — add rule_text: String field to CoachingPatternDetected
crates/cognitive/src/mirror/subscribers/... — N/A (mirror is out of scope for v1.5)

crates/feature-coaching/Cargo.toml          — add ai-core dep
crates/feature-coaching/src/lib.rs          — mod consumer; re-export CoachingSignalConsumer
crates/feature-coaching/src/service.rs      — receive AiSignal via consumer, not DomainEvent directly
crates/feature-coaching/src/signal_accumulator/mod.rs — push_event(&AiSignal) replaces push_event(&DomainEvent)
crates/feature-coaching/src/signal_accumulator/conversion.rs — DELETE
crates/feature-coaching/src/signal_accumulator/types.rs — SignalMetadata::from_ai_signal, audit default_conditions
crates/feature-coaching/src/pattern_detector/mod.rs — replace "tasks"/"finance" literals with RecallDomain
crates/feature-coaching/src/feedback.rs     — accepts DetectedPattern with RecallDomain

crates/cognitive/Cargo.toml                 — (already has ai-core from v1)
crates/cognitive/src/pipeline/signal.rs     — CognitiveSignal.domain: String → RecallDomain
crates/cognitive/src/pipeline/atom_collector.rs      — SignalConsumer impl (no DomainEvent subscription)
crates/cognitive/src/pipeline/chat_turn_collector.rs — same
crates/cognitive/src/pipeline/coaching_collector.rs  — same; delete pattern_to_rule()
crates/cognitive/src/pipeline/recall_collector.rs    — same
crates/cognitive/src/pipeline/session_collector.rs   — same
crates/cognitive/src/pipeline/mod.rs        — drop event_rx wiring, export Arc<dyn SignalConsumer>
crates/cognitive/src/pipeline/consolidator.rs — consume RecallDomain instead of String
crates/cognitive/src/pipeline/writer.rs     — same
crates/cognitive/src/services/context_source.rs — call RecallProviderRegistry instead of hardcoded domain vec

crates/storage/src/repos/retrieval_feedback.rs — avg_precision_by_domain_since returns Vec<(RecallDomain, f64)>
crates/storage/src/repos/tests/retrieval_feedback_tests.rs — update to RecallDomain
crates/storage/Cargo.toml                   — add ai-core dep

crates/app-core/src/init/ai_pipeline.rs     — translate() covers ChatTurnCompleted/SessionEnded/AtomReinforced/CoachingPatternDetected
crates/app-core/src/init/mod.rs             — register CoachingSignalConsumer + five cognitive collectors as SignalConsumers
crates/app-core/src/init/coaching.rs        — CoachingService::start wired via CoachingSignalConsumer's channel
```

### Task Overview

| # | Task | Phase |
|---|---|---|
| 1 | Add `AiMetrics` + enrich `AiSignal` with `metrics`, `coaching_signal`, `coaching_rule` | Macros |
| 2 | `#[ai(coaching_signal(...))]` attr on `AiEvent` — parse + emit metrics | Macros |
| 3 | `#[ai(recall_boost_when=..., recall_priority_field=..., recall_recency_field=..., recall_status_filter=...)]` on `AiFeature` | Macros |
| 4 | Extend `RecallProvider` trait + generate impl from feature attrs | Macros |
| 5 | Create `RecallProviderRegistry` in `ai-core` | Foundation |
| 6 | Extend `ai_pipeline::translate()` to cover system events (ChatTurnCompleted, SessionEnded, AtomReinforced, CoachingPatternDetected) | Wiring |
| 7 | Add `rule_text` to `CoachingPatternDetected`; populate in `pattern_detector::detect_patterns` | Coaching |
| 8 | Annotate Task/Finance event variants with `coaching_signal(...)` to drive coaching metadata | Coaching |
| 9 | Create `CoachingSignalConsumer`; replace `CoachingService::start(broadcast::Receiver<DomainEvent>, ...)` | Coaching |
| 10 | `SignalAccumulator::push_event` takes `&AiSignal`; delete `signal_accumulator/conversion.rs` | Coaching |
| 11 | Audit `default_conditions()` — document rationale, remove any that no longer have evaluation logic | Coaching |
| 12 | Change `CognitiveSignal::domain` to `RecallDomain`; fix `pattern_detector/mod.rs` string literals | Collectors |
| 13 | Convert `ChatTurnCollector` to `SignalConsumer` | Collectors |
| 14 | Convert `RecallCollector` to `SignalConsumer` | Collectors |
| 15 | Convert `SessionCollector` to `SignalConsumer` | Collectors |
| 16 | Convert `AtomCollector` to `SignalConsumer` | Collectors |
| 17 | Convert `CoachingCollector` to `SignalConsumer`; delete `pattern_to_rule()` | Collectors |
| 18 | Update `pipeline/consolidator.rs` + `writer.rs` for `RecallDomain` | Collectors |
| 19 | Annotate `TasksFeature` + `FinanceFeature` with `recall_*` attrs | Retrieval |
| 20 | Wire `CognitiveContextSource` to iterate `RecallProviderRegistry` | Retrieval |
| 21 | `RetrievalFeedbackRepo` returns typed `RecallDomain` tuples | Retrieval |
| 22 | Register all v1.5 consumers in `app-core/src/init/mod.rs` Phase 8 | Wiring |
| 23 | Integration: `AiSignal` → `CoachingSignalConsumer` → trigger fires | Tests |
| 24 | Integration: `DomainEvent` → translator → all 5 collectors receive matching `AiSignal` | Tests |
| 25 | Invariant: `grep -R '"general"\|"tasks"\|"finance"'` returns 0 hits in `crates/cognitive`, `crates/feature-coaching`, `crates/storage/src/repos/retrieval_feedback.rs` | Tests |
| 26 | Final verification: clippy, nextest, plan checklist | Done |

---

## Task 1: Add `AiMetrics` + Enrich `AiSignal`

**Files:**
- Create: `crates/ai-core/src/metrics.rs`
- Modify: `crates/ai-core/src/signal.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Modify: `crates/ai-core/tests/signal_test.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/signal_test.rs`:

```rust
use ai_core::{AiMetrics, AiSignal};

#[test]
fn signal_carries_metrics_and_coaching_flags() {
    let metrics = AiMetrics {
        app: Some("reddit".into()),
        amount: Some(42.0),
        category: Some("food".into()),
    };
    let sig = AiSignal {
        domain: ai_core::RecallDomain::Finance,
        event_kind: "BudgetAlert",
        importance: 0.9,
        salience: ai_core::SalienceVerdict::Extract,
        content: "alert".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: metrics.clone(),
        coaching_signal: true,
        coaching_rule: Some("Review spending when budget pressure rises".into()),
    };
    assert_eq!(sig.metrics.app.as_deref(), Some("reddit"));
    assert_eq!(sig.metrics.amount, Some(42.0));
    assert!(sig.coaching_signal);
    assert!(sig.coaching_rule.is_some());
}

#[test]
fn metrics_default_all_none() {
    let m = AiMetrics::default();
    assert!(m.app.is_none() && m.amount.is_none() && m.category.is_none());
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p ai-core signal`
Expected: FAIL — `AiMetrics`, new fields don't exist.

- [ ] **Step 3: Create `metrics.rs`**

Create `crates/ai-core/src/metrics.rs`:

```rust
/// Well-known coaching-side metrics extracted from an event payload.
///
/// Populated by the `#[ai(coaching_signal(...))]` attribute via
/// derive-generated code. Absent fields are `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiMetrics {
    pub app: Option<String>,
    pub amount: Option<f64>,
    pub category: Option<String>,
}
```

- [ ] **Step 4: Extend `AiSignal`**

Replace the `AiSignal` struct in `crates/ai-core/src/signal.rs`:

```rust
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
}
```

(Keep `SalienceVerdict` and `EntityRef` unchanged.)

- [ ] **Step 5: Update `lib.rs`**

In `crates/ai-core/src/lib.rs`, add `pub mod metrics;` and `pub use metrics::AiMetrics;`.

- [ ] **Step 6: Fix call sites**

The v1 generated macro code writes `AiSignal { domain: String::new(), event_kind, importance, salience, content, entity, timestamp: ... }` — this will now fail to compile because:
- `domain` must be `RecallDomain` (fixed in Task 3, so for now hard-code `RecallDomain::General`)
- Three new fields need defaults.

Patch the macro output in `crates/ai-core-macros/src/ai_event.rs::render_variant` — in the `::ai_core::AiSignal { ... }` literal, update the `domain` line and add the trailing fields:

```rust
::ai_core::AiSignal {
    domain: ::ai_core::RecallDomain::General,   // overwritten by router via AiFeature::DOMAIN in Task 3
    event_kind: #kind_lit,
    importance: #importance_expr,
    salience: #salience_expr,
    content: #content_expr,
    entity: #entity_expr,
    timestamp: ::jiff::Timestamp::now(),
    raw_event: None,
    metrics: ::ai_core::AiMetrics::default(),
    coaching_signal: false,
    coaching_rule: None,
}
```

Also fix `crates/app-core/src/init/ai_pipeline.rs`: the `translate()` function builds `TaskEvent`/`FinanceEvent` then calls `.to_signal()`. No change needed at call sites; the router still sets `signal.raw_event = Some(event)` after translation.

- [ ] **Step 7: Run**

Run: `cargo nextest run -p ai-core -p ai-core-macros`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/ai-core crates/ai-core-macros
git commit -m "feat(ai-core): add AiMetrics + coaching_signal/coaching_rule fields on AiSignal"
```

---

## Task 2: `#[ai(coaching_signal(...))]` Attribute — Parse + Emit

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`
- Modify: `crates/ai-core-macros/src/ai_event.rs`
- Create: `crates/ai-core-macros/tests/expand/coaching_signal.rs`

- [ ] **Step 1: Write failing trybuild test**

Create `crates/ai-core-macros/tests/expand/coaching_signal.rs`:

```rust
use ai_core::{AiEventMeta, AiSignal};
use ai_core_macros::AiEvent;

#[derive(AiEvent)]
pub enum TestEvent {
    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "alert {category} {spent}/{limit}",
        coaching_signal(
            app_from = "category",
            amount_from = "spent",
            category_from = "category",
            rule = "Review spending when budget pressure rises",
        ),
    )]
    BudgetAlert { category: String, spent: f64, limit: f64 },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "distraction {app}",
    )]
    Distraction { app: String },
}

fn main() {
    let sig: AiSignal = TestEvent::BudgetAlert {
        category: "food".into(),
        spent: 450.0,
        limit: 500.0,
    }.to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.coaching_rule.as_deref(), Some("Review spending when budget pressure rises"));
    assert_eq!(sig.metrics.amount, Some(450.0));
    assert_eq!(sig.metrics.category.as_deref(), Some("food"));
    assert_eq!(sig.metrics.app.as_deref(), Some("food"));

    let sig: AiSignal = TestEvent::Distraction { app: "reddit".into() }.to_signal();
    assert!(!sig.coaching_signal);
    assert!(sig.coaching_rule.is_none());
    assert!(sig.metrics.app.is_none());
}
```

Wire it into the trybuild harness by adding `t.pass("tests/expand/coaching_signal.rs");` to `crates/ai-core-macros/tests/expand_smoke.rs`.

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p ai-core-macros`
Expected: FAIL — `coaching_signal` attribute unknown.

- [ ] **Step 3: Extend `attrs.rs`**

In `crates/ai-core-macros/src/attrs.rs`, add to the `AiEventAttr` struct:

```rust
pub struct AiEventAttr {
    pub importance: Option<f64>,
    pub importance_fn: Option<syn::Path>,
    pub salience: SalienceSpec,
    pub observation_template: Option<String>,
    pub entity_bridge: Option<EntityBridge>,
    pub coaching: Option<CoachingSignalSpec>,
}

pub struct CoachingSignalSpec {
    pub app_from: Option<syn::Ident>,
    pub amount_from: Option<syn::Ident>,
    pub category_from: Option<syn::Ident>,
    pub rule: Option<String>,
}
```

In `parse_ai_event_attr`, add an arm:

```rust
"coaching_signal" => {
    coaching = Some(parse_coaching_signal(&meta)?);
}
```

Add the parser below:

```rust
fn parse_coaching_signal(
    meta: &syn::meta::ParseNestedMeta,
) -> syn::Result<CoachingSignalSpec> {
    let mut app_from = None;
    let mut amount_from = None;
    let mut category_from = None;
    let mut rule = None;
    meta.parse_nested_meta(|inner| {
        let key = inner.path.get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "app_from"      => app_from      = Some(inner.value()?.parse::<syn::Ident>()?),
            "amount_from"   => amount_from   = Some(inner.value()?.parse::<syn::Ident>()?),
            "category_from" => category_from = Some(inner.value()?.parse::<syn::Ident>()?),
            "rule" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                rule = Some(s.value());
            }
            other => return Err(inner.error(format!(
                "unknown coaching_signal key: {other}"
            ))),
        }
        Ok(())
    })?;
    Ok(CoachingSignalSpec { app_from, amount_from, category_from, rule })
}
```

- [ ] **Step 4: Emit metrics + flags in `ai_event.rs`**

In `crates/ai-core-macros/src/ai_event.rs::render_variant`, after `entity_expr` compute:

```rust
let (coaching_flag, rule_expr, app_expr, amount_expr, category_expr) = match &attr.coaching {
    None => (
        quote! { false },
        quote! { None },
        quote! { None },
        quote! { None },
        quote! { None },
    ),
    Some(c) => {
        let rule = match &c.rule {
            Some(s) => quote! { Some(#s.to_string()) },
            None    => quote! { None },
        };
        let app = match &c.app_from {
            Some(id) => quote! { Some(#id.to_string()) },
            None     => quote! { None },
        };
        let amount = match &c.amount_from {
            Some(id) => quote! { Some(#id as f64) },
            None     => quote! { None },
        };
        let category = match &c.category_from {
            Some(id) => quote! { Some(#id.to_string()) },
            None     => quote! { None },
        };
        (quote! { true }, rule, app, amount, category)
    }
};
```

Then in the `::ai_core::AiSignal { ... }` literal, replace the trailing defaults with:

```rust
raw_event: None,
metrics: ::ai_core::AiMetrics {
    app: #app_expr,
    amount: #amount_expr,
    category: #category_expr,
},
coaching_signal: #coaching_flag,
coaching_rule: #rule_expr,
```

Note: `amount` may be `f64` or integer (e.g. `spent: i64`). The `as f64` cast covers all numeric types. For string fields without the cast we use `.to_string()`. If a field's type is `Option<T>`, the user should use a wrapper — out of scope for v1.5.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core-macros`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): coaching_signal attribute — emits metrics + rule"
```

---

## Task 3: `recall_*` Attributes on `AiFeature` — Parse

**Files:**
- Modify: `crates/ai-core-macros/src/attrs.rs`
- Modify: `crates/ai-core-macros/src/ai_feature.rs`
- Create: `crates/ai-core-macros/tests/expand/recall_attrs.rs`

- [ ] **Step 1: Write failing trybuild test**

Create `crates/ai-core-macros/tests/expand/recall_attrs.rs`:

```rust
use ai_core::{AiFeature, RecallDomain, RecallProvider, RecallQuery};
use ai_core_macros::{AiEvent, AiFeature};
use bus::DomainEvent;

#[derive(AiEvent)]
pub enum ProbeEvent {
    #[ai(importance = 0.5, salience = "accumulate", observation_template = "probe")]
    Probe,
}

impl From<ProbeEvent> for DomainEvent {
    fn from(_: ProbeEvent) -> Self {
        DomainEvent::ChatTurnCompleted { session_key: String::new(), user_message: None }
    }
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::ProbeEvent",
    recall_boost_when = "query.message.contains(\"deadline\")",
    recall_priority_field = "priority",
    recall_recency_field = "updated_at",
    recall_status_filter = "status != \"archived\"",
)]
pub struct ProbeFeature;

fn main() {
    assert_eq!(<ProbeFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    let provider: Box<dyn RecallProvider> = Box::new(ProbeFeature);
    assert_eq!(provider.domain(), RecallDomain::Tasks);

    let deadline_query = RecallQuery { message: "when is the deadline?".into(), intent_summary: None };
    let casual_query = RecallQuery { message: "hello".into(), intent_summary: None };
    assert!(provider.score_query(&deadline_query) > provider.score_query(&casual_query));
}
```

Wire into trybuild: `t.pass("tests/expand/recall_attrs.rs");`.

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p ai-core-macros recall_attrs`
Expected: FAIL.

- [ ] **Step 3: Extend `attrs.rs` with feature-level parser**

Add to `crates/ai-core-macros/src/attrs.rs`:

```rust
pub struct AiFeatureAttr {
    pub recall_domain: syn::Ident,
    pub skill: String,
    pub event: syn::Path,
    pub recall_boost_when: Option<syn::Expr>,
    pub recall_priority_field: Option<syn::Ident>,
    pub recall_recency_field: Option<syn::Ident>,
    pub recall_status_filter: Option<syn::Expr>,
}

pub fn parse_ai_feature_attr(attrs: &[syn::Attribute]) -> syn::Result<AiFeatureAttr> {
    let ai_attr = attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(),
            "AiFeature derive requires #[ai(...)]"))?;

    let mut recall_domain = None;
    let mut skill = None;
    let mut event = None;
    let mut recall_boost_when = None;
    let mut recall_priority_field = None;
    let mut recall_recency_field = None;
    let mut recall_status_filter = None;

    ai_attr.parse_nested_meta(|meta| {
        let k = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match k.as_str() {
            "recall_domain" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_domain = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "skill" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                skill = Some(s.value());
            }
            "event" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                event = Some(syn::parse_str(&s.value())?);
            }
            "recall_boost_when" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_boost_when = Some(syn::parse_str(&s.value())?);
            }
            "recall_priority_field" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_priority_field = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "recall_recency_field" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_recency_field = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "recall_status_filter" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_status_filter = Some(syn::parse_str(&s.value())?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {other}"))),
        }
        Ok(())
    })?;

    Ok(AiFeatureAttr {
        recall_domain: recall_domain.ok_or_else(|| syn::Error::new(
            proc_macro2::Span::call_site(), "recall_domain is required"))?,
        skill: skill.ok_or_else(|| syn::Error::new(
            proc_macro2::Span::call_site(), "skill is required"))?,
        event: event.ok_or_else(|| syn::Error::new(
            proc_macro2::Span::call_site(), "event path is required"))?,
        recall_boost_when,
        recall_priority_field,
        recall_recency_field,
        recall_status_filter,
    })
}
```

(Delete the old `parse_ai_feature_attr` helper in favour of this one.)

- [ ] **Step 4: Run**

Run: `cargo nextest run -p ai-core-macros`
Expected: compiles; trybuild still FAILs (expansion doesn't emit RecallProvider yet — Task 4).

- [ ] **Step 5: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): parse recall_* attributes on AiFeature"
```

---

## Task 4: Extend `RecallProvider` Trait + Generate Impl from Feature Attrs

**Files:**
- Modify: `crates/ai-core/src/traits.rs`
- Modify: `crates/ai-core/src/recall.rs`
- Modify: `crates/ai-core-macros/src/ai_feature.rs`

- [ ] **Step 1: Write failing assertion (reuse Task 3's test)**

The test from Task 3 expects `provider.score_query(deadline_query) > provider.score_query(casual_query)`. Leave the file in place.

Run: `cargo nextest run -p ai-core-macros recall_attrs`
Expected: FAIL — `score_query` always returns `0.0`.

- [ ] **Step 2: Update `RecallProvider`**

Replace the trait in `crates/ai-core/src/traits.rs`:

```rust
pub trait RecallProvider: Send + Sync {
    fn domain(&self) -> crate::RecallDomain;

    /// Relevance score for a query in this domain. 0.0 = irrelevant.
    /// Default returns 1.0 if `recall_boost_when` matched, else 0.3.
    fn score_query(&self, _query: &crate::RecallQuery) -> f64 { 0.3 }

    fn candidates(&self, _query: &crate::RecallQuery) -> Vec<crate::RecallItem> { Vec::new() }
}
```

- [ ] **Step 3: Emit `RecallProvider` impl in `ai_feature.rs`**

In `crates/ai-core-macros/src/ai_feature.rs::expand`, after generating the `AiFeature` impl, emit:

```rust
let boost_expr = match &feat.recall_boost_when {
    Some(expr) => quote! {
        fn score_query(&self, query: &::ai_core::RecallQuery) -> f64 {
            if { #expr } { 1.0 } else { 0.3 }
        }
    },
    None => quote! {},
};

let recall_domain_ident = &feat.recall_domain;

quote! {
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
}
```

Note: `recall_priority_field`, `recall_recency_field`, `recall_status_filter` are consumed by `candidates()` which queries the feature's DB. For v1.5, they're parsed and stored in the generated `FEATURE_RECALL_SPEC` const so retrieval code can read them at runtime. Add:

```rust
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

// Append to the expansion:
impl #struct_ident {
    pub const RECALL_SPEC: ::ai_core::RecallSpec = ::ai_core::RecallSpec {
        priority_field: #priority,
        recency_field: #recency,
        status_filter: #status,
    };
}
```

- [ ] **Step 4: Add `RecallSpec` to ai-core**

In `crates/ai-core/src/recall.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct RecallSpec {
    pub priority_field: Option<&'static str>,
    pub recency_field: Option<&'static str>,
    pub status_filter: Option<&'static str>,
}
```

Re-export from `lib.rs`.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core-macros recall_attrs`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core crates/ai-core-macros
git commit -m "feat(ai-core-macros): generate RecallProvider impl + RecallSpec from recall_* attrs"
```

---

## Task 5: Create `RecallProviderRegistry`

**Files:**
- Create: `crates/ai-core/src/recall_provider_registry.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Modify: `crates/ai-core/tests/traits_test.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/ai-core/tests/traits_test.rs`:

```rust
use ai_core::{RecallDomain, RecallProvider, RecallProviderRegistry, RecallQuery};

struct FakeProvider(RecallDomain, f64);
impl RecallProvider for FakeProvider {
    fn domain(&self) -> RecallDomain { self.0 }
    fn score_query(&self, _q: &RecallQuery) -> f64 { self.1 }
}

#[test]
fn registry_iterates_providers() {
    let reg = RecallProviderRegistry::new()
        .with(FakeProvider(RecallDomain::Tasks, 0.9))
        .with(FakeProvider(RecallDomain::Finance, 0.4));
    let q = RecallQuery { message: "deadline".into(), intent_summary: None };
    let ranked = reg.rank(&q);
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].0, RecallDomain::Tasks);
    assert!((ranked[0].1 - 0.9).abs() < 1e-9);
}

#[test]
fn registry_filters_zero_scores() {
    let reg = RecallProviderRegistry::new()
        .with(FakeProvider(RecallDomain::Tasks, 0.0));
    let q = RecallQuery { message: "x".into(), intent_summary: None };
    assert!(reg.rank(&q).is_empty());
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p ai-core registry`
Expected: FAIL.

- [ ] **Step 3: Implement `RecallProviderRegistry`**

Create `crates/ai-core/src/recall_provider_registry.rs`:

```rust
use crate::{RecallDomain, RecallProvider, RecallQuery};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct RecallProviderRegistry {
    providers: Vec<Arc<dyn RecallProvider>>,
}

impl RecallProviderRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn with<P: RecallProvider + 'static>(mut self, p: P) -> Self {
        self.providers.push(Arc::new(p));
        self
    }

    pub fn register<P: RecallProvider + 'static>(&mut self, p: P) {
        self.providers.push(Arc::new(p));
    }

    /// Score every provider for the query; return `(domain, score)` pairs
    /// sorted descending, dropping zeros.
    pub fn rank(&self, query: &RecallQuery) -> Vec<(RecallDomain, f64)> {
        let mut out: Vec<_> = self.providers
            .iter()
            .map(|p| (p.domain(), p.score_query(query)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn RecallProvider>> {
        self.providers.iter()
    }
}
```

- [ ] **Step 4: Re-export**

Add to `crates/ai-core/src/lib.rs`:

```rust
pub mod recall_provider_registry;
pub use recall_provider_registry::RecallProviderRegistry;
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core registry`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core
git commit -m "feat(ai-core): RecallProviderRegistry — typed fan-out to per-feature RecallProviders"
```

---

## Task 6: Extend `ai_pipeline::translate()` for System Events

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Write failing test**

Create `crates/app-core/tests/translate_system_events.rs`:

```rust
use ai_core::RecallDomain;
use app_core::init::ai_pipeline::translate;
use bus::DomainEvent;

#[test]
fn chat_turn_completed_translates_to_general_signal() {
    let e = DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        user_message: Some("hi".into()),
    };
    let sig = translate(&e).expect("should translate");
    assert_eq!(sig.domain, RecallDomain::General);
    assert_eq!(sig.event_kind, "ChatTurnCompleted");
}

#[test]
fn session_ended_translates() {
    let e = DomainEvent::SessionEnded {
        session_id: "s1".into(),
        session_type: "focus".into(),
        duration_secs: 3600,
        quality_score: Some(0.8),
        category_purity: 0.9,
    };
    let sig = translate(&e).expect("should translate");
    assert_eq!(sig.event_kind, "SessionEnded");
}

#[test]
fn coaching_pattern_translates() {
    let e = DomainEvent::CoachingPatternDetected {
        pattern_name: "afternoon_energy_drop".into(),
        confidence: 0.8,
        description: "desc".into(),
        domain: "productivity".into(),
        signal_count: 3,
        rule_text: "Schedule demanding tasks in the morning".into(),
    };
    let sig = translate(&e).expect("should translate");
    assert_eq!(sig.event_kind, "CoachingPatternDetected");
    assert_eq!(sig.content, "Schedule demanding tasks in the morning");
}

#[test]
fn atom_reinforced_translates() {
    let e = DomainEvent::AtomReinforced {
        atom_id: "a1".into(),
        subject: "rust errors".into(),
        domain: "learning".into(),
        reinforcement_count: 3,
    };
    let sig = translate(&e).expect("should translate");
    assert_eq!(sig.event_kind, "AtomReinforced");
}
```

To reach `translate`, export it at `crates/app-core/src/init/mod.rs`:

```rust
pub mod ai_pipeline;
```

and add `pub fn translate` is already public.

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p app-core translate_system`
Expected: FAIL — translate returns None for these events.

- [ ] **Step 3: Extend `translate`**

In `crates/app-core/src/init/ai_pipeline.rs`, add a new helper:

```rust
fn translate_system_event(event: &DomainEvent) -> Option<AiSignal> {
    use ai_core::{AiMetrics, RecallDomain, SalienceVerdict};
    use jiff::Timestamp;

    let now = Timestamp::now();
    let base = AiSignal {
        domain: RecallDomain::General,
        event_kind: "",
        importance: 0.3,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: now,
        raw_event: None,
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    };

    match event {
        DomainEvent::ChatTurnCompleted { user_message, .. } => Some(AiSignal {
            event_kind: "ChatTurnCompleted",
            content: user_message.clone().unwrap_or_default(),
            ..base
        }),
        DomainEvent::SessionEnded { session_id, quality_score, .. } => Some(AiSignal {
            event_kind: "SessionEnded",
            importance: quality_score.unwrap_or(0.5),
            content: session_id.clone(),
            metrics: AiMetrics { amount: *quality_score, ..AiMetrics::default() },
            ..base
        }),
        DomainEvent::CoachingPatternDetected {
            pattern_name, confidence, rule_text, domain, ..
        } => Some(AiSignal {
            event_kind: "CoachingPatternDetected",
            importance: *confidence,
            content: rule_text.clone(),
            metrics: AiMetrics {
                category: Some(pattern_name.clone()),
                ..AiMetrics::default()
            },
            domain: match domain.as_str() {
                "tasks" => RecallDomain::Tasks,
                "finance" => RecallDomain::Finance,
                _ => RecallDomain::General,
            },
            ..base
        }),
        DomainEvent::AtomReinforced {
            atom_id, subject, domain, reinforcement_count,
        } => Some(AiSignal {
            event_kind: "AtomReinforced",
            importance: (0.5 + *reinforcement_count as f64 * 0.15).min(0.95),
            content: subject.clone(),
            metrics: AiMetrics {
                category: Some(domain.clone()),
                ..AiMetrics::default()
            },
            entity: Some(ai_core::EntityRef {
                entity_type: "knowledge_atom",
                id: atom_id.clone(),
                name: subject.clone(),
            }),
            ..base
        }),
        DomainEvent::DistractionDetected { app, .. } => Some(AiSignal {
            event_kind: "DistractionDetected",
            content: app.clone(),
            metrics: AiMetrics { app: Some(app.clone()), ..AiMetrics::default() },
            coaching_signal: true,
            ..base
        }),
        DomainEvent::FocusSessionStarted { .. } => Some(AiSignal {
            event_kind: "FocusSessionStarted",
            coaching_signal: true,
            ..base
        }),
        DomainEvent::FocusSessionEnded { quality, .. } => Some(AiSignal {
            event_kind: "FocusSessionEnded",
            importance: *quality,
            metrics: AiMetrics { amount: Some(*quality), ..AiMetrics::default() },
            coaching_signal: true,
            ..base
        }),
        _ => None,
    }
}
```

Then update `translate()`:

```rust
pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    if let Some(e) = try_into_task_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Tasks;
        return Some(sig);
    }
    if let Some(e) = try_into_finance_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Finance;
        return Some(sig);
    }
    translate_system_event(event)
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p app-core translate_system`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): extend translate() for ChatTurn/Session/AtomReinforced/CoachingPattern/Distraction events"
```

---

## Task 7: Add `rule_text` to `CoachingPatternDetected`; Populate at Pattern Source

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/feature-coaching/src/pattern_detector/mod.rs`
- Modify: `crates/feature-coaching/src/pattern_detector/learning_patterns.rs`
- Modify: any emitter of `DomainEvent::CoachingPatternDetected`

- [ ] **Step 1: Write failing test**

In `crates/feature-coaching/src/pattern_detector/mod.rs::tests`, add:

```rust
#[test]
fn test_detected_pattern_carries_rule_text() {
    let mut detector = PatternDetector::new();
    for _ in 0..4 {
        detector.record_trigger(&trigger("task_avoidance", "x"));
    }
    let patterns = detector.detect_patterns();
    let p = patterns.iter().find(|p| p.name == "chronic_task_avoidance").unwrap();
    assert!(p.rule_text.contains("smaller steps"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-coaching rule_text`
Expected: FAIL — `DetectedPattern.rule_text` doesn't exist.

- [ ] **Step 3: Add field to `DetectedPattern` and populate**

In `crates/feature-coaching/src/pattern_detector/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub name: String,
    pub confidence: f64,
    pub signal_count: i32,
    pub description: String,
    pub domain: ai_core::RecallDomain,
    pub rule_text: String,
}
```

Populate `rule_text` at every construction site. Use the 8 strings previously in `coaching_collector::pattern_to_rule`:

| Pattern name | Rule text |
|---|---|
| `afternoon_energy_drop` | `"Schedule demanding tasks in the morning; take breaks in the afternoon when energy drops"` |
| `chronic_task_avoidance` | `"Break avoided tasks into smaller steps to overcome procrastination"` |
| `habitual_context_switching` | `"Batch similar tasks together to reduce context switching overhead"` |
| `declining_focus_quality` | `"Take a break when focus quality starts declining"` |
| `recurring_budget_pressure` | `"Review spending patterns when budget pressure is detected"` |
| `study_streak_at_risk` | `"Complete at least one review session to maintain the study streak"` |
| `retention_decay_detected` | `"Schedule review sessions for domains with declining retention"` |
| `learning_momentum_create_heavy` | `"Balance content creation with review sessions to avoid review backlog"` |

Also replace every `domain: "tasks".into()` / `"finance".into()` / `"productivity".into()` / `"learning".into()` with the matching `ai_core::RecallDomain::*`. Note: `productivity`/`learning` are not yet variants — add them now to `crates/ai-core/src/recall_domain.rs`:

```rust
pub enum RecallDomain {
    General, Tasks, Finance, Productivity, Learning,
}
// update as_str() accordingly
```

Mirror the same update in `learning_patterns.rs` — every `DetectedPattern { ..., domain, rule_text, .. }` construction fills in the declared rule_text from the table.

- [ ] **Step 4: Add field to `DomainEvent::CoachingPatternDetected`**

In `crates/bus/src/domain_events.rs`, find:

```rust
CoachingPatternDetected {
    pattern_name: String,
    confidence: f64,
    description: String,
    domain: String,
    signal_count: i64,
}
```

Add `rule_text: String`.

- [ ] **Step 5: Populate `rule_text` at emission site**

Find the emitter (likely in `coaching/src/pattern_detector/` or `app-core/src/handlers/coaching.rs`). Update the construction to pass `rule_text: p.rule_text.clone()` from the `DetectedPattern`.

Grep: `grep -rn "CoachingPatternDetected {" crates/ --include='*.rs'` — patch every hit.

- [ ] **Step 6: Run**

Run: `cargo nextest run -p feature-coaching -p bus`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/bus crates/feature-coaching crates/ai-core
git commit -m "feat(coaching): carry rule_text on DetectedPattern + CoachingPatternDetected"
```

---

## Task 8: Annotate Task/Finance Events with `coaching_signal(...)`

**Files:**
- Modify: `crates/feature-tasks/src/events.rs`
- Modify: `crates/feature-finance/src/events.rs`

- [ ] **Step 1: Write failing test**

Create `crates/feature-coaching/tests/coaching_attrs_test.rs`:

```rust
use ai_core::AiEventMeta;
use feature_finance::events::FinanceEvent;
use feature_tasks::events::TaskEvent;

#[test]
fn budget_alert_is_coaching_signal() {
    let sig = FinanceEvent::BudgetAlert {
        category: "food".into(),
        spent: 450,
        limit: 500,
    }.to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.metrics.category.as_deref(), Some("food"));
    assert_eq!(sig.metrics.amount, Some(450.0));
}

#[test]
fn transaction_recorded_has_amount_metric() {
    let sig = FinanceEvent::TransactionRecorded {
        _tx_id: String::new(),
        category: "groceries".into(),
        amount: 42,
        currency: "USD".into(),
        _is_over_budget: false,
    }.to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.metrics.amount, Some(42.0));
}

#[test]
fn task_completed_is_coaching_signal() {
    let sig = TaskEvent::Completed {
        task_id: "t1".into(),
        title: "x".into(),
        deviation_pct: Some(20.0),
    }.to_signal();
    assert!(sig.coaching_signal);
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-coaching coaching_attrs`
Expected: FAIL.

- [ ] **Step 3: Annotate FinanceEvent**

In `crates/feature-finance/src/events.rs`, patch variants:

```rust
#[ai(
    importance = 0.5,
    salience = "accumulate",
    observation_template = "Transaction: {category} {amount} {currency}",
    coaching_signal(
        category_from = "category",
        amount_from = "amount",
    ),
)]
TransactionRecorded { ... }

#[ai(
    importance = 0.9,
    salience = "extract",
    observation_template = "Budget alert: {category} spent {spent} of {limit}",
    entity_bridge(type = "finance_category", name_from = "category", id_from = "category"),
    coaching_signal(
        category_from = "category",
        amount_from = "spent",
        rule = "Review spending patterns when budget pressure is detected",
    ),
)]
BudgetAlert { ... }
```

- [ ] **Step 4: Annotate TaskEvent**

In `crates/feature-tasks/src/events.rs`, add `coaching_signal` to:
- `Created` — no metrics, just the flag (still signals to coaching)
- `Completed` — `coaching_signal(amount_from = "deviation_pct")` (treats deviation as metric)
- `Deferred` — `coaching_signal(rule = "Break avoided tasks into smaller steps to overcome procrastination")`

If `deviation_pct` is `Option<f64>`, the generated `Some(deviation_pct as f64)` won't compile. In that case add a small helper expression in the macro: if the field type is `Option<T>`, emit `deviation_pct.map(|v| v as f64)` — out of scope. Pragmatic fix: for Task 8, skip `amount_from = "deviation_pct"` and just use `coaching_signal(rule = "...")`.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p feature-coaching coaching_attrs`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks crates/feature-finance
git commit -m "feat(features): annotate Task/Finance events with coaching_signal(...)"
```

---

## Task 9: Create `CoachingSignalConsumer`; Rewire `CoachingService`

**Files:**
- Create: `crates/feature-coaching/src/consumer.rs`
- Modify: `crates/feature-coaching/Cargo.toml`
- Modify: `crates/feature-coaching/src/lib.rs`
- Modify: `crates/feature-coaching/src/service.rs`

- [ ] **Step 1: Add ai-core dep**

In `crates/feature-coaching/Cargo.toml`:

```toml
ai-core.workspace = true
```

- [ ] **Step 2: Write failing test**

Create `crates/feature-coaching/tests/consumer_test.rs`:

```rust
use ai_core::{AiSignal, SignalConsumer};
use feature_coaching::CoachingSignalConsumer;
use tokio::sync::mpsc;

#[tokio::test]
async fn non_coaching_signals_are_dropped() {
    let (tx, mut rx) = mpsc::channel(8);
    let consumer = CoachingSignalConsumer::new(tx);
    let mut sig = dummy_signal();
    sig.coaching_signal = false;
    consumer.consume(&sig).await.unwrap();
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn coaching_signals_forwarded() {
    let (tx, mut rx) = mpsc::channel(8);
    let consumer = CoachingSignalConsumer::new(tx);
    let mut sig = dummy_signal();
    sig.coaching_signal = true;
    consumer.consume(&sig).await.unwrap();
    let fwd = rx.recv().await.unwrap();
    assert_eq!(fwd.event_kind, sig.event_kind);
}

fn dummy_signal() -> AiSignal {
    AiSignal {
        domain: ai_core::RecallDomain::Finance,
        event_kind: "BudgetAlert",
        importance: 0.9,
        salience: ai_core::SalienceVerdict::Extract,
        content: "alert".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: true,
        coaching_rule: None,
    }
}
```

- [ ] **Step 3: Verify failure**

Run: `cargo nextest run -p feature-coaching consumer_test`
Expected: FAIL — module doesn't exist.

- [ ] **Step 4: Implement the consumer**

Create `crates/feature-coaching/src/consumer.rs`:

```rust
use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Forwards every `coaching_signal`-flagged `AiSignal` into the accumulator's
/// processing channel. The receiving end is driven by `CoachingService`.
pub struct CoachingSignalConsumer {
    tx: mpsc::Sender<AiSignal>,
}

impl CoachingSignalConsumer {
    pub fn new(tx: mpsc::Sender<AiSignal>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl SignalConsumer for CoachingSignalConsumer {
    fn name(&self) -> &'static str { "coaching" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if !signal.coaching_signal {
            return Ok(());
        }
        let _ = self.tx.send(signal.clone()).await;  // drop on full — coaching is best-effort
        Ok(())
    }
}
```

- [ ] **Step 5: Re-export**

In `crates/feature-coaching/src/lib.rs`:

```rust
mod consumer;
pub use consumer::CoachingSignalConsumer;
```

- [ ] **Step 6: Update `CoachingService::start`**

Replace `event_rx: broadcast::Receiver<DomainEvent>` with `signal_rx: mpsc::Receiver<AiSignal>`. Update the event-matching logic in `service.rs`:

- `DomainEvent::FocusSessionStarted` → `signal.event_kind == "FocusSessionStarted"`
- `DomainEvent::FocusSessionEnded { quality, interruptions, duration_secs }` → read from `signal.raw_event` when it's `Some(DomainEvent::FocusSessionEnded { .. })`. Keep the raw_event destructure — it's the escape hatch for fields not yet in `AiMetrics`.
- `accumulator.push_event(&event)` → `accumulator.push_event(signal)` (Task 10 migrates `push_event`).
- `update_situation_from_event(&situation, &event)` → `update_situation_from_signal(&situation, signal)` (same shape, reads `signal.raw_event` for fields it can't see in AiSignal).

Adjust `build_focus_debrief` to take quality/interruptions/duration via parameters extracted from the signal's `raw_event` — the call site already has them.

The existing tests for `CoachingService::start` must be updated to publish via `DomainEventBus::publish` + a `SignalRouter` + the consumer's mpsc receiver. Keep the mock flow identical; only the input channel type changes.

- [ ] **Step 7: Run**

Run: `cargo nextest run -p feature-coaching`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-coaching
git commit -m "feat(feature-coaching): CoachingSignalConsumer replaces direct DomainEvent subscription"
```

---

## Task 10: `SignalAccumulator::push_event` Takes `&AiSignal`; Delete `conversion.rs`

**Files:**
- Modify: `crates/feature-coaching/src/signal_accumulator/mod.rs`
- Modify: `crates/feature-coaching/src/signal_accumulator/types.rs`
- Delete: `crates/feature-coaching/src/signal_accumulator/conversion.rs`

- [ ] **Step 1: Write failing test**

Replace the accumulator's tests that construct `DomainEvent::BudgetAlert` with `AiSignal`-based inputs. Add:

```rust
#[test]
fn test_push_ai_signal_updates_window() {
    let mut acc = SignalAccumulator::new();
    let sig = AiSignal {
        domain: ai_core::RecallDomain::Finance,
        event_kind: "BudgetAlert",
        importance: 0.9,
        salience: ai_core::SalienceVerdict::Extract,
        content: "".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics { category: Some("food".into()), amount: Some(450.0), app: None },
        coaching_signal: true,
        coaching_rule: None,
    };
    acc.push_event(&sig);
    assert_eq!(acc.window_size(), 1);
    let front = acc.signals().front().unwrap();
    assert_eq!(front.event_type, "BudgetAlert");
    assert_eq!(front.metadata.category.as_deref(), Some("food"));
    assert_eq!(front.metadata.amount, Some(450.0));
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p feature-coaching push_ai_signal`
Expected: FAIL.

- [ ] **Step 3: Replace the conversion path**

In `crates/feature-coaching/src/signal_accumulator/types.rs`, add:

```rust
impl SignalMetadata {
    pub fn from_ai_signal(sig: &ai_core::AiSignal) -> Self {
        Self {
            app: sig.metrics.app.clone(),
            task_id: sig.entity.as_ref()
                .filter(|e| e.entity_type == "task")
                .map(|e| e.id.clone()),
            category: sig.metrics.category.clone(),
            amount: sig.metrics.amount,
        }
    }
}

impl Signal {
    pub fn from_ai_signal(sig: &ai_core::AiSignal) -> Self {
        Self {
            event_type: sig.event_kind.to_string(),
            timestamp: sig.timestamp,
            metadata: SignalMetadata::from_ai_signal(sig),
        }
    }
}
```

- [ ] **Step 4: Update `push_event` signature**

In `crates/feature-coaching/src/signal_accumulator/mod.rs`:

```rust
pub fn push_event(&mut self, signal: &ai_core::AiSignal) {
    let s = Signal::from_ai_signal(signal);
    self.window.push_back(s);
    self.prune_old(jiff::Timestamp::now());
}
```

Delete the `mod conversion;` + `use conversion::event_to_signal;` lines.

- [ ] **Step 5: Delete `conversion.rs`**

```bash
git rm crates/feature-coaching/src/signal_accumulator/conversion.rs
```

- [ ] **Step 6: Update existing tests**

The old tests used `DomainEvent::BudgetAlert` directly. Convert them to build an `AiSignal` via the translator or via direct construction. If a test relied on a `DomainEvent::DistractionDetected { app, .. }` arm that's now dropped from the enum (see kill-list), skip that test — delete it per pre-release policy.

- [ ] **Step 7: Run**

Run: `cargo nextest run -p feature-coaching signal_accumulator`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-coaching
git commit -m "refactor(feature-coaching): accumulator consumes AiSignal; delete 14-arm conversion.rs"
```

---

## Task 11: Audit `default_conditions()`

**Files:**
- Modify: `crates/feature-coaching/src/signal_accumulator/types.rs`
- Modify: `crates/feature-coaching/src/signal_accumulator/mod.rs`

- [ ] **Step 1: Enumerate conditions + evaluation coverage**

Run: `grep -n 'condition.name.as_str' crates/feature-coaching/src/signal_accumulator/mod.rs`
Expected output: match arms for `low_productivity`, `deadline_approaching`, `focus_quality_declining`, `budget_warning`, `task_avoidance`.

Compare against `default_conditions()`. Every condition there must have a matching arm in `evaluate_condition`. The current set matches 1:1, so nothing to delete.

- [ ] **Step 2: Document rationale**

Replace the trailing comments in `default_conditions()` with a single rationale block justifying the current set:

```rust
/// Default built-in trigger conditions. Each condition here MUST have a
/// matching arm in `SignalAccumulator::evaluate_condition`. Conditions that
/// lost their arm during v1.5 (learning triggers, distraction_streak,
/// context_switch_overload) are handled by pattern_detector alone — keep
/// them out of this list to avoid no-op evaluations.
pub(super) fn default_conditions() -> Vec<TriggerCondition> {
    vec![
        TriggerCondition::new("low_productivity", 1800),
        TriggerCondition::new("deadline_approaching", 3600),
        TriggerCondition::new("focus_quality_declining", 1800),
        TriggerCondition::new("budget_warning", 3600),
        TriggerCondition::new("task_avoidance", 1800),
    ]
}
```

- [ ] **Step 3: Add an invariant test**

Append to `mod tests` in `signal_accumulator/mod.rs`:

```rust
#[test]
fn every_default_condition_has_evaluator() {
    let acc = SignalAccumulator::new();
    let sit = UserSituation::default();
    // For each condition, calling evaluate_condition with a matching
    // synthesized situation must either fire or return None — never panic
    // with `_ => None` hitting a condition name without a real evaluator.
    for c in &acc.conditions {
        let _ = acc.evaluate_condition(c, &sit, jiff::Timestamp::now());
    }
}
```

Expose `evaluate_condition` with `pub(crate)` so the test can reach it.

- [ ] **Step 4: Run**

Run: `cargo nextest run -p feature-coaching`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-coaching
git commit -m "refactor(feature-coaching): audit default_conditions — every entry has an evaluator"
```

---

## Task 12: `CognitiveSignal::domain` → `RecallDomain`

**Files:**
- Modify: `crates/cognitive/src/pipeline/signal.rs`
- Modify: every collector touching `CognitiveSignal`
- Modify: `crates/cognitive/src/pipeline/consolidator.rs`
- Modify: `crates/cognitive/src/pipeline/writer.rs`
- Modify: `crates/feature-coaching/src/pattern_detector/mod.rs` (already patched in Task 7, confirm)

- [ ] **Step 1: Write failing test**

Add to `crates/cognitive/tests/pipeline_types_test.rs`:

```rust
use ai_core::RecallDomain;
use cognitive::pipeline::{CognitiveSignal, SignalContext, SignalSource};
use jiff::Timestamp;

#[test]
fn cognitive_signal_uses_typed_domain() {
    let s = CognitiveSignal {
        source: SignalSource::ChatTurn,
        content: "x".into(),
        domain: RecallDomain::General,
        confidence: 0.5,
        context: SignalContext::default(),
        timestamp: Timestamp::now(),
    };
    assert_eq!(s.domain, RecallDomain::General);
    assert_eq!(s.domain.as_str(), "general");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive pipeline_types`
Expected: FAIL — field is currently `String`.

- [ ] **Step 3: Change the field type**

In `crates/cognitive/src/pipeline/signal.rs`, change `pub domain: String` to `pub domain: ai_core::RecallDomain`.

- [ ] **Step 4: Sweep call sites**

Run: `grep -rn 'CognitiveSignal {' crates/cognitive/src/pipeline --include='*.rs'` — every hit needs its `domain:` field migrated:
- `atom_collector.rs:33` — the domain comes from `DomainEvent::AtomReinforced { domain, .. }` (a String). Replace with:

  ```rust
  let recall_domain = match domain.as_str() {
      "tasks" => RecallDomain::Tasks,
      "finance" => RecallDomain::Finance,
      "learning" => RecallDomain::Learning,
      _ => RecallDomain::General,
  };
  ```

  *Note: after Task 13, this collector reads from `AiSignal.metrics.category` instead, so the match dies.*

- `chat_turn_collector.rs:38` — `RecallDomain::General`
- `coaching_collector.rs:32` — from `DomainEvent::CoachingPatternDetected { domain, .. }` — same match helper as atom_collector
- `recall_collector.rs:105` — `RecallDomain::General`
- `session_collector.rs:81` — `RecallDomain::General`
- Anywhere `.domain` is read as a `&str`, replace with `.domain.as_str()`.

- [ ] **Step 5: Run**

Run: `cargo check -p cognitive`
Then: `cargo nextest run -p cognitive pipeline_types`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): CognitiveSignal.domain is RecallDomain, not String"
```

---

## Task 13: `ChatTurnCollector` → `SignalConsumer`

**Files:**
- Modify: `crates/cognitive/src/pipeline/chat_turn_collector.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/cognitive/src/pipeline/chat_turn_collector.rs::tests`:

```rust
#[tokio::test]
async fn consumer_forwards_chat_turn_signals() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(8);
    let collector = ChatTurnCollector::new(tx);

    let sig = ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: "ChatTurnCompleted",
        importance: 0.3,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: "A long enough message to pass the min length filter".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    };
    collector.consume(&sig).await.unwrap();
    let out = rx.recv().await.unwrap();
    assert_eq!(out.source, SignalSource::ChatTurn);
}

#[tokio::test]
async fn consumer_ignores_non_chat_events() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(8);
    let collector = ChatTurnCollector::new(tx);

    let sig = ai_core::AiSignal {
        event_kind: "SessionEnded",
        ..dummy_ai_signal()
    };
    collector.consume(&sig).await.unwrap();
    assert!(rx.try_recv().is_err());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive chat_turn_collector`
Expected: FAIL.

- [ ] **Step 3: Convert the collector**

Replace the entire `ChatTurnCollector` impl in `crates/cognitive/src/pipeline/chat_turn_collector.rs`:

```rust
use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;
use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const MIN_MESSAGE_LEN: usize = 20;

pub struct ChatTurnCollector {
    tx: SignalSender,
}

impl ChatTurnCollector {
    pub fn new(tx: SignalSender) -> Self { Self { tx } }
}

#[async_trait]
impl SignalConsumer for ChatTurnCollector {
    fn name(&self) -> &'static str { "cognitive.chat_turn" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "ChatTurnCompleted" { return Ok(()); }
        if signal.content.len() < MIN_MESSAGE_LEN { return Ok(()); }

        let session_key = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::ChatTurnCompleted { session_key, .. } => Some(session_key.clone()),
            _ => None,
        });

        let out = CognitiveSignal {
            source: SignalSource::ChatTurn,
            content: signal.content.clone(),
            domain: RecallDomain::General,
            confidence: 0.6,
            context: SignalContext {
                session_key,
                source_count: 1,
                ..Default::default()
            },
            timestamp: Timestamp::now(),
        };
        let _ = self.tx.send(out).await;
        Ok(())
    }
}
```

Delete the old `::start(event_rx, ...)` method entirely. Callers in `app-core/init/mod.rs` now instantiate the collector as `Arc<dyn SignalConsumer>` and pass it to `ai_pipeline::start`.

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive chat_turn_collector`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): ChatTurnCollector is a SignalConsumer"
```

---

## Task 14: `RecallCollector` → `SignalConsumer`

**Files:**
- Modify: `crates/cognitive/src/pipeline/recall_collector.rs`

- [ ] **Step 1: Write failing test**

Append to the module's `mod tests`:

```rust
#[tokio::test]
async fn recall_consumer_buffers_and_flushes() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(32);
    let collector = RecallCollector::new(tx);

    for i in 0..BUFFER_FLUSH_SIZE {
        let sig = make_chat_signal(&format!("rust error handling best practices msg {i}"), &format!("s{}", i % 3));
        collector.consume(&sig).await.unwrap();
    }
    // after BUFFER_FLUSH_SIZE messages flush happens; at least one cluster promoted
    let promoted = rx.try_recv();
    assert!(promoted.is_ok());
}

fn make_chat_signal(msg: &str, session: &str) -> ai_core::AiSignal {
    ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: "ChatTurnCompleted",
        importance: 0.3,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: msg.to_string(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::ChatTurnCompleted {
            session_key: session.to_string(),
            user_message: Some(msg.to_string()),
        }),
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    }
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive recall_consumer`
Expected: FAIL.

- [ ] **Step 3: Convert**

Because `RecallCollector` holds a growing buffer, wrap it in `Arc<Mutex<...>>`:

```rust
use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct RecallCollector {
    tx: SignalSender,
    buffer: Arc<Mutex<Vec<BufferedMessage>>>,
}

impl RecallCollector {
    pub fn new(tx: SignalSender) -> Self {
        Self { tx, buffer: Arc::new(Mutex::new(Vec::new())) }
    }

    async fn flush_if_needed(&self) {
        let mut buf = self.buffer.lock().await;
        if buf.len() >= BUFFER_FLUSH_SIZE {
            Self::flush_buffer(&mut buf, &self.tx).await;
        }
    }
}

#[async_trait]
impl SignalConsumer for RecallCollector {
    fn name(&self) -> &'static str { "cognitive.recall" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "ChatTurnCompleted" { return Ok(()); }
        if signal.content.len() <= MIN_MESSAGE_LEN { return Ok(()); }

        let session_key = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::ChatTurnCompleted { session_key, .. } => Some(session_key.clone()),
            _ => None,
        }).unwrap_or_default();

        {
            let mut buf = self.buffer.lock().await;
            buf.push(BufferedMessage {
                content: signal.content.clone(),
                session_key,
                timestamp: jiff::Timestamp::now(),
            });
        }
        self.flush_if_needed().await;
        Ok(())
    }
}
```

Keep `flush_buffer`, `cluster_messages`, constants identical — just change `RecallDomain::General` where the promoted signal is built.

Delete the old `start(event_rx, ...)` method.

- [ ] **Step 4: Run**

Run: `cargo nextest run -p cognitive recall_consumer`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): RecallCollector is a SignalConsumer"
```

---

## Task 15: `SessionCollector` → `SignalConsumer`

**Files:**
- Modify: `crates/cognitive/src/pipeline/session_collector.rs`

- [ ] **Step 1: Write failing test**

Append to the module:

```rust
#[tokio::test]
async fn session_consumer_ignores_non_session_events() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(8);
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repo = storage::SessionMemoryRepo::new(pool.inner().clone());
    let collector = SessionCollector::new(tx, repo);
    let sig = ai_core::AiSignal {
        event_kind: "ChatTurnCompleted",
        ..session_dummy()
    };
    collector.consume(&sig).await.unwrap();
    assert!(rx.try_recv().is_err());
}
```

(Add a `session_dummy()` helper analogous to `dummy_ai_signal()`.)

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive session_consumer`
Expected: FAIL.

- [ ] **Step 3: Convert**

```rust
use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;

pub struct SessionCollector {
    tx: SignalSender,
    repo: SessionMemoryRepo,
}

impl SessionCollector {
    pub fn new(tx: SignalSender, repo: SessionMemoryRepo) -> Self {
        Self { tx, repo }
    }
}

#[async_trait]
impl SignalConsumer for SessionCollector {
    fn name(&self) -> &'static str { "cognitive.session" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "SessionEnded" { return Ok(()); }
        let session_id = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::SessionEnded { session_id, .. } => Some(session_id.clone()),
            _ => None,
        });
        let Some(session_id) = session_id else { return Ok(()); };

        Self::handle(&session_id, &self.repo, &self.tx).await;
        Ok(())
    }
}
```

Keep `handle`, `extract_insight_sentences`, `keyword_confidence` identical; swap `"general".into()` → `RecallDomain::General`.

Delete the old `start(event_rx, ...)`.

- [ ] **Step 4: Run + commit**

Run: `cargo nextest run -p cognitive session_consumer`

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): SessionCollector is a SignalConsumer"
```

---

## Task 16: `AtomCollector` → `SignalConsumer`

**Files:**
- Modify: `crates/cognitive/src/pipeline/atom_collector.rs`

- [ ] **Step 1: Test**

```rust
#[tokio::test]
async fn atom_consumer_forwards() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(8);
    let collector = AtomCollector::new(tx);
    let sig = ai_core::AiSignal {
        event_kind: "AtomReinforced",
        content: "rust errors".into(),
        metrics: ai_core::AiMetrics { category: Some("learning".into()), ..Default::default() },
        raw_event: Some(bus::DomainEvent::AtomReinforced {
            atom_id: "a1".into(),
            subject: "rust errors".into(),
            domain: "learning".into(),
            reinforcement_count: 3,
        }),
        ..atom_dummy()
    };
    collector.consume(&sig).await.unwrap();
    let out = rx.recv().await.unwrap();
    assert_eq!(out.source, SignalSource::AtomReinforcement);
    assert_eq!(out.domain, ai_core::RecallDomain::Learning);
}
```

- [ ] **Step 2: Verify + implement**

```rust
#[async_trait]
impl SignalConsumer for AtomCollector {
    fn name(&self) -> &'static str { "cognitive.atom" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "AtomReinforced" { return Ok(()); }
        let (atom_id, count) = match signal.raw_event.as_ref() {
            Some(bus::DomainEvent::AtomReinforced { atom_id, reinforcement_count, .. })
                if *reinforcement_count >= MIN_REINFORCEMENT =>
                (atom_id.clone(), *reinforcement_count),
            _ => return Ok(()),
        };
        let confidence = (0.5 + count as f64 * 0.15).min(0.95);
        let domain = signal.metrics.category.as_deref()
            .and_then(|s| match s {
                "tasks" => Some(RecallDomain::Tasks),
                "finance" => Some(RecallDomain::Finance),
                "learning" => Some(RecallDomain::Learning),
                _ => None,
            })
            .unwrap_or(RecallDomain::General);
        let out = CognitiveSignal {
            source: SignalSource::AtomReinforcement,
            content: signal.content.clone(),
            domain,
            confidence,
            context: SignalContext {
                related_atom_ids: vec![atom_id],
                source_count: count as u32,
                ..Default::default()
            },
            timestamp: jiff::Timestamp::now(),
        };
        let _ = self.tx.send(out).await;
        Ok(())
    }
}
```

- [ ] **Step 3: Run + commit**

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): AtomCollector is a SignalConsumer"
```

---

## Task 17: `CoachingCollector` → `SignalConsumer`; Delete `pattern_to_rule()`

**Files:**
- Modify: `crates/cognitive/src/pipeline/coaching_collector.rs`

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn coaching_consumer_uses_declared_rule_text() {
    use ai_core::SignalConsumer;
    let (tx, mut rx) = super::signal_queue(8);
    let collector = CoachingCollector::new(tx);
    let sig = ai_core::AiSignal {
        event_kind: "CoachingPatternDetected",
        content: "Schedule demanding tasks in the morning".into(),
        importance: 0.85,
        raw_event: Some(bus::DomainEvent::CoachingPatternDetected {
            pattern_name: "afternoon_energy_drop".into(),
            confidence: 0.85,
            description: "3/4 after 3pm".into(),
            domain: "productivity".into(),
            signal_count: 4,
            rule_text: "Schedule demanding tasks in the morning".into(),
        }),
        ..coaching_dummy()
    };
    collector.consume(&sig).await.unwrap();
    let out = rx.recv().await.unwrap();
    assert_eq!(out.content, "Schedule demanding tasks in the morning");
    assert_eq!(out.source, SignalSource::CoachingPattern);
}
```

- [ ] **Step 2: Implement**

```rust
use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;

pub struct CoachingCollector { tx: SignalSender }

impl CoachingCollector {
    pub fn new(tx: SignalSender) -> Self { Self { tx } }
}

#[async_trait]
impl SignalConsumer for CoachingCollector {
    fn name(&self) -> &'static str { "cognitive.coaching" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "CoachingPatternDetected" { return Ok(()); }
        let Some(bus::DomainEvent::CoachingPatternDetected {
            domain, signal_count, ..
        }) = signal.raw_event.as_ref() else { return Ok(()); };

        let recall_domain = match domain.as_str() {
            "tasks" => RecallDomain::Tasks,
            "finance" => RecallDomain::Finance,
            "productivity" => RecallDomain::Productivity,
            "learning" => RecallDomain::Learning,
            _ => RecallDomain::General,
        };

        let out = CognitiveSignal {
            source: SignalSource::CoachingPattern,
            content: signal.content.clone(),  // rule_text lives here — no match required
            domain: recall_domain,
            confidence: signal.importance,
            context: SignalContext {
                source_count: *signal_count as u32,
                ..Default::default()
            },
            timestamp: jiff::Timestamp::now(),
        };
        let _ = self.tx.send(out).await;
        Ok(())
    }
}
```

**Delete** the `pattern_to_rule` function and its tests. The 8 mapping strings now live exclusively in `pattern_detector/mod.rs` (populated into `DetectedPattern.rule_text` in Task 7).

- [ ] **Step 3: Run + commit**

Run: `cargo nextest run -p cognitive coaching_collector`

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): CoachingCollector is a SignalConsumer; delete pattern_to_rule"
```

---

## Task 18: Update `consolidator.rs` + `writer.rs` for `RecallDomain`

**Files:**
- Modify: `crates/cognitive/src/pipeline/consolidator.rs`
- Modify: `crates/cognitive/src/pipeline/writer.rs`

- [ ] **Step 1: Patch**

Run: `grep -n '\.domain\b' crates/cognitive/src/pipeline/consolidator.rs crates/cognitive/src/pipeline/writer.rs`

For every read of `.domain` as `&str`, insert `.as_str()`. For every store into a repo column (domain TEXT), use `signal.domain.as_str()`. The DB column remains TEXT — the conversion is only at boundaries.

For `group_signals` that uses `domain` as a hashmap key, switch key type from `String` to `RecallDomain`.

- [ ] **Step 2: Run**

Run: `cargo nextest run -p cognitive`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive
git commit -m "refactor(cognitive): consolidator/writer thread RecallDomain through the convergence layer"
```

---

## Task 19: Annotate `TasksFeature` + `FinanceFeature` with `recall_*` Attrs

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs`
- Modify: `crates/feature-finance/src/lib.rs`

- [ ] **Step 1: Write failing test**

Create `tests/recall_provider_registration.rs` at workspace root:

```rust
use ai_core::{AiFeature, RecallProvider, RecallQuery};
use feature_finance::FinanceFeature;
use feature_tasks::TasksFeature;

#[test]
fn tasks_feature_scores_deadline_queries_higher() {
    let f = TasksFeature::default();
    let hot = RecallQuery { message: "when is the deadline?".into(), intent_summary: None };
    let cold = RecallQuery { message: "what is machine learning".into(), intent_summary: None };
    assert!(f.score_query(&hot) > f.score_query(&cold));
}

#[test]
fn finance_feature_scores_money_queries_higher() {
    let f = FinanceFeature::default();
    let hot = RecallQuery { message: "how much did I spend on food".into(), intent_summary: None };
    let cold = RecallQuery { message: "what time is it".into(), intent_summary: None };
    assert!(f.score_query(&hot) > f.score_query(&cold));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p klyntbot recall_provider_registration`
Expected: FAIL — both features use default `score_query` returning 0.3.

- [ ] **Step 3: Annotate features**

In `crates/feature-tasks/src/lib.rs`, find the `#[derive(AiFeature)]` on `TasksFeature` and add:

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
)]
pub struct TasksFeature { /* existing fields */ }
```

In `crates/feature-finance/src/lib.rs`:

```rust
#[derive(AiFeature)]
#[ai(
    recall_domain = "Finance",
    skill = "finance-management",
    event = "crate::events::FinanceEvent",
    recall_boost_when = "query.message.to_lowercase().contains(\"spend\") || query.message.to_lowercase().contains(\"budget\") || query.message.to_lowercase().contains(\"money\")",
    recall_priority_field = "amount",
    recall_recency_field = "occurred_at",
    recall_status_filter = "status != \"cancelled\"",
)]
pub struct FinanceFeature { /* existing fields */ }
```

If `TasksFeature` / `FinanceFeature` don't have `Default`, add it — required by the test `.default()` call. (They don't hold any non-defaultable fields; `Default` + `#[derive(Default)]` should suffice.)

- [ ] **Step 4: Run**

Run: `cargo nextest run recall_provider_registration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks crates/feature-finance tests
git commit -m "feat(features): declare recall_* attrs on TasksFeature and FinanceFeature"
```

---

## Task 20: Wire `CognitiveContextSource` via `RecallProviderRegistry`

**Files:**
- Modify: `crates/cognitive/src/services/context_source.rs`
- Modify: `crates/app-core/src/init/mod.rs` (registry construction)

- [ ] **Step 1: Write failing test**

Add to `context_source.rs::tests`:

```rust
#[tokio::test]
async fn context_source_uses_recall_registry() {
    let pool = setup().await;
    let registry = ai_core::RecallProviderRegistry::new()
        .with(feature_tasks::TasksFeature::default())
        .with(feature_finance::FinanceFeature::default());

    let fact_repo = SemanticFactRepo::new(pool.clone());
    let rule_repo = ProceduralRuleRepo::new(pool);
    let source = CognitiveContextSource::new(fact_repo, rule_repo)
        .with_recall_registry(registry);

    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c".into(),
        message: Some("when is my deadline".into()),
        intent_summary: None,
        project_id: None,
    };
    let out = source.provide(&ctx).await.unwrap();
    // Registry-ranked feature recommendations appear in the output
    assert!(out.contains("Relevant Domains") || out.contains("tasks"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive context_source_uses_recall_registry`
Expected: FAIL — new method doesn't exist.

- [ ] **Step 3: Wire the registry**

In `CognitiveContextSource`, add:

```rust
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    cache: Mutex<Option<CachedModel>>,
    static_fact_limit: usize,
    confidence_bits: Option<Arc<AtomicU32>>,
    recall_registry: Option<ai_core::RecallProviderRegistry>,
}

impl CognitiveContextSource {
    pub fn with_recall_registry(mut self, reg: ai_core::RecallProviderRegistry) -> Self {
        self.recall_registry = Some(reg);
        self
    }
}
```

In `ContextSource::provide`, after the existing static sections, append:

```rust
if let (Some(reg), Some(msg)) = (&self.recall_registry, _ctx.message.as_deref()) {
    let query = ai_core::RecallQuery {
        message: msg.to_string(),
        intent_summary: _ctx.intent_summary.clone(),
    };
    let ranked = reg.rank(&query);
    if !ranked.is_empty() {
        let lines: Vec<String> = ranked.iter()
            .map(|(d, s)| format!("- {} (score {:.2})", d.as_str(), s))
            .collect();
        sections.push(format!("## Relevant Domains\n{}", lines.join("\n")));
    }
}
```

Note: `_ctx.message` must become `ctx.message` (drop the underscore) since it's now used. Rename the parameter.

- [ ] **Step 4: Wire in `app-core`**

In `crates/app-core/src/init/mod.rs`, build the registry when constructing `CognitiveContextSource`:

```rust
let recall_registry = ai_core::RecallProviderRegistry::new()
    .with(feature_tasks::TasksFeature::default())
    .with(feature_finance::FinanceFeature::default());
let context_source = CognitiveContextSource::new(fact_repo, rule_repo)
    .with_recall_registry(recall_registry);
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive context_source`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive crates/app-core
git commit -m "feat(cognitive): CognitiveContextSource reads RecallProviderRegistry for domain hints"
```

---

## Task 21: `RetrievalFeedbackRepo` Returns Typed `RecallDomain` Tuples

**Files:**
- Modify: `crates/storage/Cargo.toml`
- Modify: `crates/storage/src/repos/retrieval_feedback.rs`
- Modify: `crates/storage/src/repos/tests/retrieval_feedback_tests.rs`
- Modify: callers of `avg_precision_by_domain_since`

- [ ] **Step 1: Add ai-core dep**

In `crates/storage/Cargo.toml`:

```toml
ai-core.workspace = true
```

- [ ] **Step 2: Write failing test**

In `retrieval_feedback_tests.rs`, update an existing test (or add a new one):

```rust
#[tokio::test]
async fn avg_precision_by_domain_returns_typed_recall_domain() {
    let pool = setup().await;
    seed_feedback(&pool).await;
    let repo = RetrievalFeedbackRepo::new(pool);
    let rows: Vec<(ai_core::RecallDomain, f64)> = repo.avg_precision_by_domain_since(7).await.unwrap();
    assert!(rows.iter().any(|(d, _)| *d == ai_core::RecallDomain::Tasks));
}
```

- [ ] **Step 3: Verify failure**

Run: `cargo nextest run -p storage avg_precision_by_domain`
Expected: FAIL.

- [ ] **Step 4: Update the repo method**

```rust
use ai_core::RecallDomain;

pub async fn avg_precision_by_domain_since(
    &self,
    days: i64,
) -> Result<Vec<(RecallDomain, f64)>, sqlx::Error> {
    let since = (jiff::Timestamp::now()
        - jiff::SignedDuration::from_hours(days * 24)).as_millisecond();
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT f.domain, AVG(rf.precision) as avg_precision
         FROM retrieval_feedback rf,
              json_each(rf.retrieved_fact_ids) je
         JOIN semantic_facts f ON f.id = je.value
         WHERE rf.created_at > ?1
         GROUP BY f.domain
         HAVING COUNT(*) >= 3
         ORDER BY avg_precision ASC",
    )
    .bind(since)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows.into_iter()
        .map(|(s, score)| (parse_recall_domain(&s), score))
        .collect())
}

fn parse_recall_domain(s: &str) -> RecallDomain {
    match s {
        "tasks" => RecallDomain::Tasks,
        "finance" => RecallDomain::Finance,
        "productivity" => RecallDomain::Productivity,
        "learning" => RecallDomain::Learning,
        _ => RecallDomain::General,
    }
}
```

- [ ] **Step 5: Fix callers**

Run: `grep -rn 'avg_precision_by_domain_since' crates/` — every caller that reads the returned `String` must now read `RecallDomain` (use `.as_str()` if it passes on as a string).

- [ ] **Step 6: Run**

Run: `cargo nextest run -p storage`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/storage
git commit -m "feat(storage): RetrievalFeedbackRepo::avg_precision_by_domain returns RecallDomain"
```

---

## Task 22: Register v1.5 Consumers in `app-core/src/init/mod.rs`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Replace Phase 8 block**

Find the existing Phase 8 block (around line 418):

```rust
// ── Phase 8: AI Pipeline — SignalRouter + IngestionConsumer ──────
let ai_pipeline_router = {
    let ingestion = /* ... */;
    let router = ai_pipeline::start(Arc::clone(&domain_event_bus), vec![ingestion]);
    ...
};
```

Replace with:

```rust
// ── Phase 8: AI Pipeline — SignalRouter + all v1.5 consumers ──────
let ai_pipeline_router = {
    let ingestion: Arc<dyn SignalConsumer> = Arc::new(IngestionConsumer::new(
        observation_repo.clone(), entity_repo.clone(),
    ));

    // 5 cognitive collectors — each SignalConsumer pushes CognitiveSignals
    // to the existing consolidator tx (signal_queue).
    let (cognitive_tx, cognitive_rx) = cognitive::pipeline::signal_queue(128);
    let chat_turn: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::ChatTurnCollector::new(cognitive_tx.clone()),
    );
    let recall: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::RecallCollector::new(cognitive_tx.clone()),
    );
    let session: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::SessionCollector::new(cognitive_tx.clone(), session_memory_repo.clone()),
    );
    let atom: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::AtomCollector::new(cognitive_tx.clone()),
    );
    let coaching_collector: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::CoachingCollector::new(cognitive_tx.clone()),
    );

    // Coaching
    let (coaching_signal_tx, coaching_signal_rx) = tokio::sync::mpsc::channel(256);
    let coaching_consumer: Arc<dyn SignalConsumer> = Arc::new(
        feature_coaching::CoachingSignalConsumer::new(coaching_signal_tx),
    );

    let router = ai_pipeline::start(
        Arc::clone(&domain_event_bus),
        vec![ingestion, chat_turn, recall, session, atom, coaching_collector, coaching_consumer],
    );
    info!("AI pipeline SignalRouter started with 7 consumers (ingestion + 5 cognitive + coaching)");

    // Launch the cognitive consolidator task (reads cognitive_rx)
    tokio::spawn(cognitive::pipeline::consolidator::run(cognitive_rx, /* writer args */));

    // CoachingService now reads AiSignals instead of DomainEvents
    CoachingService::start(coaching_signal_rx, accumulator, /* existing args */);

    router
};
```

(The exact method names around consolidator startup and CoachingService arguments are whatever already exists in Phase 8 — this task just rewires the inputs.)

- [ ] **Step 2: Run**

Run: `cargo build --workspace`
Expected: PASS.

Run: `cargo nextest run --workspace`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): register 5 cognitive collectors + CoachingSignalConsumer in Phase 8"
```

---

## Task 23: Integration — `AiSignal` → `CoachingSignalConsumer` → Trigger Fires

**Files:**
- Create: `tests/coaching_pipeline_integration.rs`

- [ ] **Step 1: Write**

```rust
use ai_core::SignalConsumer;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn budget_alert_event_drives_coaching_intervention_via_ai_pipeline() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));

    let (coaching_tx, coaching_rx) = mpsc::channel(32);
    let consumer: Arc<dyn SignalConsumer> = Arc::new(
        feature_coaching::CoachingSignalConsumer::new(coaching_tx),
    );
    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    let (intervention_tx, mut intervention_rx) = mpsc::channel(32);
    let accumulator = Arc::new(Mutex::new(feature_coaching::SignalAccumulator::new()));
    let detector = Arc::new(Mutex::new(feature_coaching::PatternDetector::new()));
    let router = Arc::new(Mutex::new(feature_coaching::InterventionRouter::default()));
    let feedback = Arc::new(Mutex::new(feature_coaching::FeedbackTracker::new()));
    let situation = Arc::new(Mutex::new(cognitive::situation::UserSituation {
        coaching_receptivity: 0.8,
        ..Default::default()
    }));
    let mock_reasoner = /* ... mock returning should_intervene=true with message */;
    let cancel = CancellationToken::new();

    let _service = feature_coaching::CoachingService::start(
        coaching_rx, accumulator, detector, router, feedback, situation,
        mock_reasoner, intervention_tx, None, cancel.clone(),
    );

    // Publish the event: the router translates → AiSignal → consumer →
    // CoachingService → trigger fires
    bus.publish(bus::DomainEvent::BudgetAlert {
        category: "food".into(), spent: 450.0, limit: 500.0,
    });

    let intervention = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        intervention_rx.recv(),
    ).await.expect("timeout").expect("channel closed");
    assert!(intervention.message.contains("budget") || intervention.trigger_name == "budget_warning");
    cancel.cancel();
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test coaching_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(ai-pipeline): e2e coaching — BudgetAlert → AiSignal → intervention delivered"
```

---

## Task 24: Integration — `DomainEvent` → All 5 Collectors

**Files:**
- Create: `tests/collector_fanout_integration.rs`

- [ ] **Step 1: Write**

```rust
use ai_core::SignalConsumer;
use std::sync::Arc;

#[tokio::test]
async fn chat_turn_event_reaches_chat_and_recall_collectors_only() {
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let (tx, mut rx) = cognitive::pipeline::signal_queue(32);

    let chat_turn: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::ChatTurnCollector::new(tx.clone())
    );
    let recall: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::RecallCollector::new(tx.clone())
    );
    let atom: Arc<dyn SignalConsumer> = Arc::new(
        cognitive::pipeline::AtomCollector::new(tx.clone())
    );

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![chat_turn, recall, atom],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(bus::DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        user_message: Some("a sufficiently long message to exceed the min filter".into()),
    });

    let sig = tokio::time::timeout(
        std::time::Duration::from_secs(1), rx.recv(),
    ).await.unwrap().unwrap();
    assert_eq!(sig.source, cognitive::pipeline::SignalSource::ChatTurn);
    // Recall buffers silently; Atom ignores this event → no further CognitiveSignals
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test collector_fanout_integration`

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test(ai-pipeline): e2e — ChatTurnCompleted reaches ChatTurn collector via AiSignal"
```

---

## Task 25: Invariant — No Raw Domain Literals in AI Crates

**Files:**
- Create: `tests/ai_no_domain_literals.rs`

- [ ] **Step 1: Write**

```rust
//! Invariant: RecallDomain is the source of truth; no AI crate may carry
//! raw "general" / "tasks" / "finance" / "productivity" / "learning"
//! string literals outside the RecallDomain::as_str match itself.

use std::path::PathBuf;

const FORBIDDEN: &[&str] = &[
    "\"general\"", "\"tasks\"", "\"finance\"", "\"productivity\"", "\"learning\"",
];

const SCAN_DIRS: &[&str] = &[
    "crates/cognitive/src/pipeline",
    "crates/feature-coaching/src/pattern_detector",
    "crates/feature-coaching/src/signal_accumulator",
];

// Files allowed to carry the raw string (e.g. RecallDomain::as_str, repo
// impls that parse columns).
const ALLOWLIST: &[&str] = &[
    "crates/ai-core/src/recall_domain.rs",
    "crates/storage/src/repos/retrieval_feedback.rs",
];

#[test]
fn no_raw_domain_literals_in_ai_crates() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations: Vec<String> = Vec::new();
    for dir in SCAN_DIRS {
        for entry in walkdir::WalkDir::new(root.join(dir)) {
            let entry = entry.unwrap();
            if !entry.file_name().to_string_lossy().ends_with(".rs") { continue; }
            let path = entry.path();
            if ALLOWLIST.iter().any(|a| path.ends_with(a)) { continue; }
            let text = std::fs::read_to_string(path).unwrap();
            for (lineno, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") { continue; }
                for lit in FORBIDDEN {
                    if line.contains(lit) {
                        violations.push(format!("{}:{}: {}", path.display(), lineno + 1, line.trim()));
                    }
                }
            }
        }
    }
    assert!(violations.is_empty(), "domain literal violations:\n{}", violations.join("\n"));
}
```

Add `walkdir = "2"` to root `[workspace.dev-dependencies]` (or the facade crate's `dev-dependencies`).

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_no_domain_literals`
Expected: PASS. If it fails, convert flagged literals to `RecallDomain` values; do not weaken the test.

- [ ] **Step 3: Commit**

```bash
git add tests Cargo.toml
git commit -m "test(ai-pipeline): invariant — no raw domain literals in AI crates"
```

---

## Task 26: Final Verification

**Files:** none (verification only)

- [ ] **Step 1: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: 0 warnings (except pre-existing `desktop` crate exceptions).

- [ ] **Step 2: Full test pass**

Run: `cargo nextest run --workspace`
Expected: PASS.

Run: `cargo test --workspace --doc`
Expected: PASS.

- [ ] **Step 3: Grep sanity checks**

```bash
# No DomainEvent broadcast::Receiver outside the router + activity-log
rg 'broadcast::Receiver<.*DomainEvent' crates/cognitive/src crates/feature-coaching/src
# Expected: 0 hits.

# No pattern_to_rule function anywhere
rg 'fn pattern_to_rule' crates/
# Expected: 0 hits.

# No signal_accumulator/conversion.rs
ls crates/feature-coaching/src/signal_accumulator/
# Expected: mod.rs types.rs  (no conversion.rs)

# No hardcoded 14-variant match for coaching
rg 'KIND_TASK_COMPLETED.*=>.*Signal' crates/feature-coaching/
# Expected: 0 hits.
```

All expected to be 0-hit. Any hit is a plan-scope violation — fix before closing.

- [ ] **Step 4: Manual smoke**

Run `cargo tauri dev` and:
1. Trigger a budget overrun — observe a coaching intervention appear in chat.
2. Complete a task — observe it show up in the cognitive observation log.
3. Issue a chat query mentioning "deadline" — observe the context source log the Tasks domain as relevant.

(These are explanatory checks, not automated, so don't block the merge — but do them before marking v1.5 done.)

- [ ] **Step 5: Commit (summary / empty)**

If earlier tasks already commit verification fixups, there's nothing to commit here. Otherwise:

```bash
git commit --allow-empty -m "chore(ai-pipeline): close v1.5 — all consumers migrated, invariants green"
```

---

## v1.5 Done Criteria (from spec §6)

- [ ] No string domain literals in AI subsystems (Task 25 invariant test green).
- [ ] Coaching signal whitelist is empty — every prior `conversion.rs` arm is now a declared attribute (Task 10 file deleted).
- [ ] All cognitive collectors consume `AiSignal` via `SignalRouter`, not raw `DomainEvent` (Tasks 13–17; no `broadcast::Receiver<DomainEvent>` in the 5 collector files).
- [ ] `RetrievalFeedbackRepo::avg_precision_by_domain_since` returns `RecallDomain`, not `String` (Task 21).
- [ ] `CognitiveContextSource` reads `RecallProviderRegistry` instead of a hardcoded domain list (Task 20).
- [ ] `cargo clippy --workspace --all-targets --all-features` is clean (Task 26).
- [ ] `cargo nextest run --workspace` is green (Task 26).
