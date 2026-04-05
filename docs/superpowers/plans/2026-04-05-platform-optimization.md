# Platform Memory & Performance Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce desktop app RSS from 15.89 GB to under 2 GB by fixing memory leaks, reducing clone pressure, adding periodic compaction, bounding unbounded growth, and sharing resources across subsystems.

**Architecture:** The app is a Tauri 2 desktop shell over a 34-crate Rust workspace. Memory pressure comes from 5 compound sources: (1) LanceDB fragment accumulation without periodic compaction, (2) 25+ broadcast subscribers cloning every event on a 32-slot ring, (3) ONNX embedding model permanently resident due to continuous embed calls defeating the idle timer, (4) 8+ separate reqwest connection pools (primary driver of 551 ports), and (5) unbounded growth in threshold history, SSE subscribers, and activity log fire-and-forget spawns. Investigation by 6 parallel code explorers identified 20+ discrete issues across all 8 crate layers.

**Tech Stack:** Rust 1.93, Tauri 2, tokio, LanceDB, SQLite, fastembed/ONNX, reqwest, mimalloc

**Guiding Principle:** All changes are pure optimization — no logic changes. Every fix preserves existing behavior while reducing resource consumption.

---

## File Map

| File | Responsibility | Action |
|---|---|---|
| `crates/agent/src/services/memory_maintenance.rs` | LanceDB periodic maintenance | Modify: add compaction call after dedup |
| `crates/storage/src/vector_store/mod.rs` | VectorStore struct + LanceDB connect | Modify: add table handle cache |
| `crates/storage/src/vector_store/crud.rs` | LanceDB CRUD operations | Modify: use cached table handles |
| `crates/app-core/src/init/mod.rs` | App init, bus creation | Modify: increase bus capacity to 256 |
| `crates/bus/src/domain_events.rs` | DomainEvent enum | Modify: reduce ChatTurnCompleted payload |
| `crates/agent/src/execution/core.rs` | Tool execution | Modify: cap args_preview to 200 chars |
| `crates/context_engine/src/assembler/cache.rs` | Context assembly cache | Modify: reduce capacity from 8 to 2 |
| `crates/common/src/http.rs` | HTTP client builder | Modify: add shared client + pool limits |
| `crates/agent/src/learning/types.rs` | Adaptive threshold state | Modify: cap threshold_history to 100 |
| `crates/agent/src/learning/adaptive.rs` | Threshold adjustment logic | Modify: use push_change() instead of direct push |
| `crates/cognitive/src/repos/semantic_fact.rs` | Semantic fact queries | Modify: add LIMIT to unbounded queries |
| `crates/app-core/src/state.rs` | AppCore struct + shutdown | Modify: cancel mirror/voice in shutdown |
| `crates/app-core/src/handlers/chat/streaming.rs` | Stream relay + StreamGuard | Modify: clean pending_interactions in drop |
| `crates/desktop/src/dev_server/streaming.rs` | SSE channels | Modify: evict stale channels on connect |
| `crates/activity-log/src/service.rs` | Activity ingestion | Modify: bounded channel replaces fire-and-forget |
| `crates/desktop/src/lazy_window.rs` | Lazy WebView creation | Modify: add destroy_if_hidden for idle windows |
| `crates/desktop/tauri.conf.json` | Window declarations | Modify: remove eager auxiliary windows |
| `crates/agent/src/services/session_cleanup.rs` | Session cleanup service | Modify: add TTL for interaction/decision/usage logs |
| `crates/desktop/src/commands/window.rs` | quit_app command | Modify: add graceful shutdown |
| `crates/agent/src/execution/scratchpad.rs` | Reasoning traces | Modify: trim old traces |

---

## Task 1: LanceDB Compaction in MemoryMaintenanceService + Table Handle Caching

**Impact:** PRIMARY — LanceDB copy-on-write fragments accumulate and get mmap'd, growing RSS unboundedly. `optimize_all_tables()` only runs once at startup (60s delay). The `MemoryMaintenanceService` prunes and deduplicates but never compacts. Additionally, `open_table` is called per operation across 12 tables — each call re-loads manifests from disk.

**Files:**
- Modify: `crates/agent/src/services/memory_maintenance.rs:114`
- Modify: `crates/storage/src/vector_store/mod.rs`
- Modify: `crates/storage/src/vector_store/crud.rs`
- Modify: `crates/storage/src/vector_store/maintenance.rs`

- [ ] **Step 1: Add table handle cache to VectorStore**

In `crates/storage/src/vector_store/mod.rs`, add a `DashMap` field to cache opened table handles:

```rust
use dashmap::DashMap;
use lancedb::Table;

pub struct VectorStore {
    pub(crate) db: Arc<Connection>,
    /// Cached table handles — avoids re-opening manifests on every operation.
    pub(crate) table_cache: DashMap<String, Table>,
}
```

Update `connect()` to initialize the cache. Add a helper:

```rust
impl VectorStore {
    pub(crate) async fn get_table(&self, name: &str) -> Result<Table, StorageError> {
        if let Some(tbl) = self.table_cache.get(name) {
            return Ok(tbl.clone());
        }
        let tbl = self.db.open_table(name).execute().await
            .map_err(|e| StorageError::Vector(format!("open table {name}: {e}")))?;
        self.table_cache.insert(name.to_string(), tbl.clone());
        Ok(tbl)
    }

    pub fn invalidate_table_cache(&self) {
        self.table_cache.clear();
    }
}
```

- [ ] **Step 2: Replace all `open_table` calls with `get_table`**

In `crud.rs`, `cognitive.rs`, `conv.rs`, `tree_node.rs`, `community.rs` — replace `self.db.open_table(table).execute().await...` with `self.get_table(table).await?`. In `maintenance.rs`, after `optimize_all_tables()` completes, call `self.invalidate_table_cache()`.

- [ ] **Step 3: Add compaction to MemoryMaintenanceService**

In `crates/agent/src/services/memory_maintenance.rs`, after the dedup loop (line 114), add:

```rust
                    // Compact fragment files to reclaim mmap'd memory.
                    if let Err(e) = self.store.optimize_all_tables().await {
                        warn!(error = %e, "MemoryMaintenanceService: LanceDB compaction failed");
                    } else {
                        info!("MemoryMaintenanceService: LanceDB compaction complete");
                    }
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p storage -p agent && cargo nextest run -p storage -p agent && cargo clippy -p storage -p agent --all-targets`

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/vector_store/ crates/agent/src/services/memory_maintenance.rs
git commit -m "perf(storage): cache LanceDB table handles + periodic compaction in maintenance

Table handles cached in DashMap to avoid re-opening manifests per operation.
MemoryMaintenanceService now calls optimize_all_tables() after dedup to
compact fragment files that were causing unbounded mmap RSS growth."
```

---

## Task 2: Remove Eager WebView Windows + Destroy Idle Auxiliary Windows

**Impact:** HIGH — 4 auxiliary windows declared in tauri.conf.json are created at startup before lazy_window.rs can intercept. Each hidden WKWebView allocates ~300-500 MB GPU buffers. Total: up to 1.5 GB wasted.

**Files:**
- Modify: `crates/desktop/tauri.conf.json` (remove 4 auxiliary window entries)
- Modify: `crates/desktop/src/lazy_window.rs` (add destroy_if_hidden)

- [ ] **Step 1: Remove auxiliary windows from tauri.conf.json**

Keep only the `"main"` window in the `"windows"` array. Remove `launcher`, `tray`, `distraction-overlay`, `voice-orb`.

- [ ] **Step 2: Add destroy_if_hidden to lazy_window.rs**

```rust
pub fn destroy_if_hidden(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        if !w.is_visible().unwrap_or(true) {
            if let Err(e) = w.destroy() {
                warn!("Failed to destroy window '{label}': {e}");
            } else {
                info!("Destroyed idle window '{label}' to reclaim GPU memory");
            }
        }
    }
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/tauri.conf.json crates/desktop/src/lazy_window.rs
git commit -m "perf(desktop): remove eager WebView windows + add destroy_if_hidden

Remove 4 auxiliary windows from tauri.conf.json so they are created on
demand via lazy_window.rs. Add destroy_if_hidden() for reclaiming GPU
memory from idle auxiliary windows (~300-500 MB each)."
```

---

## Task 3: Reduce DomainEventBus Pressure (Capacity + Payload Size)

**Impact:** HIGH — 25+ subscribers on a 32-slot broadcast, each cloning every DomainEvent. ChatTurnCompleted carries full user_message, ToolCallExecuted serializes full args JSON. Under load: 32 × 25 = 800+ live clones.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:138`
- Modify: `crates/bus/src/domain_events.rs` (ChatTurnCompleted variant)
- Modify: `crates/agent/src/execution/core.rs:688`

- [ ] **Step 1: Increase bus capacity from 32 to 256**

In `crates/app-core/src/init/mod.rs:138`:

```rust
        let domain_event_bus = Arc::new(bus::DomainEventBus::new(256));
```

Update the comment above to reflect 256 slots and ~25 subscribers.

- [ ] **Step 2: Remove user_message from ChatTurnCompleted**

In `crates/bus/src/domain_events.rs`, remove the `user_message: String` field from `ChatTurnCompleted`. Update all publishers and subscribers (fix compilation errors).

- [ ] **Step 3: Cap args_preview in ToolCallExecuted to 200 chars**

In `crates/agent/src/execution/core.rs:688`:

```rust
args_preview: {
    let full = r.arguments.to_string();
    Some(if full.len() > 200 { format!("{}...", &full[..200]) } else { full })
},
```

- [ ] **Step 4: Build workspace**

Run: `cargo build --workspace`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/bus/src/domain_events.rs crates/agent/src/execution/core.rs
git commit -m "perf(bus): increase capacity to 256 + reduce DomainEvent payload sizes

Bus capacity 32→256 for 25+ subscribers. Remove user_message from
ChatTurnCompleted (subscribers fetch from session). Cap ToolCallExecuted
args_preview at 200 chars. Reduces clone amplification under load."
```

---

## Task 4: Share Single reqwest::Client + Pool Limits

**Impact:** HIGH for ports — 8+ separate reqwest::Client instances each maintain their own connection pool. Primary driver of 551 open ports.

**Files:**
- Modify: `crates/common/src/http.rs`
- Modify: All files calling `build_http_client` / `build_http_client_with_builder`

- [ ] **Step 1: Add shared client and pool limits**

In `crates/common/src/http.rs`, add:

```rust
use std::sync::OnceLock;

static SHARED_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn shared_http_client() -> Client {
    SHARED_CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(60))
                .pool_max_idle_per_host(10)
                .pool_idle_timeout(Duration::from_secs(30))
                .build()
                .expect("shared HTTP client build should not fail")
        })
        .clone()
}
```

Also add pool limits to `build_http_client_with_builder`:

```rust
    let builder = Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(4);
```

- [ ] **Step 2: Update tool/provider/channel clients to use shared_http_client()**

Replace per-instance client creation with `common::shared_http_client()` in:
- `crates/tools/src/system/web.rs` (WebSearchTool, WebFetchTool)
- `crates/providers/src/adapters/openai_compat.rs`
- `crates/providers/src/adapters/anthropic_native.rs`
- `crates/providers/src/adapters/transcription.rs`
- `crates/channels/src/adapters/slack.rs`, `discord.rs`, `telegram.rs`
- `crates/feature-finance/src/price_service.rs`
- `crates/voice-engine/src/model_manager.rs`

Use `RequestBuilder::timeout()` for per-request timeouts where needed.

- [ ] **Step 3: Build and test**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets`

- [ ] **Step 4: Commit**

```bash
git add crates/common/src/http.rs crates/tools/ crates/providers/ crates/channels/ crates/feature-finance/ crates/voice-engine/
git commit -m "perf(http): share single reqwest::Client across all components

Replace 8+ separate reqwest::Client instances with shared_http_client().
Each instance maintained its own connection pool — primary driver of 551
ports. Pool limits: 10 idle per host, 30s idle timeout."
```

---

## Task 5: Reduce ContextCache Capacity from 8 to 2

**Impact:** MEDIUM-HIGH — During ReAct loops, each iteration changes the cache key. With capacity 8, up to 8 full conversation snapshots coexist (each with complete Vec<Message> including 50KB tool results).

**Files:**
- Modify: `crates/context_engine/src/assembler/cache.rs:6`

- [ ] **Step 1: Change capacity**

```rust
pub(super) const DEFAULT_CACHE_CAPACITY: usize = 2;
```

- [ ] **Step 2: Build and test**

Run: `cargo nextest run -p context-engine && cargo clippy -p context-engine --all-targets`

- [ ] **Step 3: Commit**

```bash
git add crates/context_engine/src/assembler/cache.rs
git commit -m "perf(context): reduce ContextCache from 8 to 2 to cut snapshot memory"
```

---

## Task 6: Cap threshold_history + Add LIMIT to Semantic Fact Queries

**Impact:** MEDIUM — threshold_history grows unboundedly and is persisted to SQLite as JSON. Semantic fact queries load ALL active facts into RAM on fallback path.

**Files:**
- Modify: `crates/agent/src/learning/types.rs`
- Modify: `crates/agent/src/learning/adaptive.rs`
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`

- [ ] **Step 1: Cap threshold_history**

Add constant and method to `types.rs`:

```rust
const MAX_THRESHOLD_HISTORY: usize = 100;

impl AdaptiveThresholdState {
    pub fn push_change(&mut self, change: ThresholdChange) {
        self.threshold_history.push(change);
        if self.threshold_history.len() > MAX_THRESHOLD_HISTORY {
            let excess = self.threshold_history.len() - MAX_THRESHOLD_HISTORY;
            self.threshold_history.drain(..excess);
        }
    }
}
```

Update `adaptive.rs` to use `push_change()` instead of direct push.

- [ ] **Step 2: Add LIMIT to semantic fact queries**

In `semantic_fact.rs`, add `LIMIT 500` to `list_active` and `LIMIT 1000` to `list_all_active`.

- [ ] **Step 3: Build and test**

Run: `cargo nextest run -p agent -p cognitive && cargo clippy -p agent -p cognitive --all-targets`

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/learning/ crates/cognitive/src/repos/semantic_fact.rs
git commit -m "perf: cap threshold_history to 100 + add LIMIT to semantic fact queries"
```

---

## Task 7: Bounded Activity Ingestion + Graceful Shutdown Fixes

**Impact:** MEDIUM — `ingest_fire_and_forget` spawns unbounded detached tasks. Mirror/voice handles not cancelled in shutdown(). quit_app doesn't call shutdown().

**Files:**
- Modify: `crates/activity-log/src/service.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/desktop/src/commands/window.rs`

- [ ] **Step 1: Add bounded ingestion channel**

In `service.rs`, add `BatchIngestionService`:

```rust
pub struct BatchIngestionService {
    tx: tokio::sync::mpsc::Sender<ActivityLogEntry>,
}

impl BatchIngestionService {
    pub fn new(service: Arc<ActivityIngestionService>, buffer: usize) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ActivityLogEntry>(buffer);
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                if let Err(e) = service.ingest(entry).await {
                    warn!("Activity batch ingestion failed: {e}");
                }
            }
        });
        Self { tx }
    }

    pub fn ingest_nonblocking(&self, entry: ActivityLogEntry) {
        if self.tx.try_send(entry).is_err() {
            warn!("Activity ingestion buffer full — dropping entry");
        }
    }
}
```

- [ ] **Step 2: Cancel mirror subscribers + abort voice loop in shutdown()**

In `state.rs` `shutdown()`, before `self.shutdown_token.cancel()`:

```rust
        if let Some(ref token) = self._mirror_shutdown {
            token.cancel();
        }
        if let Some(ref handle) = self.voice_loop_handle {
            handle.abort();
        }
```

- [ ] **Step 3: Add graceful shutdown to quit_app**

In `crates/desktop/src/commands/window.rs`, make `quit_app` async and call `core.shutdown().await` before `app.exit(0)`.

- [ ] **Step 4: Build and test**

Run: `cargo build -p activity-log -p app-core -p desktop && cargo clippy --workspace --all-targets`

- [ ] **Step 5: Commit**

```bash
git add crates/activity-log/src/service.rs crates/app-core/src/state.rs crates/desktop/src/commands/window.rs
git commit -m "perf: bounded activity ingestion + graceful shutdown for mirror/voice/quit"
```

---

## Task 8: SSE Channel Cleanup + StreamGuard pending_interactions Fix

**Impact:** MEDIUM — SSE channels only evicted on chat_send (stale channels accumulate). StreamGuard doesn't clean pending_interactions on drop.

**Files:**
- Modify: `crates/desktop/src/dev_server/streaming.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Extract and call eviction from sse_handler**

Create `evict_stale_channels()` helper, call it from both `dispatch_chat_send` and `sse_handler`.

- [ ] **Step 2: Add pending_interactions to StreamGuard**

Add `pending: Arc<DashMap<...>>` field to `StreamGuard`. In `Drop::drop`, add `self.pending.remove(&self.key)`.

- [ ] **Step 3: Build**

Run: `cargo build -p desktop -p app-core`

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/dev_server/streaming.rs crates/app-core/src/handlers/chat/streaming.rs
git commit -m "fix: evict stale SSE channels on connect + clean pending_interactions in StreamGuard"
```

---

## Task 9: Scratchpad Trimming + Session Log TTL Cleanup

**Impact:** LOW-MEDIUM — Scratchpad traces grow unbounded per execution. interaction_log/decision_log/tool_usage tables grow without TTL.

**Files:**
- Modify: `crates/agent/src/execution/scratchpad.rs`
- Modify: `crates/agent/src/services/session_cleanup.rs`

- [ ] **Step 1: Trim scratchpad traces to 20**

```rust
const MAX_TRACES: usize = 20;

pub fn add(&mut self, trace: ReasoningTrace) {
    self.traces.push(trace);
    if self.traces.len() > MAX_TRACES {
        self.traces.drain(..self.traces.len() - MAX_TRACES);
    }
}
```

- [ ] **Step 2: Add TTL cleanup for log tables**

In `session_cleanup.rs`, after session cleanup, add DELETE queries for `interaction_log`, `decision_log`, `tool_usage` older than `max_age_days`.

- [ ] **Step 3: Build and test**

Run: `cargo nextest run -p agent && cargo clippy -p agent --all-targets`

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/execution/scratchpad.rs crates/agent/src/services/session_cleanup.rs
git commit -m "perf(agent): trim scratchpad traces + TTL cleanup for log tables"
```

---

## Task 10: Final Verification

- [ ] **Step 1:** `cargo build --workspace`
- [ ] **Step 2:** `cargo clippy --workspace --all-targets --all-features`
- [ ] **Step 3:** `cargo fmt --all --check`
- [ ] **Step 4:** `cargo nextest run --workspace`

---

## Expected Impact

| Task | Estimated Savings |
|---|---|
| 1. LanceDB compaction + table cache | 3-6 GB (mmap fragments) |
| 2. Lazy WebView + destroy idle | 0.5-1.5 GB (GPU buffers) |
| 3. Bus capacity + payload reduction | 200-500 MB (under load) |
| 4. Shared reqwest client | 50-200 MB RAM, 400+ ports |
| 5. ContextCache 8→2 | 100-400 MB (per ReAct loop) |
| 6. Threshold cap + LIMIT queries | 50-200 MB (long-running) |
| 7. Bounded ingestion + shutdown | Prevents accumulation |
| 8. SSE + StreamGuard cleanup | Prevents DashMap growth |
| 9. Scratchpad + log TTL | 10-50 MB + SQLite size |

**Total estimated reduction: 4-9 GB**, bringing RSS from 15.89 GB to ~2-4 GB under normal use.
