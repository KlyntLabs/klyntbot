# Memory Optimization v2 — Debug Build & Runtime (15GB → <1GB)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the Klyntbot desktop app's runtime memory from 15GB (debug) / 8-11GB (observed) to under 1GB by fixing the root cause (unoptimized debug build) and applying targeted runtime optimizations.

**Architecture:** The #1 problem is running a pure debug build (`target/debug/desktop`, 697MB binary, opt-level=0) with 298 lance/arrow/fastembed crates compiled without optimization. Debug builds of Arrow/Lance alone can consume 5-10x more memory than release due to unoptimized data structures, no inlining, and no dead-code elimination. After fixing the build profile, we address runtime issues: duplicate HTTP clients, tokio thread overprovisioning, and LanceDB table handle lifecycle.

**Tech Stack:** Rust (Cargo profiles, mimalloc, LanceDB, fastembed, Arrow), React 19 + Tauri 2, Vite

---

## Estimated Memory Budget

| Component | Current (debug) | After Profile Fix | After Runtime Opts | 
|-----------|-----------------|-------------------|-------------------|
| Arrow/Lance runtime overhead | ~6-8 GB | ~500 MB | ~200 MB |
| ONNX embedding (when loaded) | ~800 MB (debug) | ~420 MB | ~420 MB (idle: 0) |
| Tokio runtime + threads | ~500 MB | ~200 MB | ~100 MB |
| App state (sessions, bus, tools) | ~300 MB | ~100 MB | ~80 MB |
| Duplicate reqwest pools | ~200 MB | ~100 MB | ~20 MB |
| Frontend WebView | ~500 MB | ~500 MB | ~300 MB |
| **Total** | **~10-15 GB** | **~1.8 GB** | **~800 MB-1.2 GB** |

---

## Phase 1: Build Profile — The 80% Fix (est. savings: ~8-12 GB)

### Task 1: Add `[profile.dev]` with optimized dependencies

The single highest-impact change. Rust's `opt-level=0` (debug default) means Arrow's columnar operations, Lance's mmap management, and fastembed's ONNX inference all run unoptimized — using 5-10x more memory than necessary. Cargo supports optimizing dependencies while keeping your own code at debug level for fast compilation.

**Files:**
- Modify: `Cargo.toml` (workspace root, after line ~212 `[profile.release]` section)

- [ ] **Step 1: Add dev profile with optimized deps**

Add this section to the **end** of the workspace `Cargo.toml`, after the existing `[profile.release]` block:

```toml
# === Dev profile: fast local builds, but optimize heavy deps ===
# Without this, Arrow/Lance/fastembed run at opt-level=0 and consume
# 5-10x more memory than release. This keeps our own code at opt-level=0
# (fast incremental compiles) while heavy deps get opt-level=2.
[profile.dev]
opt-level = 0

# Optimize ALL dependencies in dev mode — Arrow, Lance, fastembed, ONNX,
# serde, regex, etc. run orders-of-magnitude faster and use far less memory.
[profile.dev.package."*"]
opt-level = 2

# Critical memory-hungry deps get maximum optimization (same as release).
[profile.dev.package.arrow]
opt-level = 3
[profile.dev.package.arrow-array]
opt-level = 3
[profile.dev.package.arrow-buffer]
opt-level = 3
[profile.dev.package.arrow-data]
opt-level = 3
[profile.dev.package.arrow-schema]
opt-level = 3
[profile.dev.package.lance]
opt-level = 3
[profile.dev.package.lance-core]
opt-level = 3
[profile.dev.package.lance-io]
opt-level = 3
[profile.dev.package.lance-file]
opt-level = 3
[profile.dev.package.lance-index]
opt-level = 3
[profile.dev.package.lance-linalg]
opt-level = 3
[profile.dev.package.lance-table]
opt-level = 3
[profile.dev.package.lancedb]
opt-level = 3
[profile.dev.package.fastembed]
opt-level = 3
[profile.dev.package.ort]
opt-level = 3
[profile.dev.package.mimalloc]
opt-level = 3
[profile.dev.package.sqlx]
opt-level = 2
[profile.dev.package.reqwest]
opt-level = 2
[profile.dev.package.serde]
opt-level = 2
[profile.dev.package.serde_json]
opt-level = 2
[profile.dev.package.regex]
opt-level = 2
[profile.dev.package.tokio]
opt-level = 2
```

- [ ] **Step 2: Rebuild and verify it compiles**

Run: `cargo build -p desktop 2>&1 | tail -5`
Expected: Compiles successfully. First build will be slower (deps recompile with optimization). Subsequent incremental builds remain fast because only your code is at opt-level=0.

- [ ] **Step 3: Measure memory improvement**

Run:
```bash
# Kill existing instance
pkill -f "target/debug/desktop" || true
sleep 2

# Start fresh and measure after 30s idle
cargo tauri dev &
sleep 30
ps -o rss,vsz,comm -p $(pgrep -f "target/debug/desktop") | awk 'NR==2{printf "RSS: %.0f MB, VSZ: %.0f MB\n", $1/1024, $2/1024}'
```

Expected: RSS should drop from ~15GB to ~1.5-3GB. This alone is the majority of savings.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "perf: optimize deps in dev profile to cut debug memory 5-10x

Arrow/Lance/fastembed at opt-level=0 caused 15GB RSS in debug builds.
Dependencies now compile at opt-level=2/3 while workspace code stays at
opt-level=0 for fast incremental builds."
```

---

## Phase 2: Backend Runtime Optimizations (est. savings: ~300-500 MB)

### Task 2: Consolidate duplicate reqwest HTTP clients

Six places create `reqwest::Client::new()` instead of using `common::shared_http_client()`. Each spawns a separate connection pool with its own DNS resolver, TLS session cache, and idle connections — wasting ~30-50MB per client.

**Files:**
- Modify: `crates/tools/src/embedding/embedding_engine.rs:75,88`
- Modify: `crates/voice-engine/src/engines/cloud_tts.rs:22`
- Modify: `crates/voice-engine/src/engines/cloud_asr.rs:24`
- Modify: `crates/plugin-runtime/src/host/mod.rs:127`
- Modify: `crates/desktop/src/oauth/flow.rs:116`

- [ ] **Step 1: Replace all `reqwest::Client::new()` with shared client**

In each file listed above, replace `reqwest::Client::new()` with `common::shared_http_client().clone()`.

For `crates/tools/src/embedding/embedding_engine.rs` lines 75 and 88, both instances of:
```rust
http_client: reqwest::Client::new(),
```
become:
```rust
http_client: common::shared_http_client().clone(),
```

For `crates/voice-engine/src/engines/cloud_tts.rs` line 22:
```rust
client: reqwest::Client::new(),
```
becomes:
```rust
client: common::shared_http_client().clone(),
```

For `crates/voice-engine/src/engines/cloud_asr.rs` line 24:
```rust
client: reqwest::Client::new(),
```
becomes:
```rust
client: common::shared_http_client().clone(),
```

For `crates/plugin-runtime/src/host/mod.rs` line 127:
```rust
http_client: reqwest::Client::new(),
```
becomes:
```rust
http_client: common::shared_http_client().clone(),
```

For `crates/desktop/src/oauth/flow.rs` line 116:
```rust
let client = reqwest::Client::new();
```
becomes:
```rust
let client = common::shared_http_client().clone();
```

Add `use common;` or ensure `common` is in scope for each crate. If `common` isn't a dependency, add it to that crate's `Cargo.toml`.

- [ ] **Step 2: Verify no remaining `Client::new()` outside common**

Run: `grep -rn "reqwest::Client::new()" crates/ --include="*.rs" | grep -v "common/src/http.rs" | grep -v "test"`
Expected: Zero results (or only test code).

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace -E 'not test(/plugin/)' 2>&1 | tail -10`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/
git commit -m "perf: consolidate 6 duplicate reqwest clients into shared pool

Each Client::new() spawns its own connection pool, DNS resolver, and TLS
cache. Replaced with common::shared_http_client() which is bounded to
10 idle connections per host with 30s timeout."
```

---

### Task 3: Cap tokio worker threads for single-user app

Tokio defaults to one worker thread per CPU core. On an M-series Mac with 10+ cores, that's 10+ threads × ~10MB stack each = ~100MB+ just for the thread pool. A single-user desktop app doesn't need that many.

**Files:**
- Modify: `crates/desktop/src/main.rs` (the `run_desktop_app()` function or Tauri builder)
- Modify: `crates/desktop/tauri.conf.json` (if Tauri manages the runtime)

- [ ] **Step 1: Find where the tokio runtime is created for the desktop app**

The MCP path creates its own runtime at `main.rs:119`. For the desktop path, Tauri manages the runtime. Check if `tauri::async_runtime` can be configured.

Run: `grep -n "async_runtime\|Runtime::new\|Builder::new_multi_thread\|worker_threads" crates/desktop/src/ -r`

- [ ] **Step 2: Configure Tauri's async runtime with bounded threads**

If Tauri uses its own tokio runtime (likely), add before the Tauri builder in `run_desktop_app()`:

```rust
// Cap worker threads — a single-user app doesn't need one per core.
// 4 workers is enough for concurrent LLM calls + DB + UI events.
tauri::async_runtime::set(
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(2 * 1024 * 1024) // 2MB stacks instead of default 8MB
        .enable_all()
        .build()
        .expect("tokio runtime"),
);
```

**Important:** This must be called **before** `tauri::Builder::default()`. If Tauri has already initialized its runtime, this will panic. Place it at the very start of `run_desktop_app()`.

- [ ] **Step 3: Also cap the MCP runtime**

In `run_mcp_stdio()` at `main.rs:119`, replace:
```rust
let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
```
with:
```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .thread_stack_size(2 * 1024 * 1024)
    .enable_all()
    .build()
    .expect("Failed to create tokio runtime");
```

- [ ] **Step 4: Run the app and verify it works**

Run: `cargo tauri dev` — verify the app starts, can send a message, and responds normally.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "perf: cap tokio to 4 workers with 2MB stacks for desktop app

Default was 1 thread per core (10+) × 8MB stack = 80MB+ just for idle
threads. 4 workers with 2MB stacks = 8MB, sufficient for single-user
concurrent LLM + DB + UI workload."
```

---

### Task 4: Lazy-open LanceDB tables instead of eagerly opening all 12

Currently, all 12 embedding tables are opened at startup (each loads manifests, potentially mmap's fragment files). Most users won't use all 12 features in a session.

**Files:**
- Modify: `crates/storage/src/vector_store/mod.rs:93-212` (the `connect()` method)

- [ ] **Step 1: Remove eager table creation from `connect()`**

The current `connect()` method calls `ensure_table()` for all 12 tables. Change it to only create the `VectorStore` and cache — tables open on first access via `get_table()`.

Replace the section after `let store = Self { ... }` (approximately lines 93-212) that contains all the `ensure_table` calls. Remove all the `let existing = ...` and individual table creation blocks. The method should just return the store:

```rust
pub async fn connect(data_dir: &Path) -> Result<Self, StorageError> {
    let lance_dir = data_dir.join("lance");
    std::fs::create_dir_all(&lance_dir)
        .map_err(|e| StorageError::Vector(format!("Failed to create lance dir: {e}")))?;
    let path_str = lance_dir
        .to_str()
        .ok_or_else(|| StorageError::Vector("lance dir path is not valid UTF-8".to_string()))?;
    let session = Arc::new(lance::session::Session::new(
        LANCE_INDEX_CACHE_BYTES,
        LANCE_METADATA_CACHE_BYTES,
        Arc::new(lance_io::object_store::ObjectStoreRegistry::default()),
    ));
    let db = lancedb::connect(path_str)
        .session(session)
        .execute()
        .await
        .map_err(|e| StorageError::Vector(format!("LanceDB connect failed: {e}")))?;
    Ok(Self {
        db: Arc::new(db),
        table_cache: Arc::new(DashMap::new()),
    })
}
```

- [ ] **Step 2: Make `get_table()` create-on-first-access**

The existing `get_table()` should check the cache, then try to open the table, and if it doesn't exist, create it with the right schema. Modify `get_table()` to look up the schema from a helper:

```rust
/// Get or lazily open a table, creating it with the correct schema if needed.
pub(crate) async fn get_table(&self, name: &str) -> Result<Table, StorageError> {
    if let Some(entry) = self.table_cache.get(name) {
        return Ok(entry.value().clone());
    }
    
    // Try to open existing table first.
    let table = match self.db.open_table(name).execute().await {
        Ok(t) => t,
        Err(_) => {
            // Table doesn't exist — create with schema.
            let schema = schemas::schema_for_table(name)
                .ok_or_else(|| StorageError::Vector(format!("Unknown table: {name}")))?;
            self.db
                .create_empty_table(name, schema)
                .execute()
                .await
                .map_err(|e| StorageError::Vector(format!("create {name}: {e}")))?
        }
    };
    
    self.table_cache.insert(name.to_string(), table.clone());
    Ok(table)
}
```

- [ ] **Step 3: Add `schema_for_table()` helper in schemas.rs**

In `crates/storage/src/vector_store/schemas.rs`, add a function that returns the correct Arrow schema for a given table name:

```rust
/// Return the Arrow schema for a known embedding table, or None for unknown tables.
pub fn schema_for_table(name: &str) -> Option<Arc<arrow_schema::Schema>> {
    match name {
        "todo_embeddings" => Some(todo_schema()),
        "task_embeddings" => Some(task_schema()),
        "note_embeddings" => Some(note_schema()),
        "conv_embeddings" => Some(conv_schema()),
        "cognitive_fact_embeddings" => Some(cognitive_fact_schema()),
        "activity_embeddings" => Some(activity_schema()),
        "work_context_embeddings" => Some(work_context_schema()),
        "flashcard_embeddings" => Some(flashcard_schema()),
        "tree_node_embeddings" => Some(tree_node_schema()),
        "community_embeddings" => Some(community_schema()),
        "insight_embeddings" => Some(insight_schema()),
        "entity_embeddings" => Some(entity_schema()),
        _ => None,
    }
}
```

- [ ] **Step 4: Run vector store tests**

Run: `cargo nextest run -p storage -E 'test(/vector/)' 2>&1 | tail -15`
Expected: All pass. Tables are created on first access, same behavior as before but deferred.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/vector_store/
git commit -m "perf: lazy-open LanceDB tables on first access instead of startup

12 tables eagerly opened at startup loaded manifests and mmap'd fragments
even for unused features. Now tables open on first query, reducing startup
memory and keeping unused table files unmapped."
```

---

## Phase 3: Frontend Optimizations (est. savings: ~200-400 MB)

### Task 5: Lazy-load Mermaid and Three.js

These are the two heaviest JS dependencies (~2-3MB parsed each). They should only load when the user navigates to features that need them.

**Files:**
- Identify: Find all static `import ... from "mermaid"` and `import ... from "three"` across `desktop-ui/src/`
- Modify: Each file to use dynamic `import()` or React `lazy()`

- [ ] **Step 1: Find all eager imports of heavy deps**

Run: `grep -rn "from ['\"]mermaid['\"]\\|from ['\"]three['\"]\\|from ['\"]react-force-graph-3d['\"]" desktop-ui/src/`

- [ ] **Step 2: Convert each to lazy/dynamic import**

For React components, wrap with `React.lazy()`:
```tsx
const MermaidDiagram = lazy(() => import("./MermaidDiagram"));
```

For non-component usage (e.g., calling `mermaid.render()` in a hook), use dynamic import:
```tsx
const renderMermaid = async (code: string) => {
    const { default: mermaid } = await import("mermaid");
    return mermaid.render("mermaid-svg", code);
};
```

- [ ] **Step 3: Verify lazy loading works**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds. Mermaid/Three.js should appear as separate chunks in the build output.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/
git commit -m "perf(ui): lazy-load Mermaid and Three.js to cut initial bundle"
```

---

### Task 6: Lower useQuery cache limits

200 cache entries with 5-minute TTL is excessive for a desktop app where the user typically views one page at a time.

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useQuery.ts:32-33`

- [ ] **Step 1: Reduce cache limits**

Change:
```typescript
const MAX_CACHE_ENTRIES = 200;
const CACHE_TTL = 5 * 60_000;
```
to:
```typescript
const MAX_CACHE_ENTRIES = 50;
const CACHE_TTL = 60_000; // 1 minute
```

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test 2>&1 | tail -10`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/hooks/useQuery.ts
git commit -m "perf(ui): reduce query cache from 200/5min to 50/1min

Desktop app shows one page at a time — 50 entries with 1 minute TTL
is sufficient and releases stale data faster."
```

---

### Task 7: Lower MessageList virtualization threshold

Currently renders 50 message DOM nodes before switching to virtualization. Rich markdown messages can be heavy — lower to 20.

**Files:**
- Modify: `desktop-ui/src/features/chat/components/MessageList.tsx:54`

- [ ] **Step 1: Lower threshold**

Change:
```typescript
const isVirtualized = messages.length > 50;
```
to:
```typescript
const isVirtualized = messages.length > 20;
```

- [ ] **Step 2: Verify chat still works**

Run: `cd desktop-ui && bun run build && bun run test`
Expected: Build + tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/components/MessageList.tsx
git commit -m "perf(ui): virtualize messages at 20+ instead of 50+

Rich markdown messages with code blocks are DOM-heavy. Virtualizing
earlier reduces peak DOM node count."
```

---

## Phase 4: Verification

### Task 8: Full memory benchmark

- [ ] **Step 1: Clean rebuild**

Run:
```bash
cargo clean
cargo build -p desktop 2>&1 | tail -3
```

- [ ] **Step 2: Measure idle memory**

```bash
# Start the app
cargo tauri dev &
sleep 45

# Measure
ps -o rss,vsz,comm -p $(pgrep -f "target/debug/desktop") | awk 'NR==2{printf "RSS: %.0f MB\n", $1/1024}'
```

Expected: RSS under 1.5 GB at idle. Under 2 GB during active use with embedding model loaded.

- [ ] **Step 3: Measure under load**

Open a chat, send 3 messages, open notes graph, wait 30s, then measure again.
Expected: RSS under 2.5 GB.

- [ ] **Step 4: Document results**

Add a comment to the commit or PR with before/after numbers.

- [ ] **Step 5: Commit all remaining changes**

```bash
git add -A
git commit -m "perf: memory optimization v2 — verified RSS reduction"
```
