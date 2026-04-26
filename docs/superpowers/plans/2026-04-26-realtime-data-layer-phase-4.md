# Real-time Data Layer Phase 4 — Distiller Domain Events + `data_version` Polling Fallback

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close two remaining gaps in real-time invalidation:
1. The coding-memory **Distiller** writes to `episodic_memories` and `semantic_facts` SQLite tables but never publishes a `DomainEvent`. After this plan, every distillation pass fires `DomainEvent::CodingMemoryUpdated`, which the desktop forwards as a Tauri event so any "recall" UI panel auto-refreshes.
2. **Last-resort fallback for missed cross-process events.** Add a low-frequency polling task in the desktop that compares `PRAGMA data_version` between ticks; if it bumps unexpectedly (i.e., another connection wrote without the bridge firing), broadcast a generic `entity:updated` to invalidate everything. This catches edge cases like the desktop launching mid-MCP-session, or the bridge socket going stale.

**Architecture:** Domain-event addition: extend `DomainEvent` with `CodingMemoryUpdated { kind: "fact" | "episode", id }`; the existing `app_core.rs:321` forwarder serializes it to a Tauri `entity:updated` event with a new `EntityKind::CodingFact` / `CodingEpisode`. Polling fallback: a tokio task in `StoragePool::start_data_version_watcher` polls `PRAGMA data_version` at 5-second intervals; on any unexpected delta, publishes a generic `DomainEvent::DataVersionBumped` (which the FE bridge converts to a broad invalidation).

**Tech Stack:** Rust, existing `bus::DomainEvent`, existing `crates/coding-memory/src/distiller`, `crates/storage/src/pool.rs`, FE `tauriEventBridge.ts`.

**Master plan context:** Plan 4 of 4. Depends on Plan 3 (the existing `entity:updated` forwarding path is what these new events ride on). Independent of Plan 2.

---

## File Structure

### Files to modify

| Path | Change |
|---|---|
| `crates/bus/src/lib.rs` | Add `DomainEvent::CodingMemoryUpdated` and `DomainEvent::DataVersionBumped` variants. |
| `crates/desktop-shared/src/types.rs` | Add `EntityKind::CodingFact` and `EntityKind::CodingEpisode`. |
| `crates/coding-memory/src/distiller/mod.rs` | Publish `CodingMemoryUpdated` after each successful distill_turn write. |
| `crates/desktop/src/app_core.rs` (forwarder ~line 303) | Map new variants to `entity:updated` Tauri payloads. |
| `crates/storage/src/pool.rs` | New `start_data_version_watcher` method; spawned by desktop boot. |
| `crates/desktop/src/app_core.rs` (boot) | Call `pool.start_data_version_watcher(bus.clone())`. |
| `desktop-ui/src/lib/query/entityKindMap.ts` | Add `codingFact`, `codingEpisode` kinds. |
| `desktop-ui/src/lib/query/queryKeys.ts` | Add `qk.codingMemory.facts()` / `episodes()`. |
| `desktop-ui/src/lib/query/tauriEventBridge.ts` | Add ENTITY_INVALIDATIONS routes for the new kinds + a "broad invalidate" route for `data_version_bumped`. |

### New files

| Path | Responsibility |
|---|---|
| `crates/coding-memory/src/distiller/tests/distillation_events.rs` | Verifies `CodingMemoryUpdated` is published after a distillation pass. |
| `crates/storage/src/tests/data_version_watcher.rs` | Verifies the watcher fires `DataVersionBumped` when another connection writes. |

---

## Phase A — Domain event additions

### Task A1: Add `DomainEvent::CodingMemoryUpdated` and `DataVersionBumped`

**Files:**
- Modify: `crates/bus/src/lib.rs`

- [ ] **Step 1: Inspect the enum**

```bash
grep -nE "pub enum DomainEvent|EntityUpdated" /Users/jayden/Projects/Klynt/bot/crates/bus/src/*.rs | head -20
```

- [ ] **Step 2: Add variants**

In `bus/src/lib.rs`, add to `DomainEvent`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainEvent {
    // ... existing
    EntityUpdated { entity_kind: String, id: String },
    CodingMemoryUpdated {
        kind: CodingMemoryKind,
        id: String,
    },
    DataVersionBumped { previous: u32, current: u32 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingMemoryKind {
    Fact,
    Episode,
}
```

Update `domain()` and `variant_name()` to handle the new variants.

- [ ] **Step 3: Run bus tests**

```bash
cargo nextest run -p bus 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src
git commit -m "feat(bus): add CodingMemoryUpdated + DataVersionBumped DomainEvent variants"
```

### Task A2: Extend `EntityKind` in `desktop-shared`

**Files:**
- Modify: `crates/desktop-shared/src/types.rs`

- [ ] **Step 1: Add variants**

```rust
pub enum EntityKind {
    // existing
    CodingFact,
    CodingEpisode,
}
```

Update the `parse` impl:

```rust
"coding_fact" | "codingfact" => Some(Self::CodingFact),
"coding_episode" | "codingepisode" => Some(Self::CodingEpisode),
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p desktop-shared 2>&1 | tail -5
git add crates/desktop-shared/src/types.rs
git commit -m "feat(desktop-shared): EntityKind::CodingFact + CodingEpisode"
```

---

## Phase B — Distiller emission

### Task B1: Publish `CodingMemoryUpdated` after distillation

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`

The Distiller writes to `episodic_memories` and `semantic_facts` inside `distill_turn`. Add a `bus.publish` call after each successful row insertion.

- [ ] **Step 1: Locate the insertion sites**

```bash
grep -nE "INSERT INTO episodic_memories|INSERT INTO semantic_facts|insert_fact|insert_episode" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/distiller
```

- [ ] **Step 2: Verify the Distiller has access to the bus**

If the Distiller struct already holds an `Arc<DomainEventBus>`, great. If not, plumb it through (constructor + caller in `app-core/src/init/mod.rs:1148`).

- [ ] **Step 3: Write a failing test**

Create `crates/coding-memory/src/distiller/tests/distillation_events.rs`:

```rust
use bus::{CodingMemoryKind, DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

// Pseudo-test scaffold: replace with the project's actual distiller harness.
#[tokio::test]
async fn distill_turn_publishes_coding_memory_updated() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let distiller = make_test_distiller(bus.clone()).await;

    distiller.accept_event(make_test_event()).await;
    distiller.flush_turn().await;

    let evt = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("closed");
    matches!(
        evt,
        DomainEvent::CodingMemoryUpdated {
            kind: CodingMemoryKind::Fact | CodingMemoryKind::Episode,
            ..
        }
    );
}
```

(Use whatever harness already exists; if none, write a minimal one with in-memory SQLite.)

- [ ] **Step 4: Run — fails because publish call doesn't exist**

```bash
cargo nextest run -p coding-memory --test distillation_events 2>&1 | tail -10
```

- [ ] **Step 5: Wire the publish**

After each `INSERT INTO episodic_memories` success, add:

```rust
self.bus.publish(DomainEvent::CodingMemoryUpdated {
    kind: bus::CodingMemoryKind::Episode,
    id: episode_id.to_string(),
});
```

After each `INSERT INTO semantic_facts` success, add:

```rust
self.bus.publish(DomainEvent::CodingMemoryUpdated {
    kind: bus::CodingMemoryKind::Fact,
    id: fact_id.to_string(),
});
```

- [ ] **Step 6: Run — green**

```bash
cargo nextest run -p coding-memory --test distillation_events 2>&1 | tail -10
```

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/distiller
git commit -m "feat(coding-memory): publish CodingMemoryUpdated after distill_turn writes"
```

---

## Phase C — Forwarder routing

### Task C1: Forward `CodingMemoryUpdated` and `DataVersionBumped` as Tauri events

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

In the existing forwarder (~line 303), extend the `match` on `DomainEvent` to include the new variants.

- [ ] **Step 1: Add match arms**

```rust
match &event {
    DomainEvent::EntityUpdated { entity_kind, id } => {
        // (existing — added in Plan 3 Phase F1)
    }
    DomainEvent::CodingMemoryUpdated { kind, id } => {
        let entity_kind = match kind {
            bus::CodingMemoryKind::Fact => common::types::EntityKind::CodingFact,
            bus::CodingMemoryKind::Episode => common::types::EntityKind::CodingEpisode,
        };
        let payload = desktop_shared::events::EntityUpdatedPayload {
            entity_kind,
            id: id.clone(),
        };
        let _ = app_handle_clone.emit(
            desktop_shared::events::ENTITY_UPDATED,
            &payload,
        );
    }
    DomainEvent::DataVersionBumped { previous, current } => {
        // Broad invalidation — fire entity:updated with a synthetic
        // "unknown" kind that the FE bridge maps to "invalidate everything".
        let _ = app_handle_clone.emit(
            "entity:updated",
            &serde_json::json!({
                "entityKind": "all",
                "id": format!("data_version:{}->{}", previous, current),
            }),
        );
    }
    _ => {}
}
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p desktop 2>&1 | tail -5
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): forward CodingMemoryUpdated + DataVersionBumped as Tauri events"
```

---

## Phase D — `PRAGMA data_version` polling fallback

### Task D1: `StoragePool::start_data_version_watcher`

**Files:**
- Modify: `crates/storage/src/pool.rs`

`PRAGMA data_version` returns a counter that increments on each write committed by **another connection**. If we open one watcher connection and poll it, it'll detect MCP child writes (different process, different connection pool) without polling the actual data.

- [ ] **Step 1: Write a failing test**

Create `crates/storage/src/tests/data_version_watcher.rs`:

```rust
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn watcher_fires_when_other_connection_writes() {
    // Open two pools to the same in-memory DB
    let pool_a = StoragePool::connect_in_memory().await.unwrap();
    let pool_b = pool_a.clone(); // shared sqlite

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _watcher = pool_a
        .start_data_version_watcher(bus.clone(), Duration::from_millis(50))
        .await;

    // Bump the counter via a different "connection" (sqlx pool grabs a fresh one per op)
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(pool_b.as_ref())
        .await
        .unwrap();
    sqlx::query("INSERT INTO t VALUES (1)")
        .execute(pool_b.as_ref())
        .await
        .unwrap();

    let evt = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("closed");
    matches!(evt, DomainEvent::DataVersionBumped { .. });
}
```

- [ ] **Step 2: Run — fails**

```bash
cargo nextest run -p storage --test data_version_watcher 2>&1 | tail -10
```

- [ ] **Step 3: Implement watcher**

In `crates/storage/src/pool.rs`:

```rust
impl StoragePool {
    pub async fn start_data_version_watcher(
        &self,
        bus: Arc<bus::DomainEventBus>,
        interval: std::time::Duration,
    ) -> tokio_util::sync::CancellationToken {
        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel_child = cancel.clone();
        let pool = self.clone();
        tokio::spawn(async move {
            let mut last: u32 = read_data_version(&pool).await.unwrap_or(0);
            let mut ticker = tokio::time::interval(interval);
            // Skip the immediate fire after start
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = cancel_child.cancelled() => break,
                    _ = ticker.tick() => {
                        let current = match read_data_version(&pool).await {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!("data_version: {e}");
                                continue;
                            }
                        };
                        if current != last {
                            bus.publish(bus::DomainEvent::DataVersionBumped {
                                previous: last,
                                current,
                            });
                            last = current;
                        }
                    }
                }
            }
        });
        cancel
    }
}

async fn read_data_version(pool: &StoragePool) -> sqlx::Result<u32> {
    let row: (i64,) = sqlx::query_as("PRAGMA data_version")
        .fetch_one(pool.as_ref())
        .await?;
    Ok(row.0 as u32)
}
```

- [ ] **Step 4: Run — green**

```bash
cargo nextest run -p storage --test data_version_watcher 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src
git commit -m "feat(storage): start_data_version_watcher polling fallback"
```

### Task D2: Spawn the watcher from desktop boot

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

- [ ] **Step 1: Wire it**

After `BridgeServer::start` in `app_core.rs`:

```rust
let _data_version_token = pool
    .start_data_version_watcher(
        channels.domain_event_bus.clone(),
        std::time::Duration::from_secs(5),
    )
    .await;
self.data_version_watcher_token = Some(_data_version_token);
```

Add a corresponding field on `AppCore`.

- [ ] **Step 2: Build + commit**

```bash
cargo build -p desktop 2>&1 | tail -5
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): start PRAGMA data_version watcher (5s poll)"
```

---

## Phase E — FE bridge updates

### Task E1: Add coding-memory keys + entity kinds

**Files:**
- Modify: `desktop-ui/src/lib/query/entityKindMap.ts`
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tauriEventBridge.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`
- Modify: `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`

- [ ] **Step 1: Extend tests**

In `tests/queryKeys.test.ts`:

```ts
describe("coding memory keys", () => {
	it("codingMemory.facts / episodes", () => {
		expect(qk.codingMemory.facts()).toEqual(["codingMemory", "facts"]);
		expect(qk.codingMemory.episodes()).toEqual(["codingMemory", "episodes"]);
	});
});
```

In `tests/tauriEventBridge.test.ts`:

```ts
it("entity:updated{kind:'codingFact'} invalidates codingMemory.facts", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("entity:updated", { entityKind: "codingFact", id: "f1" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.codingMemory.all() });
	stop();
});

it("entity:updated{kind:'all'} invalidates everything (data_version fallback)", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("entity:updated", { entityKind: "all", id: "data_version:0->1" });
	// "all" fires invalidateQueries with no key prefix → match all queries
	expect(spy).toHaveBeenCalledWith({ queryKey: undefined as any });
	stop();
});
```

- [ ] **Step 2: Extend `entityKindMap.ts`**

Add:

```ts
export type EntityKind =
	// ... existing
	| "codingFact"
	| "codingEpisode"
	| "all"; // data_version fallback marker

const PREFIX_TABLE: ReadonlyArray<readonly [string, EntityKind]> = [
	// ... existing
	["coding_fact_", "codingFact"],
	["coding_episode_", "codingEpisode"],
];
```

- [ ] **Step 3: Extend `queryKeys.ts`**

Add:

```ts
codingMemory: {
	all: () => ["codingMemory"] as const,
	facts: () => ["codingMemory", "facts"] as const,
	episodes: () => ["codingMemory", "episodes"] as const,
},
```

- [ ] **Step 4: Extend `tauriEventBridge.ts`**

In `ENTITY_INVALIDATIONS`:

```ts
codingFact: [qk.codingMemory.all()],
codingEpisode: [qk.codingMemory.all()],
all: [], // sentinel; handled by special branch below
```

Modify the `entity:updated` listener to handle the `all` sentinel:

```ts
const offEntity = await listen("entity:updated", (payload) => {
	const p = payload as EntityUpdatedPayload;
	if (p.entityKind === "all") {
		// Broad invalidate — every query refetches.
		client.invalidateQueries();
		return;
	}
	const keys = ENTITY_INVALIDATIONS[p.entityKind as EntityKind];
	if (!keys) return;
	for (const queryKey of keys) {
		client.invalidateQueries({ queryKey });
	}
});
```

- [ ] **Step 5: Run tests — green**

```bash
cd desktop-ui && bun run test src/lib/query
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/lib/query
git commit -m "feat(desktop-ui): add coding-memory query keys + data_version-fallback route"
```

---

## Phase F — Verification

### Task F1: Manual end-to-end

- [ ] **Step 1: Start desktop**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev
```

- [ ] **Step 2: Trigger a coding-memory write via Claude Code**

In a separate terminal, simulate a Claude Code hook that completes a turn:

```bash
echo '{"event":"PostToolUse","tool":"Edit","file":"/tmp/x.rs"}' | klyntbot --hook claude-code PostToolUse
```

(Adjust to actual Claude Code hook payload.)

- [ ] **Step 3: Verify**

In a coding-memory recall panel (if one exists in the FE), confirm new facts/episodes appear without manual refresh.

- [ ] **Step 4: Verify `data_version` fallback**

Stop the bridge server (kill desktop temporarily). Run an MCP tool call to mutate a task. Restart desktop. Within 5 seconds, the `data_version` watcher should detect the bumped counter and broadcast `DataVersionBumped`, triggering broad invalidation. Tray + main app refresh.

```bash
# while desktop is restarting:
echo '...' | cargo run -p desktop -- mcp serve --stdio
# then start desktop. observe.
```

- [ ] **Step 5: Confirm devtools**

In React Query devtools (any window), the "broad invalidation" event should show every query going stale at the same instant.

- [ ] **Step 6: No commit needed (manual)**

---

## Self-Review

**1. Spec coverage:**
- DomainEvent variants → A1 ✓
- EntityKind variants → A2 ✓
- Distiller emission → B1 ✓
- Forwarder routing → C1 ✓
- `PRAGMA data_version` polling → D1, D2 ✓
- FE bridge updates → E1 ✓
- Verification → F1 ✓

**2. Placeholder scan:** Test scaffolds have "use the project's actual distiller harness" — that's a real instruction (the engineer must look up the existing distillation test setup). Acceptable.

**3. Type consistency:** `CodingMemoryKind::Fact` ↔ `EntityKind::CodingFact` ↔ `"codingFact"` (camelCase serde) ↔ `qk.codingMemory.facts()` — same concept, four representations, all matched.

---

## Definition of Done (Plan 4)

- New domain-event variants compile + tests green.
- Distiller publishes events confirmed via integration test.
- `data_version` watcher confirmed to fire when another connection writes (`crates/storage/src/tests/data_version_watcher.rs` green).
- Manual: Claude Code's coding-memory writes appear in the desktop UI within ~50 ms (when bridge is up) or ≤5 seconds (when bridge missed it; fallback path).
- React Query devtools shows correct invalidation events.
- After Plans 1-4 are all merged, **every mutation source — desktop UI, in-process automation, MCP child via bridge, MCP child via fallback polling, hook CLI distillation — propagates to every webview without manual refresh.**

---

## End of master plan

This is the last of four plans. Combined deliverables across Plans 1-4:
- `desktop-ui/src/lib/query/` — full TanStack Query foundation with event bridge.
- All FE features migrated off `useState + ipc` to typed cache.
- `crates/ipc-bridge/` — cross-process socket bridge.
- Distiller and storage layer publishing real-time events.
- React Query devtools available in every webview.
- Maintainable single-source-of-truth for query keys, entity kinds, and event routes.
