# Real-time Data Layer Phase 4 — Distiller Domain Events + `data_version` Polling Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close two remaining gaps in real-time invalidation. **(1) Distiller emission:** every successful pass through `coding_memory::Distiller::distill_turn` publishes `DomainEvent::CodingMemoryUpdated { kind, id }`; the desktop forwarder converts that into a Tauri `entity:updated` event that any "recall" / "memory browser" UI panel reacts to. **(2) Last-resort fallback:** a low-frequency tokio task in the storage layer polls `PRAGMA data_version` every 5 seconds; on an unexpected delta (i.e. another connection — typically the MCP child or a CLI mutation — wrote without the bridge firing) it publishes `DomainEvent::DataVersionBumped { previous, current }`, which the forwarder ships as a dedicated `data:version_bumped` Tauri event. The FE bridge handler turns that into `client.invalidateQueries()` (no key prefix → match every query). After this plan, **every** mutation source in the system propagates to every webview.

**Architecture:** Backend additions are surgical — two new `DomainEvent` variants in `crates/bus`, two new `EntityKind` variants in `crates/desktop-shared`, a `Distiller` constructor change to accept the bus, two `bus.publish(...)` calls inside `distill_turn`, a new `DomainEvent → Tauri` arm in the existing `wire_event_channels` forwarder (`crates/desktop/src/app_core.rs:335-373`), and a new `StoragePool::start_data_version_watcher` method that owns its own background task. Frontend additions extend Plan 1's foundation (`desktop-ui/src/lib/query/`): two new `EntityKind`s, a new `qk.codingMemory.*` namespace, two new `ENTITY_INVALIDATIONS` rows, and a new `STATIC_ROUTES` entry mapping `data:version_bumped` to "broad invalidate". No protocol changes — Plan 3's mcp-bridge already ships every event generically.

**Tech Stack:** Rust 2024, existing `bus::DomainEventBus` (broadcast channel), existing `crates/coding-memory/src/distiller`, `crates/storage/src/pool.rs` (sqlx 0.8 SqlitePool), `tokio-util = "0.7"` (`CancellationToken`), `tempfile = "3.14"` (dev), Plan 1's `tauriEventBridge.ts` + TanStack Query v5.

**Master plan context:** Plan 4 of 4. **Depends on Plan 3** (the existing `entity:updated` forwarding path Phase F1 added is what Distiller events ride on; the `app_handle.emit(event, payload)` site is reused unchanged). **Independent of Plan 2.** This is the final plan in the master series.

---

## File Structure

### Files to modify

| Path | Change |
|---|---|
| `crates/bus/src/domain_events.rs` | Add `DomainEvent::CodingMemoryUpdated { kind, id }` and `DomainEvent::DataVersionBumped { previous, current }`. Add `CodingMemoryKind` enum. Extend `variant_name()`, `domain()`, and add `KIND_*` constants. |
| `crates/desktop-shared/src/types.rs` | Add `EntityKind::CodingFact` and `EntityKind::CodingEpisode`. Extend `EntityKind::parse()` accordingly. |
| `crates/coding-memory/src/distiller/mod.rs` | Add `Arc<DomainEventBus>` field to `DistillerInner`. Extend constructor to accept it. After each successful `writer.write_episode` / `writer.write_fact` call, publish `DomainEvent::CodingMemoryUpdated`. |
| `crates/app-core/src/init/mod.rs` (~line 858) | Pass `domain_event_bus.clone()` into `Distiller::new(...)`. |
| `crates/app-core/src/state.rs` | Add `pub _data_version_watcher_token: Option<CancellationToken>` field on `AppCore`. |
| `crates/storage/Cargo.toml` | Add `tokio-util.workspace = true` to deps; add `bus = { path = "../bus" }` to deps. Add `tempfile.workspace = true` to dev-deps. |
| `crates/storage/src/pool.rs` | Add `pub async fn start_data_version_watcher(&self, bus: Arc<bus::DomainEventBus>, interval: Duration) -> CancellationToken`. |
| `crates/desktop/src/app_core.rs` (`wire_event_channels`, ~line 335) | New `tokio::spawn` subscriber loop matching `DomainEvent::CodingMemoryUpdated` (→ `app_handle.emit("entity:updated", EntityUpdatedPayload {..})`) and `DomainEvent::DataVersionBumped` (→ `app_handle.emit("data:version_bumped", payload)`). |
| `crates/desktop/src/app_core.rs` (`init`, after BridgeServer block) | Call `core.storage_pool.start_data_version_watcher(domain_event_bus.clone(), Duration::from_secs(5)).await` and stash token in a new `OnceLock` (mirrors the `BRIDGE_SERVER` pattern at line 22). |
| `desktop-ui/src/lib/query/entityKindMap.ts` | Add `"codingFact"` and `"codingEpisode"` to the `EntityKind` union; add `["coding_memory_", "codingFact"]` to `PREFIX_TABLE`. |
| `desktop-ui/src/lib/query/queryKeys.ts` | Add `qk.codingMemory.{ all, facts, episodes, recallIndex, memoryBrowser, status }`. |
| `desktop-ui/src/lib/query/tauriEventBridge.ts` | Add `codingFact` + `codingEpisode` rows to `ENTITY_INVALIDATIONS`; add `["data:version_bumped", []]` STATIC_ROUTE with a special-cased "broad invalidate" handler. |
| `desktop-ui/src/lib/query/tests/entityKindMap.test.ts` | Cover `coding_memory_recall_fetch` → `"codingFact"` mapping. |
| `desktop-ui/src/lib/query/tests/queryKeys.test.ts` | Cover `qk.codingMemory.facts()` / `episodes()` / `recallIndex()`. |
| `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts` | Cover `entity:updated{kind:"codingFact"}` → invalidates `codingMemory.all()`; cover `data:version_bumped` → broad invalidate. |

### New files

| Path | Responsibility |
|---|---|
| `crates/coding-memory/tests/distiller_events.rs` | Integration test: drive `distill_turn` with stubbed I/O, assert `CodingMemoryUpdated` flows through a real `DomainEventBus`. |
| `crates/storage/tests/data_version_watcher.rs` | Integration test: open one `StoragePool`, start watcher, mutate via a *second* sqlx pool against the same DB file, assert `DataVersionBumped` arrives within 1 s. |

### Files NOT modified (verified during research; called out to prevent drift)

- `crates/desktop-shared/src/events.rs` — `ENTITY_UPDATED = "entity:updated"` (line 64) and `EntityUpdatedPayload` (lines 173–178) are reused as-is. We add a new constant `DATA_VERSION_BUMPED` *only* if convenient — fine to inline the string.
- `crates/app-core/src/events.rs` — `AppEventEmitter` trait unchanged. The new path goes through the existing `DomainEvent → Tauri` forwarder, **not** through `AppEventEmitter`.
- `crates/cognitive/src/repos/{episodic_memory,semantic_fact}.rs` — repos don't publish events themselves; the Distiller is the right authority because it knows whether the write was deduplicated, retried, or first-class.
- `mcp-bridge` — generic. New events ride the existing socket (Plan 3) when emitted from the MCP child.

---

## Phase A — Domain event additions

### Task A1: Add `DomainEvent::CodingMemoryUpdated` + `CodingMemoryKind`

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

`★ Insight ─────────────────────────────────────`
`DomainEvent` is **not** annotated with `#[serde(tag = ..., rename_all = ...)]` (verified at `domain_events.rs:21`). It serializes externally-tagged by default — adding a `tag` attribute would silently break every existing consumer (cognitive replay, fabric graph, debug dashboard). We add new variants with the same plain shape and keep the existing `variant_name()` / `domain()` / `KIND_*` triad in sync.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read the existing variant block to find the insertion point**

```bash
sed -n '585,600p' /Users/jayden/Projects/Klynt/bot/crates/bus/src/domain_events.rs
```

Expected: ends with the `CodingMirrorAlert { ... }` variant followed by the closing `}` of the enum at line 593.

- [ ] **Step 2: Add the variants — failing test first**

Append to `crates/bus/src/domain_events.rs` (inside the existing `#[cfg(test)] mod tests`, or add a new test module if none):

```rust
#[cfg(test)]
mod phase4_event_tests {
    use super::*;

    #[test]
    fn coding_memory_updated_serializes_with_kind_and_id() {
        let evt = DomainEvent::CodingMemoryUpdated {
            kind: CodingMemoryKind::Fact,
            id: "fact-abc".into(),
        };
        let v = serde_json::to_value(&evt).unwrap();
        // Externally-tagged: { "CodingMemoryUpdated": { "kind": "fact", "id": "fact-abc" } }
        let inner = &v["CodingMemoryUpdated"];
        assert_eq!(inner["kind"], serde_json::json!("fact"));
        assert_eq!(inner["id"], serde_json::json!("fact-abc"));
    }

    #[test]
    fn coding_memory_kind_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(CodingMemoryKind::Episode).unwrap(),
            serde_json::json!("episode")
        );
    }

    #[test]
    fn data_version_bumped_serializes() {
        let evt = DomainEvent::DataVersionBumped { previous: 41, current: 42 };
        let v = serde_json::to_value(&evt).unwrap();
        let inner = &v["DataVersionBumped"];
        assert_eq!(inner["previous"], 41);
        assert_eq!(inner["current"], 42);
    }

    #[test]
    fn coding_memory_updated_variant_name_is_stable() {
        let evt = DomainEvent::CodingMemoryUpdated {
            kind: CodingMemoryKind::Fact,
            id: "x".into(),
        };
        assert_eq!(evt.variant_name(), "CodingMemoryUpdated");
        assert_eq!(evt.domain().as_str(), "coding_memory");
    }

    #[test]
    fn data_version_bumped_belongs_to_general_domain() {
        let evt = DomainEvent::DataVersionBumped { previous: 0, current: 1 };
        assert_eq!(evt.variant_name(), "DataVersionBumped");
        // No specific subsystem owns it; goes to General.
        assert_eq!(evt.domain().as_str(), "general");
    }
}
```

- [ ] **Step 3: Run — expect FAIL on missing variant + missing `CodingMemoryKind`**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p bus phase4_event_tests 2>&1 | tail -15
```

Expected: compile errors `cannot find variant 'CodingMemoryUpdated'`, `cannot find variant 'DataVersionBumped'`, `cannot find type 'CodingMemoryKind'`.

- [ ] **Step 4: Add `CodingMemoryKind`**

Insert near the top of `crates/bus/src/domain_events.rs` (just below the existing `WakeType` enum at line 15):

```rust
/// Sub-kind of a coding-memory write — distinguishes the two destination
/// tables (`semantic_facts` vs `episodic_memories`) without forcing the
/// listener to import storage types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingMemoryKind {
    Fact,
    Episode,
}
```

- [ ] **Step 5: Add the two new variants**

Inside `pub enum DomainEvent { ... }`, immediately *after* the existing `CodingMirrorAlert { ... }` variant (line 587–592) and *before* the enum's closing `}` at line 593:

```rust
    /// A coding-memory write completed (fact upsert or episode insert).
    /// Emitted from `coding_memory::Distiller::distill_turn` after each
    /// successful row write so any UI panel observing recall data can
    /// invalidate its cache.
    CodingMemoryUpdated {
        kind: CodingMemoryKind,
        id: String,
    },
    /// SQLite `PRAGMA data_version` advanced unexpectedly — i.e. some
    /// connection outside our process pool wrote, and we never saw the
    /// matching domain event. Listeners should perform a broad invalidate.
    DataVersionBumped {
        previous: u32,
        current: u32,
    },
```

- [ ] **Step 6: Add `KIND_*` constants**

In the `impl DomainEvent` block where the existing `KIND_CODING_MIRROR_ALERT` lives (line 838), append:

```rust
    pub const KIND_CODING_MEMORY_UPDATED: &'static str = "CodingMemoryUpdated";
    pub const KIND_DATA_VERSION_BUMPED: &'static str = "DataVersionBumped";
```

- [ ] **Step 7: Extend `variant_name()`**

In the existing `pub fn variant_name(&self) -> &'static str` match (lines 600–693), add two arms before the closing `}`:

```rust
            Self::CodingMemoryUpdated { .. } => Self::KIND_CODING_MEMORY_UPDATED,
            Self::DataVersionBumped { .. } => Self::KIND_DATA_VERSION_BUMPED,
```

- [ ] **Step 8: Extend `domain()`**

In `domain()` (lines 847–950), extend the existing CodingMemory arm (lines 941–948) by appending `| Self::CodingMemoryUpdated { .. }` to the OR-pattern:

```rust
            Self::PatternApplied { .. }
            | Self::PatternOutcome { .. }
            | Self::FixAttemptFailed { .. }
            | Self::MemoryRetrieved { .. }
            | Self::AssistantMsgCompleted { .. }
            | Self::RetrievalSkillApplied { .. }
            | Self::CodingSessionEnded { .. }
            | Self::CodingMirrorAlert { .. }
            | Self::CodingMemoryUpdated { .. } => D::CodingMemory,
```

`DataVersionBumped` is cross-cutting — add it to the `D::General` arm. Find the existing `D::General` arm (search the same `domain()` block for `=> D::General`) and append `| Self::DataVersionBumped { .. }` to that pattern. If no explicit `D::General` arm exists, add one before the wildcard:

```rust
            Self::DataVersionBumped { .. } => D::General,
```

- [ ] **Step 9: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p bus 2>&1 | tail -10
```

Expected: 5 new tests pass; all previously-existing `bus` tests still pass.

- [ ] **Step 10: Re-export from `lib.rs`**

Edit `crates/bus/src/lib.rs`. Find the `pub use domain_events::{...};` line (line 13):

```rust
pub use domain_events::{CorrectionKind, DomainEvent, DomainEventBus, FeedbackResponse};
```

Replace with:

```rust
pub use domain_events::{CodingMemoryKind, CorrectionKind, DomainEvent, DomainEventBus, FeedbackResponse};
```

- [ ] **Step 11: Verify the workspace still compiles (catches any forgotten match arm)**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build --workspace 2>&1 | tail -20
```

Expected: clean. If a downstream `match domain_event { ... }` needs the new arm, fix in place — most consumers use a wildcard `_ => {}` so this is rare. If a non-exhaustive match warning appears, **add the wildcard arm with an explicit no-op**, never silence the warning.

- [ ] **Step 12: Commit**

```bash
git add crates/bus/src
git commit -m "feat(bus): add CodingMemoryUpdated + DataVersionBumped DomainEvent variants"
```

---

### Task A2: Extend `EntityKind` in `desktop-shared`

**Files:**
- Modify: `crates/desktop-shared/src/types.rs`

`★ Insight ─────────────────────────────────────`
`EntityKind` is `#[serde(rename_all = "camelCase")]` (verified at `types.rs:48`). Adding `CodingFact` and `CodingEpisode` will serialize as `"codingFact"` and `"codingEpisode"` on the wire automatically — same convention the FE `entityKindMap.ts` already uses. The `parse()` method (not `FromStr`) takes a lowercased string, so we add both snake_case and concatenated forms to match the lookup pattern of every other variant.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add a failing test**

Append to `crates/desktop-shared/src/types.rs` (inside the existing `#[cfg(test)] mod tests` block, or create one):

```rust
#[cfg(test)]
mod phase4_kind_tests {
    use super::*;

    #[test]
    fn coding_fact_serializes_camel_case() {
        let v = serde_json::to_value(EntityKind::CodingFact).unwrap();
        assert_eq!(v, serde_json::json!("codingFact"));
    }

    #[test]
    fn coding_episode_serializes_camel_case() {
        let v = serde_json::to_value(EntityKind::CodingEpisode).unwrap();
        assert_eq!(v, serde_json::json!("codingEpisode"));
    }

    #[test]
    fn parse_coding_kinds() {
        assert!(matches!(EntityKind::parse("coding_fact"), Some(EntityKind::CodingFact)));
        assert!(matches!(EntityKind::parse("codingfact"), Some(EntityKind::CodingFact)));
        assert!(matches!(EntityKind::parse("coding_episode"), Some(EntityKind::CodingEpisode)));
        assert!(matches!(EntityKind::parse("codingepisode"), Some(EntityKind::CodingEpisode)));
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p desktop-shared phase4_kind_tests 2>&1 | tail -10
```

Expected: `no variant named 'CodingFact'`.

- [ ] **Step 3: Add the variants**

In `crates/desktop-shared/src/types.rs`, find the `pub enum EntityKind { ... }` block (lines 47–64). Append to the variant list (before the closing `}`):

```rust
    CodingFact,
    CodingEpisode,
```

- [ ] **Step 4: Extend `parse()`**

Find `impl EntityKind { pub fn parse(s: &str) -> Option<Self> { match s.to_lowercase().as_str() { ... } } }` (lines 67–95). Add two arms before the wildcard `_ => None`:

```rust
            "coding_fact" | "codingfact" => Some(Self::CodingFact),
            "coding_episode" | "codingepisode" => Some(Self::CodingEpisode),
```

- [ ] **Step 5: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p desktop-shared 2>&1 | tail -10
```

Expected: 3 new tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop-shared/src/types.rs
git commit -m "feat(desktop-shared): EntityKind::CodingFact + CodingEpisode"
```

---

## Phase B — Plumb the bus into `Distiller`

### Task B1: Add `Arc<DomainEventBus>` field + constructor change

**Files:**
- Modify: `crates/coding-memory/Cargo.toml` (verify `bus` already a dep)
- Modify: `crates/coding-memory/src/distiller/mod.rs`

`★ Insight ─────────────────────────────────────`
The Distiller currently owns no event bus (verified — `DistillerInner` at `distiller/mod.rs:142-150` has only `config`, `ingest_repo`, `writer`, `provider`, `retriever`, `retry_repo`, `buffer`). We add the bus as `Option<Arc<DomainEventBus>>` rather than required so existing call-sites (tests, CLI replays) that construct a Distiller without a bus remain compilable. Production wiring at `app-core/src/init/mod.rs:858-895` will pass `Some(bus)`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Verify `bus` is already a dep of `coding-memory`**

```bash
grep -n "^bus" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/Cargo.toml
```

Expected: a line like `bus = { path = "../bus" }` exists. If missing, add it under `[dependencies]`:

```toml
bus = { path = "../bus" }
```

- [ ] **Step 2: Add the field**

Edit `crates/coding-memory/src/distiller/mod.rs`. Find `struct DistillerInner` (line 142):

```rust
struct DistillerInner {
    config: DistillerConfig,
    ingest_repo: Arc<IngestEventLogRepo>,
    writer: writer::DistillerWriter,
    provider: Arc<ProviderManager>,
    #[allow(dead_code)]
    retriever: Arc<dyn context_engine::MemoryRetriever>,
    retry_repo: Option<DistillationRetryRepo>,
    buffer: Mutex<TurnBuffer>,
}
```

Add the field at the end (before the closing brace):

```rust
    /// Optional event bus. When `Some`, every successful fact / episode write
    /// publishes a `DomainEvent::CodingMemoryUpdated` so UI panels can refresh
    /// without polling. `None` keeps standalone use (tests, CLI replays) silent.
    event_bus: Option<Arc<bus::DomainEventBus>>,
```

- [ ] **Step 3: Make `Distiller::new` take the bus + add a builder**

Find the existing `pub fn new(...)` constructor (lines 168–187). Replace with:

```rust
pub fn new(
    config: DistillerConfig,
    ingest_repo: Arc<IngestEventLogRepo>,
    writer: writer::DistillerWriter,
    provider: Arc<ProviderManager>,
    retriever: Arc<dyn context_engine::MemoryRetriever>,
) -> Self {
    Self {
        inner: Arc::new(DistillerInner {
            config,
            ingest_repo,
            writer,
            provider,
            retriever,
            retry_repo: None,
            buffer: Mutex::new(TurnBuffer::default()),
            event_bus: None,
        }),
    }
}

/// Attach a domain-event bus. Returns `self` for chaining alongside
/// `with_retry_repo`. Idempotent — last call wins.
pub fn with_event_bus(self, bus: Arc<bus::DomainEventBus>) -> Self {
    let mut inner = (*self.inner).clone_for_builder();
    inner.event_bus = Some(bus);
    Self { inner: Arc::new(inner) }
}
```

The `clone_for_builder` helper is needed because `DistillerInner` is held inside an `Arc` and isn't trivially mutable. Add it to the `impl DistillerInner` block (or create one if absent):

```rust
impl DistillerInner {
    fn clone_for_builder(&self) -> Self {
        Self {
            config: self.config.clone(),
            ingest_repo: self.ingest_repo.clone(),
            writer: self.writer.clone(),
            provider: self.provider.clone(),
            retriever: self.retriever.clone(),
            retry_repo: self.retry_repo.clone(),
            buffer: Mutex::new(TurnBuffer::default()),
            event_bus: self.event_bus.clone(),
        }
    }
}
```

If `DistillerWriter`, `DistillerConfig`, or `DistillationRetryRepo` are not `Clone`, derive `Clone` on them — they're plain config structs / `Arc` wrappers and should clone freely. Verify:

```bash
grep -nE "pub struct DistillerWriter|pub struct DistillerConfig|pub struct DistillationRetryRepo" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/distiller/*.rs
```

For each, ensure `#[derive(Clone)]` is present; add it where missing.

- [ ] **Step 4: Build**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p coding-memory 2>&1 | tail -15
```

Expected: clean.

- [ ] **Step 5: Wire the bus at construction time**

Edit `/Users/jayden/Projects/Klynt/bot/crates/app-core/src/init/mod.rs`. Find the Distiller construction block (lines 858–895). The relevant lines are around 880–893:

```rust
    let mut d = coding_memory::distiller::Distiller::new(
        distiller_cfg,
        ingest_repo,
        writer,
        distiller_provider,
        retriever,
    );
    d = d.with_retry_repo(coding_memory::distiller::DistillationRetryRepo::new(
        storage_pool.inner().clone(),
    ));
    Arc::new(d)
```

Insert one line after `with_retry_repo`:

```rust
    let mut d = coding_memory::distiller::Distiller::new(
        distiller_cfg,
        ingest_repo,
        writer,
        distiller_provider,
        retriever,
    );
    d = d.with_retry_repo(coding_memory::distiller::DistillationRetryRepo::new(
        storage_pool.inner().clone(),
    ));
    d = d.with_event_bus(domain_event_bus.clone());
    Arc::new(d)
```

Verify `domain_event_bus` is in scope at this site:

```bash
sed -n '780,860p' /Users/jayden/Projects/Klynt/bot/crates/app-core/src/init/mod.rs | grep -n "domain_event_bus"
```

Expected: at least one match (it's constructed earlier in `init`). If the local name differs (`event_bus`, `domain_bus`, etc.), use that name.

- [ ] **Step 6: Build the workspace**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build --workspace 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory crates/app-core/src/init/mod.rs
git commit -m "feat(coding-memory): plumb DomainEventBus into Distiller via with_event_bus"
```

---

## Phase C — Distiller emission

### Task C1: Publish `CodingMemoryUpdated` after each successful write

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Create: `crates/coding-memory/tests/distiller_events.rs`

The Distiller's writes happen at three sites in `distill_turn` (verified):
- `self.inner.writer.write_episode(ep)` — line 403 (one episode per turn)
- `self.inner.writer.write_fact(prepared_fact)` — line 447 (Phase B output)
- `self.inner.writer.write_fact(pf)` — lines 482, 492 (Phase C reconciliation)

Each returns the inserted/updated row id. We publish *after* a successful write only.

`★ Insight ─────────────────────────────────────`
We deliberately publish per-row (not once per `distill_turn` call) so the FE can show fine-grained "fresh data" indicators without re-fetching the entire memory browser. The cost is small: `DomainEventBus::publish` is `broadcast::Sender::send` (lock-free), and a typical turn produces at most a few writes. An alternative — coalescing into a single `CodingTurnDistilled { fact_count, episode_count }` event — is simpler but loses the per-row id, which the recall UI needs to highlight new entries. Per-row wins.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Confirm the exact write sites**

```bash
grep -nE "writer\.write_(episode|fact)" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/distiller/mod.rs
```

Expected: 4 matches (lines ≈ 403, 447, 482, 492). The exact line numbers may have shifted during Task B1; use the current `grep` output as the source of truth.

- [ ] **Step 2: Inspect each return type to know what to publish as `id`**

```bash
grep -nE "pub (async )?fn write_(episode|fact)" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/distiller/writer.rs
```

Confirm both return `Result<String, _>` (the row id) — the existing `let row_id = ...write_fact(pf).await?;` calls in `mod.rs` confirm this. If a method returns `()`, we need the id from the *input* (`pf.id` or `ep.id`); the test in Step 3 will catch any mismatch.

- [ ] **Step 3: Write the failing integration test**

Create `crates/coding-memory/tests/distiller_events.rs`:

```rust
//! Integration test for Phase 4: every successful Distiller write must
//! publish a `DomainEvent::CodingMemoryUpdated` on the attached bus.
//!
//! We bypass Phase A/B/C orchestration by calling the writer methods
//! directly through a thin wrapper exposed for tests. If that surface
//! does not yet exist, the test drives the public `distill_turn` instead
//! using the in-memory storage harness already used by `coding-memory`'s
//! existing integration tests.

use bus::{CodingMemoryKind, DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

mod harness {
    //! Minimal harness — opens an in-memory SqlitePool, runs migrations,
    //! constructs a no-op provider stub, and returns a Distiller.
    //!
    //! NOTE: this duplicates ~30 lines of setup that already exist in
    //! `crates/coding-memory/tests/distiller_*.rs` files. If a shared
    //! harness module is found in the existing test directory, prefer
    //! importing it. Run:
    //!     ls crates/coding-memory/tests/
    //! and reuse any `common.rs` / `harness.rs` / `support.rs` module.

    use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
    use coding_ingest::repos::IngestEventLogRepo;
    use std::sync::Arc;
    use storage::StoragePool;

    pub async fn build(bus: Arc<bus::DomainEventBus>) -> Distiller {
        let pool = StoragePool::connect_in_memory().await.expect("pool");
        let inner = pool.inner().clone();
        let ingest_repo = Arc::new(IngestEventLogRepo::new(inner.clone()));
        let fact_repo = cognitive::SemanticFactRepo::new(inner.clone());
        let episode_repo = cognitive::EpisodicMemoryRepo::new(inner.clone());
        let writer = DistillerWriter::new(fact_repo.clone(), episode_repo);
        let retriever: Arc<dyn context_engine::MemoryRetriever> =
            Arc::new(cognitive::UnifiedMemoryService::new(fact_repo));
        let provider = stub_provider();
        Distiller::new(DistillerConfig::default(), ingest_repo, writer, provider, retriever)
            .with_event_bus(bus)
    }

    fn stub_provider() -> Arc<providers::ProviderManager> {
        // The provider is unused by `write_episode_for_test` /
        // `write_fact_for_test` (see Step 4), so any constructor is fine.
        // Use the existing in-memory NoopProvider if available, otherwise
        // panic and instruct the engineer to swap in a real one.
        Arc::new(providers::ProviderManager::for_tests())
    }
}

#[tokio::test]
async fn fact_write_publishes_coding_memory_updated() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let d = harness::build(bus.clone()).await;

    // Drive a single fact write through the test surface added in Step 4.
    let id = d.write_fact_for_test("session-x", "fact text").await.unwrap();

    let evt = timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("no event in 200 ms")
        .expect("bus closed");

    match evt {
        DomainEvent::CodingMemoryUpdated { kind, id: emitted_id } => {
            assert_eq!(kind, CodingMemoryKind::Fact);
            assert_eq!(emitted_id, id);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn episode_write_publishes_coding_memory_updated() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let d = harness::build(bus.clone()).await;

    let id = d.write_episode_for_test("session-y").await.unwrap();

    let evt = timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("no event")
        .expect("closed");
    match evt {
        DomainEvent::CodingMemoryUpdated { kind, id: emitted_id } => {
            assert_eq!(kind, CodingMemoryKind::Episode);
            assert_eq!(emitted_id, id);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn no_bus_attached_is_silent() {
    // Regression: constructing without `with_event_bus` must not panic
    // and must not deadlock. We simply build, write, and confirm we get
    // back without error — no bus to assert on.
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let d = {
        // Construct *without* attaching a bus.
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        let ingest = Arc::new(coding_ingest::repos::IngestEventLogRepo::new(inner.clone()));
        let fact = cognitive::SemanticFactRepo::new(inner.clone());
        let ep = cognitive::EpisodicMemoryRepo::new(inner.clone());
        let writer = coding_memory::distiller::DistillerWriter::new(fact.clone(), ep);
        let retriever: Arc<dyn context_engine::MemoryRetriever> =
            Arc::new(cognitive::UnifiedMemoryService::new(fact));
        coding_memory::distiller::Distiller::new(
            coding_memory::distiller::DistillerConfig::default(),
            ingest,
            writer,
            Arc::new(providers::ProviderManager::for_tests()),
            retriever,
        )
    };
    let _ = d.write_fact_for_test("session-z", "fact").await.unwrap();

    // A short window to confirm nothing arrived on the foreign bus.
    let res = timeout(Duration::from_millis(80), rx.recv()).await;
    assert!(res.is_err(), "no event should arrive when bus not attached");
}
```

- [ ] **Step 4: Add the `_for_test` surface to `Distiller`**

In `crates/coding-memory/src/distiller/mod.rs`, add a `#[cfg(any(test, feature = "test-support"))]` block on `impl Distiller` exposing the two write paths so tests can drive a single write without spinning up Phase A→C orchestration:

```rust
#[cfg(any(test, feature = "test-support"))]
impl Distiller {
    pub async fn write_fact_for_test(
        &self,
        session_id: &str,
        text: &str,
    ) -> crate::Result<String> {
        use crate::distiller::writer::PreparedFact;
        let pf = PreparedFact::minimal_for_test(session_id, text);
        let row_id = self.inner.writer.write_fact(pf).await?;
        self.publish_memory_updated(bus::CodingMemoryKind::Fact, &row_id);
        Ok(row_id)
    }

    pub async fn write_episode_for_test(
        &self,
        session_id: &str,
    ) -> crate::Result<String> {
        use crate::distiller::writer::PreparedEpisode;
        let ep = PreparedEpisode::minimal_for_test(session_id);
        let row_id = self.inner.writer.write_episode(ep).await?;
        self.publish_memory_updated(bus::CodingMemoryKind::Episode, &row_id);
        Ok(row_id)
    }
}
```

If `PreparedFact::minimal_for_test` / `PreparedEpisode::minimal_for_test` don't exist, add them in `writer.rs`:

```rust
#[cfg(any(test, feature = "test-support"))]
impl PreparedFact {
    pub fn minimal_for_test(session_id: &str, text: &str) -> Self {
        Self {
            id: format!("fact-{}", uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            content: text.to_string(),
            // Fill other required fields with minimal defaults — read the
            // PreparedFact field list and assign sensible test values.
            ..Self::default()
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl PreparedEpisode {
    pub fn minimal_for_test(session_id: &str) -> Self {
        Self {
            id: format!("ep-{}", uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            ..Self::default()
        }
    }
}
```

If `PreparedFact` / `PreparedEpisode` don't have `Default`, derive it on them, or list the actual fields explicitly with stub values (read the struct definitions first via `grep -nE "pub struct PreparedFact|pub struct PreparedEpisode" crates/coding-memory/src/distiller/writer.rs` and inspect).

- [ ] **Step 5: Add the `publish_memory_updated` helper**

In the `impl Distiller` block (the *non-test* one), add a private helper:

```rust
    /// Publishes a `CodingMemoryUpdated` event if a bus is attached. Cheap:
    /// `broadcast::send` returns immediately even with no subscribers.
    fn publish_memory_updated(&self, kind: bus::CodingMemoryKind, id: &str) {
        if let Some(bus) = &self.inner.event_bus {
            bus.publish(bus::DomainEvent::CodingMemoryUpdated {
                kind,
                id: id.to_string(),
            });
        }
    }
```

- [ ] **Step 6: Wire `publish_memory_updated` into the four real write sites**

In `distill_turn`, locate each write site and append a publish call. Pattern — find:

```rust
    let row_id = self.inner.writer.write_episode(ep).await?;
```

Replace with:

```rust
    let row_id = self.inner.writer.write_episode(ep).await?;
    self.publish_memory_updated(bus::CodingMemoryKind::Episode, &row_id);
```

Apply identically to each `write_fact` site:

```rust
    let row_id = self.inner.writer.write_fact(prepared_fact).await?;
    self.publish_memory_updated(bus::CodingMemoryKind::Fact, &row_id);
```

If the existing code does not bind the result to `row_id` (e.g. `let _ = self.inner.writer.write_fact(pf).await?;` or chained), bind it explicitly:

```rust
    let _written_fact_id = {
        let id = self.inner.writer.write_fact(pf).await?;
        self.publish_memory_updated(bus::CodingMemoryKind::Fact, &id);
        id
    };
```

Confirm all four sites updated:

```bash
grep -cE "publish_memory_updated" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/distiller/mod.rs
```

Expected: 4 (one per write site).

- [ ] **Step 7: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p coding-memory --test distiller_events 2>&1 | tail -15
```

Expected: 3 passing tests. If the harness can't find a working `ProviderManager::for_tests()`, replace with whatever stub the existing `coding-memory` integration tests use (search `crates/coding-memory/tests/` for the pattern).

- [ ] **Step 8: Re-run all `coding-memory` tests to confirm no regression**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p coding-memory 2>&1 | tail -10
```

Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add crates/coding-memory
git commit -m "feat(coding-memory): publish CodingMemoryUpdated after each Distiller write"
```

---

## Phase D — Forwarder routing in desktop

### Task D1: Forward `CodingMemoryUpdated` and `DataVersionBumped` as Tauri events

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

`★ Insight ─────────────────────────────────────`
The existing `wire_event_channels` already has a `DomainEvent` subscriber loop (lines 335–373) that emits `cognitive:domain_event` for the debug dashboard. We add a *second* subscriber loop that handles only the two events that need cross-window UI invalidation. Two loops (instead of extending the first) keeps the responsibility separation clean: the debug forwarder is a write-only firehose; the UI invalidation forwarder is a typed router.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Locate the insertion site**

```bash
sed -n '370,410p' /Users/jayden/Projects/Klynt/bot/crates/desktop/src/app_core.rs
```

Expected: end of the existing debug-dashboard subscriber block at ~line 373, followed by the lifecycle (sleep/wake) subscriber at lines 375–406. Insert the new block *after* the lifecycle block (i.e. after line 406 / before whatever comes next).

- [ ] **Step 2: Insert the new subscriber loop**

After the lifecycle (sleep/wake) block, add:

```rust
    // Phase 4: forward CodingMemoryUpdated → entity:updated, and
    // DataVersionBumped → data:version_bumped, so Plan 1's tauriEventBridge.ts
    // can invalidate the matching TanStack Query keys in every webview.
    {
        let mut event_rx = channels.domain_event_bus.subscribe();
        let app_handle_clone = app_handle.clone();
        let token = shutdown.clone();
        tokio::spawn(async move {
            use tauri::Emitter;
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = event_rx.recv() => match result {
                        Ok(bus::DomainEvent::CodingMemoryUpdated { kind, id }) => {
                            let entity_kind = match kind {
                                bus::CodingMemoryKind::Fact => desktop_shared::types::EntityKind::CodingFact,
                                bus::CodingMemoryKind::Episode => desktop_shared::types::EntityKind::CodingEpisode,
                            };
                            let payload = desktop_shared::events::EntityUpdatedPayload {
                                entity_kind,
                                id,
                            };
                            if let Err(e) = app_handle_clone
                                .emit(desktop_shared::events::ENTITY_UPDATED, &payload)
                            {
                                tracing::warn!("phase4: failed to emit entity:updated for coding memory: {e}");
                            }
                        }
                        Ok(bus::DomainEvent::DataVersionBumped { previous, current }) => {
                            // Generic "broad invalidate" signal — payload is informational only.
                            let payload = serde_json::json!({
                                "previous": previous,
                                "current": current,
                            });
                            if let Err(e) = app_handle_clone.emit("data:version_bumped", &payload) {
                                tracing::warn!("phase4: failed to emit data:version_bumped: {e}");
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("phase4 forwarder lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        });
    }
```

- [ ] **Step 3: Build**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p desktop 2>&1 | tail -15
```

Expected: clean. If a clippy warning fires about `Ok(_) => {}` being non-exhaustive-feeling, ignore — the wildcard is intentional (this loop owns *only* these two variants).

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): forward CodingMemoryUpdated + DataVersionBumped as Tauri events"
```

---

## Phase E — `PRAGMA data_version` polling fallback

### Task E1: Add `tokio-util` + `bus` to `storage` crate

**Files:**
- Modify: `crates/storage/Cargo.toml`

`★ Insight ─────────────────────────────────────`
`crates/storage` currently has no dep on `bus` or `tokio-util` (verified). Adding `bus` creates a one-way dep `storage → bus` — fine because `bus` is L1 and `storage` is L2; the workspace dependency graph (`CLAUDE.md` "Workspace" section) flows strictly upward. We use `CancellationToken` from `tokio-util::sync` so the watcher integrates with the existing `shutdown_token` pattern in `AppCore`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Read the current manifest**

```bash
cat /Users/jayden/Projects/Klynt/bot/crates/storage/Cargo.toml
```

- [ ] **Step 2: Add deps**

In `[dependencies]`, add (alphabetical placement preferred):

```toml
bus = { path = "../bus" }
tokio-util = { workspace = true }
```

In `[dev-dependencies]`, ensure:

```toml
tempfile = { workspace = true }
```

(If `tempfile = "3"` is already present pinned locally, replace it with the workspace pin so versions stay consistent.)

- [ ] **Step 3: Build**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p storage 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/Cargo.toml
git commit -m "chore(storage): add bus + tokio-util deps for data_version watcher"
```

---

### Task E2: Implement `StoragePool::start_data_version_watcher`

**Files:**
- Modify: `crates/storage/src/pool.rs`
- Create: `crates/storage/tests/data_version_watcher.rs`

`★ Insight ─────────────────────────────────────`
SQLite's `PRAGMA data_version` returns a counter that bumps **only when a different connection commits a write** (per the SQLite docs). Within the same connection, you'll see a stale value forever. So the watcher must hold its *own* dedicated connection; we satisfy this by calling `sqlx::query_scalar("PRAGMA data_version").fetch_one(&self.0)` — sqlx's pool will hand us a fresh connection per call and our writes will go through other pool checkouts. The 5-second poll cadence is conservative: the bridge (Plan 3) handles the fast path; this catches *missed* events only.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing integration test**

Create `crates/storage/tests/data_version_watcher.rs`:

```rust
//! Phase 4 integration test: `start_data_version_watcher` fires
//! `DomainEvent::DataVersionBumped` when *another* connection mutates
//! the database.
//!
//! We can't use `connect_in_memory` here because each in-memory
//! connection gets its own private database — `PRAGMA data_version`
//! would never bump across pools. Use a `tempfile::NamedTempFile`
//! as a shared on-disk database and open two pools against it.

use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use std::time::Duration;
use storage::StoragePool;
use tokio::time::timeout;

async fn open_shared_pool(path: &std::path::Path) -> sqlx::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
        )
        .await
        .expect("open second pool")
}

#[tokio::test]
async fn watcher_fires_when_other_pool_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.db");
    // Pre-create the file so both pools open the same DB.
    std::fs::File::create(&path).unwrap();

    // Pool A: the watcher's pool.
    let pool_a = StoragePool::from_existing(open_shared_pool(&path).await);
    // Create a benign table so subsequent writes have somewhere to go.
    sqlx::query("CREATE TABLE IF NOT EXISTS t (x INTEGER)")
        .execute(pool_a.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _token = pool_a
        .start_data_version_watcher(bus.clone(), Duration::from_millis(50))
        .await;

    // Yield long enough for the watcher to read its initial baseline.
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Pool B: simulates the MCP child process writing to the same file.
    let pool_b = open_shared_pool(&path).await;
    sqlx::query("INSERT INTO t (x) VALUES (1)")
        .execute(&pool_b)
        .await
        .unwrap();

    let evt = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("watcher did not fire within 2 s")
        .expect("bus closed");
    match evt {
        DomainEvent::DataVersionBumped { previous, current } => {
            assert!(current > previous, "current ({current}) should exceed previous ({previous})");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn watcher_does_not_fire_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idle.db");
    std::fs::File::create(&path).unwrap();

    let pool = StoragePool::from_existing(open_shared_pool(&path).await);
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(pool.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _token = pool
        .start_data_version_watcher(bus.clone(), Duration::from_millis(50))
        .await;

    let res = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        res.is_err(),
        "watcher fired despite no writes: {res:?}"
    );
}

#[tokio::test]
async fn cancelling_token_stops_the_watcher() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancel.db");
    std::fs::File::create(&path).unwrap();

    let pool = StoragePool::from_existing(open_shared_pool(&path).await);
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(pool.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let token = pool
        .start_data_version_watcher(bus.clone(), Duration::from_millis(50))
        .await;
    token.cancel();
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Now write — the watcher should have already exited.
    let pool_b = open_shared_pool(&path).await;
    sqlx::query("INSERT INTO t (x) VALUES (1)")
        .execute(&pool_b)
        .await
        .unwrap();

    let res = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(res.is_err(), "watcher fired after cancel: {res:?}");
}
```

- [ ] **Step 2: Run — expect FAIL on missing method**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p storage --test data_version_watcher 2>&1 | tail -15
```

Expected: `no method named 'start_data_version_watcher' found`.

- [ ] **Step 3: Implement the watcher**

Edit `crates/storage/src/pool.rs`. Add the imports at the top of the file (under existing `use` statements):

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
```

Append a new `impl StoragePool` block at the bottom of the file (or merge into the existing one):

```rust
impl StoragePool {
    /// Spawn a background task that polls `PRAGMA data_version` at the given
    /// interval. When the version changes between ticks (i.e. a different
    /// connection wrote since we last looked), publish
    /// `DomainEvent::DataVersionBumped` on `bus`. Returns a
    /// `CancellationToken` that the caller stores to keep the watcher alive
    /// and to stop it on shutdown.
    ///
    /// IMPORTANT: SQLite's `PRAGMA data_version` only advances when *other*
    /// connections commit. Holding a long-lived borrow on a single connection
    /// would mask all updates — that's why we use `fetch_one(&self.0)`
    /// (a sqlx pool) on every tick, which checks out a fresh connection.
    pub async fn start_data_version_watcher(
        &self,
        bus: Arc<bus::DomainEventBus>,
        interval: Duration,
    ) -> CancellationToken {
        let token = CancellationToken::new();
        let token_child = token.clone();
        let pool = self.clone();
        tokio::spawn(async move {
            let mut last = match read_data_version(&pool).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("data_version_watcher: initial read failed: {e}");
                    return;
                }
            };
            let mut ticker = tokio::time::interval(interval);
            // Skip the immediate first tick — interval fires once at start.
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = token_child.cancelled() => {
                        tracing::debug!("data_version_watcher: cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        match read_data_version(&pool).await {
                            Ok(current) if current != last => {
                                bus.publish(bus::DomainEvent::DataVersionBumped {
                                    previous: last,
                                    current,
                                });
                                last = current;
                            }
                            Ok(_) => {} // no change
                            Err(e) => {
                                tracing::warn!("data_version_watcher: read failed: {e}");
                            }
                        }
                    }
                }
            }
        });
        token
    }
}

async fn read_data_version(pool: &StoragePool) -> sqlx::Result<u32> {
    // `PRAGMA data_version` returns one row, one column (an i64).
    let v: i64 = sqlx::query_scalar("PRAGMA data_version")
        .fetch_one(pool.inner())
        .await?;
    Ok(v as u32)
}
```

- [ ] **Step 4: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p storage --test data_version_watcher 2>&1 | tail -15
```

Expected: 3 passing tests within ~3 seconds total.

- [ ] **Step 5: Re-run the full storage test suite**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p storage 2>&1 | tail -10
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/pool.rs crates/storage/tests/data_version_watcher.rs
git commit -m "feat(storage): add start_data_version_watcher polling fallback"
```

---

### Task E3: Wire the watcher into desktop boot

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/desktop/src/app_core.rs`

- [ ] **Step 1: Add the field on `AppCore`**

Edit `crates/app-core/src/state.rs`. Find the existing `_config_watcher_token` field at line 149:

```rust
    pub _config_watcher_token: Option<CancellationToken>,
```

Insert immediately below:

```rust
    /// Phase 4 polling fallback. Held forever so the watcher runs for the
    /// process lifetime; cancelled implicitly on `AppCore` drop.
    pub _data_version_watcher_token: Option<CancellationToken>,
```

Find the `AppCore { ... }` constructor literal (search for `_config_watcher_token: None,`) and add the matching field initializer:

```rust
            _data_version_watcher_token: None,
```

- [ ] **Step 2: Verify the workspace still compiles**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build --workspace 2>&1 | tail -10
```

Expected: clean (no constructor sites need updating because the field is `Option` initialized to `None`).

- [ ] **Step 3: Start the watcher from desktop boot**

Edit `crates/desktop/src/app_core.rs`. Find the BridgeServer block (~line 74) — the `match mcp_bridge::BridgeServer::start(...)` from Plan 3 Phase F1. Insert immediately *after* the closing `}` of that match block (still inside `init`):

```rust
    // Phase 4: PRAGMA data_version polling fallback. Catches writes that
    // bypassed the bridge (e.g. a CLI mutation, or the MCP child running
    // with the bridge socket unreachable). 5s cadence is conservative —
    // this is a safety net, not a primary signal.
    let dv_token = core.storage_pool
        .start_data_version_watcher(
            channels.domain_event_bus.clone(),
            std::time::Duration::from_secs(5),
        )
        .await;
    if DATA_VERSION_WATCHER.set(dv_token).is_err() {
        tracing::warn!("data_version_watcher: already initialized");
    }
```

Add a sibling `OnceLock` near the existing `BRIDGE_SERVER` declaration at line 22:

```rust
static DATA_VERSION_WATCHER: OnceLock<tokio_util::sync::CancellationToken> = OnceLock::new();
```

If `tokio_util` isn't already imported in this file (check with `grep tokio_util crates/desktop/src/app_core.rs`), the fully-qualified path above is fine — no new `use` needed.

- [ ] **Step 4: Verify `core.storage_pool` is accessible at the insertion site**

The BridgeServer block is inside `init()` where `core: AppCore` exists; `core.storage_pool` is `pub` (verified at `state.rs:43`). Sanity-check:

```bash
sed -n '60,95p' /Users/jayden/Projects/Klynt/bot/crates/desktop/src/app_core.rs
```

Expected: `let (core, channels) = AppCore::init_with_sender(...)?;` precedes the BridgeServer block, so both `core.storage_pool` and `channels.domain_event_bus` are in scope.

- [ ] **Step 5: Build**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p desktop 2>&1 | tail -15
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/state.rs crates/desktop/src/app_core.rs
git commit -m "feat(desktop): start PRAGMA data_version watcher (5s poll) during init"
```

---

## Phase F — Frontend bridge updates

### Task F1: Add coding-memory keys, kinds, and bridge routes

**Files:**
- Modify: `desktop-ui/src/lib/query/entityKindMap.ts`
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tauriEventBridge.ts`
- Modify: `desktop-ui/src/lib/query/tests/entityKindMap.test.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`
- Modify: `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`

`★ Insight ─────────────────────────────────────`
The recall UI surface is exposed through Tauri commands prefixed `coding_memory_*` (verified — there's no dedicated React feature folder yet, but the commands in `app-core/src/coding_memory/` are the canonical surface). So `entityKindMap.ts`'s prefix lookup gets one entry — `["coding_memory_", "codingFact"]` — that defaults the "kind" for any future mutation through `useTauriMutation`. Sub-kind disambiguation (`fact` vs `episode`) only matters at the *event* layer, where the backend already distinguishes them; both invalidate the same `qk.codingMemory.all()` key, so a single broad bucket is enough for cache correctness.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend `entityKindMap.ts` — failing test first**

Edit `desktop-ui/src/lib/query/tests/entityKindMap.test.ts`. Add to the existing `describe`:

```ts
it.each<[string, EntityKind | null]>([
    ["coding_memory_recall_fetch", "codingFact"],
    ["coding_memory_distill_now", "codingFact"],
])("maps coding-memory commands to codingFact: %s", (cmd, kind) => {
    expect(entityKindForCommand(cmd)).toBe(kind);
});
```

Run:

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query/tests/entityKindMap.test.ts
```

Expected: FAIL — `"codingFact"` is not a valid `EntityKind`.

- [ ] **Step 2: Extend the union + table**

Edit `desktop-ui/src/lib/query/entityKindMap.ts`. Replace the union (lines 4–19):

```ts
export type EntityKind =
    | "task"
    | "project"
    | "objective"
    | "area"
    | "keyResult"
    | "focusSession"
    | "productivity"
    | "note"
    | "notebook"
    | "finance"
    | "source"
    | "conversation"
    | "mirrorSnippet"
    | "brainVersion"
    | "pendingMemory"
    | "codingFact"
    | "codingEpisode";
```

In `PREFIX_TABLE`, add the entry (single command prefix collapses both fact + episode mutations into the broad `codingFact` bucket — sub-kind isn't observable at the call site):

```ts
const PREFIX_TABLE: ReadonlyArray<readonly [string, EntityKind]> = [
    ["notebook_", "notebook"],
    ["note_", "note"],
    ["task_", "task"],
    ["project_", "project"],
    ["objective_", "objective"],
    ["area_", "area"],
    ["key_result_", "keyResult"],
    ["focus_", "focusSession"],
    ["productivity_", "productivity"],
    ["finance_", "finance"],
    ["source_", "source"],
    ["conversation_", "conversation"],
    ["coding_memory_", "codingFact"],
];
```

- [ ] **Step 3: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query/tests/entityKindMap.test.ts
```

Expected: all tests pass.

- [ ] **Step 4: Extend `queryKeys.ts` — failing test first**

Edit `desktop-ui/src/lib/query/tests/queryKeys.test.ts`. Append:

```ts
describe("codingMemory keys", () => {
    it("all is the root", () => {
        expect(qk.codingMemory.all()).toEqual(["codingMemory"]);
    });
    it("facts / episodes / recallIndex / memoryBrowser / status are stable", () => {
        expect(qk.codingMemory.facts()).toEqual(["codingMemory", "facts"]);
        expect(qk.codingMemory.episodes()).toEqual(["codingMemory", "episodes"]);
        expect(qk.codingMemory.recallIndex()).toEqual(["codingMemory", "recallIndex"]);
        expect(qk.codingMemory.memoryBrowser()).toEqual(["codingMemory", "memoryBrowser"]);
        expect(qk.codingMemory.status()).toEqual(["codingMemory", "status"]);
    });
});
```

Run:

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: FAIL.

- [ ] **Step 5: Add the namespace**

Edit `desktop-ui/src/lib/query/queryKeys.ts`. Add inside the `qk` literal (alphabetical placement; pick a spot near other domain namespaces):

```ts
    codingMemory: {
        all: () => ["codingMemory"] as const,
        facts: () => ["codingMemory", "facts"] as const,
        episodes: () => ["codingMemory", "episodes"] as const,
        recallIndex: () => ["codingMemory", "recallIndex"] as const,
        memoryBrowser: () => ["codingMemory", "memoryBrowser"] as const,
        status: () => ["codingMemory", "status"] as const,
    },
```

Run again — green:

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

- [ ] **Step 6: Extend `tauriEventBridge.ts` — failing tests first**

Edit `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`. Append:

```ts
describe("coding memory + data_version", () => {
    it("entity:updated{kind:'codingFact'} invalidates codingMemory.all()", async () => {
        const client = new QueryClient();
        const spy = vi.spyOn(client, "invalidateQueries");
        const { listen, fire } = fakeListenFactory();

        const stop = await startTauriEventBridge(client, listen);
        fire("entity:updated", { entityKind: "codingFact", id: "fact-1" });

        expect(spy).toHaveBeenCalledWith({ queryKey: qk.codingMemory.all() });
        stop();
    });

    it("entity:updated{kind:'codingEpisode'} invalidates codingMemory.all()", async () => {
        const client = new QueryClient();
        const spy = vi.spyOn(client, "invalidateQueries");
        const { listen, fire } = fakeListenFactory();

        const stop = await startTauriEventBridge(client, listen);
        fire("entity:updated", { entityKind: "codingEpisode", id: "ep-1" });

        expect(spy).toHaveBeenCalledWith({ queryKey: qk.codingMemory.all() });
        stop();
    });

    it("data:version_bumped triggers a broad invalidate (no key prefix)", async () => {
        const client = new QueryClient();
        // Seed two unrelated queries so we can prove BOTH refetch.
        client.setQueryData(qk.tasks.today(), [{ id: "t1" }]);
        client.setQueryData(qk.codingMemory.facts(), [{ id: "f1" }]);
        const spy = vi.spyOn(client, "invalidateQueries");
        const { listen, fire } = fakeListenFactory();

        const stop = await startTauriEventBridge(client, listen);
        fire("data:version_bumped", { previous: 41, current: 42 });

        // Broad invalidate: called with no queryKey filter.
        expect(spy).toHaveBeenCalledWith();
        stop();
    });
});
```

Run:

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: FAIL on all three (codingFact/codingEpisode unmapped; `data:version_bumped` not subscribed).

- [ ] **Step 7: Add the routes + special-case handler**

Edit `desktop-ui/src/lib/query/tauriEventBridge.ts`. Update `ENTITY_INVALIDATIONS` (lines 16–32):

```ts
const ENTITY_INVALIDATIONS: Record<EntityKind, QueryKey[]> = {
    task: [qk.tasks.all()],
    project: [qk.tasks.all()],
    objective: [],
    area: [],
    keyResult: [],
    focusSession: [qk.focus.todaySessions(), qk.focus.status()],
    productivity: [],
    note: [],
    notebook: [],
    finance: [],
    source: [],
    conversation: [],
    mirrorSnippet: [],
    brainVersion: [],
    pendingMemory: [],
    codingFact: [qk.codingMemory.all()],
    codingEpisode: [qk.codingMemory.all()],
};
```

Add a new event subscription *after* the existing `STATIC_ROUTES` `for` loop (find the section that pushes `unlisteners` for `STATIC_ROUTES`). Insert immediately after that loop:

```ts
    // Phase 4 broad-invalidate fallback. Fired by the desktop's
    // `start_data_version_watcher` when a foreign connection wrote and
    // we never saw the matching `entity:updated`. Invalidating with no
    // query-key filter matches every query in the cache — refetches are
    // de-duped by TanStack so the cost is one network round-trip per
    // distinct query, not per cached entry.
    const offBroad = await listen("data:version_bumped", () => {
        client.invalidateQueries();
    });
    unlisteners.push(offBroad);
```

Update the `ALL_EVENTS` debug constant to include the new event name:

```ts
const ALL_EVENTS = [
    "entity:updated",
    "data:version_bumped",
    ...STATIC_ROUTES.map(([n]) => n),
];
```

- [ ] **Step 8: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test src/lib/query
```

Expected: all `lib/query` tests pass (the original Plan 1 suite + the 5 new tests added in this task).

- [ ] **Step 9: Typecheck**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bunx tsc --noEmit 2>&1 | tail -10 && echo "---DONE---"
```

Expected: only `---DONE---` (no errors).

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/lib/query
git commit -m "feat(desktop-ui): add coding-memory query keys + data_version broad-invalidate route"
```

---

## Phase G — End-to-end verification

### Task G1: Workspace-wide test + lint sweep

**Files:** none (verification only).

- [ ] **Step 1: Full Rust test suite**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run --workspace 2>&1 | tail -20
```

Expected: all green. New tests: 5 in `bus`, 3 in `desktop-shared`, 3 in `coding-memory --test distiller_events`, 3 in `storage --test data_version_watcher` = 14 added tests, plus everything from Plans 1–3 still passes.

- [ ] **Step 2: Doctests**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo test --workspace --doc 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 3: Clippy (zero-warning policy)**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20
```

Expected: no new warnings. The `desktop` crate has pre-existing exceptions (per `CLAUDE.md`) — only fix warnings introduced by this plan.

- [ ] **Step 4: Format**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo fmt --all --check 2>&1 | tail -5
```

If output is non-empty, run `cargo fmt --all` then re-stage and amend the most recent commit.

- [ ] **Step 5: Frontend lint + typecheck + tests**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run lint && bun run typecheck && bun run test
```

Expected: all green.

---

### Task G2: Manual cross-process verification — happy path

**Files:** none.

`★ Insight ─────────────────────────────────────`
The two new signals are observable at completely different timescales. `CodingMemoryUpdated` fires immediately (microseconds) when the Distiller finishes a turn, then rides Plan 3's bridge socket → Tauri global → bridge handler → cache invalidation in ~10–50 ms. `DataVersionBumped` is a 5-second poll, so the worst-case latency for the broad-invalidate fallback is ~5 seconds. Both should be observable in the React Query devtools.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Start the desktop in dev mode**

Terminal 1:
```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run dev
```
Terminal 2:
```bash
cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev
```

In stderr, you should see two new lines (in addition to Plan 3's `mcp-bridge: listening at ...`):
- `data_version_watcher: cancelled` will NOT appear (that's only on shutdown).
- A baseline `PRAGMA data_version` read happens silently — no log.

- [ ] **Step 2: Open the React Query devtools in any window**

Click the floating "TanStack" button. Confirm `["codingMemory"]`-prefixed queries are *not yet present* (no UI is fetching them yet — the queries exist in the key factory but no panel uses them).

For this manual test, open the browser console and seed a query so we have something observable:

```js
// In any webview's devtools console:
window.__klyntbot_test_seed = () => {
    const q = window.__TANSTACK_QUERY_CLIENT__;
    if (!q) console.warn("No client on window — devtools-inspect a real query instead");
    else q.setQueryData(["codingMemory", "facts"], [{ id: "seed", text: "manual test" }]);
};
window.__klyntbot_test_seed();
```

(If the QueryClient isn't exposed on `window`, skip this step — the devtools panel will show the query the moment any panel mounts that uses `useTauriQuery({ queryKey: qk.codingMemory.facts(), ... })`.)

- [ ] **Step 3: Trigger a Distiller pass via the CLI**

The Distiller runs automatically when an MCP turn completes. To force one synchronously, call the existing `coding_memory_distill_now` Tauri command from the desktop's debug surface (menu → Debug → Coding Memory → "Distill now"), or via the CLI:

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo run -p desktop -- mcp serve --stdio <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory","arguments":{"action":"distill_now"}}}
EOF
```

(If the `memory` tool doesn't expose a `distill_now` action, use the existing `coding_memory_distill_now` Tauri command from the desktop debug menu.)

- [ ] **Step 4: Observe in devtools**

Within ~50 ms of the distill, the React Query devtools should show `["codingMemory"]`-prefixed queries flip to "stale" → "fetching" → "fresh". If a recall panel is open, its content updates without manual refresh.

- [ ] **Step 5: No commit needed (manual)**

---

### Task G3: Manual cross-process verification — `data_version` fallback path

**Files:** none.

The fallback path requires that the bridge *not* deliver an event (otherwise the fast path wins). Two ways to simulate:

- [ ] **Step 1: Stop the bridge socket without restarting the desktop**

Easier alternative: with the desktop running, mutate the DB through a *different* binary path that doesn't go through `AppEventEmitter` at all — a CLI tool or a direct sqlite3 INSERT.

```bash
sqlite3 "${KLYNTBOT_HOME:-$HOME/.klyntbot}/data.db" "INSERT INTO tasks (id, title, completed, created_at, updated_at) VALUES ('manual-test-$(date +%s)', 'data_version smoke test', 0, datetime('now'), datetime('now'));"
```

(Adjust column names to the actual `tasks` table schema if needed — query `PRAGMA table_info(tasks);` first.)

- [ ] **Step 2: Watch the desktop logs and devtools**

Within 5 seconds (one full poll interval), the desktop stderr should show no warnings, and the React Query devtools should briefly mark **every** query as stale — that's the broad-invalidate firing. The new task should appear in the tray + main window.

- [ ] **Step 3: Confirm the event in the debug dashboard**

If the cognitive debug dashboard is enabled, the `cognitive:domain_event` stream should show a `DataVersionBumped` event with `previous` and `current` values.

- [ ] **Step 4: Verify clean shutdown**

Cmd+Q the Tauri window. The `OnceLock<CancellationToken>` for the watcher won't fire `cancel()` explicitly (it's a process-lifetime token), but the spawned task ends when the runtime drops. Confirm the next `cargo tauri dev` start doesn't complain about a stuck task.

- [ ] **Step 5: No commit needed (manual)**

---

## Self-Review

(Run after writing the plan. Findings reported below; fixes applied inline before publishing.)

**1. Spec coverage:**

| Goal | Tasks | Status |
|---|---|---|
| `DomainEvent::CodingMemoryUpdated` + `DataVersionBumped` variants | A1 | ✓ |
| `EntityKind::CodingFact` + `CodingEpisode` | A2 | ✓ |
| `Distiller` holds `Arc<DomainEventBus>` | B1 | ✓ |
| Distiller publishes after each successful write | C1 | ✓ |
| Desktop forwarder maps both new variants to Tauri events | D1 | ✓ |
| `StoragePool::start_data_version_watcher` | E1, E2 | ✓ |
| Watcher spawned during desktop boot | E3 | ✓ |
| Frontend `entityKindMap`, `queryKeys`, `tauriEventBridge` updated | F1 | ✓ |
| `data:version_bumped` triggers broad invalidate | F1 step 7 | ✓ |
| Cross-process e2e tests (Rust unit + integration) | A1, A2, C1, E2 | ✓ |
| Manual end-to-end (happy path + fallback) | G2, G3 | ✓ |

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" / "add error handling later" / "verify against actual code" patterns. Each test fixture has a fallback path documented (e.g. "if `ProviderManager::for_tests()` doesn't exist, search existing tests for the pattern"). The `harness::build` test scaffold is concrete; the only deferred decision is reusing an existing harness module if one is found, which is good practice not laziness.

**3. Type consistency:**
- `CodingMemoryKind::{Fact, Episode}` (Rust) ↔ `"fact"` / `"episode"` on the wire (snake_case serde, A1) ↔ mapped in the desktop forwarder D1 to `EntityKind::{CodingFact, CodingEpisode}` ↔ serializes as `"codingFact"` / `"codingEpisode"` (camelCase serde, A2) ↔ read by the FE bridge handler in F1 step 7. Four-layer chain, all matched.
- `qk.codingMemory.all()` produces `["codingMemory"]` (queryKeys.ts F1 step 5) ↔ asserted in tests F1 step 6 ↔ used in `ENTITY_INVALIDATIONS` F1 step 7. Consistent.
- `DataVersionBumped { previous: u32, current: u32 }` (A1) ↔ forwarder emits `{ "previous": 41, "current": 42 }` (D1) ↔ FE handler uses no payload (F1 step 7). Backend payload exists for debugging visibility only — FE intentionally ignores it.
- `start_data_version_watcher(&self, bus: Arc<bus::DomainEventBus>, interval: Duration) -> CancellationToken` defined in E2 ↔ called identically in E3 step 3.
- `with_event_bus(self, bus: Arc<bus::DomainEventBus>) -> Self` defined in B1 step 3 ↔ called in B1 step 5 wiring.

**4. Pattern adherence (per `CLAUDE.md`):**
- Conventional commits: every step uses `feat(scope):` / `chore(scope):` / `test(scope):`. ✓
- Tests use ephemeral resources: `tempfile::tempdir()`, `connect_in_memory()`. ✓
- Surgical changes: no edits to unrelated subsystems; the bus enum gets two strictly additive variants; `EntityKind` gets two strictly additive variants; the Distiller's existing API is preserved (the new `with_event_bus` is opt-in). ✓
- Dependency inversion: the watcher takes a `bus: Arc<DomainEventBus>` rather than reaching into AppCore for it. ✓
- Errors: watcher logs `tracing::warn!` and continues — no panic on transient sqlx errors. ✓
- Pre-release migration policy: this plan adds no new SQL tables, so no migration considerations. ✓
- Tauri command coverage: this plan adds no new `#[tauri::command]` functions, so no `DEV_COMMANDS` updates required. ✓

---

## Out-of-scope notes

- A dedicated React **coding-memory recall panel** that *uses* `qk.codingMemory.*` is not in this plan. The keys exist for consumers; the consumers themselves (a `<RecallPanel/>` using `useTauriQuery({ queryKey: qk.codingMemory.recallIndex(), command: "coding_memory_recall_index" })`) are a UI task to be tracked separately. Without a consumer, the keys still serve as the single source of truth so future panels don't drift.
- **Two-way bridge** (desktop → MCP child push) — same out-of-scope note as Plan 3.
- **Smarter polling** (e.g. exponential back-off when the desktop is idle, or pause when no webviews are open) — premature optimization; 5-second polls cost a single sqlite read per tick.
- **Per-table `data_version`** (SQLite has no per-table version; the global counter forces broad invalidation). If a future need emerges to invalidate only specific keys on fallback, we'd need to enrich the watcher with a journaled write-log table — explicitly out of scope.

---

## Definition of Done (Plan 4)

- `cargo build --workspace` clean, zero new warnings.
- `cargo nextest run --workspace` green: 14 new tests added (5 bus + 3 desktop-shared + 3 coding-memory + 3 storage). All Plan 1–3 tests still pass.
- `cargo test --workspace --doc` clean.
- `cargo clippy --workspace --all-targets --all-features` zero new warnings.
- `cargo fmt --all --check` clean.
- `cd desktop-ui && bun run lint && bun run typecheck && bun run test` green; 5 new FE tests pass.
- Manual G2: Distiller pass updates a coding-memory recall query in <100 ms.
- Manual G3: a sqlite3 CLI write reaches every webview within 5 seconds via the broad-invalidate fallback.
- All commits on the working branch use conventional-commit format and reference the right scope.

---

## End of master plan

This is the last of four plans. **Combined deliverables across Plans 1–4:**

- `desktop-ui/src/lib/query/` — full TanStack Query foundation with cross-window event bridge (Plan 1).
- All FE features migrated off `useState + invoke` to typed cache (Plans 1–2).
- `crates/mcp-bridge/` — cross-process Unix-socket bridge so MCP child events reach the desktop in <50 ms (Plan 3).
- Distiller publishing real-time `CodingMemoryUpdated` events; storage-layer `PRAGMA data_version` polling fallback for missed cross-process writes (Plan 4).
- React Query devtools available in every webview, single source of truth for query keys + entity kinds + event routes.
- **Net result:** every mutation source — desktop UI, in-process automation, MCP child via bridge, MCP child or CLI via fallback polling, hook CLI distillation — propagates to every webview without manual refresh.
