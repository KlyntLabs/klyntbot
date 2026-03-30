# Code Diet: Lightweight Second Brain on M1 Air 8GB

**Date:** 2026-03-30
**Approach:** B (Measure-Cut-Measure) + selective C (compile-time gating for power-user features)
**Target hardware:** MacBook Air M1 8GB — non-negotiable hard floor
**Duration:** 3 weeks, then never touch loading/init/pressure layer again

## Design Principle

The second brain must be invisible in terms of resource usage. It should feel like a native macOS extension — always-on, never making the user feel like "something heavy is running." On an 8GB Air, if the app heats up the machine, spins the fan, or drains battery, users will never feel "this is my second brain" — they'll just see "another heavy AI app."

Runtime awareness over compile-time surgery: a user who enables finance + coaching + mirror should still get <160MB idle. The system is smart about *when* to think, not just *whether* to think.

## Resource Budgets

| Metric | Target | Hard Fail |
|--------|--------|-----------|
| Binary size (default features) | <100MB | >120MB |
| Cold launch to interactive | <1.8s | >2.5s |
| Idle RAM after 5 min | <160MB | >200MB |
| Background CPU (30 min average) | <2% | >5% |
| Launcher popup (first open) | <300ms | >500ms |
| Tray countdown visible | <1s after launch | >2s |
| First message response (cold) | <2.2s | >3s |
| Morning cron (5 jobs, Normal pressure) | Max 2 concurrent | >3 concurrent |
| Frontend initial JS parse | <3MB | >5MB |

Benchmarked on real M1 Air 8GB with Safari (5 tabs) + Messages + Slack running.

---

## Section 1: Feature Tiers & Init Architecture

### Tier 1 — Always Loaded (core second brain, <80MB)

| Service | Init Phase | Rationale |
|---------|-----------|-----------|
| Chat pipeline + ReAct loop | Phase 3 (agent) | The heartbeat |
| TaskRepo + ReminderEngine + RecurringTaskSpawner | Phase 3 | Core value prop |
| SessionManager + SessionCleanupService | Phase 3 | Lightweight, needed for any interaction |
| ConfigWatcher | Phase 2 | Hot-reload, negligible cost |
| Tray countdown | Phase 4 | The "always visible" presence |
| SQLite-based memory retrieval (episodic + semantic facts) | Phase 1 (storage) | "Already knows you" — no vectors, SQL lookups only |
| SignalAccumulator + trigger evaluation (coaching) | Phase 3 | Lightweight signal collection, no LLM. Makes Klyntbot feel aware |
| Finance basic CRUD + budget alerts + net-worth snapshot | Phase 3 | Notices overspend without being asked. <5MB, near-zero CPU idle |
| Basic MirrorFacade (routing snapshots + meta-rule detection) | Phase 3 | Seed of personality/self-improvement. Passive event logging, <1MB |

**Launcher micro-warm:** Pre-load 2KB frequency table + top-10 most-used items into memory during Phase 4. Full search cache stays lazy. Makes the first Cmd+Space feel native-fast.

**Finance silent update:** When net-worth snapshot changes by >3%, emit a silent `entity:updated` event (no toast, no notification). Dashboard reflects instantly.

**Mirror pressure safeguard:** RoutingMirrorSubscriber + MetaRuleDetector drop events silently under Critical pressure to prevent write contention on a stressed machine.

### Tier 2 — Lazy Init (loaded on first use, released when idle)

| Service | Trigger | Idle Release |
|---------|---------|--------------|
| LanceDB + VectorStore | First semantic search, consolidation, or InsightForge query | 10 min (5 min under Elevated, force under Critical) |
| Fastembed / embedding engine | First embedding request | 10 min. ~30MB freed on release |
| Voice engine (STT/TTS + Whisper) | Voice mode activation | 5 min after voice ends. ~150MB freed |
| Launcher search cache (full) | First launcher open (Cmd+Space) | 10 min after launcher closes. Frequency table (2KB) stays in Tier 1 |
| Finance price refresh + exchange rates | First portfolio view or cron trigger | 10 min. Rate cache persists in SQLite |
| Proactive scan (task suggestions) | User opens tasks dashboard, weekly review, or "plan my day" | No persistent state — runs once, returns result |
| Coaching InterventionRouter + LLM reasoning | Signal threshold crossed | Runs once per intervention, no persistent hold |
| Memory prefetch + RAG query rewriting | First semantic query or consolidation trigger | Tied to LanceDB lifecycle |

**Implementation:** `Arc<RwLock<Option<T>>>` per service. No generic `LazyService<T>` — each service manages its own lifecycle because init/release patterns differ (LanceDB needs connection pool teardown, voice needs model unload, launcher drops cache).

**Idle-release mechanism:** Per-service `tokio::spawn` background task checks last-access timestamp every 60s. If `now - last_access > idle_threshold`, replaces contents with `None`. Under Critical pressure, all idle thresholds drop to 0 (force-release).

### Tier 3 — Compile-Time Gated (off by default)

| Feature | Cargo Flag | Dependencies Removed |
|---------|-----------|---------------------|
| WASM plugin runtime | `plugin-integration` (exists, already off) | extism + WASM deps |
| Email channel | `email` (exists, flip default to off) | async-imap, lettre, native-tls, tokio-native-tls |
| MCP HTTP server | `mcp-http` (new) | HTTP listener in klyntbot-server (stdio stays) |
| AutoTuner experiments | `autotuner` (new) | AutoTunerOrchestrator, shadow trial infra |
| Mirror weekly narrative | `mirror-narratives` (new) | LLM narrative generation, JOB_MIRROR_WEEKLY_NARRATIVE |
| Weekly reflection + report | `weekly-reports` (new) | JOB_WEEKLY_REFLECTION, JOB_WEEKLY_REPORT cron jobs |

**Gating strategy:** `#[cfg(feature = "...")]` guards at the wiring level (module registration in app-core, cron registration, AppCore fields), not at the crate level. Crates always compile so `cargo clippy --workspace --all-features` and `cargo nextest run --workspace --features full` still cover gated code.

**Root Cargo.toml:**
```toml
[features]
default = []  # was ["email"]
email = ["channels/email"]
browser-integration = ["tools/browser-integration"]
plugin-integration = ["plugin-runtime/plugin-integration"]
autotuner = []
mirror-narratives = []
weekly-reports = []
mcp-http = []
full = ["email", "autotuner", "mirror-narratives", "weekly-reports", "mcp-http", "plugin-integration"]
```

### AppCore Init Phases (new order)

```
Phase 1: Storage (SQLite pool, repos — always)
Phase 2: Config watcher (always)
Phase 3: Agent core (chat pipeline, ReAct, session — always)
         + Task services (reminders, recurring, focus check — always)
         + Signal accumulator (coaching — always)
         + Finance basic (CRUD, budget alerts — always)
         + Basic MirrorFacade (routing snapshots, meta-rules — always)
Phase 4: Tray countdown (always) + launcher micro-warm (frequency table + top-10)
Phase 5: Register lazy service slots (empty Arc<RwLock<Option<T>>> for each Tier 2 service)
Phase 6: Register cron jobs (only non-gated jobs, with BackgroundSemaphore)
Phase 7: Channel manager (Slack/Discord — always, lightweight)

Gated phases (only if feature flag enabled):
Phase G1: Mirror narrative engine (if mirror-narratives)
Phase G2: AutoTuner (if autotuner)
Phase G3: Email channel (if email)
```

---

## Section 2: Runtime Resource Awareness

### SystemPressureMonitor

Lightweight singleton in `app-core` reading macOS system stats via `platform-macos` crate.

**Data sources:**
- CPU load average (1-min) via `host_processor_info()` or `sysctl`
- Memory pressure via `dispatch_source_memorypressure` (macOS native: normal/warn/critical) — better than raw MB because macOS memory compression makes raw numbers misleading
- Thermal state via `ProcessInfo.thermalState` (nominal/fair/serious/critical)

**API:**
```rust
pub enum PressureLevel { Normal, Elevated, Critical }

impl SystemPressureMonitor {
    pub fn current_pressure(&self) -> PressureLevel;
    pub fn is_under_pressure(&self) -> bool;
    pub fn should_defer_background_work(&self) -> bool;
}
```

**Implementation:** Single `tokio::spawn` task polling every 5 seconds. Latest reading stored in `AtomicU8`. Zero allocations on read.

**Mapping rules:**

| macOS Signal | PressureLevel | Effect |
|---|---|---|
| Memory: normal, CPU <45%, thermal: nominal/fair | `Normal` | All services run freely |
| Memory: warn OR CPU 45-70% OR thermal: serious | `Elevated` | Lazy services defer init, cron jobs space out, idle-release timers shorten to 5 min |
| Memory: critical OR CPU >70% OR thermal: critical | `Critical` | All background work pauses, lazy services force-release, only Tier 1 stays active |

### BackgroundSemaphore (pressure-aware)

A fixed `tokio::sync::Semaphore(2)` limits concurrent background LLM calls to 2. Pressure-awareness is a pre-gate check before acquiring a permit:

```rust
// Pre-gate: check pressure before attempting to acquire
match pressure_monitor.current_pressure() {
    PressureLevel::Critical => { reschedule_in(15_min); return; }
    PressureLevel::Elevated => {
        // Under Elevated, only proceed if no other background LLM job is running
        // (try_acquire instead of acquire — don't wait, reschedule if busy)
        match semaphore.try_acquire() {
            Ok(permit) => { /* proceed with permit */ }
            Err(_) => { reschedule_in(5_min); return; }
        }
    }
    PressureLevel::Normal => {
        // Under Normal, wait for a permit (max 2 concurrent)
        let permit = semaphore.acquire().await;
        // proceed with permit
    }
}
```

Under Normal: up to 2 concurrent background LLM calls. Under Elevated: at most 1 (try_acquire fails if another is running). Under Critical: fully deferred.

**Time-of-day jitter (morning thundering herd fix):**
- `daily_planning` (8am): runs at 8:00 + random(0-5min)
- `finance_daily_review` (8am): runs at 8:00 + random(5-10min)
- `proactive_scan`: no longer cron-based — trigger-based only (Tier 2)
- `weekly_reflection` (Sunday): moved to midnight
- `weekly_report` (Sunday): chained after weekly_reflection completes (sequential)

---

## Section 3: Compile-Time Gating & Binary Diet

### `#[cfg]` guard locations (per feature)

**`autotuner`:**
- `app-core/src/init/cron.rs` — guard AutoTunerOrchestrator registration + nightly cycle job
- `app-core/src/state.rs` — guard AutoTunerOrchestrator field

**`mirror-narratives`:**
- `app-core/src/init/cron.rs` — guard JOB_MIRROR_WEEKLY_NARRATIVE + JOB_MIRROR_CLEANUP registration
- `app-core/src/init/mod.rs` — guard MirrorEngine::start() call (basic MirrorFacade stays always-loaded)

**`weekly-reports`:**
- `app-core/src/init/cron.rs` — guard JOB_WEEKLY_REFLECTION + JOB_WEEKLY_REPORT

**`email`:**
- Already exists. Flip `channels/Cargo.toml` default from `["email"]` to `[]`

**`mcp-http`:**
- `klyntbot-server/src/main.rs` — guard HTTP listener setup (stdio always available)

### Dead code removal

| Target | Location | Action |
|--------|----------|--------|
| `InsightCacheRepo` + `InsightCacheRow` | `cognitive/src/repos/insight_cache.rs` | Delete file, remove re-exports |
| `#[allow(dead_code)]` — voice-engine (2 files) | `voice-engine/src/service.rs`, `engines/whisper_local.rs` | Remove dead fields/methods |
| `#[allow(dead_code)]` — mcp client | `mcp/src/client/manager.rs` | Remove unused builder methods |
| `#[allow(dead_code)]` — skill-system persona | `skill-system/src/persona.rs` | Remove partial implementation |
| `#[allow(dead_code)]` — plugin-runtime | `plugin-runtime/src/manager.rs` | Skip — already gated behind plugin-integration |
| `#[allow(dead_code)]` — agent decomposition | `agent/src/handlers/decomposition.rs` | Keep — needed for proactive scan / cross-domain insights |
| `todo_embeddings` fallback | `tools/src/embedding/embedding_engine.rs` | Verify usage, remove if legacy-only |

**Rule:** Don't remove code with git blame <2 weeks old or tied to in-progress features.

### Expected binary savings

| Gate | Estimated Code Removed | Primary Dependency Savings |
|------|----------------------|--------------------------|
| `email` off | ~2K LOC | async-imap, lettre, native-tls (~1-2MB) |
| `plugin-integration` off | Already off | extism (~3-5MB, already not compiled) |
| `autotuner` off | ~3K LOC wiring | Minimal (no unique deps) |
| `mirror-narratives` off | ~1K LOC | Minimal |
| `weekly-reports` off | ~500 LOC | Minimal |
| `mcp-http` off | ~1K LOC | Potentially saves axum if not shared (verify) |
| Dead code removal | ~500-1K LOC | Cleaner binary, fewer monomorphizations |

Estimated total: **3-8MB** binary reduction.

---

## Section 4: Frontend Quick Wins

30-45 minutes of work, measurable perceived-speed improvement.

### 1. Vite manual chunks (`desktop-ui/vite.config.ts`)

```ts
output: {
  manualChunks: (id) => {
    if (id.includes('@tiptap') || id.includes('tiptap')) return 'vendor-tiptap';
    if (id.includes('three') || id.includes('react-force-graph')) return 'vendor-three';
    if (id.includes('mermaid')) return 'vendor-mermaid';
    if (id.includes('recharts') || id.includes('d3-')) return 'vendor-charts';
    if (id.includes('katex')) return 'vendor-katex';
  },
}
```

### 2. Lazy routes for non-core windows

**Window-level:**
- `/#/launcher` → `React.lazy(() => import('@features/launcher/LauncherView'))`
- `/#/tray` → `React.lazy(() => import('@features/tray/TrayView'))`
- `/#/distraction-overlay` → `React.lazy(() => import('@features/focus/DistractionOverlay'))`
- `/#/quick-capture` → `React.lazy(() => import('@features/capture/QuickCapture'))`

**Component-level (within main window):**
- Notes editor (Tiptap) → `React.lazy` behind notes tab
- Graph view (Three.js) → `React.lazy` behind graph/knowledge tab
- Finance dashboard (Recharts) → `React.lazy` behind finance tab
- Productivity dashboard (Recharts) → `React.lazy` behind productivity tab
- Mermaid renderer → `React.lazy` wherever diagrams appear

### 3. Smart fallbacks (personality, not spinners)

**Launcher + quick-capture:** Zero-skeleton contract. Fallback shows top-3 frequency results + "Thinking about your day..." with memory pulse dot. Data comes from Rust Tier 1 micro-warm (already in memory).

**Distraction overlay:** 150ms breathing micro-animation (soft glow pulsing in sync with memory pulse dot). Turns lazy-load moment into "your second brain is stepping in to protect you."

**All other lazy components:** Minimal skeleton via `<Suspense fallback={<Skeleton />}>`.

### 4. Production build

```ts
build: {
  minify: 'esbuild',
  target: 'esnext',
  sourcemap: false,
}
```

### Expected impact

| Window | Before | After |
|--------|--------|-------|
| Launcher (Cmd+Space) | ~9.3MB parsed | ~2MB (core only) |
| Tray popup | ~9.3MB parsed | ~1.5MB |
| Main window (chat tab) | ~9.3MB parsed | ~3MB |
| Main + notes tab | N/A | +2-3MB on tab switch |
| Main + graph tab | N/A | +500KB on tab switch |

Estimated **200-400ms faster perceived launch** for launcher and tray on M1 Air.

---

## Section 5: Code Quality & Concurrency Sweep

### Arc<Mutex<>> → RwLock conversions (app-core state)

| Field | Read Freq | Write Freq | Action |
|-------|-----------|------------|--------|
| `ChannelManager` | Every message | Init only | → `RwLock` |
| `ProductivityEngine` | Every focus check | Config change | → `RwLock` |
| `SignalAccumulator` | Every message | Threshold update | → `RwLock` |
| `PatternDetector` | Every message | Pattern learned | → `RwLock` |
| `UserSituation` | Every message | Situation change | → `RwLock` |
| `CoachingService` | Dashboard query | Config change | → `RwLock` |
| `InterventionRouter` | Intervention check | Route update | → `RwLock` (moves to Tier 2 lazy) |

**Keep as Mutex:** `NudgeService` (write-heavy), `DistractionInterceptor` (write-heavy), `FeedbackTracker` (balanced).

### DashMap simplification

| Current | Location | Action |
|---------|----------|--------|
| DashMap for MCP circuit breaker | `mcp/src/circuit_breaker.rs` | → `AtomicU32` + `AtomicBool` (1-2 keys only) |
| DashMap for context_engine circuit breaker | `context_engine/src/insight_forge/` | → `AtomicU32` + `AtomicBool` |
| All other DashMaps (15) | Various | Keep — genuinely concurrent with dynamic keys |

### Hot path: chat pipeline

**Concurrent prefetch with 800ms timeout:**
```rust
let (rewritten, memories, intent) = tokio::join!(
    tokio::time::timeout(Duration::from_millis(800), rewrite_query(&msg)),
    tokio::time::timeout(Duration::from_millis(800), prefetch_memories(&msg)),
    tokio::time::timeout(Duration::from_millis(800), analyze_intent(&msg)),
);
```
On timeout, proceed with default/empty result and emit `agent:thinking-light` event (lights up memory pulse dot with "Pulling what I already know about you...").

**Tool execution semaphore (pressure-aware):**
```rust
let max_tools = match pressure_monitor.current_pressure() {
    PressureLevel::Normal   => 10,
    PressureLevel::Elevated => 6,
    PressureLevel::Critical => 3,
};
```

---

## Section 6: Execution Timeline

### Week 1: Measure, Gate, Fix Biggest Pain Points

**Day 1-2:** Baseline & binary diet
- `cargo bloat` on M1 Air. Record binary size, section breakdown, top 50 functions.
- Flip `email` default off. Add new feature flags (`autotuner`, `mirror-narratives`, `weekly-reports`, `mcp-http`, `full`).
- `#[cfg]` guards at wiring level for all Tier 3 features.
- Measure binary size delta.

**Day 3:** Cron thundering herd fix
- BackgroundSemaphore (pressure-aware dynamic 0/1/2) in cron handler.
- Jitter windows for morning LLM jobs.
- Chain weekly_reflection → weekly_report (sequential).
- Move proactive_scan from cron to trigger-based.

**Day 4-5:** Lazy init for Tier 2 services
- `Arc<RwLock<Option<T>>>` for LanceDB, fastembed, voice, launcher full cache, price service.
- Idle-release timers.
- Launcher micro-warm (2KB frequency table + top-10).
- Restructure AppCore::init() into 7-phase order.

**Week 1 milestone:** Build with `default = []`, launch on M1 Air:
- Binary size: <100MB
- Cold launch: <2.0s
- Idle RAM: <200MB

### Week 2: Runtime Awareness & Concurrency Sweep

**Day 6-7:** SystemPressureMonitor
- Implement in `platform-macos` using macOS native APIs.
- 5-second polling, AtomicU8 storage.
- Wire into AppCore as `Arc<SystemPressureMonitor>`.

**Day 8:** Wire pressure into services
- Cron handler: check + reschedule if Critical.
- Lazy services: shorten idle threshold under Elevated, force-release under Critical.
- Mirror subscribers: drop events under Critical.
- Tool semaphore: pressure-aware (10/6/3).

**Day 9-10:** Concurrency refactor
- 7x Mutex → RwLock in app-core state.
- 2x DashMap → AtomicU32 + AtomicBool.
- Hot path: concurrent prefetch with 800ms timeout + thinking-light event.

**Week 2 milestone:**
- Idle RAM: <170MB
- Background CPU (30 min): <3%
- Morning cron: max 2 concurrent under Normal, 1 under Elevated
- Cold launch: <2.0s

### Week 3: Frontend, Dead Code, Final Polish

**Day 11:** Frontend quick wins
- manualChunks (5 vendor splits).
- React.lazy for all non-core routes + heavy components.
- Smart fallbacks (launcher personality, distraction breathing animation).
- Verify production build settings.

**Day 12:** Dead code & quality
- Delete InsightCacheRepo + InsightCacheRow.
- Audit 12 `#[allow(dead_code)]` files (keep decomposition handlers).
- Verify todo_embeddings usage.
- `cargo clippy --workspace --all-targets --all-features` — zero warnings.
- `cargo nextest run --workspace` + `--features full` — all green.

**Day 13-15:** Final benchmark & polish
- Full benchmark suite on M1 Air 8GB (with Safari 5 tabs + Messages + Slack).
- Fix anything exceeding hard-fail thresholds.
- Profile with Instruments if between target and hard-fail.

### Verification Gate

All targets must pass on real M1 Air 8GB hardware. No simulated, no Pro.

**Qualitative personality check (manual, non-negotiable):**
- Open launcher 5 times (Cmd+Space) → must feel "it was just there"
- Trigger distraction overlay during focus → must feel "it was already watching"
- Send one chat message after cold launch → must feel "it already knows me"

If hard-fail thresholds are exceeded or personality check fails, fix before moving to UX polish.

### Post-Diet Rule

Never touch the loading/init/pressure layer again unless a metric regression is detected. Move straight into UX polish:
1. Memory pulse visibility
2. Proactive cross-domain insights
3. Protective focus bubble
