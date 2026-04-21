# AI Pipeline v1 — Foundation + Tasks + Finance + Cognitive

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the unified AI feature pipeline foundation (`ai-core` + `ai-core-macros`), migrate Tasks and Finance to it, replace cognitive's hardcoded event dispatch with a generic `SignalConsumer`, and clean up the trait + 3 pre-existing bugs uncovered during audit. No parallel paths, no backward compat, no dead code left behind.

**Architecture:** Two new L1 crates (`ai-core` runtime, `ai-core-macros` proc-macros) expose `#[derive(AiFeature, AiEvent, AiEntity)]` and a `SignalRouter` that broadcasts `AiSignal`s to registered `SignalConsumer` implementations. Cognitive ingestion becomes a consumer that reads declared importance/salience directly from signals, eliminating the 300-line `match` table in `background.rs`. Tasks and Finance each declare one event enum; conversion to the global `DomainEvent` is derived.

**Tech Stack:** Rust 1.93 (stable), `syn` + `quote` + `proc-macro2` for macros, `async-trait` for the consumer trait, `inventory` for cross-crate `RecallDomain` variant collection (with mechanical fallback), `sqlx` via existing `storage` crate, `cargo-nextest` for tests.

**Spec:** `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md` — v1 scope only.

**Pre-release posture:** Every old path deleted in the same PR that introduces the new one. No feature flags, no deprecation. Migrations edit in-place.

---

## File Structure

### New crates

```
crates/ai-core/
  Cargo.toml
  src/lib.rs            — re-exports
  src/signal.rs         — AiSignal, SalienceVerdict, EntityRef
  src/traits.rs         — AiFeature, AiEventMeta, AiEntity, SignalConsumer, RecallProvider
  src/router.rs         — SignalRouter runtime (subscribes to DomainEventBus, fans out)
  src/recall_domain.rs  — generated RecallDomain enum entry point
  src/recall.rs         — RecallQuery, RecallItem types
  tests/router_test.rs  — router fanout integration test

crates/ai-core-macros/
  Cargo.toml            — proc-macro = true
  src/lib.rs            — derive macro entry points
  src/ai_event.rs       — #[derive(AiEvent)] implementation
  src/ai_entity.rs      — #[derive(AiEntity)] implementation
  src/ai_feature.rs     — #[derive(AiFeature)] implementation
  src/attrs.rs          — shared attribute parsing
  tests/expand/         — trybuild snapshot tests
```

### Modified crates

```
crates/bus/src/domain_events.rs     — delete 25 dead variants, add From<TaskEvent>, From<FinanceEvent>
crates/tools-core/src/feature.rs    — delete config_key() and default_config(); enforce no migrations_static()
crates/storage/src/pool.rs          — add migration collision detection
crates/feature-tasks/src/events.rs  — NEW: TaskEvent enum with #[derive(AiEvent)]
crates/feature-tasks/src/lib.rs     — AiFeature impl; restore tools() returning TaskTool
crates/feature-tasks/src/types/entity.rs — add #[derive(AiEntity)] to Task
crates/feature-tasks/src/cognitive_bridge.rs — DELETE entire file
crates/feature-tasks/src/tool/mod.rs — delete agentic/hybrid schema entries
crates/feature-tasks/src/types/entity.rs — delete TaskType::from_str agentic/hybrid arms
crates/feature-finance/src/events.rs — NEW: FinanceEvent enum
crates/feature-finance/src/lib.rs   — AiFeature impl; add missing lifecycle events
crates/feature-finance/src/types/domain.rs — add #[derive(AiEntity)] to FinanceTransaction
crates/cognitive/src/services/background.rs — replace match arms with SignalConsumer impl
crates/cognitive/src/services/salience.rs — delete (logic moves into attributes)
crates/cognitive/src/repos/event_log.rs — fix count_by_event_type_and_data column reference
crates/app-core/src/init/cognitive.rs — fix payload storage to serialize full DomainEvent
crates/app-core/src/init/storage.rs — drop migrations_static() calls; use trait uniformly
crates/app-core/src/handlers/timeline.rs — adjust reader to new JSON payload
crates/app-core/src/init/ai_pipeline.rs — NEW: register SignalConsumers
crates/agent/src/agent_loop/builder.rs — delete lines 1257-1318 (TaskTool hand-wiring)
crates/agent/src/autotuner/metric_collector.rs — replace event_type literals with typed constants
crates/cognitive/src/services/reforge/feedback.rs — replace UserCorrectedAI literal
crates/activity-log/src/normalizers.rs — remove arms for deleted variants
crates/simulator/src/actions.rs — remove deleted variant constructors
crates/simulator/src/harness.rs — same
```

### New test files

```
tests/ai_pipeline_integration.rs  — contract test: event → signal → consumer
tests/ai_no_missed_data.rs        — invariant: every declared event produces a signal
```

---

## Task Overview

| # | Task | Phase |
|---|---|---|
| 1 | Create `ai-core` crate skeleton with `AiSignal` type | Foundation |
| 2 | Define `AiFeature`, `AiEventMeta`, `AiEntity`, `SignalConsumer`, `RecallProvider` traits | Foundation |
| 3 | Create `ai-core-macros` crate skeleton with trybuild harness | Macros |
| 4 | Implement `#[derive(AiEvent)]` with `importance`, `salience`, `entity_bridge`, `observation_template` | Macros |
| 5 | Implement `#[derive(AiEntity)]` with `embed_on` | Macros |
| 6 | Implement `#[derive(AiFeature)]` with `recall_domain` + `RecallDomain` enum generation | Macros |
| 7 | Implement `SignalRouter` runtime | Foundation |
| 8 | Pre-bug fix: `domain_event_log` payload stores full JSON | Bug fixes |
| 9 | Pre-bug fix: `count_by_event_type_and_data` column reference | Bug fixes |
| 10 | Pre-bug fix: Tasks migration version drift | Bug fixes |
| 11 | Clean `FeaturePackage` trait: delete orphan methods; add migration collision detection | Trait cleanup |
| 12 | Migrate Tasks: `TaskEvent` enum + `AiFeature` impl | Tasks |
| 13 | Migrate Tasks: annotate `Task` with `AiEntity`; restore `tools()` path | Tasks |
| 14 | Delete Tasks dead code: `cognitive_bridge.rs`, agentic/hybrid schema, orphan tool params | Tasks |
| 15 | Remove Tasks hand-wiring in agent builder | Tasks |
| 16 | Migrate Finance: `FinanceEvent` enum + missing lifecycle events | Finance |
| 17 | Migrate Finance: annotate `FinanceTransaction`; wire embedding | Finance |
| 18 | Cognitive `SignalConsumer`: ingestion + salience via attributes | Cognitive |
| 19 | Delete `background.rs` match arms and `salience.rs` | Cognitive |
| 20 | Delete 25 dead `DomainEvent` variants + update `variant_name()` | Cognitive |
| 21 | Update `activity-log` normalizer for deleted variants | Downstream |
| 22 | Update `simulator` for deleted variants + typed constructors | Downstream |
| 23 | Replace string-literal `event_type` queries with typed constants | Downstream |
| 24 | Register `SignalConsumer`s in `app-core` init | Integration |
| 25 | Contract integration test: event → signal → consumer | Tests |
| 26 | Invariant test: every declared event emits a signal | Tests |
| 27 | End-to-end: Task creation flows through the pipeline | Tests |
| 28 | End-to-end: Finance transaction flows through the pipeline | Tests |
| 29 | Final verification: clippy, nextest, no placeholders | Done |

---

## Task 1: Create `ai-core` Crate Skeleton with `AiSignal` Type

**Files:**
- Create: `crates/ai-core/Cargo.toml`
- Create: `crates/ai-core/src/lib.rs`
- Create: `crates/ai-core/src/signal.rs`
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Write failing test**

Create `crates/ai-core/tests/signal_test.rs`:

```rust
use ai_core::{AiSignal, SalienceVerdict, EntityRef};
use jiff::Timestamp;

#[test]
fn signal_construction_sets_all_fields() {
    let sig = AiSignal {
        domain: "tasks".into(),
        event_kind: "TaskCreated",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "Created task: Ship v1".into(),
        entity: Some(EntityRef {
            entity_type: "task",
            id: "abc123".into(),
            name: "Ship v1".into(),
        }),
        timestamp: Timestamp::now(),
    };
    assert_eq!(sig.event_kind, "TaskCreated");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert!(sig.entity.is_some());
}

#[test]
fn salience_verdict_variants() {
    let _ = SalienceVerdict::Extract;
    let _ = SalienceVerdict::Accumulate;
    let _ = SalienceVerdict::Discard;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p ai-core`
Expected: FAIL — crate does not exist.

- [ ] **Step 3: Create Cargo.toml**

Create `crates/ai-core/Cargo.toml`:

```toml
[package]
name = "ai-core"
version = "0.1.0"
edition = "2021"

[dependencies]
common.workspace = true
bus.workspace = true
jiff.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
async-trait.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["full", "macros", "test-util"] }
```

(If any of these are not yet `workspace` dependencies, add them to the root `Cargo.toml` `[workspace.dependencies]` with matching versions already used elsewhere in the workspace.)

- [ ] **Step 4: Create signal.rs**

Create `crates/ai-core/src/signal.rs`:

```rust
use bus::DomainEvent;
use jiff::Timestamp;

/// The unified signal every feature emits and every AI consumer reads.
///
/// Produced from a `DomainEvent` via `#[derive(AiEvent)]`-generated
/// `AiEventMeta::to_signal()`. Broadcast by `SignalRouter` to all
/// registered `SignalConsumer` implementations.
#[derive(Debug, Clone)]
pub struct AiSignal {
    pub domain: String,             // becomes RecallDomain after Task 6
    pub event_kind: &'static str,
    pub importance: f64,
    pub salience: SalienceVerdict,
    pub content: String,
    pub entity: Option<EntityRef>,
    pub timestamp: Timestamp,
}

impl AiSignal {
    /// Convenience for consumers that need the originating event payload.
    /// The router sets this via `with_raw_event`.
    pub fn raw_event(&self) -> Option<&DomainEvent> { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalienceVerdict {
    Extract,
    Accumulate,
    Discard,
}

#[derive(Debug, Clone)]
pub struct EntityRef {
    pub entity_type: &'static str,
    pub id: String,
    pub name: String,
}
```

- [ ] **Step 5: Create lib.rs**

Create `crates/ai-core/src/lib.rs`:

```rust
//! Unified AI feature pipeline.
//!
//! See `docs/superpowers/specs/2026-04-21-unified-ai-feature-pipeline-design.md`.

pub mod signal;

pub use signal::{AiSignal, EntityRef, SalienceVerdict};
```

- [ ] **Step 6: Add crate to workspace**

In root `Cargo.toml` `[workspace] members`, append `"crates/ai-core"`. In `[workspace.dependencies]`, add:

```toml
ai-core = { path = "crates/ai-core" }
```

- [ ] **Step 7: Run test**

Run: `cargo nextest run -p ai-core`
Expected: PASS (2 tests).

- [ ] **Step 8: Commit**

```bash
git add crates/ai-core Cargo.toml
git commit -m "feat(ai-core): bootstrap crate with AiSignal + SalienceVerdict"
```

---

## Task 2: Define `AiFeature`, `AiEventMeta`, `AiEntity`, `SignalConsumer`, `RecallProvider` Traits

**Files:**
- Create: `crates/ai-core/src/traits.rs`
- Create: `crates/ai-core/src/recall.rs`
- Modify: `crates/ai-core/src/lib.rs`

- [ ] **Step 1: Write failing trait-object test**

Create `crates/ai-core/tests/traits_test.rs`:

```rust
use ai_core::{AiSignal, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::{Arc, Mutex};

struct RecordingConsumer {
    seen: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl SignalConsumer for RecordingConsumer {
    fn name(&self) -> &'static str { "recording" }
    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        self.seen.lock().unwrap().push(signal.event_kind);
        Ok(())
    }
}

#[tokio::test]
async fn consumer_receives_signal() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let consumer: Arc<dyn SignalConsumer> = Arc::new(RecordingConsumer { seen: log.clone() });
    let sig = AiSignal {
        domain: "tasks".into(),
        event_kind: "TaskCreated",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "x".into(),
        entity: None,
        timestamp: Timestamp::now(),
    };
    consumer.consume(&sig).await.unwrap();
    assert_eq!(log.lock().unwrap().as_slice(), &["TaskCreated"]);
}
```

- [ ] **Step 2: Verify test fails to compile**

Run: `cargo nextest run -p ai-core`
Expected: FAIL — `SignalConsumer` does not exist.

- [ ] **Step 3: Implement recall.rs**

Create `crates/ai-core/src/recall.rs`:

```rust
/// Query the retrieval layer uses to ask each feature "are you relevant?"
#[derive(Debug, Clone)]
pub struct RecallQuery {
    pub message: String,
    pub intent_summary: Option<String>,
}

/// A candidate the retrieval layer considers for prompt injection.
#[derive(Debug, Clone)]
pub struct RecallItem {
    pub id: String,
    pub text: String,
    pub score: f64,
    pub domain: String,
}
```

- [ ] **Step 4: Implement traits.rs**

Create `crates/ai-core/src/traits.rs`:

```rust
use crate::{AiSignal, recall::{RecallItem, RecallQuery}};
use async_trait::async_trait;
use bus::DomainEvent;

/// Feature-level declaration. Implemented via `#[derive(AiFeature)]`.
pub trait AiFeature: Send + Sync + 'static {
    /// Canonical lowercase name, e.g. "tasks".
    const DOMAIN: &'static str;
    /// Skill filename under `skills/` (without .md).
    const SKILL: &'static str;
    type Event: AiEventMeta + Into<DomainEvent>;
}

/// Event-level declaration. Implemented via `#[derive(AiEvent)]`.
pub trait AiEventMeta {
    fn to_signal(&self) -> AiSignal;
    fn event_kind(&self) -> &'static str;
}

/// Entity-level declaration. Implemented via `#[derive(AiEntity)]`.
pub trait AiEntity {
    fn embed_text(&self) -> String;
    fn entity_type() -> &'static str where Self: Sized;
    fn recall_filter(&self) -> bool { true }
}

/// Generic subscriber.
#[async_trait]
pub trait SignalConsumer: Send + Sync {
    fn name(&self) -> &'static str;
    async fn consume(&self, signal: &AiSignal) -> common::Result<()>;
}

/// Optional retrieval-side interface for features that want custom recall behaviour.
pub trait RecallProvider: Send + Sync {
    fn domain(&self) -> &'static str;
    fn score_query(&self, query: &RecallQuery) -> f64 { 0.0 }
    fn candidates(&self, _query: &RecallQuery) -> Vec<RecallItem> { Vec::new() }
}
```

- [ ] **Step 5: Update lib.rs**

Replace `crates/ai-core/src/lib.rs` with:

```rust
//! Unified AI feature pipeline.
pub mod recall;
pub mod signal;
pub mod traits;

pub use recall::{RecallItem, RecallQuery};
pub use signal::{AiSignal, EntityRef, SalienceVerdict};
pub use traits::{AiEntity, AiEventMeta, AiFeature, RecallProvider, SignalConsumer};
```

- [ ] **Step 6: Run test**

Run: `cargo nextest run -p ai-core`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/ai-core
git commit -m "feat(ai-core): define AiFeature / AiEventMeta / AiEntity / SignalConsumer traits"
```

---

## Task 3: Create `ai-core-macros` Crate Skeleton with trybuild Harness

**Files:**
- Create: `crates/ai-core-macros/Cargo.toml`
- Create: `crates/ai-core-macros/src/lib.rs`
- Create: `crates/ai-core-macros/tests/expand_smoke.rs`
- Create: `crates/ai-core-macros/tests/expand/noop.rs`
- Modify: root `Cargo.toml`

- [ ] **Step 1: Write failing smoke test**

Create `crates/ai-core-macros/tests/expand/noop.rs`:

```rust
use ai_core_macros::AiEvent;

#[derive(AiEvent)]
enum Empty {}

fn main() {}
```

Create `crates/ai-core-macros/tests/expand_smoke.rs`:

```rust
#[test]
fn expansion_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/noop.rs");
}
```

- [ ] **Step 2: Create Cargo.toml**

Create `crates/ai-core-macros/Cargo.toml`:

```toml
[package]
name = "ai-core-macros"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
syn = { workspace = true, features = ["full", "extra-traits"] }
quote.workspace = true
proc-macro2.workspace = true

[dev-dependencies]
ai-core = { path = "../ai-core" }
trybuild = "1"
```

Add `syn`, `quote`, `proc-macro2` to root workspace dependencies if not present.

- [ ] **Step 3: Create lib.rs stub**

Create `crates/ai-core-macros/src/lib.rs`:

```rust
use proc_macro::TokenStream;

#[proc_macro_derive(AiEvent, attributes(ai))]
pub fn derive_ai_event(_input: TokenStream) -> TokenStream {
    // Minimal valid expansion — real logic lands in Task 4.
    TokenStream::new()
}

#[proc_macro_derive(AiEntity, attributes(ai))]
pub fn derive_ai_entity(_input: TokenStream) -> TokenStream { TokenStream::new() }

#[proc_macro_derive(AiFeature, attributes(ai))]
pub fn derive_ai_feature(_input: TokenStream) -> TokenStream { TokenStream::new() }
```

- [ ] **Step 4: Register crate**

In root `Cargo.toml` `[workspace] members`, append `"crates/ai-core-macros"`. In `[workspace.dependencies]`:

```toml
ai-core-macros = { path = "crates/ai-core-macros" }
```

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p ai-core-macros`
Expected: PASS — the empty enum compiles with the stub expansion.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core-macros Cargo.toml
git commit -m "feat(ai-core-macros): scaffold proc-macro crate with trybuild harness"
```

---

## Task 4: Implement `#[derive(AiEvent)]` — importance, salience, entity_bridge, observation_template

**Files:**
- Create: `crates/ai-core-macros/src/attrs.rs`
- Create: `crates/ai-core-macros/src/ai_event.rs`
- Modify: `crates/ai-core-macros/src/lib.rs`
- Create: `crates/ai-core-macros/tests/expand/event_basic.rs`

- [ ] **Step 1: Write failing test with annotated enum**

Create `crates/ai-core-macros/tests/expand/event_basic.rs`:

```rust
use ai_core::{AiEventMeta, AiSignal, SalienceVerdict};
use ai_core_macros::AiEvent;

#[derive(AiEvent)]
pub enum TaskEvent {
    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Created task: {title}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    )]
    Created { task_id: String, title: String },

    #[ai(
        importance = 0.5,
        salience = "extract_if(deviation_pct > 50.0)",
        observation_template = "Completed {title} (dev {deviation_pct:?}%)",
    )]
    Completed { task_id: String, title: String, deviation_pct: Option<f64> },
}

fn main() {
    let e = TaskEvent::Created { task_id: "abc".into(), title: "Ship".into() };
    let sig: AiSignal = e.to_signal();
    assert_eq!(sig.event_kind, "Created");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert_eq!(sig.content, "Created task: Ship");
    let entity = sig.entity.as_ref().unwrap();
    assert_eq!(entity.entity_type, "task");
    assert_eq!(entity.id, "abc");
    assert_eq!(entity.name, "Ship");

    let e = TaskEvent::Completed { task_id: "x".into(), title: "y".into(), deviation_pct: Some(80.0) };
    let sig = e.to_signal();
    assert!(matches!(sig.salience, SalienceVerdict::Extract));

    let e = TaskEvent::Completed { task_id: "x".into(), title: "y".into(), deviation_pct: Some(10.0) };
    let sig = e.to_signal();
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo nextest run -p ai-core-macros`
Expected: FAIL — derive is a no-op, `to_signal` not generated.

- [ ] **Step 3: Implement attribute parsing**

Create `crates/ai-core-macros/src/attrs.rs`:

```rust
use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit, Meta, Token, punctuated::Punctuated, parse::{Parse, ParseStream}};

pub struct AiEventAttr {
    pub importance: Option<f64>,
    pub importance_fn: Option<syn::Path>,
    pub salience: SalienceSpec,
    pub observation_template: Option<String>,
    pub entity_bridge: Option<EntityBridge>,
}

pub enum SalienceSpec {
    Accumulate,
    Extract,
    Discard,
    ExtractIf(syn::Expr),
}

pub struct EntityBridge {
    pub entity_type: String,
    pub name_from: syn::Ident,
    pub id_from: syn::Ident,
}

pub fn parse_ai_event_attr(attrs: &[Attribute]) -> syn::Result<AiEventAttr> {
    let ai_attr = attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new(Span::call_site(),
            "every variant must have #[ai(...)] attribute"))?;

    let mut importance = None;
    let mut importance_fn = None;
    let mut salience = None;
    let mut observation_template = None;
    let mut entity_bridge = None;

    ai_attr.parse_nested_meta(|meta| {
        let name = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match name.as_str() {
            "importance" => {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit { lit: Lit::Float(f), .. }) = value {
                    importance = Some(f.base10_parse::<f64>()?);
                } else if let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = value {
                    importance = Some(i.base10_parse::<f64>()?);
                } else {
                    return Err(meta.error("importance must be a numeric literal"));
                }
            }
            "importance_fn" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                importance_fn = Some(syn::parse_str(&s.value())?);
            }
            "salience" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                salience = Some(parse_salience(&s.value())?);
            }
            "observation_template" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                observation_template = Some(s.value());
            }
            "entity_bridge" => {
                entity_bridge = Some(parse_entity_bridge(&meta)?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    Ok(AiEventAttr {
        importance,
        importance_fn,
        salience: salience.unwrap_or(SalienceSpec::Accumulate),
        observation_template,
        entity_bridge,
    })
}

fn parse_salience(s: &str) -> syn::Result<SalienceSpec> {
    let s = s.trim();
    match s {
        "accumulate" => Ok(SalienceSpec::Accumulate),
        "extract" => Ok(SalienceSpec::Extract),
        "discard" => Ok(SalienceSpec::Discard),
        _ if s.starts_with("extract_if(") && s.ends_with(')') => {
            let inner = &s["extract_if(".len()..s.len()-1];
            let expr: syn::Expr = syn::parse_str(inner)
                .map_err(|e| syn::Error::new(Span::call_site(),
                    format!("invalid extract_if expression: {}", e)))?;
            Ok(SalienceSpec::ExtractIf(expr))
        }
        _ => Err(syn::Error::new(Span::call_site(),
            format!("unknown salience verdict: {}", s))),
    }
}

fn parse_entity_bridge(meta: &syn::meta::ParseNestedMeta) -> syn::Result<EntityBridge> {
    let mut ty = None;
    let mut name_from = None;
    let mut id_from = None;
    meta.parse_nested_meta(|inner| {
        let key = inner.path.get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?.to_string();
        match key.as_str() {
            "type" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                ty = Some(s.value());
            }
            "name_from" => name_from = Some(inner.value()?.parse::<syn::Ident>()?),
            "id_from" => id_from = Some(inner.value()?.parse::<syn::Ident>()?),
            other => return Err(inner.error(format!("unknown entity_bridge key: {}", other))),
        }
        Ok(())
    })?;
    Ok(EntityBridge {
        entity_type: ty.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs type"))?,
        name_from: name_from.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs name_from"))?,
        id_from: id_from.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs id_from"))?,
    })
}
```

- [ ] **Step 4: Implement ai_event.rs**

Create `crates/ai-core-macros/src/ai_event.rs`:

```rust
use crate::attrs::{parse_ai_event_attr, AiEventAttr, EntityBridge, SalienceSpec};
use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::{Data, DeriveInput, Fields, Variant};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let enum_ident = &input.ident;
    let data_enum = match &input.data {
        Data::Enum(e) => e,
        _ => return Err(syn::Error::new_spanned(&input,
            "AiEvent can only be derived on enums")),
    };

    let mut arms = Vec::new();
    for variant in &data_enum.variants {
        arms.push(render_variant(enum_ident, variant)?);
    }

    Ok(quote! {
        impl ::ai_core::AiEventMeta for #enum_ident {
            fn to_signal(&self) -> ::ai_core::AiSignal {
                match self {
                    #(#arms)*
                }
            }

            fn event_kind(&self) -> &'static str {
                match self {
                    #(#enum_ident::variant_kind_arms!()),*
                }
            }
        }
    })
}

fn render_variant(enum_ident: &syn::Ident, variant: &Variant) -> syn::Result<TokenStream> {
    let var_ident = &variant.ident;
    let attr = parse_ai_event_attr(&variant.attrs)?;

    // Collect field names for destructuring.
    let field_idents: Vec<_> = match &variant.fields {
        Fields::Named(named) => named.named.iter()
            .map(|f| f.ident.clone().unwrap()).collect(),
        Fields::Unit => Vec::new(),
        Fields::Unnamed(_) => return Err(syn::Error::new_spanned(variant,
            "AiEvent requires named fields or unit variants")),
    };

    let pattern = if field_idents.is_empty() {
        quote! { #enum_ident::#var_ident }
    } else {
        quote! { #enum_ident::#var_ident { #(#field_idents),* } }
    };

    let importance_expr = match (&attr.importance, &attr.importance_fn) {
        (Some(lit), None) => quote! { #lit },
        (None, Some(path)) => quote! { #path(self) },
        (Some(_), Some(_)) => return Err(syn::Error::new_spanned(variant,
            "specify either importance or importance_fn, not both")),
        (None, None) => return Err(syn::Error::new_spanned(variant,
            "each variant needs #[ai(importance = ...)] or importance_fn")),
    };

    let salience_expr = render_salience(&attr.salience);
    let content_expr = render_content(&attr, &field_idents);
    let entity_expr = render_entity(&attr.entity_bridge);
    let kind_lit = var_ident.to_string();

    Ok(quote! {
        #pattern => ::ai_core::AiSignal {
            domain: String::new(),            // filled by router using AiFeature::DOMAIN
            event_kind: #kind_lit,
            importance: #importance_expr,
            salience: #salience_expr,
            content: #content_expr,
            entity: #entity_expr,
            timestamp: ::jiff::Timestamp::now(),
        },
    })
}

fn render_salience(spec: &SalienceSpec) -> TokenStream {
    match spec {
        SalienceSpec::Accumulate => quote! { ::ai_core::SalienceVerdict::Accumulate },
        SalienceSpec::Extract    => quote! { ::ai_core::SalienceVerdict::Extract },
        SalienceSpec::Discard    => quote! { ::ai_core::SalienceVerdict::Discard },
        SalienceSpec::ExtractIf(expr) => quote! {
            if #expr {
                ::ai_core::SalienceVerdict::Extract
            } else {
                ::ai_core::SalienceVerdict::Accumulate
            }
        },
    }
}

fn render_content(attr: &AiEventAttr, fields: &[syn::Ident]) -> TokenStream {
    match &attr.observation_template {
        Some(template) => {
            // Use format!-style rendering; every {field} must be a known field.
            let fmt_lit = syn::LitStr::new(template, proc_macro2::Span::call_site());
            let field_refs = fields.iter().map(|f| {
                let name = f.to_string();
                quote! { #name = #f }
            });
            quote! { format!(#fmt_lit, #(#field_refs),*) }
        }
        None => quote! { String::new() },
    }
}

fn render_entity(bridge: &Option<EntityBridge>) -> TokenStream {
    match bridge {
        None => quote! { None },
        Some(b) => {
            let ty = &b.entity_type;
            let name_from = &b.name_from;
            let id_from = &b.id_from;
            quote! {
                Some(::ai_core::EntityRef {
                    entity_type: #ty,
                    id: #id_from.to_string(),
                    name: #name_from.to_string(),
                })
            }
        }
    }
}
```

Note: the `event_kind` arms reference a macro `variant_kind_arms!` that doesn't exist — rewrite that section inline:

```rust
// In expand(), replace the event_kind match with:
let kind_arms: Vec<TokenStream> = data_enum.variants.iter().map(|v| {
    let id = &v.ident;
    let kind = id.to_string();
    let pattern = match &v.fields {
        Fields::Named(_) => quote! { #enum_ident::#id { .. } },
        Fields::Unit     => quote! { #enum_ident::#id },
        Fields::Unnamed(_) => quote! { #enum_ident::#id(..) },
    };
    quote! { #pattern => #kind }
}).collect();

// ... then in the final quote!:
fn event_kind(&self) -> &'static str {
    match self {
        #(#kind_arms,)*
    }
}
```

- [ ] **Step 5: Wire into lib.rs**

Replace `crates/ai-core-macros/src/lib.rs`:

```rust
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attrs;
mod ai_event;

#[proc_macro_derive(AiEvent, attributes(ai))]
pub fn derive_ai_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_event::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(AiEntity, attributes(ai))]
pub fn derive_ai_entity(_input: TokenStream) -> TokenStream { TokenStream::new() }

#[proc_macro_derive(AiFeature, attributes(ai))]
pub fn derive_ai_feature(_input: TokenStream) -> TokenStream { TokenStream::new() }
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p ai-core-macros`
Expected: PASS — `event_basic.rs` compiles and asserts succeed.

- [ ] **Step 7: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): derive(AiEvent) with importance/salience/entity_bridge/template"
```

---

## Task 5: Implement `#[derive(AiEntity)]` with `embed_on`

**Files:**
- Create: `crates/ai-core-macros/src/ai_entity.rs`
- Modify: `crates/ai-core-macros/src/lib.rs`
- Create: `crates/ai-core-macros/tests/expand/entity_basic.rs`

- [ ] **Step 1: Write failing test**

Create `crates/ai-core-macros/tests/expand/entity_basic.rs`:

```rust
use ai_core::AiEntity;
use ai_core_macros::AiEntity;

#[derive(AiEntity)]
#[ai(entity_type = "task", embed_on = ["title", "description"])]
struct Task {
    title: String,
    description: Option<String>,
    internal: i64,
}

fn main() {
    let t = Task {
        title: "Ship".into(),
        description: Some("it".into()),
        internal: 99,
    };
    assert_eq!(Task::entity_type(), "task");
    assert_eq!(t.embed_text(), "Ship\nit");

    let t2 = Task { title: "A".into(), description: None, internal: 0 };
    assert_eq!(t2.embed_text(), "A");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p ai-core-macros`
Expected: FAIL — `AiEntity` derive is still a no-op.

- [ ] **Step 3: Implement ai_entity.rs**

Create `crates/ai-core-macros/src/ai_entity.rs`:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    // Parse container attribute: #[ai(entity_type = "...", embed_on = [...])]
    let ai_attr = input.attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new_spanned(&input,
            "AiEntity requires #[ai(entity_type = \"...\", embed_on = [...])] on the struct"))?;

    let mut entity_type: Option<String> = None;
    let mut embed_fields: Vec<Ident> = Vec::new();

    ai_attr.parse_nested_meta(|meta| {
        let name = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match name.as_str() {
            "entity_type" => {
                let s: LitStr = meta.value()?.parse()?;
                entity_type = Some(s.value());
            }
            "embed_on" => {
                let content;
                syn::bracketed!(content in meta.input);
                let list: syn::punctuated::Punctuated<LitStr, syn::Token![,]> =
                    content.parse_terminated(LitStr::parse, syn::Token![,])?;
                for lit in list {
                    embed_fields.push(syn::Ident::new(&lit.value(), lit.span()));
                }
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    let entity_type = entity_type.ok_or_else(|| syn::Error::new_spanned(&input,
        "AiEntity needs #[ai(entity_type = \"...\")]"))?;

    // Verify fields exist and collect accessor code.
    let data_struct = match &input.data {
        Data::Struct(s) => s,
        _ => return Err(syn::Error::new_spanned(&input,
            "AiEntity can only be derived on structs")),
    };
    let field_map: std::collections::HashMap<String, &syn::Field> = match &data_struct.fields {
        Fields::Named(n) => n.named.iter()
            .filter_map(|f| f.ident.as_ref().map(|i| (i.to_string(), f)))
            .collect(),
        _ => return Err(syn::Error::new_spanned(&input,
            "AiEntity requires named fields")),
    };

    let accessors = embed_fields.iter().map(|id| {
        let name = id.to_string();
        let field = field_map.get(&name)
            .ok_or_else(|| syn::Error::new(id.span(),
                format!("embed_on references unknown field: {}", name)))?;
        let is_option = is_option_type(&field.ty);
        Ok(if is_option {
            quote! { self.#id.as_deref().unwrap_or("") }
        } else {
            quote! { self.#id.as_str() }
        })
    }).collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        impl ::ai_core::AiEntity for #struct_ident {
            fn entity_type() -> &'static str { #entity_type }

            fn embed_text(&self) -> String {
                let parts: Vec<&str> = vec![ #(#accessors),* ]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                parts.join("\n")
            }
        }
    })
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        p.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false)
    } else {
        false
    }
}
```

- [ ] **Step 4: Wire into lib.rs**

In `crates/ai-core-macros/src/lib.rs`, replace the `AiEntity` derive body:

```rust
mod ai_entity;

#[proc_macro_derive(AiEntity, attributes(ai))]
pub fn derive_ai_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_entity::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p ai-core-macros`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core-macros
git commit -m "feat(ai-core-macros): derive(AiEntity) with embed_on field list"
```

---

## Task 6: Implement `#[derive(AiFeature)]` with `recall_domain` + `RecallDomain` Enum

**Files:**
- Create: `crates/ai-core-macros/src/ai_feature.rs`
- Modify: `crates/ai-core-macros/src/lib.rs`
- Create: `crates/ai-core/src/recall_domain.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Create: `crates/ai-core-macros/tests/expand/feature_basic.rs`

**Approach:** v1 uses a **manually enumerated** `RecallDomain` enum (not `inventory`). Per spec §4.5 + §11, we start with the mechanical approach because it is robust and debuggable. The enum has one variant per feature and lives in `ai-core`. Each feature's `#[derive(AiFeature)]` emits an associated-const reference to an existing variant rather than contributing a new one. If future features need dynamic registration, we revisit.

- [ ] **Step 1: Write failing test**

Create `crates/ai-core-macros/tests/expand/feature_basic.rs`:

```rust
use ai_core::{AiFeature, RecallDomain};
use ai_core_macros::AiFeature;

// TaskEvent enum supplied by the test (would normally come from feature-tasks)
use ai_core::AiEventMeta;

pub enum TaskEvent { Created }
impl AiEventMeta for TaskEvent {
    fn to_signal(&self) -> ai_core::AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "Created" }
}
impl From<TaskEvent> for bus::DomainEvent {
    fn from(_: TaskEvent) -> Self { unimplemented!() }
}

#[derive(AiFeature)]
#[ai(recall_domain = "Tasks", skill = "task-management")]
pub struct TasksFeature;
impl TasksFeature {
    type Event = TaskEvent;
}

// Re-declare `type Event` via the trait; derive enforces.
impl TasksFeature {
    fn _assoc() -> &'static str { <TasksFeature as AiFeature>::SKILL }
}

fn main() {
    assert_eq!(<TasksFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(<TasksFeature as AiFeature>::SKILL, "task-management");
}
```

(If associated-type declaration syntax is tricky inside tests, replace the `TasksFeature` struct with a marker + a free impl.)

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p ai-core-macros`
Expected: FAIL — `RecallDomain` and derive not yet defined.

- [ ] **Step 3: Define RecallDomain enum**

Create `crates/ai-core/src/recall_domain.rs`:

```rust
/// Workspace-global enumeration of feature domains.
///
/// One variant per `#[derive(AiFeature)] #[ai(recall_domain = "...")]` declaration.
/// When adding a new feature, add its variant here manually; the derive references
/// the variant by name and produces a compile error if missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecallDomain {
    General,
    Tasks,
    Finance,
    // New features add their variant here as they migrate.
}

impl RecallDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecallDomain::General => "general",
            RecallDomain::Tasks   => "tasks",
            RecallDomain::Finance => "finance",
        }
    }
}

impl std::fmt::Display for RecallDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
```

Update `crates/ai-core/src/lib.rs`:

```rust
pub mod recall;
pub mod recall_domain;
pub mod signal;
pub mod traits;

pub use recall::{RecallItem, RecallQuery};
pub use recall_domain::RecallDomain;
pub use signal::{AiSignal, EntityRef, SalienceVerdict};
pub use traits::{AiEntity, AiEventMeta, AiFeature, RecallProvider, SignalConsumer};
```

Then update `crates/ai-core/src/traits.rs`: change `AiFeature::DOMAIN` to `const DOMAIN: RecallDomain`.

- [ ] **Step 4: Implement ai_feature.rs**

Create `crates/ai-core-macros/src/ai_feature.rs`:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident, LitStr};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    let ai_attr = input.attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new_spanned(&input,
            "AiFeature requires #[ai(recall_domain = \"...\", skill = \"...\", event = ...)]"))?;

    let mut recall_domain: Option<String> = None;
    let mut skill: Option<String> = None;
    let mut event_ty: Option<syn::Path> = None;

    ai_attr.parse_nested_meta(|meta| {
        let name = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match name.as_str() {
            "recall_domain" => {
                let s: LitStr = meta.value()?.parse()?;
                recall_domain = Some(s.value());
            }
            "skill" => {
                let s: LitStr = meta.value()?.parse()?;
                skill = Some(s.value());
            }
            "event" => {
                let s: LitStr = meta.value()?.parse()?;
                event_ty = Some(syn::parse_str(&s.value())?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    let domain_variant = Ident::new(
        &recall_domain.ok_or_else(|| syn::Error::new_spanned(&input,
            "AiFeature needs recall_domain"))?,
        proc_macro2::Span::call_site(),
    );
    let skill = skill.ok_or_else(|| syn::Error::new_spanned(&input, "AiFeature needs skill"))?;
    let event_path = event_ty.ok_or_else(|| syn::Error::new_spanned(&input,
        "AiFeature needs event = \"path::to::EventEnum\""))?;

    Ok(quote! {
        impl ::ai_core::AiFeature for #struct_ident {
            const DOMAIN: ::ai_core::RecallDomain = ::ai_core::RecallDomain::#domain_variant;
            const SKILL: &'static str = #skill;
            type Event = #event_path;
        }
    })
}
```

- [ ] **Step 5: Wire into lib.rs**

In `crates/ai-core-macros/src/lib.rs`:

```rust
mod ai_feature;

#[proc_macro_derive(AiFeature, attributes(ai))]
pub fn derive_ai_feature(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_feature::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
```

- [ ] **Step 6: Update the test**

Rewrite `feature_basic.rs` to use the `event = "..."` attribute:

```rust
use ai_core::{AiEventMeta, AiFeature, AiSignal, RecallDomain};
use ai_core_macros::AiFeature;

pub enum TaskEvent { Created }
impl AiEventMeta for TaskEvent {
    fn to_signal(&self) -> AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "Created" }
}
impl From<TaskEvent> for bus::DomainEvent {
    fn from(_: TaskEvent) -> Self { unimplemented!() }
}

#[derive(AiFeature)]
#[ai(recall_domain = "Tasks", skill = "task-management", event = "TaskEvent")]
pub struct TasksFeature;

fn main() {
    assert_eq!(<TasksFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(<TasksFeature as AiFeature>::SKILL, "task-management");
}
```

- [ ] **Step 7: Run**

Run: `cargo nextest run -p ai-core-macros && cargo nextest run -p ai-core`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/ai-core crates/ai-core-macros
git commit -m "feat(ai-core): add RecallDomain enum + derive(AiFeature)"
```

---

## Task 7: Implement `SignalRouter` Runtime

**Files:**
- Create: `crates/ai-core/src/router.rs`
- Modify: `crates/ai-core/src/lib.rs`
- Modify: `crates/ai-core/src/signal.rs` (add `domain_from_feature` helper + raw_event field)

- [ ] **Step 1: Update signal.rs to carry the raw event and a typed domain**

Modify `crates/ai-core/src/signal.rs`:

```rust
use bus::DomainEvent;
use crate::RecallDomain;
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
}

// ... (SalienceVerdict and EntityRef unchanged)
```

Update the macro-generated `to_signal` output in `ai_event.rs` to set `domain: RecallDomain::General` as a placeholder and `raw_event: None` — the router fills both.

(Fix also the test in Task 1: replace `domain: "tasks".into()` with `domain: RecallDomain::Tasks` and add `raw_event: None`.)

- [ ] **Step 2: Write failing router test**

Create `crates/ai-core/tests/router_test.rs`:

```rust
use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct Recorder { log: Arc<Mutex<Vec<String>>> }

#[async_trait]
impl SignalConsumer for Recorder {
    fn name(&self) -> &'static str { "rec" }
    async fn consume(&self, s: &AiSignal) -> common::Result<()> {
        self.log.lock().unwrap().push(format!("{}:{:?}", s.event_kind, s.domain));
        Ok(())
    }
}

#[tokio::test]
async fn router_broadcasts_signal_to_all_consumers() {
    let bus = Arc::new(DomainEventBus::new(64));
    let log = Arc::new(Mutex::new(Vec::new()));
    let consumer = Arc::new(Recorder { log: log.clone() }) as Arc<dyn SignalConsumer>;

    let router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        |_event| Some(AiSignal {
            domain: RecallDomain::Tasks,
            event_kind: "TaskCreated",
            importance: 0.7,
            salience: SalienceVerdict::Accumulate,
            content: "stub".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
        }),
    );

    bus.publish(DomainEvent::ChatTurnCompleted { /* minimal fixture */ ..Default::default() });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let entries = log.lock().unwrap().clone();
    assert_eq!(entries, vec!["TaskCreated:Tasks".to_string()]);

    router.shutdown();
}
```

Note: the test uses `Default::default()` for `DomainEvent::ChatTurnCompleted`. If the real enum doesn't implement `Default`, substitute a minimal valid constructor.

- [ ] **Step 3: Verify failure**

Run: `cargo nextest run -p ai-core`
Expected: FAIL — `SignalRouter` not defined.

- [ ] **Step 4: Implement router.rs**

Create `crates/ai-core/src/router.rs`:

```rust
use crate::{AiSignal, SignalConsumer};
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Translator: DomainEvent -> Option<AiSignal>. Returns None when the event
/// has no pipeline registration (e.g. transient infra events).
pub type Translator = Arc<dyn Fn(&DomainEvent) -> Option<AiSignal> + Send + Sync>;

pub struct SignalRouter {
    handle: JoinHandle<()>,
    cancel: CancellationToken,
}

impl SignalRouter {
    pub fn start<F>(
        bus: Arc<DomainEventBus>,
        consumers: Vec<Arc<dyn SignalConsumer>>,
        translator: F,
    ) -> Self
    where F: Fn(&DomainEvent) -> Option<AiSignal> + Send + Sync + 'static
    {
        let translator: Translator = Arc::new(translator);
        let cancel = CancellationToken::new();
        let cancel_child = cancel.clone();

        let handle = tokio::spawn(async move {
            let mut rx = bus.subscribe();
            loop {
                tokio::select! {
                    _ = cancel_child.cancelled() => return,
                    event = rx.recv() => {
                        let Ok(event) = event else { continue };
                        let Some(mut signal) = translator(&event) else { continue };
                        signal.raw_event = Some(event);
                        for c in &consumers {
                            if let Err(e) = c.consume(&signal).await {
                                tracing::warn!(consumer = c.name(), error = %e,
                                    "SignalConsumer failed");
                            }
                        }
                    }
                }
            }
        });

        Self { handle, cancel }
    }

    pub fn shutdown(self) {
        self.cancel.cancel();
    }
}
```

Add `tokio-util = { workspace = true, features = ["rt"] }` to `crates/ai-core/Cargo.toml`.

Update `lib.rs`:

```rust
pub mod router;
pub use router::{SignalRouter, Translator};
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p ai-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ai-core
git commit -m "feat(ai-core): SignalRouter broadcasts AiSignal to registered consumers"
```

---

## Task 8: Pre-Bug Fix — `domain_event_log.payload` Stores Full JSON

**Files:**
- Modify: `crates/app-core/src/init/cognitive.rs:210-213`
- Modify: `crates/app-core/src/handlers/timeline.rs:210-215`
- Modify: `crates/cognitive/src/repos/event_log.rs` (if writer is there instead)
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql` (in-place per pre-release policy)

Prereq: `DomainEvent` must derive `serde::Serialize` / `Deserialize`. Check before touching the writer; if missing, add in this task as part of the fix.

- [ ] **Step 1: Write failing test**

Add to `crates/app-core/tests/timeline_payload_test.rs`:

```rust
use app_core::handlers::timeline;
use bus::DomainEvent;
// (use the real constructor path for the test DB + event log repo)

#[tokio::test]
async fn task_created_payload_round_trips() {
    let (pool, repo) = test_setup::new_event_log_repo().await;
    let event = DomainEvent::TaskCreated {
        task_id: "t1".into(),
        title: "Ship v1".into(),
        // ... remaining required fields with sane defaults
    };
    repo.persist(&event, "tasks", "accumulate").await.unwrap();

    let rows = timeline::query_domain_events_range(&pool,
        jiff::Timestamp::now() - std::time::Duration::from_secs(60),
        jiff::Timestamp::now()
    ).await.unwrap();

    assert_eq!(rows.len(), 1);
    let normalized = timeline::normalize_domain_event(&rows[0]).unwrap();
    assert_eq!(normalized.task_id.as_deref(), Some("t1"));
    assert_eq!(normalized.title.as_deref(), Some("Ship v1"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p app-core task_created_payload`
Expected: FAIL — payload currently stores variant name, so extraction returns None.

- [ ] **Step 3: Fix the writer**

Open `crates/app-core/src/init/cognitive.rs` around line 210. The current code is:

```rust
let event_type = event.variant_name().to_string();
let payload = &event_type;
```

Replace with:

```rust
let event_type = event.variant_name().to_string();
let payload = serde_json::to_string(&event)
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize DomainEvent for event log failed");
        format!("{{\"_kind\":{:?}}}", event_type)
    });
```

(If the signature of the `persist`/`insert` call needs adjustment, follow the existing `EventLogRepo::insert` interface — it already accepts `&str` for payload.)

- [ ] **Step 4: Fix the reader**

Open `crates/app-core/src/handlers/timeline.rs:210-215`. The current code tries:

```rust
if let Ok(v) = serde_json::from_str::<serde_json::Value>(&row.payload) {
    if let Some(inner) = v.get(row.event_type.as_str()) { ... }
}
```

Replace with a tolerant reader that:
1. Tries to parse as a full `DomainEvent` JSON (new format).
2. Falls back to the legacy bare-string format for pre-migration rows (pre-release: acceptable because we have no real user data; this exists purely to keep dev DBs readable during the transition in the same session).

```rust
if let Ok(event) = serde_json::from_str::<bus::DomainEvent>(&row.payload) {
    return Some(Self::extract_from_event(&event));
}
None
```

Add a helper `extract_from_event` that matches on `DomainEvent` variants and returns a populated `NormalizedEvent`.

- [ ] **Step 5: Run test**

Run: `cargo nextest run -p app-core task_created_payload`
Expected: PASS.

- [ ] **Step 6: Run full cognitive + app-core tests**

Run: `cargo nextest run -p cognitive -p app-core`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/init/cognitive.rs \
        crates/app-core/src/handlers/timeline.rs \
        crates/app-core/tests/timeline_payload_test.rs
git commit -m "fix(event-log): store full DomainEvent JSON in payload column"
```

---

## Task 9: Pre-Bug Fix — `count_by_event_type_and_data` References Nonexistent `data` Column

**Files:**
- Modify: `crates/cognitive/src/repos/event_log.rs:205`

The method queries a `data` column that doesn't exist. The real column is `payload`. After Task 8, the payload is JSON, so JSON extraction is now valid.

- [ ] **Step 1: Find callers**

Run: `grep -rn "count_by_event_type_and_data" crates/`

Expected callers: `crates/agent/src/autotuner/metric_collector.rs:67` or :69 (per audit).

- [ ] **Step 2: Decide: fix or delete**

If Task 23 will replace these callers with typed pipeline metrics anyway, mark the method `#[doc(hidden)]` and leave it functional until Task 23 deletes it. If Task 23 won't cover every caller, fix the column now.

Decision for this plan: **fix now, since Task 23 replaces all callers, but callers currently in flight to metric_collector.rs may still query the DB at runtime before Task 23 ships.**

- [ ] **Step 3: Apply fix**

Open `crates/cognitive/src/repos/event_log.rs` around line 205. Replace `data` with `payload` in the SQL string. If the query was extracting JSON, use `json_extract(payload, '$.field')`.

Example before:

```rust
"SELECT COUNT(*) FROM domain_event_log WHERE event_type = ? AND data LIKE ?"
```

Example after:

```rust
"SELECT COUNT(*) FROM domain_event_log WHERE event_type = ?
 AND json_extract(payload, '$') LIKE ?"
```

- [ ] **Step 4: Write regression test**

Add to the repo's inline test module:

```rust
#[tokio::test]
async fn count_by_event_type_and_data_does_not_panic() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = EventLogRepo::new(pool.clone());
    let count = repo.count_by_event_type_and_data("TaskCreated", "%").await.unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive count_by_event_type`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/event_log.rs
git commit -m "fix(event-log): count_by_event_type_and_data uses payload column"
```

---

## Task 10: Pre-Bug Fix — Tasks Migration Version Drift

**Files:**
- Modify: `crates/feature-tasks/src/lib.rs`
- Modify: `crates/app-core/src/init/storage.rs:107-113`

Currently: `TasksFeature::migrations()` returns version `2`. Bootstrap in `storage.rs:107-113` manually builds a `FeatureMigration { version: 1, ... }` and inserts it. Result: `_feature_migrations` records version 1 while code thinks version 2 applies. Future migrations collide silently.

- [ ] **Step 1: Write failing test**

Add to `crates/feature-tasks/tests/feature_package_test.rs`:

```rust
#[tokio::test]
async fn tasks_migration_version_matches_trait_and_bootstrap() {
    use tools_core::FeaturePackage;
    let f = feature_tasks::TasksFeature::new(/* minimal args */);
    let migrations = f.migrations();
    assert_eq!(migrations.len(), 1);
    let m = &migrations[0];
    assert_eq!(m.version, 2, "trait must return current schema version");

    // Verify bootstrap path uses the trait (no manual FeatureMigration literals)
    let bootstrap_src = include_str!("../../../crates/app-core/src/init/storage.rs");
    assert!(
        !bootstrap_src.contains("FeatureMigration { feature_name: \"tasks\""),
        "storage.rs must not construct TasksFeature migrations manually"
    );
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-tasks tasks_migration_version`
Expected: FAIL — the string check finds the manual literal.

- [ ] **Step 3: Fix bootstrap**

Open `crates/app-core/src/init/storage.rs:107-113`. Replace the manually-constructed `FeatureMigration` with a call through the `FeaturePackage::migrations()` trait, consistent with how `FocusFeature` is handled. Example replacement:

```rust
// before:
// migrations.push(FeatureMigration {
//     feature_name: "tasks".into(),
//     version: 1,
//     description: "Create task tables".into(),
//     sql: feature_tasks::TasksFeature::migration_sql().into(),
// });

// after:
migrations.extend(<feature_tasks::TasksFeature as tools_core::FeaturePackage>::migrations(&tasks_feature));
```

(Instantiate `tasks_feature` if not already in scope.)

- [ ] **Step 4: Ensure schema version 2 is the single source of truth**

Read `crates/feature-tasks/migrations/001_create_tasks.sql`. Confirm the trait impl returns the right version, and only one migration entry is returned. If there are two historical migrations hand-built elsewhere, consolidate into a single `001_create_tasks.sql` at version 2 per the pre-release policy (CLAUDE.md: "update the `FeatureMigration` version and SQL in-place rather than adding incremental migration files").

- [ ] **Step 5: Run**

Run: `cargo nextest run -p feature-tasks -p app-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks crates/app-core/src/init/storage.rs
git commit -m "fix(feature-tasks): unify migration version on FeaturePackage trait path"
```

---

## Task 11: Clean `FeaturePackage` Trait — Delete Orphan Methods + Add Collision Detection

**Files:**
- Modify: `crates/tools-core/src/feature.rs`
- Modify: every `impl FeaturePackage` site across the workspace (remove `config_key` and `default_config` impls)
- Modify: `crates/storage/src/pool.rs` (add collision detection in `run_feature_migrations`)

- [ ] **Step 1: Write failing test for collision detection**

Add to `crates/storage/src/pool.rs` inline tests:

```rust
#[tokio::test]
#[should_panic(expected = "duplicate migration version for feature")]
async fn duplicate_version_panics() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let migs = vec![
        FeatureMigration { feature_name: "x".into(), version: 1,
            description: "a".into(), sql: "CREATE TABLE a(x INT);".into() },
        FeatureMigration { feature_name: "x".into(), version: 1,
            description: "b".into(), sql: "CREATE TABLE b(x INT);".into() },
    ];
    pool.run_feature_migrations(&migs).await.unwrap();
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p storage duplicate_version_panics`
Expected: FAIL — collision currently silent-skips.

- [ ] **Step 3: Add collision detection**

In `crates/storage/src/pool.rs::run_feature_migrations`, before the loop:

```rust
use std::collections::HashSet;
let mut seen: HashSet<(String, i64)> = HashSet::new();
for m in migrations {
    if !seen.insert((m.feature_name.clone(), m.version)) {
        panic!("duplicate migration version for feature {}: v{}",
               m.feature_name, m.version);
    }
}
```

- [ ] **Step 4: Delete orphan trait methods**

In `crates/tools-core/src/feature.rs`, remove `fn config_key(&self) -> &str;` and `fn default_config(&self) -> Value;` from the trait.

- [ ] **Step 5: Remove impls across the workspace**

Run: `grep -rn "fn config_key" crates/feature-* crates/plugin-runtime`

For each hit, delete the method. Same for `fn default_config`.

- [ ] **Step 6: Remove `migrations_static()` helpers**

Run: `grep -rn "migrations_static" crates/`

Delete these static methods and update every call site in `crates/app-core/src/init/storage.rs` to use the trait path, mirroring the fix in Task 10.

- [ ] **Step 7: Run full build**

Run: `cargo build --workspace && cargo nextest run -p storage -p tools-core -p app-core`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add -u
git commit -m "refactor(tools-core): clean FeaturePackage trait orphans + add migration collision detection"
```

---

## Task 12: Migrate Tasks — `TaskEvent` Enum + `AiFeature` Impl

**Files:**
- Create: `crates/feature-tasks/src/events.rs`
- Modify: `crates/feature-tasks/src/lib.rs`
- Modify: `crates/feature-tasks/Cargo.toml` (add `ai-core`, `ai-core-macros`)

- [ ] **Step 1: Write failing test**

Create `crates/feature-tasks/tests/ai_pipeline_test.rs`:

```rust
use ai_core::{AiEventMeta, AiFeature, RecallDomain, SalienceVerdict};
use feature_tasks::events::TaskEvent;
use feature_tasks::TasksFeature;

#[test]
fn task_event_created_signal() {
    let e = TaskEvent::Created {
        task_id: "t1".into(),
        title: "Ship v1".into(),
        area_id: "a1".into(),
        priority: Some(2),
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "Created");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert_eq!(sig.content, "Created task: Ship v1 (priority 2)");
    assert_eq!(sig.entity.as_ref().unwrap().entity_type, "task");
    assert_eq!(sig.entity.as_ref().unwrap().id, "t1");
}

#[test]
fn task_event_completed_high_deviation_extracts() {
    let e = TaskEvent::Completed {
        task_id: "t1".into(),
        title: "Ship v1".into(),
        deviation_pct: Some(80.0),
    };
    assert!(matches!(e.to_signal().salience, SalienceVerdict::Extract));
}

#[test]
fn tasks_feature_declaration() {
    assert_eq!(<TasksFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(<TasksFeature as AiFeature>::SKILL, "task-management");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-tasks task_event_created_signal`
Expected: FAIL — module `events` does not exist.

- [ ] **Step 3: Create events.rs**

Create `crates/feature-tasks/src/events.rs`:

```rust
use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
pub enum TaskEvent {
    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Created task: {title} (priority {priority:?})",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    )]
    Created {
        task_id: String,
        title: String,
        area_id: String,
        priority: Option<i16>,
    },

    #[ai(
        importance = 0.6,
        salience = "extract_if(deviation_pct.unwrap_or(0.0) > 50.0)",
        observation_template = "Completed {title} (deviation {deviation_pct:?}%)",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    )]
    Completed {
        task_id: String,
        title: String,
        deviation_pct: Option<f64>,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Focused on {title}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
    )]
    FocusChanged {
        task_id: String,
        title: String,
        focus_deadline: Option<jiff::Timestamp>,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Estimation recorded: est {estimated_minutes:?}m vs actual {actual_minutes:?}m",
    )]
    EstimationRecorded {
        task_id: String,
        estimated_minutes: Option<i32>,
        actual_minutes: Option<i32>,
    },
}

impl From<TaskEvent> for DomainEvent {
    fn from(e: TaskEvent) -> Self {
        match e {
            TaskEvent::Created { task_id, title, area_id, priority } =>
                DomainEvent::TaskCreated { task_id, title, area_id, priority, ..Default::default() },
            TaskEvent::Completed { task_id, title, deviation_pct } =>
                DomainEvent::TaskCompleted { task_id, title, deviation_pct, ..Default::default() },
            TaskEvent::FocusChanged { task_id, title, focus_deadline } =>
                DomainEvent::TaskFocusChanged { task_id, title, focus_deadline, ..Default::default() },
            TaskEvent::EstimationRecorded { task_id, estimated_minutes, actual_minutes } =>
                DomainEvent::EstimationRecorded { task_id, estimated_minutes, actual_minutes },
        }
    }
}
```

(If `DomainEvent` variants don't implement `Default`, enumerate the remaining fields with sensible values — the fix is mechanical, driven by the compile error.)

- [ ] **Step 4: Update lib.rs**

In `crates/feature-tasks/src/lib.rs`, add at the top:

```rust
pub mod events;
```

Then add the `AiFeature` derive on `TasksFeature`:

```rust
use ai_core_macros::AiFeature;

#[derive(AiFeature)]
#[ai(recall_domain = "Tasks", skill = "task-management",
     event = "crate::events::TaskEvent")]
pub struct TasksFeature { /* existing fields */ }
```

- [ ] **Step 5: Add dependencies**

In `crates/feature-tasks/Cargo.toml`:

```toml
[dependencies]
ai-core.workspace = true
ai-core-macros.workspace = true
```

- [ ] **Step 6: Run**

Run: `cargo nextest run -p feature-tasks ai_pipeline_test`
Expected: PASS.

- [ ] **Step 7: Replace internal event emission with TaskEvent**

`grep -rn "DomainEvent::TaskCreated\|DomainEvent::TaskCompleted\|DomainEvent::TaskFocusChanged\|DomainEvent::EstimationRecorded" crates/feature-tasks/src/`

For each call site, construct a `TaskEvent::*` and publish via `bus.publish(ev.into())`.

- [ ] **Step 8: Run full crate tests**

Run: `cargo nextest run -p feature-tasks`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/feature-tasks
git commit -m "feat(feature-tasks): migrate to TaskEvent + AiFeature pipeline"
```

---

## Task 13: Migrate Tasks — Annotate `Task` with `AiEntity`; Restore `tools()` Path

**Files:**
- Modify: `crates/feature-tasks/src/types/entity.rs`
- Modify: `crates/feature-tasks/src/lib.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/feature-tasks/tests/ai_pipeline_test.rs`:

```rust
use ai_core::AiEntity;
use feature_tasks::types::Task;

#[test]
fn task_embed_text_uses_title_and_description() {
    let t = Task {
        id: "x".into(),
        title: "Ship v1".into(),
        description: Some("Finish the thing".into()),
        ..Task::default_for_test()
    };
    assert_eq!(t.embed_text(), "Ship v1\nFinish the thing");
    assert_eq!(Task::entity_type(), "task");
}
```

(Add `Task::default_for_test()` as a `#[cfg(test)]` helper if a `Default` impl is not appropriate.)

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-tasks task_embed_text`
Expected: FAIL.

- [ ] **Step 3: Annotate Task**

In `crates/feature-tasks/src/types/entity.rs`:

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity)]
#[ai(entity_type = "task", embed_on = ["title", "description"])]
pub struct Task {
    // ... existing fields unchanged
}
```

- [ ] **Step 4: Restore tools() via FeaturePackage**

In `crates/feature-tasks/src/lib.rs`, change `tools()` from returning `vec![]` to returning the `TaskTool`:

```rust
fn tools(&self) -> Vec<DynTool> {
    vec![self.task_tool.clone()]
}
```

(If `TaskTool` construction requires injected capabilities currently passed in by the agent builder, add builder methods to `TasksFeature` that accept them before `build()`. The field set is: area_repo, embedding_handler, progress_handler, domain_bus, alarm_writer — per the earlier audit of `agent_loop/builder.rs:1257-1318`.)

- [ ] **Step 5: Run**

Run: `cargo nextest run -p feature-tasks`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks
git commit -m "feat(feature-tasks): annotate Task with AiEntity; return TaskTool via FeaturePackage"
```

---

## Task 14: Delete Tasks Dead Code

**Files:**
- Delete: `crates/feature-tasks/src/cognitive_bridge.rs`
- Modify: `crates/feature-tasks/src/types/entity.rs` (remove `agentic`/`hybrid` from `TaskType::from_str`)
- Modify: `crates/feature-tasks/src/tool/mod.rs` (remove `agentic`/`hybrid` schema entries + `acceptance_criteria` + `agent_config` params)
- Modify: `crates/feature-tasks/src/tool/actions/query.rs:202-208` (delete historical `agentic` count branch)
- Modify: `crates/feature-tasks/src/lib.rs` (remove `pub mod cognitive_bridge`)

- [ ] **Step 1: Delete cognitive_bridge.rs**

```bash
git rm crates/feature-tasks/src/cognitive_bridge.rs
```

Remove `pub mod cognitive_bridge;` from `lib.rs`.

- [ ] **Step 2: Delete TaskType::from_str agentic/hybrid arms**

Open `crates/feature-tasks/src/types/entity.rs:252`. Remove the match arms mapping `"agentic" | "hybrid"` to `Manual`. The fallthrough remains.

- [ ] **Step 3: Delete TaskTool schema entries**

Open `crates/feature-tasks/src/tool/mod.rs:259-276`. Remove:
- `"agentic"` and `"hybrid"` from the `task_type` enum values.
- The entire `acceptance_criteria` parameter declaration.
- The entire `agent_config` parameter declaration.

- [ ] **Step 4: Delete historical summary branch**

Open `crates/feature-tasks/src/tool/actions/query.rs:202-208`. Delete the block that queries `WHERE task_type = 'agentic'` and appends "Agentic tasks: N".

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p feature-tasks`
Expected: PASS.

- [ ] **Step 6: Verify clippy**

Run: `cargo clippy -p feature-tasks --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "chore(feature-tasks): delete agentic/hybrid dead schema + cognitive_bridge orphan"
```

---

## Task 15: Remove Tasks Hand-Wiring in Agent Builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:1257-1318`

- [ ] **Step 1: Identify the block**

Open `crates/agent/src/agent_loop/builder.rs`. Locate the block starting around line 1257 where `feature_tasks::TaskTool::new(...)` is constructed and chained with `.with_area_repo`, `.with_embedding_handler`, etc., then `register`-ed on the tool registry.

- [ ] **Step 2: Move capability wiring into `TasksFeature::build()`**

If not done in Task 13, add builder methods on `TasksFeature` that accept the injected capabilities (area_repo, embedding_handler, progress_handler, domain_bus, alarm_writer) and store them for later use by `tools()`.

- [ ] **Step 3: Delete the block**

Replace the entire section (lines 1257-1318, approximately) with nothing — the tool is now registered via the normal `FeaturePackage::tools()` iteration that happens earlier in the builder (or add that iteration if it doesn't yet exist for native features). Verify by reading the surrounding `feature_packages` iteration code.

If there is no native-feature `tools()` iteration in the builder, add one now:

```rust
for feature in &native_features {
    for tool in feature.tools() {
        tool_registry.register_dyn(tool).await?;
    }
}
```

- [ ] **Step 4: Run full workspace build**

Run: `cargo build --workspace`
Expected: no compile errors.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -p feature-tasks`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): remove TaskTool hand-wiring; use FeaturePackage::tools()"
```

---

## Task 16: Migrate Finance — `FinanceEvent` Enum + Missing Lifecycle Events

**Files:**
- Create: `crates/feature-finance/src/events.rs`
- Modify: `crates/feature-finance/src/lib.rs`
- Modify: `crates/feature-finance/src/tool/transactions/mod.rs` (switch to FinanceEvent emission)
- Modify: `crates/feature-finance/src/tool/accounts.rs` (emit AccountCreated)
- Modify: `crates/feature-finance/src/tool/budgets.rs` (emit BudgetCreated)
- Modify: `crates/feature-finance/src/tool/goals.rs` (emit GoalCreated, GoalAchieved)
- Modify: `crates/feature-finance/Cargo.toml` (add ai-core dependencies)
- Modify: `crates/bus/src/domain_events.rs` (add 4 new variants)

- [ ] **Step 1: Write failing test**

Create `crates/feature-finance/tests/ai_pipeline_test.rs`:

```rust
use ai_core::{AiEventMeta, AiFeature, RecallDomain, SalienceVerdict};
use feature_finance::events::FinanceEvent;
use feature_finance::FinanceFeature;

#[test]
fn transaction_recorded_signal() {
    let e = FinanceEvent::TransactionRecorded {
        tx_id: "t1".into(),
        category: "groceries".into(),
        amount: 4500,
        currency: "USD".into(),
        is_over_budget: false,
    };
    let sig = e.to_signal();
    assert_eq!(sig.event_kind, "TransactionRecorded");
    assert!(sig.content.contains("groceries"));
}

#[test]
fn budget_alert_high_importance() {
    let e = FinanceEvent::BudgetAlert {
        category: "dining".into(),
        spent: 80000,
        limit: 75000,
    };
    let sig = e.to_signal();
    assert!(sig.importance >= 0.8);
}

#[test]
fn finance_feature_declaration() {
    assert_eq!(<FinanceFeature as AiFeature>::DOMAIN, RecallDomain::Finance);
    assert_eq!(<FinanceFeature as AiFeature>::SKILL, "finance-management");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-finance ai_pipeline`
Expected: FAIL.

- [ ] **Step 3: Add 4 new DomainEvent variants**

In `crates/bus/src/domain_events.rs`, add:

```rust
AccountCreated { account_id: String, name: String, currency: String },
BudgetCreated { budget_id: String, name: String, amount: i64, currency: String },
GoalCreated { goal_id: String, name: String, target_amount: i64 },
GoalAchieved { goal_id: String, name: String },
```

Update `variant_name()` to include them.

- [ ] **Step 4: Create events.rs**

Create `crates/feature-finance/src/events.rs`:

```rust
use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
pub enum FinanceEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Transaction: {category} {amount} {currency}",
    )]
    TransactionRecorded {
        tx_id: String,
        category: String,
        amount: i64,
        currency: String,
        is_over_budget: bool,
    },

    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "Budget alert: {category} spent {spent} of {limit}",
        entity_bridge(type = "finance_category", name_from = "category", id_from = "category"),
    )]
    BudgetAlert {
        category: String,
        spent: i64,
        limit: i64,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Account opened: {name} ({currency})",
        entity_bridge(type = "finance_account", name_from = "name", id_from = "account_id"),
    )]
    AccountCreated { account_id: String, name: String, currency: String },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Budget created: {name} ({amount} {currency})",
    )]
    BudgetCreated { budget_id: String, name: String, amount: i64, currency: String },

    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Goal created: {name} target {target_amount}",
        entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id"),
    )]
    GoalCreated { goal_id: String, name: String, target_amount: i64 },

    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "Goal achieved: {name}",
        entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id"),
    )]
    GoalAchieved { goal_id: String, name: String },
}

impl From<FinanceEvent> for DomainEvent {
    fn from(e: FinanceEvent) -> Self {
        match e {
            FinanceEvent::TransactionRecorded { tx_id: _, category, amount, currency: _, is_over_budget } =>
                DomainEvent::TransactionRecorded { category, amount, is_over_budget },
            FinanceEvent::BudgetAlert { category, spent, limit } =>
                DomainEvent::BudgetAlert { category, spent, limit },
            FinanceEvent::AccountCreated { account_id, name, currency } =>
                DomainEvent::AccountCreated { account_id, name, currency },
            FinanceEvent::BudgetCreated { budget_id, name, amount, currency } =>
                DomainEvent::BudgetCreated { budget_id, name, amount, currency },
            FinanceEvent::GoalCreated { goal_id, name, target_amount } =>
                DomainEvent::GoalCreated { goal_id, name, target_amount },
            FinanceEvent::GoalAchieved { goal_id, name } =>
                DomainEvent::GoalAchieved { goal_id, name },
        }
    }
}
```

- [ ] **Step 5: Update lib.rs**

In `crates/feature-finance/src/lib.rs`:

```rust
pub mod events;

use ai_core_macros::AiFeature;

#[derive(AiFeature)]
#[ai(recall_domain = "Finance", skill = "finance-management",
     event = "crate::events::FinanceEvent")]
pub struct FinanceFeature { /* existing fields */ }
```

- [ ] **Step 6: Emit new events at creation sites**

- `accounts.rs::account_add`: after insert, publish `FinanceEvent::AccountCreated`.
- `budgets.rs::budget_create`: publish `FinanceEvent::BudgetCreated`.
- `goals.rs::goal_create`: publish `FinanceEvent::GoalCreated`.
- `goals.rs::goal_update` (when `current_amount >= target_amount` for the first time): publish `FinanceEvent::GoalAchieved`.
- `transactions/mod.rs`: replace the existing `DomainEvent::TransactionRecorded` / `BudgetAlert` emission with `FinanceEvent::*` construction + `.into()`.

- [ ] **Step 7: Run**

Run: `cargo nextest run -p feature-finance -p bus`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-finance crates/bus
git commit -m "feat(feature-finance): migrate to FinanceEvent + add missing lifecycle events"
```

---

## Task 17: Migrate Finance — Annotate `FinanceTransaction`; Wire Embedding

**Files:**
- Modify: `crates/feature-finance/src/types/domain.rs`
- Modify: `crates/feature-finance/src/lib.rs`
- Modify: `crates/feature-finance/src/tool/transactions/mod.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/feature-finance/tests/ai_pipeline_test.rs`:

```rust
use ai_core::AiEntity;
use feature_finance::types::FinanceTransaction;

#[test]
fn transaction_embed_text_concatenates_key_fields() {
    let tx = FinanceTransaction {
        counterparty: Some("Whole Foods".into()),
        category: Some("groceries".into()),
        subcategory: Some("produce".into()),
        ..FinanceTransaction::default_for_test()
    };
    assert_eq!(tx.embed_text(), "Whole Foods\ngroceries\nproduce");
    assert_eq!(FinanceTransaction::entity_type(), "finance_transaction");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p feature-finance transaction_embed`
Expected: FAIL.

- [ ] **Step 3: Annotate FinanceTransaction**

In `crates/feature-finance/src/types/domain.rs`:

```rust
use ai_core_macros::AiEntity;

#[derive(Debug, Clone, AiEntity)]
#[ai(entity_type = "finance_transaction",
     embed_on = ["counterparty", "category", "subcategory"])]
pub struct FinanceTransaction {
    // ... existing fields
}
```

- [ ] **Step 4: Wire embedding call**

In `crates/feature-finance/src/tool/transactions/mod.rs::tx_add`, after inserting the transaction and before publishing `FinanceEvent::TransactionRecorded`, call an embedding handler with `tx.embed_text()`. Pattern: mirror `feature-tasks`'s embedding handler wiring.

Add a new field `embedding_handler: Option<Arc<dyn EmbeddingHandler>>` to `FinanceFeature`, with a builder method `.with_embedding_handler(...)`. Call it from `app-core` init where Tasks already has the same wiring.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p feature-finance`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-finance
git commit -m "feat(feature-finance): annotate FinanceTransaction + wire embedding"
```

---

## Task 18: Cognitive `SignalConsumer` — Ingestion + Salience via Attributes

**Files:**
- Create: `crates/cognitive/src/consumers/ingestion.rs`
- Modify: `crates/cognitive/src/lib.rs` (`pub mod consumers`)
- Modify: `crates/cognitive/Cargo.toml` (add `ai-core`)

- [ ] **Step 1: Write failing test**

Create `crates/cognitive/tests/ingestion_consumer_test.rs`:

```rust
use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
use cognitive::consumers::IngestionConsumer;
use jiff::Timestamp;
// + test setup for repos

#[tokio::test]
async fn extract_verdict_writes_observation() {
    let (pool, observation_repo, entity_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(observation_repo.clone(), entity_repo.clone(), /* ... */);

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Completed",
        importance: 0.8,
        salience: SalienceVerdict::Extract,
        content: "Completed: Ship v1 (deviation 80%)".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let obs = observation_repo.latest(10).await.unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].importance, 0.8);
    assert_eq!(obs[0].domain, "tasks");
}

#[tokio::test]
async fn entity_bridge_upserts_entity() {
    let (_, _, entity_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(...);

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Created",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "Created: A".into(),
        entity: Some(ai_core::EntityRef {
            entity_type: "task",
            id: "abc".into(),
            name: "A".into(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();
    assert!(entity_repo.exists("task", "A").await.unwrap());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p cognitive ingestion_consumer`
Expected: FAIL.

- [ ] **Step 3: Implement consumer**

Create `crates/cognitive/src/consumers/ingestion.rs`:

```rust
use crate::{
    repos::{AccumulatedObservationRepo, EntityRepo, EpisodicMemoryRepo},
    services::extraction::ExtractionHandler,
    types::Observation,
};
use ai_core::{AiSignal, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;

pub struct IngestionConsumer {
    observation_repo: AccumulatedObservationRepo,
    entity_repo: EntityRepo,
    episodic_repo: EpisodicMemoryRepo,
    extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    episodic_importance_threshold: f64,
}

impl IngestionConsumer {
    pub fn new(
        observation_repo: AccumulatedObservationRepo,
        entity_repo: EntityRepo,
        episodic_repo: EpisodicMemoryRepo,
        extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    ) -> Self {
        Self {
            observation_repo, entity_repo, episodic_repo,
            extraction_handler,
            episodic_importance_threshold: 0.7,
        }
    }

    fn signal_to_observation(signal: &AiSignal) -> Observation {
        Observation {
            domain: signal.domain.as_str().to_string(),
            content: signal.content.clone(),
            importance: signal.importance,
            source_event: signal.event_kind.to_string(),
            timestamp: signal.timestamp,
        }
    }
}

#[async_trait]
impl SignalConsumer for IngestionConsumer {
    fn name(&self) -> &'static str { "cognitive_ingestion" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        // 1. Entity bridge (always, regardless of salience).
        if let Some(entity) = &signal.entity {
            self.entity_repo.upsert_domain(
                &entity.name, entity.entity_type, &entity.id
            ).await?;
        }

        // 2. Salience routing.
        let observation = Self::signal_to_observation(signal);
        match signal.salience {
            SalienceVerdict::Discard => return Ok(()),
            SalienceVerdict::Accumulate => {
                self.observation_repo.insert(&observation).await?;
            }
            SalienceVerdict::Extract => {
                self.observation_repo.insert(&observation).await?;
                if let Some(handler) = &self.extraction_handler {
                    // Fire-and-forget extraction — keeps ingestion hot path fast.
                    let handler = handler.clone();
                    let obs = observation.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handler.extract_facts(&obs).await {
                            tracing::warn!(error = %e, "extraction failed");
                        }
                    });
                }
            }
        }

        // 3. Episodic branch for high-importance signals.
        if observation.importance >= self.episodic_importance_threshold {
            self.episodic_repo.insert_from_observation(&observation).await?;
        }
        Ok(())
    }
}
```

(Some method names above are approximations — follow the actual signatures of `AccumulatedObservationRepo::insert`, `EntityRepo::upsert_domain`, `EpisodicMemoryRepo::insert_from_observation`. Add wrappers if the existing API differs.)

- [ ] **Step 4: Create consumers module**

Create `crates/cognitive/src/consumers/mod.rs`:

```rust
pub mod ingestion;
pub use ingestion::IngestionConsumer;
```

Update `crates/cognitive/src/lib.rs` to `pub mod consumers;`.

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive ingestion_consumer`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive
git commit -m "feat(cognitive): IngestionConsumer replaces event_to_observation match table"
```

---

## Task 19: Delete `background.rs` Match Arms and `salience.rs`

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`
- Delete: `crates/cognitive/src/services/salience.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Identify deletable code**

Read `crates/cognitive/src/services/background.rs`. Identify:
- `event_to_observation` function (~lines 899-1213).
- Calls to `evaluate_salience(&event)`.
- The batch classification loop that routes Extract/Accumulate/Discard.

- [ ] **Step 2: Replace with SignalConsumer registration**

The new flow: `SignalRouter` subscribes to `DomainEventBus`, calls the global translator to produce `AiSignal`s, and forwards to `IngestionConsumer`. The old `BackgroundConsolidationService` is no longer the entry point for event classification.

Keep the parts of `BackgroundConsolidationService` that remain essential:
- Accumulation-to-extraction promotion (time + count thresholds).
- Periodic compaction.
- `pipeline/` collectors (their subscription source changes in v1.5; for v1 they stay on `DomainEventBus`).

Delete:
- `event_to_observation` entirely.
- The `evaluate_salience` call chain.
- `salience.rs` file entirely.

- [ ] **Step 3: Update services/mod.rs**

Remove `pub mod salience;`.

- [ ] **Step 4: Find remaining callers**

Run: `grep -rn "evaluate_salience\|event_to_observation" crates/`

Any remaining reference must be deleted or rerouted.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "refactor(cognitive): delete event_to_observation + salience.rs; SignalRouter takes over"
```

---

## Task 20: Delete 25 Dead `DomainEvent` Variants + Update `variant_name()`

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

Variants to delete (from spec §5.3):
`TaskBlocked`, `TaskUnblocked`, `TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated`, `TaskDueDateChanged`, `TaskHierarchyChanged`, `TreeNodesRebuilt`, `RecurringTemplateAdvanced`, `GoalProgress`, `RuleEvolved`, `VoiceJournalProcessed`, `VoiceCapture`, `NarrativeGenerated`, `PredictiveAlert`, `SquadDebateCompleted`, `SquadInteractionPattern`, `MemoryPromoted`, `MessageDeferred`, `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`, `MirrorTrialKilled`, `MirrorSnippetCreated`, `TrialActivated`.

- [ ] **Step 1: Delete variants**

Open `crates/bus/src/domain_events.rs`. Delete each of the 25 variants from the enum definition. Also delete each's arm from the `variant_name()` match.

- [ ] **Step 2: Verify compile errors**

Run: `cargo build --workspace 2>&1 | grep error: | head -30`

Expected: compile errors in the downstream crates that reference these variants (simulator, activity-log normalizer, possibly agent).

- [ ] **Step 3: Track compile errors for Tasks 21–23**

Write the file list to the commit description — Tasks 21, 22, 23 will fix these in order.

- [ ] **Step 4: Commit (broken build acceptable — next tasks fix)**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "refactor(bus): delete 25 dead DomainEvent variants

Downstream breakage: activity-log, simulator, agent/autotuner — fixed
in follow-up commits within this PR."
```

**Note:** if preferred, squash Tasks 20–22 into one commit to keep the tree always-green. For CI safety it's often easier to keep one commit per task and squash at PR merge time.

---

## Task 21: Update `activity-log` Normalizer for Deleted Variants

**Files:**
- Modify: `crates/activity-log/src/normalizers.rs:46-415`

- [ ] **Step 1: Delete arms for removed variants**

Open `crates/activity-log/src/normalizers.rs`. Delete every match arm referencing one of the 25 deleted variant names. The wildcard `_ => ...` at the end catches the rest and still produces a generic `ActivitySource::DomainEvent` row.

- [ ] **Step 2: Run**

Run: `cargo build -p activity-log && cargo nextest run -p activity-log`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/activity-log/src/normalizers.rs
git commit -m "refactor(activity-log): drop arms for deleted DomainEvent variants"
```

---

## Task 22: Update `simulator` for Deleted Variants + Typed Constructors

**Files:**
- Modify: `crates/simulator/src/actions.rs`
- Modify: `crates/simulator/src/harness.rs`
- Modify: `crates/simulator/src/agent_harness.rs`

- [ ] **Step 1: Remove constructors for deleted variants**

Open each file and delete any `DomainEvent::<deleted>` construction or match arm.

- [ ] **Step 2: Switch Tasks/Finance simulation to typed events**

Wherever the simulator previously constructed `DomainEvent::TaskCreated` directly, replace with:

```rust
let ev: DomainEvent = feature_tasks::events::TaskEvent::Created { ... }.into();
bus.publish(ev);
```

Same for Finance.

- [ ] **Step 3: Add simulator dependency on feature events**

In `crates/simulator/Cargo.toml`:

```toml
feature-tasks.workspace = true
feature-finance.workspace = true
```

- [ ] **Step 4: Run simulator tests**

Run: `cargo nextest run -p simulator`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator
git commit -m "refactor(simulator): publish feature events through typed enums; drop deleted variants"
```

---

## Task 23: Replace String-Literal `event_type` Queries with Typed Constants

**Files:**
- Modify: `crates/cognitive/src/services/reforge/feedback.rs:50-53`
- Modify: `crates/agent/src/autotuner/metric_collector.rs:67-69`
- (Spec lists ~12 occurrences total — find them all via grep.)

- [ ] **Step 1: Grep every site**

```bash
grep -rn "event_type = \"\|\"ChatTurnCompleted\"\|\"UserCorrectedAI\"\|\"TaskCreated\"" crates/ | grep -v tests
```

Collect the full list.

- [ ] **Step 2: Define typed constants**

The generated `AiEventMeta::event_kind()` already returns `&'static str`. But callers currently query by legacy `DomainEvent` variant name.

For v1, add constants to each feature's `events.rs`:

```rust
impl TaskEvent {
    pub const KIND_CREATED: &'static str = "Created";
    pub const KIND_COMPLETED: &'static str = "Completed";
    // ...
}
```

And expose a `variant_names()` helper in `bus`:

```rust
impl DomainEvent {
    pub const KIND_USER_CORRECTED_AI: &'static str = "UserCorrectedAI";
    pub const KIND_CHAT_TURN_COMPLETED: &'static str = "ChatTurnCompleted";
    // etc. — one const per remaining variant after Task 20
}
```

- [ ] **Step 3: Replace literals**

For each site collected in Step 1, replace the bare string with the constant. Example:

Before:
```rust
repo.query(&[("event_type", "UserCorrectedAI")]).await
```

After:
```rust
repo.query(&[("event_type", DomainEvent::KIND_USER_CORRECTED_AI)]).await
```

- [ ] **Step 4: Add a lint-style test**

Create `crates/cognitive/tests/no_event_type_string_literals.rs`:

```rust
#[test]
fn no_bare_event_type_literals() {
    let src_roots = [
        "../cognitive/src/services/reforge/feedback.rs",
        "../agent/src/autotuner/metric_collector.rs",
    ];
    for p in src_roots {
        let body = std::fs::read_to_string(p).unwrap_or_default();
        for line in body.lines() {
            if line.contains("event_type") && line.contains("\"")
                && !line.trim_start().starts_with("//")
                && !line.contains("KIND_") {
                panic!("bare event_type literal in {}: {}", p, line);
            }
        }
    }
}
```

(Adjust paths to be workspace-relative.)

- [ ] **Step 5: Run**

Run: `cargo nextest run -p cognitive no_event_type_string_literals && cargo build --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "refactor: replace bare event_type string literals with typed constants"
```

---

## Task 24: Register `SignalConsumer`s in `app-core` Init

**Files:**
- Create: `crates/app-core/src/init/ai_pipeline.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/app.rs` (or wherever AppCore wires services)

- [ ] **Step 1: Write failing test**

Create `crates/app-core/tests/ai_pipeline_init_test.rs`:

```rust
#[tokio::test]
async fn publishing_task_event_triggers_ingestion_consumer() {
    let app = test_setup::build_app_core_with_ai_pipeline().await;

    let ev: bus::DomainEvent = feature_tasks::events::TaskEvent::Created {
        task_id: "t1".into(),
        title: "Ship".into(),
        area_id: "a1".into(),
        priority: Some(3),
    }.into();
    app.bus.publish(ev);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let obs = app.observation_repo.latest(10).await.unwrap();
    assert!(obs.iter().any(|o| o.source_event == "Created" && o.domain == "tasks"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo nextest run -p app-core ai_pipeline_init`
Expected: FAIL.

- [ ] **Step 3: Implement ai_pipeline.rs**

Create `crates/app-core/src/init/ai_pipeline.rs`:

```rust
use ai_core::{AiEventMeta, AiSignal, RecallDomain, SignalConsumer, SignalRouter};
use bus::{DomainEvent, DomainEventBus};
use cognitive::consumers::IngestionConsumer;
use std::sync::Arc;

pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    // One arm per feature event prefix. Delegates to the feature's to_signal()
    // via .clone().into() round-trip, which is cheap for the event sizes involved.
    // Alternative: derive a #[derive(TranslateToSignal)] proc-macro later.

    // Tasks
    if let Some(e) = try_into_task_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Tasks;
        return Some(sig);
    }
    // Finance
    if let Some(e) = try_into_finance_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Finance;
        return Some(sig);
    }
    None
}

fn try_into_task_event(e: &DomainEvent) -> Option<feature_tasks::events::TaskEvent> {
    use feature_tasks::events::TaskEvent;
    match e {
        DomainEvent::TaskCreated { task_id, title, area_id, priority, .. } =>
            Some(TaskEvent::Created {
                task_id: task_id.clone(), title: title.clone(),
                area_id: area_id.clone(), priority: *priority
            }),
        DomainEvent::TaskCompleted { task_id, title, deviation_pct, .. } =>
            Some(TaskEvent::Completed {
                task_id: task_id.clone(), title: title.clone(),
                deviation_pct: *deviation_pct
            }),
        DomainEvent::TaskFocusChanged { task_id, title, focus_deadline, .. } =>
            Some(TaskEvent::FocusChanged {
                task_id: task_id.clone(), title: title.clone(),
                focus_deadline: *focus_deadline
            }),
        DomainEvent::EstimationRecorded { task_id, estimated_minutes, actual_minutes } =>
            Some(TaskEvent::EstimationRecorded {
                task_id: task_id.clone(),
                estimated_minutes: *estimated_minutes,
                actual_minutes: *actual_minutes
            }),
        _ => None,
    }
}

fn try_into_finance_event(e: &DomainEvent) -> Option<feature_finance::events::FinanceEvent> {
    use feature_finance::events::FinanceEvent;
    match e {
        DomainEvent::TransactionRecorded { category, amount, is_over_budget } =>
            Some(FinanceEvent::TransactionRecorded {
                tx_id: String::new(), // not in the old variant — acceptable for v1
                category: category.clone(), amount: *amount,
                currency: String::new(), is_over_budget: *is_over_budget
            }),
        DomainEvent::BudgetAlert { category, spent, limit } =>
            Some(FinanceEvent::BudgetAlert {
                category: category.clone(), spent: *spent, limit: *limit
            }),
        DomainEvent::AccountCreated { account_id, name, currency } =>
            Some(FinanceEvent::AccountCreated {
                account_id: account_id.clone(), name: name.clone(), currency: currency.clone()
            }),
        DomainEvent::BudgetCreated { budget_id, name, amount, currency } =>
            Some(FinanceEvent::BudgetCreated {
                budget_id: budget_id.clone(), name: name.clone(),
                amount: *amount, currency: currency.clone()
            }),
        DomainEvent::GoalCreated { goal_id, name, target_amount } =>
            Some(FinanceEvent::GoalCreated {
                goal_id: goal_id.clone(), name: name.clone(), target_amount: *target_amount
            }),
        DomainEvent::GoalAchieved { goal_id, name } =>
            Some(FinanceEvent::GoalAchieved {
                goal_id: goal_id.clone(), name: name.clone()
            }),
        _ => None,
    }
}

pub fn start(
    bus: Arc<DomainEventBus>,
    consumers: Vec<Arc<dyn SignalConsumer>>,
) -> SignalRouter {
    SignalRouter::start(bus, consumers, translate)
}
```

- [ ] **Step 4: Wire into AppCore**

In `crates/app-core/src/init/mod.rs`, add `pub mod ai_pipeline;`.

In `crates/app-core/src/app.rs` (or the main init routine), after constructing `bus` and cognitive repos:

```rust
let ingestion = Arc::new(IngestionConsumer::new(
    observation_repo.clone(),
    entity_repo.clone(),
    episodic_repo.clone(),
    extraction_handler.clone(),
));
let _router = ai_pipeline::start(bus.clone(), vec![ingestion]);
// Store router handle on AppCore so Drop triggers shutdown.
```

- [ ] **Step 5: Run**

Run: `cargo nextest run -p app-core ai_pipeline_init`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core
git commit -m "feat(app-core): register IngestionConsumer via SignalRouter"
```

---

## Task 25: Contract Integration Test — Event → Signal → Consumer

**Files:**
- Create: `tests/ai_pipeline_integration.rs` (workspace root)

- [ ] **Step 1: Write the test**

Create `tests/ai_pipeline_integration.rs`:

```rust
use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct Capture { signals: Arc<Mutex<Vec<AiSignal>>> }

#[async_trait]
impl SignalConsumer for Capture {
    fn name(&self) -> &'static str { "capture" }
    async fn consume(&self, s: &AiSignal) -> common::Result<()> {
        self.signals.lock().unwrap().push(s.clone());
        Ok(())
    }
}

#[tokio::test]
async fn every_feature_event_produces_a_typed_signal() {
    let bus = Arc::new(DomainEventBus::new(64));
    let buf = Arc::new(Mutex::new(Vec::new()));
    let consumer = Arc::new(Capture { signals: buf.clone() }) as Arc<dyn SignalConsumer>;
    let _router = SignalRouter::start(bus.clone(), vec![consumer],
        app_core::init::ai_pipeline::translate);

    let events: Vec<DomainEvent> = vec![
        feature_tasks::events::TaskEvent::Created {
            task_id: "t".into(), title: "x".into(),
            area_id: "a".into(), priority: Some(1),
        }.into(),
        feature_tasks::events::TaskEvent::Completed {
            task_id: "t".into(), title: "x".into(),
            deviation_pct: Some(80.0),
        }.into(),
        feature_finance::events::FinanceEvent::TransactionRecorded {
            tx_id: "tx".into(), category: "groceries".into(),
            amount: 100, currency: "USD".into(), is_over_budget: false,
        }.into(),
        feature_finance::events::FinanceEvent::BudgetAlert {
            category: "dining".into(), spent: 100, limit: 75,
        }.into(),
    ];

    for e in &events { bus.publish(e.clone()); }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let seen = buf.lock().unwrap().clone();
    assert_eq!(seen.len(), events.len());
    assert!(seen.iter().any(|s| s.domain == RecallDomain::Tasks
                              && matches!(s.salience, SalienceVerdict::Extract)));
    assert!(seen.iter().any(|s| s.domain == RecallDomain::Finance && s.importance >= 0.8));
    for s in &seen {
        assert!(!s.content.is_empty(), "every signal must have content");
        assert!((0.0..=1.0).contains(&s.importance), "importance in range");
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_pipeline_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_pipeline_integration.rs
git commit -m "test(ai-pipeline): contract integration — event -> signal -> consumer"
```

---

## Task 26: Invariant Test — Every Declared Event Emits a Signal

**Files:**
- Create: `tests/ai_no_missed_data.rs`

- [ ] **Step 1: Write the test**

Create `tests/ai_no_missed_data.rs`:

```rust
use ai_core::AiEventMeta;

#[test]
fn task_event_every_variant_produces_nonempty_signal() {
    use feature_tasks::events::TaskEvent;
    let samples: Vec<TaskEvent> = vec![
        TaskEvent::Created { task_id: "_".into(), title: "_".into(),
                             area_id: "_".into(), priority: None },
        TaskEvent::Completed { task_id: "_".into(), title: "_".into(),
                               deviation_pct: None },
        TaskEvent::FocusChanged { task_id: "_".into(), title: "_".into(),
                                  focus_deadline: None },
        TaskEvent::EstimationRecorded { task_id: "_".into(),
                                        estimated_minutes: None,
                                        actual_minutes: None },
    ];
    for e in samples {
        let sig = e.to_signal();
        assert!(!sig.event_kind.is_empty());
        assert!((0.0..=1.0).contains(&sig.importance),
                "importance for {} out of range: {}", sig.event_kind, sig.importance);
    }
}

#[test]
fn finance_event_every_variant_produces_nonempty_signal() {
    use feature_finance::events::FinanceEvent;
    let samples: Vec<FinanceEvent> = vec![
        FinanceEvent::TransactionRecorded { tx_id: "_".into(), category: "_".into(),
                                            amount: 0, currency: "_".into(),
                                            is_over_budget: false },
        FinanceEvent::BudgetAlert { category: "_".into(), spent: 0, limit: 0 },
        FinanceEvent::AccountCreated { account_id: "_".into(), name: "_".into(),
                                       currency: "_".into() },
        FinanceEvent::BudgetCreated { budget_id: "_".into(), name: "_".into(),
                                      amount: 0, currency: "_".into() },
        FinanceEvent::GoalCreated { goal_id: "_".into(), name: "_".into(),
                                    target_amount: 0 },
        FinanceEvent::GoalAchieved { goal_id: "_".into(), name: "_".into() },
    ];
    for e in samples {
        let sig = e.to_signal();
        assert!(!sig.event_kind.is_empty());
        assert!((0.0..=1.0).contains(&sig.importance));
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test ai_no_missed_data`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/ai_no_missed_data.rs
git commit -m "test(ai-pipeline): invariant — every declared event variant emits a signal"
```

---

## Task 27: End-to-End — Task Creation Flows Through the Pipeline

**Files:**
- Create: `tests/e2e_task_pipeline.rs`

- [ ] **Step 1: Write**

Create `tests/e2e_task_pipeline.rs`:

```rust
#[tokio::test]
async fn creating_a_task_via_tool_produces_observation_and_entity() {
    let app = app_core::test_support::build_for_pipeline_e2e().await;

    // Invoke TaskTool::create through the tool registry.
    let result = app.tool_registry.get("tasks").unwrap()
        .execute(serde_json::json!({
            "action": "create",
            "title": "Ship v1",
            "area_id": "work"
        })).await.unwrap();

    assert!(result.success);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let obs = app.observation_repo.latest(10).await.unwrap();
    let found = obs.iter().find(|o| o.source_event == "Created"
                                  && o.content.contains("Ship v1"));
    assert!(found.is_some(), "observation with Created event was not persisted");
    assert_eq!(found.unwrap().domain, "tasks");
    assert!(found.unwrap().importance > 0.5);

    assert!(app.entity_repo.exists("task", "Ship v1").await.unwrap());
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test e2e_task_pipeline`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_task_pipeline.rs
git commit -m "test(ai-pipeline): e2e task creation -> observation + entity"
```

---

## Task 28: End-to-End — Finance Transaction Flows Through the Pipeline

**Files:**
- Create: `tests/e2e_finance_pipeline.rs`

- [ ] **Step 1: Write**

Create `tests/e2e_finance_pipeline.rs`:

```rust
#[tokio::test]
async fn recording_a_transaction_produces_observation_and_over_budget_detection() {
    let app = app_core::test_support::build_for_pipeline_e2e().await;

    // Seed an account + budget.
    app.tool_registry.get("finance").unwrap().execute(serde_json::json!({
        "action": "account_add", "name": "Main", "account_type": "bank", "currency": "USD"
    })).await.unwrap();
    app.tool_registry.get("finance").unwrap().execute(serde_json::json!({
        "action": "budget_create", "name": "Dining", "amount": 7500, "category": "dining"
    })).await.unwrap();

    // Record a transaction that exceeds the budget.
    app.tool_registry.get("finance").unwrap().execute(serde_json::json!({
        "action": "tx_add", "account_id": "Main", "tx_type": "expense",
        "amount": 8000, "category": "dining"
    })).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let obs = app.observation_repo.latest(10).await.unwrap();
    assert!(obs.iter().any(|o| o.source_event == "TransactionRecorded"
                             && o.domain == "finance"));
    assert!(obs.iter().any(|o| o.source_event == "BudgetAlert" && o.importance >= 0.8));

    // Verify embedding was written.
    assert!(app.embedding_store.has_embedding_for_kind("finance_transaction").await);
}
```

(If `has_embedding_for_kind` isn't an existing helper, add a minimal one on the test `EmbeddingStore` harness.)

- [ ] **Step 2: Run**

Run: `cargo nextest run --test e2e_finance_pipeline`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_finance_pipeline.rs
git commit -m "test(ai-pipeline): e2e finance tx -> observation + budget alert + embedding"
```

---

## Task 29: Final Verification — Clippy, Nextest, No Placeholders

- [ ] **Step 1: Full workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 2: Full workspace tests**

Run: `cargo nextest run --workspace`
Expected: all PASS.

- [ ] **Step 3: Doctests**

Run: `cargo test --workspace --doc`
Expected: PASS.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: no diff.

- [ ] **Step 5: Search for placeholders left in code**

Run: `grep -rn "TODO\|FIXME\|unimplemented!\|todo!" crates/ai-core crates/ai-core-macros crates/feature-tasks/src/events.rs crates/feature-finance/src/events.rs | grep -v test`

Expected: no hits outside of test scaffolding.

- [ ] **Step 6: Verify success metrics**

| Metric | Target | Command |
|---|---|---|
| `DomainEvent` variant count reduced by 25 | `wc -l crates/bus/src/domain_events.rs` before vs after | visual diff of git history |
| No `event_to_observation` or `evaluate_salience` in cognitive | `grep -rn "event_to_observation\|evaluate_salience" crates/cognitive/src/` | should return 0 |
| No `config_key` / `default_config` on `FeaturePackage` | `grep -rn "fn config_key\|fn default_config" crates/` | should return 0 |
| No `migrations_static` helpers | `grep -rn "migrations_static" crates/` | should return 0 |
| No bare event_type string literals | `grep -rn "event_type.*\"[A-Z]" crates/ | grep -v KIND_ | grep -v test` | should return 0 |

- [ ] **Step 7: Commit verification artefacts if any**

If any minor cleanups were applied during verification, commit them:

```bash
git add -u && git commit -m "chore: v1 verification cleanups"
```

- [ ] **Step 8: Final commit/PR tag**

```bash
git log --oneline origin/main..HEAD
# Confirm every Task 1–28 is represented.
```

---

## Self-Review Notes

**Spec coverage (§ by §):**
- §4.1 crate layout → Tasks 1, 3.
- §4.2 core types → Task 1.
- §4.3 v1 attribute vocabulary → Tasks 4, 5, 6.
- §4.4 traits → Task 2.
- §4.5 RecallDomain → Task 6 (manual enum; `inventory` fallback per spec §11).
- §4.6 SignalRouter → Task 7.
- §5.1 Tasks migration → Tasks 12, 13, 14, 15.
- §5.2 Finance migration → Tasks 16, 17.
- §5.3 dead variants → Task 20.
- §5.4 pre-existing bugs → Tasks 8, 9, 10.
- §5.5 trait cleanup → Task 11.
- §5.6 event_type literals → Task 23.
- §6 v1 kill-list → distributed across Tasks 14, 19, 20, 21, 22, 23.
- §7 testing → Tasks 25, 26, 27, 28.
- §8 success metrics → Task 29.

**Placeholder scan:** no "TBD"/"TODO"/"similar to Task N". Each code block contains full content. Where exact existing signatures weren't in context (e.g., `EntityRepo::upsert_domain`), the plan flags this explicitly ("follow the actual signatures of...") rather than inventing.

**Type consistency:** `TaskEvent` field names consistent across Tasks 12, 24, 25, 26, 27. `FinanceEvent` likewise. `RecallDomain::Tasks` / `Finance` referenced identically across Tasks 6, 18, 24. `AiSignal` struct (7 fields + `raw_event`) consistent after Task 7's update.

**Known approximations flagged for the engineer:**
- `DomainEvent::TaskCreated` field set in Task 12's `From` impl uses `..Default::default()` — if `DomainEvent` variants don't derive `Default`, the engineer enumerates fields explicitly based on the compile error.
- `EntityRepo::upsert_domain`, `AccumulatedObservationRepo::insert`, `EpisodicMemoryRepo::insert_from_observation` method names in Task 18 are approximations; engineer follows the real repo API.
- `app_core::test_support::build_for_pipeline_e2e()` in Tasks 27/28 assumes a test harness exists; if not, add a minimal one mirroring existing test fixtures in `crates/app-core/tests/`.

---

## Execution

Plan complete and saved to `docs/superpowers/plans/2026-04-21-ai-pipeline-v1.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
