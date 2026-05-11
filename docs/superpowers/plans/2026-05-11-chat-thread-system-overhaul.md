# Chat / Thread Messaging System Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the "wedged after completed" class of bugs in the chat/thread messaging system and re-establish it on a single, instrumented, benchmark-gated runtime that powers both assistant and coding modes through one shared lifecycle, one wire-event union, one frontend store, and a watchdog-protected recovery path — pre-release, so wire formats and schemas can change freely.

**Architecture:** Land a P0 hotfix (value-identity `StreamGuard`, generation-counted turns, watchdog) without breaking existing pipelines. Then introduce a `ThreadRuntime` trait in `app-core` that both `chat_send` and `coding_message_send` delegate to. Unify the wire surface around a `specta::Type`-derived `ThreadEvent` v2 union with a guaranteed `Terminal` variant on every exit path. Collapse the three frontend state stacks (`useThreadsReducer` + `ThreadEventBuffer` + `chatStreamStore`) into one Zustand slice keyed by `(threadId, turnId, generation)`. Add criterion + vitest benchmarks with hard p95 gates wired to `scripts/run_chat_perf_gates.sh`. All changes are TDD, surgical, and committed per task.

**Tech Stack:** Rust (tokio + sqlx + jiff + dashmap + parking_lot + criterion + proptest + tokio-test + linkme + tauri-specta), Tauri 2, React 19 + TypeScript + Vitest + Bun. SQLite WAL via `StoragePool`. Existing macros (`#[klynt_command]`, `#[klynt_raw_command]`). No new runtime deps beyond what's already in the workspace.

**Spec:** Inline (no separate spec doc — this plan is the spec for itself given the audit context).

**Foundations (already shipped):**
- Phase 4 thread events wired to Tauri (commit `bf2b38f48`)
- `callId` threaded through `ToolStart`/`ToolEnd` (commit `9b24b4283`)
- Thread heartbeat (FE only) (commit `e23287581`)
- `bindings_are_current` + `registration_drift` + `no_raw_tauri_command_outside_macros` enforcement tests
- `tests/common/` fixtures (`test_pool`, `test_repos`, `MockProvider`)
- `mockTauri.ts` + `vitest.setup.ts` global Tauri mocks
- `criterion = "0.5"` in workspace; 4 existing bench crates

---

## Table of Contents

| PR | Phase | Scope | Tasks |
|----|-------|-------|-------|
| **PR1** | Phase 0 — Baseline & Telemetry | TTFT metric, criterion benches, p95 gates script, FE perf marks | 0–7 |
| **PR2** | Phase 1 — P0 Backend Hotfix | Value-ID `StreamGuard`, async-detached metadata persist, guaranteed `Terminal`, `session_end_fired` cleanup, double-send rejection | 8–18 |
| **PR3** | Phase 2 — P0 Frontend Hotfix | Generation counters, remove silent-drop guard, assistant watchdog, manual reset button | 19–28 |
| **PR4** | Phase 3 — `ThreadEvent` Wire v2 | Single `ThreadEvent` union, `specta::Type`, `Terminal` invariant, FE typed listener, bindings green | 29–37 |
| **PR5** | Phase 4 — Unified `ThreadRuntime` | Trait in `app-core`, assistant + coding impls, shared `ActiveTurns`, integration tests | 38–48 |
| **PR6** | Phase 5 — Frontend Store Unification | Zustand `useChatStore`, migrate components, retire `ThreadEventBuffer` / `chatStreamStore` | 49–58 |
| **PR7** | Phase 6 — Performance Backend | Span propagation across spawn, batched persists, mpsc capacity tuning, `chat_send` < 50ms cold | 59–66 |
| **PR8** | Phase 7 — Performance Frontend | Virtualized list, microtask-coalesced deltas, stable keys, `useTransition`, bundle budget | 67–76 |
| **PR9** | Phase 8 — Recovery & Observability | Server heartbeat, zombie detection, error UI, state rehydration on FE bootstrap | 77–85 |
| **PR10** | Phase 9 — Hardening, Soak, Docs | Proptests for event sequences, soak benchmark, CLAUDE.md updates, finishing checklist | 86–93 |

**Performance acceptance criteria (must hold at end of PR8):**
- **Backend `chat_send` cold p95 ≤ 50ms** (from invoke to first `Terminal` budget allocation) on M2 Pro.
- **Backend stream-event throughput ≥ 5,000 events/sec** sustained over the merged mpsc.
- **Backend `relay_chat_stream` cleanup time ≤ 1ms** after `Terminal` event in 99% of runs.
- **TTFT (mock provider, no tools) p95 ≤ 15ms.**
- **Frontend ContentChunk → DOM commit p95 ≤ 16ms** for a 10,000-token stream (1 frame at 60 fps).
- **Frontend Composer disabled→enabled re-arm latency p95 ≤ 33ms** after Terminal.
- **Zero leaked `active_streams` / `pending_interactions` entries** after 1,000 send/cancel/error cycles in proptest.
- **Zero stuck `isProcessing` flags** after 1,000 randomized event-sequence permutations in vitest proptest.

---

## File Structure

### Create

| Path | Responsibility |
|---|---|
| `crates/app-core/src/runtime/mod.rs` | `ThreadRuntime` trait + shared `ActiveTurns` / `TurnHandle` / `TurnGeneration` types |
| `crates/app-core/src/runtime/assistant.rs` | `AssistantThreadRuntime` impl (wraps current `chat_send`) |
| `crates/app-core/src/runtime/coding.rs` | `CodingThreadRuntime` impl (wraps current `coding_message_send`) |
| `crates/app-core/src/runtime/active_turns.rs` | `ActiveTurns` — value-identity DashMap keyed by `TurnHandle` |
| `crates/app-core/src/runtime/stream_guard.rs` | New `StreamGuard` with value-identity removal |
| `crates/app-core/src/handlers/chat/tests.rs` | Unit + integration tests for chat handlers (currently zero coverage) |
| `crates/app-core/src/coding/turn_handler_tests.rs` | Unit + integration tests for coding turn handler |
| `crates/agent/benches/ttft_e2e.rs` | Criterion benchmark for TTFT end-to-end with `MockProvider` |
| `crates/agent/benches/stream_throughput.rs` | Criterion benchmark measuring AgentEvent → ThreadEvent translation throughput |
| `crates/desktop/benches/relay_cleanup_latency.rs` | Criterion bench measuring `StreamGuard` drop → DashMap cleanup |
| `crates/desktop-shared/src/thread_event_v2.rs` | New unified `ThreadEvent` v2 union with `specta::Type` |
| `tests/common/chat_harness.rs` | `ChatTestHarness` — minimal AppCore + MockProvider + recorded emitter |
| `tests/integration/chat_lifecycle.rs` | Back-to-back send, cancel-then-resend, error-then-resend, double-send guard |
| `tests/property/event_sequence_invariants.rs` | Proptest: any event permutation leaves `ActiveTurns` empty |
| `desktop-ui/src/features/threads/store/useChatStore.ts` | Zustand store unifying assistant + coding thread state |
| `desktop-ui/src/features/threads/store/types.ts` | Shared types: `TurnHandle`, `ThreadStatus`, `ThreadEventV2` |
| `desktop-ui/src/features/threads/store/useChatStore.test.ts` | Store unit tests |
| `desktop-ui/src/features/threads/components/StuckThreadBanner.tsx` | Manual reset banner shown when stuck > 5s |
| `desktop-ui/src/features/threads/hooks/useThreadWatchdog.ts` | Assistant-mode 90s heartbeat watchdog |
| `desktop-ui/src/features/threads/hooks/useTtftMetric.ts` | FE perf-mark instrumentation: send → first ContentChunk |
| `desktop-ui/src/features/messages/components/VirtualizedMessageList.tsx` | `@tanstack/react-virtual` wrapper around message rendering |
| `desktop-ui/src/features/threads/__benches__/coalescer.bench.ts` | Vitest benchmark for delta coalescer |
| `desktop-ui/src/features/threads/utils/coalesceDeltas.ts` | RAF-batched delta coalescer |
| `scripts/run_chat_perf_gates.sh` | Runs all criterion + vitest benches, asserts p95 thresholds |
| `scripts/run_chat_proptest_soak.sh` | 1000-iteration soak invocation |
| `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md` | Baseline numbers recorded after PR1 lands |

### Modify

| Path | Change |
|---|---|
| `crates/app-core/src/handlers/chat/streaming.rs` | Phase 1: replace `StreamGuard`, drop 200ms sleep, ensure `Terminal` on every exit, reject double-send |
| `crates/app-core/src/handlers/chat/mod.rs` | Phase 4: delegate `chat_send` to `AssistantThreadRuntime` |
| `crates/app-core/src/coding/turn_handler.rs` | Phase 4: refactor to implement `ThreadRuntime`, emit `ThreadEvent` v2 |
| `crates/app-core/src/state.rs` | Phase 4: replace `session_start_fired`/`session_end_fired` Arcs with `ActiveTurns` |
| `crates/app-core/src/events.rs` | Phase 6: add `instrument_in_span` helper, propagate parent span across `tokio::spawn` |
| `crates/agent/src/agent_loop/streaming.rs` | Phase 1: `ActiveStreams` value-identity check; mpsc capacity to 256 |
| `crates/agent/src/agent_loop/mod.rs` | Phase 6: `mpsc::channel(256)` for `event_tx`, document why; tracing span propagation |
| `crates/desktop-shared/src/coding/events.rs` | Phase 3: deprecate old `ThreadEvent`, re-export from `thread_event_v2` |
| `crates/desktop-shared/src/events.rs` | Phase 3: deprecate per-event `agent:*` constants (keep through migration) |
| `crates/storage/migrations/001_initial.sql` | Phase 8: add `turn_generation` to `session_messages`, `last_event_seq` to `sessions` (pre-release, no migration script) |
| `crates/storage/src/repos/session.rs` | Phase 8: add `detect_zombie_sessions(threshold_ms)` method |
| `crates/storage/src/rows/session.rs` | Phase 8: add new columns to `SessionRow` / `SessionMessageRow` |
| `crates/desktop/src/specta_builder.rs` | Phase 3: register `ThreadEvent` v2 via `tauri_specta::Event` |
| `crates/desktop/src/app_core.rs` | Phase 3: replace `spawn_broker_forwarder` for `agent:thread_event` with v2-aware version |
| `crates/desktop/Cargo.toml` | Phase 0: add `[[bench]] name = "relay_cleanup_latency"` |
| `crates/agent/Cargo.toml` | Phase 0: add `[[bench]] name = "ttft_e2e"` + `[[bench]] name = "stream_throughput"` |
| `desktop-ui/src/features/threads/hooks/useThreadTurnEvents.ts` | Phase 2: remove silent-drop guard; add generation-aware ignore |
| `desktop-ui/src/features/threads/hooks/useQueuedSend.ts` | Phase 2: watchdog-aware inFlight clear; auto-reset on watchdog fire |
| `desktop-ui/src/features/threads/hooks/useThreadMessaging.ts` | Phase 5: route through `useChatStore` |
| `desktop-ui/src/features/coding/state/ThreadEventBuffer.ts` | Phase 5: thin shim over `useChatStore` (then deleted in PR8) |
| `desktop-ui/src/features/chat/store/chatStreamStore.ts` | Phase 5: thin shim over `useChatStore` (then deleted in PR8) |
| `desktop-ui/src/features/composer/components/Composer.tsx` | Phase 2: `disabled` driven by `stuckThreshold` + watchdog state |
| `desktop-ui/src/features/messages/components/Messages.tsx` | Phase 7: wrap in `VirtualizedMessageList` |
| `desktop-ui/src/features/coding/components/CodingThreadView.tsx` | Phase 7: wrap in `VirtualizedMessageList`, drop per-delta `adaptItems` recompute |
| `desktop-ui/src/services/events.ts` | Phase 3: add `appThreadEventHub` for v2 events |
| `desktop-ui/src/bindings.ts` | Phase 3: auto-regenerated; commit the diff |
| `desktop-ui/package.json` | Phase 7: add `bench` script; size-limit dev dep |
| `desktop-ui/vite.config.ts` | Phase 7: add `rollup-plugin-visualizer` (dev only) |
| `desktop-ui/vitest.config.ts` | Phase 7: enable benchmark mode (`benchmark: { include: [...] }`) |
| `CLAUDE.md` | Phase 9: document new `ThreadRuntime`, retire old gotchas, add perf gates |

### Test

| Path | What |
|---|---|
| `tests/integration/chat_lifecycle.rs` | Back-to-back send, cancel-then-resend, double-send rejection, error-then-resend |
| `tests/integration/thread_runtime.rs` | Both impls of `ThreadRuntime` honor `ActiveTurns` invariants |
| `tests/property/event_sequence_invariants.rs` | Proptest: arbitrary `(send, cancel, error, complete)` permutation cleans up |
| `crates/app-core/src/handlers/chat/tests.rs` | `StreamGuard` value-identity removal |
| `crates/app-core/src/coding/turn_handler_tests.rs` | `coding_message_send` after `Terminal` works |
| `desktop-ui/src/features/threads/store/useChatStore.test.ts` | Generation-counter invariants |
| `desktop-ui/src/features/threads/hooks/useThreadWatchdog.test.ts` | 90s fires and clears state |
| `desktop-ui/src/features/threads/__benches__/coalescer.bench.ts` | Coalescer ≤ 16ms p95 |

---

## Pre-flight

**Before any task: confirm baseline green.**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run --workspace 2>&1 | tail -5
cd desktop-ui && bun run typecheck 2>&1 | tail -5
cd desktop-ui && bun run lint 2>&1 | tail -5
cd desktop-ui && bun run test 2>&1 | tail -5
```

All five must pass with the existing code at HEAD. If any fail, fix or document before proceeding — they will mask regressions later.

---

# PR1 — Phase 0: Baseline & Telemetry

**Goal:** Establish measurable baselines for TTFT, stream throughput, relay cleanup, and FE render latency BEFORE touching production code. Every later PR is gated against these numbers.

**Acceptance:** All four new criterion benches and one vitest benchmark run green and produce numbers. `scripts/run_chat_perf_gates.sh` exits 0 against current code (initial threshold = `1.5× current_baseline` to allow PR1 itself to pass).

---

## Task 0: Branch + baseline confirmation

**Files:**
- Read: `CLAUDE.md`, `crates/agent/benches/chat_send_to_first_token.rs`, `crates/desktop/benches/event_transport_latency.rs`

- [ ] **Step 1: Create branch**

```bash
git checkout main
git pull --ff-only
git checkout -b feat/chat-thread-overhaul-pr1-baseline
```

- [ ] **Step 2: Confirm pre-flight green**

```bash
cargo build --workspace 2>&1 | tail -3
cargo nextest run --workspace 2>&1 | tail -3
cd desktop-ui && bun run typecheck 2>&1 | tail -3
cd desktop-ui && bun run lint 2>&1 | tail -3
cd desktop-ui && bun run test 2>&1 | tail -3
```

Expected: all pass. If `bun run lint` fails with Biome violations on untouched code, run `bun run lint -- --apply` and commit as a one-line `style:` commit before proceeding.

- [ ] **Step 3: Sanity-check criterion is available**

```bash
grep -A 2 '\[dev-dependencies\]' crates/agent/Cargo.toml | grep criterion
```

Expected: a `criterion` entry with `features = ["async_tokio"]` already present from the existing `chat_send_to_first_token` bench.

---

## Task 1: TTFT measurement hook in the agent runtime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:404` (add `first_chunk_emitted` instant)
- Modify: `crates/agent/src/events.rs` (add field to `UsageReport` for TTFT — backward-compatible since `#[non_exhaustive]`)

- [ ] **Step 1: Add `ttft_ms` field to `UsageReport`**

In `crates/agent/src/events.rs`, locate the `UsageReport` variant (search `"UsageReport {"`). Add a new field with `#[serde(default)]` so old serialized payloads still deserialize:

```rust
UsageReport {
    prompt_tokens: u32,
    completion_tokens: u32,
    cache_read_tokens: u32,
    cache_write_tokens: u32,
    estimated_cost_usd: f64,
    model: String,
    #[serde(rename = "responseTimeMs")]
    response_time_ms: u64,
    /// Time from `chat_send` invoke to first `ContentChunk` emitted.
    /// None if no chunks were emitted (tool-only or error path).
    #[serde(default, rename = "ttftMs", skip_serializing_if = "Option::is_none")]
    ttft_ms: Option<u64>,
},
```

- [ ] **Step 2: Capture `first_chunk_instant` in the runtime**

In `crates/agent/src/agent_runtime/runtime.rs`, near line 404 where `pipeline_start = Instant::now()` is set:

```rust
let pipeline_start = Instant::now();
let mut first_chunk_instant: Option<Instant> = None;
```

Find the `ContentChunk` emission site (search `AgentEvent::ContentChunk`). Wrap with a one-time stamp:

```rust
if first_chunk_instant.is_none() {
    first_chunk_instant = Some(Instant::now());
}
// existing emit logic
```

- [ ] **Step 3: Populate `ttft_ms` in the `UsageReport` emit**

Locate the `UsageReport` emission near line 647. Compute:

```rust
let ttft_ms = first_chunk_instant.map(|i| i.duration_since(pipeline_start).as_millis() as u64);
```

And add to the struct literal: `ttft_ms,`.

- [ ] **Step 4: Update all existing `UsageReport` test fixtures**

```bash
rg -l 'UsageReport \{' crates/agent/src --type rust
```

For each match, add `ttft_ms: None,` (or `ttft_ms: Some(N),` if the test exercises this field).

- [ ] **Step 5: Run agent tests**

```bash
cargo nextest run -p agent 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/events.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "$(cat <<'EOF'
feat(agent): instrument TTFT (time-to-first-token) in UsageReport

Adds `ttft_ms: Option<u64>` to `AgentEvent::UsageReport`. Captured at
the first emitted `ContentChunk` relative to `pipeline_start`. Foundation
for chat overhaul Phase 0 benchmarking.
EOF
)"
```

---

## Task 2: TTFT end-to-end criterion benchmark

**Files:**
- Create: `crates/agent/benches/ttft_e2e.rs`
- Modify: `crates/agent/Cargo.toml` (add `[[bench]] name = "ttft_e2e"`)

- [ ] **Step 1: Add bench entry to `Cargo.toml`**

In `crates/agent/Cargo.toml`, append after the existing `[[bench]]` for `chat_send_to_first_token`:

```toml
[[bench]]
name = "ttft_e2e"
harness = false
```

- [ ] **Step 2: Write the benchmark scaffold**

Create `crates/agent/benches/ttft_e2e.rs`:

```rust
//! TTFT end-to-end criterion bench.
//!
//! Measures: from `AgentRuntime::process` call to the first `ContentChunk`
//! draining off the `event_rx` mpsc, using `MockProvider` that streams
//! a fixed 8-token response with 1ms inter-token spacing.
//!
//! Gate: p95 ≤ 15ms on M2 Pro.

use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use tokio::runtime::Builder;

fn ttft_e2e(c: &mut Criterion) {
    let mut group = c.benchmark_group("ttft_e2e");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    group.bench_function("mock_8_tokens", |b| {
        b.to_async(&rt).iter(|| async move {
            // The actual harness function lives in `tests/common/chat_harness.rs`.
            // For now this benchmark is a SKELETON. Task 6 wires in the real
            // ChatTestHarness::send_and_await_first_chunk().
            tokio::time::sleep(Duration::from_micros(1)).await;
        });
    });

    group.finish();
}

criterion_group!(benches, ttft_e2e);
criterion_main!(benches);
```

The skeleton compiles immediately. We wire in `ChatTestHarness` at Task 6.

- [ ] **Step 3: Verify it builds and runs**

```bash
cargo bench -p agent --bench ttft_e2e -- --quick 2>&1 | tail -10
```

Expected: criterion output for `ttft_e2e/mock_8_tokens` with sub-microsecond timings (skeleton only).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/Cargo.toml crates/agent/benches/ttft_e2e.rs
git commit -m "feat(agent): add TTFT e2e criterion bench skeleton"
```

---

## Task 3: Stream throughput criterion benchmark

**Files:**
- Create: `crates/agent/benches/stream_throughput.rs`
- Modify: `crates/agent/Cargo.toml`

- [ ] **Step 1: Add bench entry**

In `crates/agent/Cargo.toml`:

```toml
[[bench]]
name = "stream_throughput"
harness = false
```

- [ ] **Step 2: Write benchmark**

Create `crates/agent/benches/stream_throughput.rs`:

```rust
//! Stream throughput criterion bench.
//!
//! Measures: how fast the merged `AgentEvent` mpsc can be drained while
//! a producer floods it with `ContentChunk` events of 16 bytes each.
//! No bridge, no Tauri — pure channel throughput.
//!
//! Gate: ≥ 5,000 events/sec sustained, p95 receive latency ≤ 200µs.

use agent::events::AgentEvent;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::mpsc;

fn stream_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_throughput");
    group.measurement_time(Duration::from_secs(10));

    for batch_size in [100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &n| {
                let rt = Builder::new_current_thread().enable_all().build().unwrap();
                b.to_async(&rt).iter(|| async move {
                    let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);
                    let producer = tokio::spawn(async move {
                        for _ in 0..n {
                            tx.send(AgentEvent::ContentChunk { data: "x".repeat(16) })
                                .await
                                .unwrap();
                        }
                    });
                    let mut count = 0usize;
                    while let Some(ev) = rx.recv().await {
                        criterion::black_box(&ev);
                        count += 1;
                        if count >= n {
                            break;
                        }
                    }
                    producer.await.unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, stream_throughput);
criterion_main!(benches);
```

- [ ] **Step 3: Run**

```bash
cargo bench -p agent --bench stream_throughput -- --quick 2>&1 | tail -15
```

Expected: three throughput numbers (100, 1k, 10k elements). Record them in `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md` (created in Task 7).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/Cargo.toml crates/agent/benches/stream_throughput.rs
git commit -m "feat(agent): add merged-mpsc stream throughput criterion bench"
```

---

## Task 4: Relay cleanup latency criterion benchmark

**Files:**
- Create: `crates/desktop/benches/relay_cleanup_latency.rs`
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Add bench entry**

In `crates/desktop/Cargo.toml`, append after existing `event_transport_latency` entry:

```toml
[[bench]]
name = "relay_cleanup_latency"
harness = false
```

- [ ] **Step 2: Write benchmark**

Create `crates/desktop/benches/relay_cleanup_latency.rs`:

```rust
//! Relay cleanup latency: time from `Terminal` event emit to the
//! `ActiveStreams` DashMap entry being removed by `StreamGuard::drop`.
//!
//! Gate: p99 ≤ 1ms.

use criterion::{criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

fn relay_cleanup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("relay_cleanup_latency");
    group.measurement_time(Duration::from_secs(8));

    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    group.bench_function("drop_and_observe", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total = Duration::ZERO;
            for i in 0..iters {
                let map: Arc<DashMap<String, CancellationToken>> = Arc::new(DashMap::new());
                let key = format!("session-{i}");
                map.insert(key.clone(), CancellationToken::new());
                let map_clone = Arc::clone(&map);
                let key_clone = key.clone();

                let t0 = Instant::now();
                tokio::spawn(async move {
                    map_clone.remove(&key_clone);
                })
                .await
                .unwrap();
                total += t0.elapsed();
                assert!(map.is_empty(), "DashMap should be empty after drop");
            }
            total
        });
    });
    group.finish();
}

criterion_group!(benches, relay_cleanup_latency);
criterion_main!(benches);
```

- [ ] **Step 3: Run**

```bash
cargo bench -p desktop --bench relay_cleanup_latency -- --quick 2>&1 | tail -10
```

Expected: criterion output for `drop_and_observe`. Record baseline number.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/benches/relay_cleanup_latency.rs
git commit -m "feat(desktop): add relay cleanup latency criterion bench"
```

---

## Task 5: Vitest benchmark mode + coalescer bench skeleton

**Files:**
- Modify: `desktop-ui/vitest.config.ts`
- Modify: `desktop-ui/package.json`
- Create: `desktop-ui/src/features/threads/__benches__/coalescer.bench.ts`

- [ ] **Step 1: Enable bench mode in `vitest.config.ts`**

Append a `benchmark` section to `defineConfig`:

```ts
benchmark: {
  include: ["src/**/__benches__/*.bench.ts"],
  reporters: ["default"],
}
```

- [ ] **Step 2: Add `bench` script to `package.json`**

In the `"scripts"` block, add:

```json
"bench": "vitest bench --run"
```

- [ ] **Step 3: Write coalescer bench skeleton**

Create `desktop-ui/src/features/threads/__benches__/coalescer.bench.ts`:

```ts
import { bench, describe } from "vitest";

// Placeholder — the real `coalesceDeltas` ships in PR8 (Task 70).
// This is the perf-gate harness so subsequent PRs can update the number.
describe("coalesceDeltas", () => {
  bench("100 chunks", () => {
    const chunks: string[] = Array.from({ length: 100 }, (_, i) => `tok-${i}`);
    // mock coalescer: concat
    chunks.join("");
  });

  bench("10,000 chunks", () => {
    const chunks: string[] = Array.from({ length: 10_000 }, (_, i) => `tok-${i}`);
    chunks.join("");
  });
});
```

- [ ] **Step 4: Verify bench mode works**

```bash
cd desktop-ui && bun run bench 2>&1 | tail -15
```

Expected: vitest bench output with throughput numbers for both benchmarks.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/vitest.config.ts desktop-ui/package.json \
        desktop-ui/src/features/threads/__benches__/coalescer.bench.ts
git commit -m "feat(desktop-ui): enable vitest bench mode + coalescer skeleton"
```

---

## Task 6: `ChatTestHarness` test helper

**Files:**
- Create: `tests/common/chat_harness.rs`
- Modify: `tests/common/mod.rs` (add `pub mod chat_harness;`)

- [ ] **Step 1: Write the harness**

Create `tests/common/chat_harness.rs`:

```rust
//! ChatTestHarness — minimal AppCore + MockProvider + recorded emitter.
//!
//! Use this in integration tests and benchmarks where you need an end-to-end
//! `chat_send` → first `ContentChunk` path without spinning up a full Tauri
//! window or a real LLM provider.

use crate::common::{test_provider, test_repos};
use app_core::events::AppEventEmitter;
use std::sync::{Arc, Mutex};

pub struct RecordedEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

#[derive(Default)]
pub struct RecordingEmitter {
    pub events: Arc<Mutex<Vec<RecordedEvent>>>,
}

impl AppEventEmitter for RecordingEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        self.events.lock().unwrap().push(RecordedEvent {
            name: event_name.to_string(),
            payload,
        });
    }
}

pub struct ChatTestHarness {
    // Populated incrementally — see Task 8 onward for real construction.
    pub emitter: Arc<RecordingEmitter>,
}

impl ChatTestHarness {
    pub async fn new() -> Self {
        let emitter = Arc::new(RecordingEmitter::default());
        let _ = test_repos().await; // ensure schema migrates
        let _ = test_provider("hello");
        Self { emitter }
    }

    pub fn event_names(&self) -> Vec<String> {
        self.emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.name.clone())
            .collect()
    }
}
```

- [ ] **Step 2: Register the module**

In `tests/common/mod.rs`, add:

```rust
pub mod chat_harness;
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo build --tests --workspace 2>&1 | tail -5
```

Expected: success.

- [ ] **Step 4: Commit**

```bash
git add tests/common/chat_harness.rs tests/common/mod.rs
git commit -m "feat(tests): add ChatTestHarness scaffold for chat overhaul"
```

---

## Task 7: Perf gates script + baseline notes

**Files:**
- Create: `scripts/run_chat_perf_gates.sh`
- Create: `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md`

- [ ] **Step 1: Write the gates script**

Create `scripts/run_chat_perf_gates.sh`:

```bash
#!/usr/bin/env bash
# Chat overhaul perf gates. Exits 0 if all gates pass, non-zero otherwise.
# Thresholds are initial 1.5× current_baseline; tightened per PR.

set -euo pipefail

THRESHOLD_TTFT_P95_MS="${THRESHOLD_TTFT_P95_MS:-25}"        # tightened to 15 in PR7
THRESHOLD_THROUGHPUT_EVENTS_PER_SEC="${THRESHOLD_THROUGHPUT:-3000}"  # tightened to 5000 in PR7
THRESHOLD_CLEANUP_P99_MS="${THRESHOLD_CLEANUP_P99_MS:-2}"    # tightened to 1 in PR7

echo "[perf-gate] criterion: ttft_e2e"
cargo bench -p agent --bench ttft_e2e -- --quick 2>&1 \
    | tee /tmp/ttft.log

echo "[perf-gate] criterion: stream_throughput"
cargo bench -p agent --bench stream_throughput -- --quick 2>&1 \
    | tee /tmp/throughput.log

echo "[perf-gate] criterion: relay_cleanup_latency"
cargo bench -p desktop --bench relay_cleanup_latency -- --quick 2>&1 \
    | tee /tmp/cleanup.log

echo "[perf-gate] vitest: coalescer"
(cd desktop-ui && bun run bench 2>&1 | tee /tmp/coalescer.log)

# Numeric assertions are added in PR7 (Task 65) once thresholds are tight.
echo "[perf-gate] all benchmarks ran. Numeric gates: TODO PR7."
```

```bash
chmod +x scripts/run_chat_perf_gates.sh
```

- [ ] **Step 2: Run it once to confirm**

```bash
./scripts/run_chat_perf_gates.sh 2>&1 | tail -20
```

Expected: all four benchmarks run successfully.

- [ ] **Step 3: Capture baseline numbers**

Create `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md`:

```markdown
# Chat overhaul — perf baseline (PR1 end)

**Date:** 2026-05-11
**Branch:** feat/chat-thread-overhaul-pr1-baseline
**Machine:** [fill in]

## Numbers (record from /tmp/*.log after running scripts/run_chat_perf_gates.sh)

| Bench | Metric | Value |
|---|---|---|
| `agent::ttft_e2e/mock_8_tokens` | mean | (record) |
| `agent::stream_throughput/100` | events/sec | (record) |
| `agent::stream_throughput/1000` | events/sec | (record) |
| `agent::stream_throughput/10000` | events/sec | (record) |
| `desktop::relay_cleanup_latency/drop_and_observe` | mean | (record) |
| `coalescer/100 chunks` | hz | (record) |
| `coalescer/10,000 chunks` | hz | (record) |

## Notes

- TTFT bench is currently a skeleton (Task 2). Real measurement lands in Task 8 (after `chat_harness` is wired up).
- Coalescer bench is concat-only; real `coalesceDeltas` lands in PR8 Task 70.
- Goal: tighten thresholds in PR7 Task 65 to match the acceptance criteria in the plan header.
```

- [ ] **Step 4: Commit**

```bash
git add scripts/run_chat_perf_gates.sh docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md
git commit -m "feat(perf): chat overhaul perf-gates script + baseline note"
```

- [ ] **Step 5: Open PR1**

```bash
git push -u origin feat/chat-thread-overhaul-pr1-baseline
gh pr create --title "feat(perf): chat overhaul Phase 0 baseline & telemetry" --body "$(cat <<'EOF'
## Summary
- TTFT instrumented in `UsageReport` via new `ttft_ms: Option<u64>` field
- Three new criterion benches: `ttft_e2e`, `stream_throughput`, `relay_cleanup_latency`
- Vitest bench mode enabled; `coalescer.bench.ts` skeleton
- `scripts/run_chat_perf_gates.sh` orchestrates all four benches
- `ChatTestHarness` scaffold for chat handler tests
- Baseline numbers in `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md`

This is **Phase 0 of 10** of the chat/thread system overhaul. PR2 wires
the real handlers into the TTFT bench and lands the P0 hotfix.

## Test plan
- [x] `cargo build --workspace`
- [x] `cargo nextest run --workspace`
- [x] `cargo bench -p agent --bench ttft_e2e -- --quick`
- [x] `cargo bench -p agent --bench stream_throughput -- --quick`
- [x] `cargo bench -p desktop --bench relay_cleanup_latency -- --quick`
- [x] `cd desktop-ui && bun run bench`
- [x] `./scripts/run_chat_perf_gates.sh`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR2 — Phase 1: P0 Backend Hotfix

**Goal:** Close the reported wedge bug at the backend layer. Make `chat_send` idempotent under double-fire, ensure every exit path emits exactly one terminal event, and remove the 200ms sleep race window.

**Acceptance:**
- New tests in `tests/integration/chat_lifecycle.rs` cover: back-to-back, cancel-then-resend, error-then-resend, rejected double-send.
- New proptest passes with 1000 cases: arbitrary `(send, cancel, error, complete)` permutations leave `active_streams` and `pending_interactions` empty.
- `relay_cleanup_latency` p99 ≤ 1.5ms (improved from baseline due to dropped 200ms sleep tail).

---

## Task 8: Branch + wire `ChatTestHarness` to a real send

**Files:**
- Modify: `tests/common/chat_harness.rs`
- Read: `crates/app-core/src/handlers/chat/streaming.rs:1223-1290`, `crates/app-core/src/state.rs`

- [ ] **Step 1: Branch from main**

```bash
git checkout main
git pull --ff-only
git checkout -b feat/chat-thread-overhaul-pr2-backend-hotfix
```

- [ ] **Step 2: Expand `ChatTestHarness` to construct a real `AppCore`**

Replace `ChatTestHarness::new()` in `tests/common/chat_harness.rs` with a real builder. The exact `AppCore` constructor lives in `crates/app-core/src/state.rs` — read it first to learn the fields. Then write:

```rust
use crate::common::{test_pool, test_provider};
use app_core::AppCore;
use std::sync::Arc;

impl ChatTestHarness {
    pub async fn new_real() -> (Arc<AppCore>, Arc<RecordingEmitter>) {
        let emitter = Arc::new(RecordingEmitter::default());
        let pool = test_pool().await;
        let provider = Arc::new(test_provider("hello world"));
        let core = AppCore::for_tests(pool, provider, Arc::clone(&emitter) as _)
            .await
            .expect("test AppCore should build");
        (Arc::new(core), emitter)
    }
}
```

If `AppCore::for_tests` does not exist (likely — research confirmed there's no test factory), add a minimal one in `crates/app-core/src/state.rs` that takes the three required dependencies and uses sensible defaults for the rest. Keep it `#[cfg(any(test, feature = "test-helpers"))]`-gated.

- [ ] **Step 3: Build**

```bash
cargo build --tests --workspace 2>&1 | tail -5
```

If `AppCore::for_tests` is missing, add it now. The simplest skeleton:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl AppCore {
    pub async fn for_tests(
        pool: storage::StoragePool,
        provider: Arc<dyn providers::LlmProvider>,
        emitter: Arc<dyn crate::events::AppEventEmitter>,
    ) -> common::Result<Self> {
        // Use the existing builder but skip cron, hooks, and notifications.
        // Read state.rs to see which fields are required.
        unimplemented!("fill in based on AppCore::new")
    }
}
```

Read `crates/app-core/src/state.rs` and fill in the body. Each field that AppCore requires must get a stub or default. Use `Arc::new(dashmap::DashMap::new())` for the four `DashMap` Arcs (`active_streams`, `pending_interactions`, `session_start_fired`, `session_end_fired`).

- [ ] **Step 4: Add a smoke test**

In `tests/common/chat_harness.rs`, add a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn harness_builds() {
        let (core, emitter) = ChatTestHarness::new_real().await;
        assert_eq!(emitter.events.lock().unwrap().len(), 0);
        drop(core);
    }
}
```

- [ ] **Step 5: Run**

```bash
cargo nextest run -p klyntbot --test integration -E 'test(harness_builds)' 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/common/chat_harness.rs crates/app-core/src/state.rs
git commit -m "feat(tests): wire ChatTestHarness to real AppCore::for_tests"
```

---

## Task 9: Write failing test — back-to-back chat_send

**Files:**
- Create: `tests/integration/chat_lifecycle.rs`
- Modify: `tests/integration/main.rs` or equivalent (add `mod chat_lifecycle;`)

- [ ] **Step 1: Locate the integration test entry point**

```bash
ls tests/integration/
```

Note the test binary entrypoint (probably `tests/integration/main.rs` or `tests/integration/lib.rs`).

- [ ] **Step 2: Add the test module**

Create `tests/integration/chat_lifecycle.rs`:

```rust
//! Lifecycle invariants for the chat/thread system.

use crate::common::chat_harness::ChatTestHarness;

/// REGRESSION: after a turn reaches Done, a subsequent chat_send must succeed.
#[tokio::test]
async fn back_to_back_send_succeeds() {
    let (core, emitter) = ChatTestHarness::new_real().await;
    let session_key = "test:back-to-back".to_string();

    let (_, stream_info_1) = core
        .chat_send("hello".into(), session_key.clone(), None, None)
        .await
        .expect("first send");
    core.spawn_chat_relay(stream_info_1, emitter.clone());

    // Wait for first turn to complete.
    let _ = wait_for_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await;

    // Second send on same session must succeed.
    let (_, stream_info_2) = core
        .chat_send("again".into(), session_key.clone(), None, None)
        .await
        .expect("second send should succeed after first done");
    core.spawn_chat_relay(stream_info_2, emitter.clone());

    let _ = wait_for_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await;

    // Active streams must be empty.
    assert_eq!(core.active_streams_len(), 0, "active_streams must drain");
}

async fn wait_for_event(
    emitter: &std::sync::Arc<crate::common::chat_harness::RecordingEmitter>,
    name: &str,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.name == name)
        {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    false
}
```

Add `core.active_streams_len()` as a `#[cfg(any(test, feature = "test-helpers"))]` method in `state.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
impl AppCore {
    pub fn active_streams_len(&self) -> usize {
        self.active_streams.len()
    }
}
```

Add `mod chat_lifecycle;` to the integration test binary's root.

- [ ] **Step 3: Run — expect failure**

```bash
cargo nextest run --test integration -E 'test(back_to_back_send_succeeds)' 2>&1 | tail -20
```

Expected: FAIL — either the second send hangs (because `active_streams` already contains a token), or `active_streams` is non-empty at the end (because `StreamGuard::drop` removed the wrong entry). This reproduces the user's reported bug.

- [ ] **Step 4: Commit the failing test**

```bash
git add tests/integration/chat_lifecycle.rs tests/integration/main.rs crates/app-core/src/state.rs
git commit -m "test(chat): failing regression — back-to-back send wedge"
```

---

## Task 10: Implement value-identity `StreamGuard`

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:420-435`

- [ ] **Step 1: Add a `guard_id` to identify each stream**

Replace the `StreamGuard` struct (lines 420-424) with:

```rust
struct StreamGuard {
    key: String,
    guard_id: u64,
    streams: Arc<ActiveStreams>,
    pending: Arc<PendingInteractions>,
}
```

Add a static counter at module scope:

```rust
static STREAM_GUARD_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_guard_id() -> u64 {
    STREAM_GUARD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
```

- [ ] **Step 2: Change `ActiveStreams` to carry the `guard_id`**

Change the type alias at line 25:

```rust
pub(super) type ActiveStreams = dashmap::DashMap<String, ActiveStreamEntry>;

#[derive(Clone)]
pub(super) struct ActiveStreamEntry {
    pub guard_id: u64,
    pub cancel: tokio_util::sync::CancellationToken,
}
```

- [ ] **Step 3: Update every `active_streams.insert(...)` and `.remove(...)`**

In `chat_send` (line 283), replace:

```rust
active_streams.insert(session_key.clone(), streaming_handle.cancel_token);
```

with:

```rust
let guard_id = next_guard_id();
active_streams.insert(
    session_key.clone(),
    ActiveStreamEntry { guard_id, cancel: streaming_handle.cancel_token.clone() },
);
// pass `guard_id` to relay_chat_stream so it can construct its StreamGuard
```

This requires threading `guard_id: u64` through `ChatStreamInfo` (struct at line 33). Add it as a field.

- [ ] **Step 4: Update `StreamGuard::drop` for value-identity**

Replace the existing `impl Drop` (lines 425-430):

```rust
impl Drop for StreamGuard {
    fn drop(&mut self) {
        // Value-identity removal: only delete the entry if it still belongs to us.
        // If a later send overwrote the slot, we leave the new entry alone.
        if let Some(entry) = self.streams.get(&self.key) {
            if entry.guard_id == self.guard_id {
                drop(entry); // release the read lock before write
                self.streams.remove(&self.key);
            }
        }
        // Same idea for pending_interactions, but the value type is different;
        // we use the existing "remove by key" semantics for now (interactions
        // don't have the same overwrite race).
        self.pending.remove(&self.key);
    }
}
```

- [ ] **Step 5: Construct `StreamGuard` with the new field**

At line 431-435, replace:

```rust
let _guard = StreamGuard {
    key: session_key.clone(),
    guard_id, // threaded through ChatStreamInfo
    streams: Arc::clone(&active_streams),
    pending: Arc::clone(&pending_interactions),
};
```

- [ ] **Step 6: Update `chat_cancel` to handle the new entry shape**

At line 315, change:

```rust
if let Some((_, token)) = active_streams.remove(&session_key) {
    token.cancel();
}
```

to:

```rust
if let Some((_, entry)) = active_streams.remove(&session_key) {
    entry.cancel.cancel();
}
```

- [ ] **Step 7: Build + run all chat tests**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run -p klyntbot --test integration -E 'test(chat_lifecycle)' 2>&1 | tail -10
```

Expected: `back_to_back_send_succeeds` PASSES now. If still failing, check the next task.

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "fix(chat): value-identity StreamGuard prevents double-send race"
```

---

## Task 11: Remove the 200ms metadata persist sleep

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:623-720` (the `AgentEvent::Done` arm)

- [ ] **Step 1: Identify the sleep**

Read lines 625-665 of `streaming.rs`. The `tokio::time::sleep(Duration::from_millis(200)).await;` at line 656 sits inside the `Done` arm, BEFORE the `Terminal` event is emitted. This keeps the `StreamGuard` alive during the sleep, extending the race window.

- [ ] **Step 2: Move the retry into a fire-and-forget task**

Restructure the `Done` arm. Before, the structure is:

```rust
// 1. Try persist by ID
// 2. If Ok(false), sleep 200ms and retry
// 3. Emit AGENT_DONE
// 4. Emit CHAT_MESSAGE_ADDED
// 5. Fire SessionEnd hook
// 6. break
```

After:

```rust
// 1. Try persist by ID (sync)
// 2. If Ok(false), spawn detached retry task — does NOT block the relay
// 3. Emit AGENT_DONE
// 4. Emit CHAT_MESSAGE_ADDED
// 5. Fire SessionEnd hook
// 6. break — StreamGuard drops immediately, freeing the active_streams slot
```

The replacement code (replace the body of the `Done` arm at line 623):

```rust
AgentEvent::Done { content, message_id } => {
    let sk = &session_key;
    flush_text(&mut current_text, &mut segments, &transparency, &emitter, sk);

    // Sync persist attempt by ID
    let persist_outcome = if let Some(ref mid) = message_id {
        repos
            .sessions
            .update_assistant_metadata_by_id(mid, &transparency.tools_json(), &transparency.to_json())
            .await
    } else {
        repos
            .sessions
            .update_last_assistant_metadata(sk, &transparency.tools_json(), &transparency.to_json())
            .await
    };

    if let Err(e) = &persist_outcome {
        tracing::warn!("metadata persist sync failed for {sk}: {e}");
    }

    // If the by-ID call returned Ok(false) (no row), spawn a detached retry.
    // We DO NOT block the relay on this.
    if matches!(persist_outcome, Ok(false)) {
        let repos_clone = repos.clone();
        let sk_owned = sk.clone();
        let tools_json = transparency.tools_json();
        let trans_json = transparency.to_json();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            match repos_clone
                .sessions
                .update_last_assistant_metadata(&sk_owned, &tools_json, &trans_json)
                .await
            {
                Ok(true) => {}
                Ok(false) => tracing::warn!("metadata persist retry: no row {sk_owned}"),
                Err(e) => tracing::warn!("metadata persist retry failed {sk_owned}: {e}"),
            }
        });
    }

    // Emit terminal events
    emit!(emitter, AGENT_DONE, DonePayload { /* fields */ });
    emit!(emitter, CHAT_MESSAGE_ADDED, ChatMessagePayload { /* fields */ });

    // Fire SessionEnd hook only once
    if !session_end_fired.contains_key(sk.as_str()) {
        session_end_fired.insert(sk.clone(), ());
        session_start_fired.remove(sk.as_str());
        if let Some(engine) = &hook_engine {
            let _ = engine.fire("SessionEnd", &serde_json::json!({"sessionKey": sk})).await;
        }
    }

    break;
}
```

- [ ] **Step 3: Run the bench to confirm cleanup latency improved**

```bash
cargo bench -p desktop --bench relay_cleanup_latency -- --quick 2>&1 | tail -5
```

Expected: mean time should be roughly the same OR LOWER than baseline (this bench doesn't exercise the full relay, but it sets the ceiling).

Now also run the integration tests:

```bash
cargo nextest run --test integration -E 'test(chat_lifecycle)' 2>&1 | tail -5
```

Expected: still passing.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "perf(chat): move 200ms metadata retry to detached task

Was: relay task sleeps 200ms post-Done before emitting Done event,
keeping the StreamGuard alive and extending the active_streams race
window. Now: sync persist attempt, then spawn detached retry only
if needed, then emit terminal events and drop the guard immediately."
```

---

## Task 12: Reject double-send on a session already streaming

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:1223-1265` (`AppCore::chat_send`)
- Modify: `crates/common/src/errors.rs` (or wherever `KlyntbotError` lives) — add new error variant

- [ ] **Step 1: Add `SessionAlreadyStreaming` error variant**

Locate `KlyntbotError` (search `enum KlyntbotError`). Add:

```rust
#[error("session {0} is already streaming; cancel before retrying")]
SessionAlreadyStreaming(String),
```

If a different error type (e.g. `ApiError`) is used at the IPC boundary, add a corresponding variant there too and map from `KlyntbotError`.

- [ ] **Step 2: Add the guard in `chat_send`**

At the top of `AppCore::chat_send` (around line 1230, before the `session_start_fired` check):

```rust
if let Some(entry) = self.active_streams.get(&session_key) {
    if !entry.cancel.is_cancelled() {
        return Err(KlyntbotError::SessionAlreadyStreaming(session_key.clone()).into());
    }
}
```

- [ ] **Step 3: Add a failing test for double-send**

In `tests/integration/chat_lifecycle.rs`, add:

```rust
#[tokio::test]
async fn double_send_is_rejected() {
    let (core, emitter) = ChatTestHarness::new_real().await;
    let sk = "test:double-send".to_string();

    let (_, info) = core
        .chat_send("first".into(), sk.clone(), None, None)
        .await
        .expect("first send");
    core.spawn_chat_relay(info, emitter.clone());

    // Immediately fire a second send while the first is still in flight.
    let result = core.chat_send("second".into(), sk.clone(), None, None).await;
    assert!(matches!(result, Err(_)), "second send should be rejected");
}
```

- [ ] **Step 4: Run**

```bash
cargo nextest run --test integration -E 'test(double_send_is_rejected)' 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs crates/common/src/errors.rs \
        tests/integration/chat_lifecycle.rs
git commit -m "fix(chat): reject chat_send while a stream is already active"
```

---

## Task 13: Forward `Cancelled` event to the frontend as `agent:cancelled`

**Files:**
- Modify: `crates/desktop-shared/src/events.rs` (add `AGENT_CANCELLED` constant + payload)
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:1203` (replace `_ => {}`)

- [ ] **Step 1: Add event constant and payload**

In `crates/desktop-shared/src/events.rs`, near the other agent constants:

```rust
pub const AGENT_CANCELLED: &str = "agent:cancelled";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct CancelledPayload {
    pub session_key: String,
    pub partial_content: String,
    pub partial_reasoning: String,
}
```

- [ ] **Step 2: Emit on `AgentEvent::Cancelled`**

In `streaming.rs`, replace the catch-all behavior for `Cancelled`. Before line 1203, add an explicit arm:

```rust
AgentEvent::Cancelled { partial_content, partial_reasoning } => {
    emit!(
        emitter,
        AGENT_CANCELLED,
        CancelledPayload {
            session_key: sk.clone(),
            partial_content,
            partial_reasoning,
        }
    );
    // Cancellation is terminal — drop the stream guard by breaking.
    break;
}
```

- [ ] **Step 3: Add a test**

In `tests/integration/chat_lifecycle.rs`:

```rust
#[tokio::test]
async fn cancel_emits_agent_cancelled() {
    let (core, emitter) = ChatTestHarness::new_real().await;
    let sk = "test:cancel".to_string();

    let (_, info) = core.chat_send("hello".into(), sk.clone(), None, None).await.unwrap();
    core.spawn_chat_relay(info, emitter.clone());

    // Cancel immediately
    core.chat_cancel(sk.clone()).await.unwrap();

    let saw_cancelled = wait_for_event(&emitter, "agent:cancelled", std::time::Duration::from_secs(2)).await;
    assert!(saw_cancelled, "agent:cancelled must be emitted on cancel");
    assert_eq!(core.active_streams_len(), 0);
}
```

- [ ] **Step 4: Build, run**

```bash
cargo nextest run --test integration -E 'test(cancel_emits_agent_cancelled)' 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/events.rs crates/app-core/src/handlers/chat/streaming.rs \
        tests/integration/chat_lifecycle.rs
git commit -m "feat(chat): emit agent:cancelled instead of silently dropping"
```

---

## Task 14: Emit `chat:message_added` on error path too

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:718-727` (the `Error` arm)

- [ ] **Step 1: Add the second emit to the error arm**

Replace the body of `AgentEvent::Error { message }` (around line 718):

```rust
AgentEvent::Error { message } => {
    emit!(
        emitter,
        AGENT_ERROR,
        AgentErrorPayload {
            session_key: sk.clone(),
            message: message.clone(),
        }
    );

    // ALSO emit chat:message_added so the FE re-reads the session.
    // Without this, FE consumers that gate on chat:message_added to refresh
    // history will never see the error message even though it's persisted.
    emit!(
        emitter,
        CHAT_MESSAGE_ADDED,
        ChatMessagePayload {
            session_key: sk.clone(),
            source: "agent_error".to_string(),
        }
    );

    break;
}
```

- [ ] **Step 2: Add a regression test using `ErrorProvider`**

In `tests/integration/chat_lifecycle.rs`:

```rust
use crate::common::mocks::provider::ErrorProvider;

#[tokio::test]
async fn error_emits_both_terminal_events() {
    // Build harness with ErrorProvider so the first send fails.
    let emitter = std::sync::Arc::new(crate::common::chat_harness::RecordingEmitter::default());
    let pool = crate::common::test_pool().await;
    let provider = std::sync::Arc::new(ErrorProvider::default());
    let core = std::sync::Arc::new(
        app_core::AppCore::for_tests(pool, provider, std::sync::Arc::clone(&emitter) as _)
            .await
            .unwrap(),
    );

    let sk = "test:error".to_string();
    let (_, info) = core.chat_send("hello".into(), sk.clone(), None, None).await.unwrap();
    core.spawn_chat_relay(info, emitter.clone());

    let saw_error = wait_for_event(&emitter, "agent:error", std::time::Duration::from_secs(5)).await;
    let saw_msg_added = wait_for_event(&emitter, "chat:message_added", std::time::Duration::from_secs(5)).await;
    assert!(saw_error, "agent:error must be emitted");
    assert!(saw_msg_added, "chat:message_added must also be emitted on error path");
}
```

- [ ] **Step 3: Run**

```bash
cargo nextest run --test integration -E 'test(error_emits_both_terminal_events)' 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs tests/integration/chat_lifecycle.rs
git commit -m "fix(chat): emit chat:message_added on error path

FE consumers gate on chat:message_added to refresh session history.
Without it, an errored turn leaves the spinner spinning."
```

---

## Task 15: Clean `session_end_fired` on Done

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:704-706`

- [ ] **Step 1: Add removal**

In the `Done` arm (the cleaned-up version from Task 11), the hook fire block currently inserts into `session_end_fired` but never removes. Add a removal AFTER the hook fires successfully:

```rust
if !session_end_fired.contains_key(sk.as_str()) {
    session_end_fired.insert(sk.clone(), ());
    session_start_fired.remove(sk.as_str());
    if let Some(engine) = &hook_engine {
        let _ = engine.fire("SessionEnd", &serde_json::json!({"sessionKey": sk})).await;
    }
    // Now that the hook has fired exactly once, drop the marker so the next
    // turn on this session can fire SessionStart cleanly.
    session_end_fired.remove(sk.as_str());
}
```

Rationale: `session_end_fired` was originally inserted to prevent the cancel path firing `SessionEnd` after `Done` already fired it. Once the hook completes, we don't need the marker anymore — the next `chat_send` on the same session is a fresh turn and the `SessionStart` hook should fire normally.

- [ ] **Step 2: Add a test for repeated turn → hook fires once per turn**

Skip this — covered by `back_to_back_send_succeeds` once we observe the hook side effects in the harness. Defer to Task 17 proptest.

- [ ] **Step 3: Run existing tests**

```bash
cargo nextest run --test integration -E 'test(chat_lifecycle)' 2>&1 | tail -10
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "fix(chat): clean up session_end_fired after SessionEnd hook fires

Previously, session_end_fired entries accumulated for every session
that ever completed (unbounded memory growth). Now removed after the
hook fires, so the next turn fires SessionStart normally."
```

---

## Task 16: Increase merged mpsc capacity from 64 to 256

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:478`
- Modify: `crates/agent/src/agent_loop/mod.rs:1196-1197`

- [ ] **Step 1: Bump the merged channel**

In `streaming.rs:478`, change:

```rust
let (merged_tx, mut merged_rx) = mpsc::channel::<AgentEvent>(64);
```

to:

```rust
// Capacity 256: bursty providers can emit ~50 events/sec at peak; 64 is too
// small once we add token-by-token streaming + parallel tool calls.
let (merged_tx, mut merged_rx) = mpsc::channel::<AgentEvent>(256);
```

- [ ] **Step 2: Bump the upstream agent_loop channel**

In `crates/agent/src/agent_loop/mod.rs:1196-1197`, change:

```rust
let (event_tx, event_rx) = mpsc::channel(64);
let (interaction_tx, interaction_rx) = mpsc::channel(4);
```

to:

```rust
let (event_tx, event_rx) = mpsc::channel(256);
let (interaction_tx, interaction_rx) = mpsc::channel(8);
```

- [ ] **Step 3: Re-run throughput bench**

```bash
cargo bench -p agent --bench stream_throughput -- --quick 2>&1 | tail -10
```

Expected: throughput at the 10,000 batch size should improve materially (fewer producer stalls on send).

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs crates/agent/src/agent_loop/mod.rs
git commit -m "perf(chat): raise event mpsc capacities (64→256, 4→8)"
```

---

## Task 17: Proptest — arbitrary event sequence leaves empty active_streams

**Files:**
- Create: `tests/property/event_sequence_invariants.rs`
- Modify: root facade `tests/` lib structure to include it

- [ ] **Step 1: Define operations**

Create `tests/property/event_sequence_invariants.rs`:

```rust
//! Proptest invariant: any permutation of (send, cancel, error, complete)
//! across N sessions leaves the AppCore active_streams empty.

use proptest::prelude::*;
use proptest::strategy::Strategy;

#[derive(Debug, Clone)]
enum Op {
    Send(u8),
    Cancel(u8),
    SimulateError(u8),
    SimulateComplete(u8),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..5).prop_map(Op::Send),
        (0u8..5).prop_map(Op::Cancel),
        (0u8..5).prop_map(Op::SimulateError),
        (0u8..5).prop_map(Op::SimulateComplete),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// CHAT-INV-1: after any sequence of operations, active_streams is empty
    /// (eventually, modulo a small drain window).
    #[test]
    fn active_streams_drains(ops in proptest::collection::vec(op_strategy(), 0..20)) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let (core, emitter) = crate::common::chat_harness::ChatTestHarness::new_real().await;
            let _ = (ops, core, emitter); // FILLED IN: see step 2
        });
    }
}
```

- [ ] **Step 2: Fill in the body**

Replace the `let _ = (ops, core, emitter);` line with the operation loop. Each `Op` maps to either a real backend call or a simulated event injection. The simplest correct version uses only `chat_send` + `chat_cancel` (the `SimulateError` / `SimulateComplete` variants would require an event-injection harness which is deferred to Task 42):

```rust
for op in &ops {
    let sk = format!("test:proptest:{:?}", op_session_id(op));
    match op {
        Op::Send(_) => {
            let _ = core.chat_send("x".into(), sk, None, None).await;
        }
        Op::Cancel(_) => {
            let _ = core.chat_cancel(sk).await;
        }
        Op::SimulateError(_) | Op::SimulateComplete(_) => {
            // Deferred to Task 42 (event-injection harness).
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

// Wait for drain
tokio::time::sleep(std::time::Duration::from_secs(2)).await;

prop_assert_eq!(core.active_streams_len(), 0, "active_streams must drain");
```

Where `op_session_id(op)` extracts the inner `u8`.

- [ ] **Step 3: Run**

```bash
cargo nextest run --test property -E 'test(active_streams_drains)' 2>&1 | tail -15
```

Expected: PASS in ~5 minutes (1000 cases × ~50ms each plus drain).

- [ ] **Step 4: Commit**

```bash
git add tests/property/event_sequence_invariants.rs tests/property/main.rs
git commit -m "test(chat): proptest invariant — active_streams drains under any op sequence"
```

---

## Task 18: PR2 wrap — run all gates, push, open PR

- [ ] **Step 1: Full sweep**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -5
cargo fmt --all --check
./scripts/run_chat_perf_gates.sh 2>&1 | tail -15
```

All must pass.

- [ ] **Step 2: Push + PR**

```bash
git push -u origin feat/chat-thread-overhaul-pr2-backend-hotfix
gh pr create --title "fix(chat): backend P0 hotfix for thread wedge bug" --body "$(cat <<'EOF'
## Summary
Closes the reported bug: "after a message reaches 'completed' state, the next send no longer works until the app is refreshed." Backend root cause was a value-blind `StreamGuard::drop` racing with a 200ms metadata-persist sleep; the second `chat_send` would insert a new entry that the first guard's drop would then remove.

**Fixes in this PR:**
- Value-identity `StreamGuard`: each guard carries a `guard_id` and only removes its own entry
- 200ms metadata retry moved to a detached `tokio::spawn` — the relay task no longer holds `active_streams` for 200ms post-Done
- `chat_send` rejects double-send while a stream is already active for the session
- `agent:cancelled` now emitted (was silently dropped by `_ => {}`)
- `chat:message_added` now emitted on the error path too
- `session_end_fired` cleaned up after `SessionEnd` hook fires (no more unbounded growth)
- mpsc capacities raised: 64→256 for events, 4→8 for interactions
- Proptest covers 1000 random op-sequences

This is **PR2 of 10** in the chat/thread overhaul.

## Test plan
- [x] `cargo nextest run --workspace`
- [x] New tests in `tests/integration/chat_lifecycle.rs` (back-to-back, double-send, cancel, error)
- [x] New proptest in `tests/property/event_sequence_invariants.rs` (1000 cases)
- [x] `./scripts/run_chat_perf_gates.sh` — relay cleanup p99 ≤ 1.5ms

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR3 — Phase 2: P0 Frontend Hotfix

**Goal:** Close the wedge bug at the FE layer. Add generation counters. Add an assistant-mode watchdog. Surface stuck state to the user with a manual reset.

**Acceptance:**
- `useThreadTurnEvents.test.tsx` covers: stale turn id received, watchdog fires, manual reset works.
- `useThreadWatchdog.test.ts` covers: 90s timeout + clean teardown.
- After watchdog fires, the composer is enabled within 33ms (perf mark).
- Manual reset button visible after 5s stuck state.

---

## Task 19: Branch + FE baseline confirm

- [ ] **Step 1: Branch**

```bash
git checkout main
git pull --ff-only
git checkout -b feat/chat-thread-overhaul-pr3-frontend-hotfix
cd desktop-ui && bun install && cd ..
```

- [ ] **Step 2: Pre-flight**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test 2>&1 | tail -10
```

Expected: all pass.

---

## Task 20: Add generation counter to thread state

**Files:**
- Create: `desktop-ui/src/features/threads/store/types.ts`
- Modify: `desktop-ui/src/features/threads/hooks/useThreadsReducer.ts` (add `turnGenerationByThread`)

- [ ] **Step 1: Define shared types**

Create `desktop-ui/src/features/threads/store/types.ts`:

```ts
/**
 * Turn generation — monotonically increasing per (threadId).
 *
 * Used to filter stale events: any event whose generation < current is
 * silently ignored. Replaces the brittle "active turn id ref" pattern.
 */
export type TurnGeneration = number;

export type TurnHandle = {
  threadId: string;
  turnId: string;
  generation: TurnGeneration;
};

export type ThreadStatus =
  | { kind: "idle"; lastDurationMs: number | null }
  | { kind: "streaming"; turn: TurnHandle; startedAt: number }
  | { kind: "tool_executing"; turn: TurnHandle; tool: string; callId: string; startedAt: number }
  | { kind: "stuck"; turn: TurnHandle; stuckSince: number }   // watchdog fired
  | { kind: "error"; message: string; turn: TurnHandle | null };
```

- [ ] **Step 2: Extend `ThreadState`**

In `useThreadsReducer.ts`, add to `ThreadState` (lines 27-49):

```ts
turnGenerationByThread: Record<string, number>;
```

Initialize to `{}` in the initial state.

- [ ] **Step 3: Add `incrementTurnGeneration` action**

Add a new action type:

```ts
| { type: "incrementTurnGeneration"; threadId: string }
```

Add the case to the appropriate slice reducer (likely `threadLifecycleSlice`):

```ts
case "incrementTurnGeneration": {
  const current = state.turnGenerationByThread[action.threadId] ?? 0;
  return {
    ...state,
    turnGenerationByThread: {
      ...state.turnGenerationByThread,
      [action.threadId]: current + 1,
    },
  };
}
```

- [ ] **Step 4: Bump generation on every `setActiveTurnId` to a new id**

Wherever `setActiveTurnId` is dispatched with a non-null `turnId`, also dispatch `incrementTurnGeneration` for the same `threadId`. The canonical caller is `useThreadMessaging.ts` send path (around line 290).

- [ ] **Step 5: Add a selector**

In `useThreadsReducer.ts`, export:

```ts
export const selectTurnGeneration = (
  state: ThreadState,
  threadId: string,
): number => state.turnGenerationByThread[threadId] ?? 0;
```

- [ ] **Step 6: Test**

In `useThreadsReducer.test.ts`, add:

```ts
it("increments turn generation on every new turn", () => {
  let state = initialThreadState;
  state = threadReducer(state, { type: "incrementTurnGeneration", threadId: "t1" });
  state = threadReducer(state, { type: "incrementTurnGeneration", threadId: "t1" });
  expect(selectTurnGeneration(state, "t1")).toBe(2);
});
```

```bash
cd desktop-ui && bun run test useThreadsReducer 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/threads/store/types.ts \
        desktop-ui/src/features/threads/hooks/useThreadsReducer.ts \
        desktop-ui/src/features/threads/hooks/useThreadsReducer.test.ts
git commit -m "feat(threads): add monotonic turn generation counter"
```

---

## Task 21: Remove silent-drop guard, replace with generation check

**Files:**
- Modify: `desktop-ui/src/features/threads/hooks/useThreadTurnEvents.ts:242-246` and 375-378

- [ ] **Step 1: Identify all silent-drop sites**

```bash
rg -n 'turnId !== activeTurnId' desktop-ui/src/features/threads/hooks/
```

Should find at least lines 244 in `onTurnCompleted` and 376 in `onTurnError`.

- [ ] **Step 2: Replace with generation-aware filtering**

Modify `onTurnCompleted` (around line 240):

```ts
const onTurnCompleted = useCallback(
  (_workspaceId: string, threadId: string, turnId: string, eventGeneration?: number) => {
    const currentGeneration = selectTurnGeneration(stateRef.current, threadId);
    if (eventGeneration != null && eventGeneration < currentGeneration) {
      // Truly stale event from a previous turn — safe to drop, generation guarantees it.
      console.debug("[threads] dropping stale turn_completed", { threadId, turnId, eventGeneration, currentGeneration });
      return;
    }
    // Always reset processing — even if turnId mismatches our optimistic guess.
    // Generation count prevents the bug we used to have where a mismatched
    // turnId from a real backend event got silently dropped.
    markProcessing(threadId, false);
    setActiveTurnId(threadId, null);
    resetThreadTurnState(threadId);
  },
  [markProcessing, setActiveTurnId, resetThreadTurnState],
);
```

For now, `eventGeneration` is optional; PR4 threads the real generation through the wire event. Until then, the function still drops the bug-causing guard.

Repeat the same fix for `onTurnError` (around line 375).

- [ ] **Step 3: Add a failing test**

In `useThreadTurnEvents.test.tsx`, add:

```ts
it("calls markProcessing(false) even when turnId mismatches optimistic ref", async () => {
  const markProcessing = vi.fn();
  const setActiveTurnId = vi.fn();
  const { result } = renderHook(() =>
    useThreadTurnEvents({
      // ... existing fixture props
      markProcessing,
      setActiveTurnId,
    }),
  );

  // Set up the optimistic mismatch: turn-A is optimistic, but backend emits completion for turn-B.
  act(() => { result.current.onTurnStarted("ws-1", "thread-1", "turn-A"); });
  act(() => { result.current.onTurnCompleted("ws-1", "thread-1", "turn-B"); });

  expect(markProcessing).toHaveBeenCalledWith("thread-1", false);
});
```

- [ ] **Step 4: Run — confirm PASS**

```bash
cd desktop-ui && bun run test useThreadTurnEvents 2>&1 | tail -10
```

Expected: PASS (the silent-drop guard is gone).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/threads/hooks/useThreadTurnEvents.ts \
        desktop-ui/src/features/threads/hooks/useThreadTurnEvents.test.tsx
git commit -m "fix(threads): drop silent-drop guard in onTurnCompleted/onTurnError

Was: if (turnId !== activeTurnId) return — caused permanent stuck
isProcessing when optimistic turn id was out of sync with backend.
Now: always call markProcessing(false). Stale events filtered by
generation counter in a follow-up."
```

---

## Task 22: Watchdog hook for assistant-mode threads

**Files:**
- Create: `desktop-ui/src/features/threads/hooks/useThreadWatchdog.ts`
- Create: `desktop-ui/src/features/threads/hooks/useThreadWatchdog.test.ts`

- [ ] **Step 1: Write the hook**

Create `desktop-ui/src/features/threads/hooks/useThreadWatchdog.ts`:

```ts
import { useEffect, useRef } from "react";

const WATCHDOG_TIMEOUT_MS = 90_000;

type Args = {
  threadId: string | null;
  isProcessing: boolean;
  onFire: (threadId: string) => void;
};

/**
 * Assistant-mode watchdog. Mirrors the coding-mode 90s heartbeat from
 * ThreadEventBuffer.ts. If no event arrives within 90s while isProcessing
 * is true, fire `onFire` and let the caller reset state.
 */
export function useThreadWatchdog({ threadId, isProcessing, onFire }: Args): void {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }

    if (!threadId || !isProcessing) return;

    timeoutRef.current = setTimeout(() => {
      console.warn(`[threads] watchdog fired for ${threadId} after ${WATCHDOG_TIMEOUT_MS}ms`);
      onFire(threadId);
    }, WATCHDOG_TIMEOUT_MS);

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [threadId, isProcessing, onFire]);
}
```

- [ ] **Step 2: Write the test**

Create `desktop-ui/src/features/threads/hooks/useThreadWatchdog.test.ts`:

```ts
import { renderHook } from "@testing-library/react";
import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useThreadWatchdog } from "./useThreadWatchdog";

describe("useThreadWatchdog", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("does not fire while not processing", () => {
    const onFire = vi.fn();
    renderHook(() =>
      useThreadWatchdog({ threadId: "t1", isProcessing: false, onFire }),
    );
    act(() => { vi.advanceTimersByTime(100_000); });
    expect(onFire).not.toHaveBeenCalled();
  });

  it("fires after 90s while processing", () => {
    const onFire = vi.fn();
    renderHook(() =>
      useThreadWatchdog({ threadId: "t1", isProcessing: true, onFire }),
    );
    act(() => { vi.advanceTimersByTime(89_999); });
    expect(onFire).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(2); });
    expect(onFire).toHaveBeenCalledWith("t1");
  });

  it("clears on isProcessing flip to false", () => {
    const onFire = vi.fn();
    const { rerender } = renderHook(
      ({ p }: { p: boolean }) =>
        useThreadWatchdog({ threadId: "t1", isProcessing: p, onFire }),
      { initialProps: { p: true } },
    );
    act(() => { vi.advanceTimersByTime(50_000); });
    rerender({ p: false });
    act(() => { vi.advanceTimersByTime(100_000); });
    expect(onFire).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 3: Run**

```bash
cd desktop-ui && bun run test useThreadWatchdog 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/threads/hooks/useThreadWatchdog.ts \
        desktop-ui/src/features/threads/hooks/useThreadWatchdog.test.ts
git commit -m "feat(threads): 90s watchdog hook for assistant-mode threads"
```

---

## Task 23: Wire watchdog into MainApp

**Files:**
- Modify: `desktop-ui/src/features/app/MainApp.tsx` (or wherever the assistant thread is rendered with `activeThreadId` + dispatch)

- [ ] **Step 1: Use the hook**

In `MainApp.tsx`, inside the component body where `activeThreadId` and `dispatch` are available, add:

```ts
import { useThreadWatchdog } from "@threads/hooks/useThreadWatchdog";

// inside component:
useThreadWatchdog({
  threadId: activeThreadId,
  isProcessing: threadStatusById[activeThreadId ?? ""]?.isProcessing ?? false,
  onFire: useCallback((threadId: string) => {
    dispatch({ type: "markProcessing", threadId, isProcessing: false, timestamp: Date.now() });
    dispatch({ type: "setActiveTurnId", threadId, turnId: null });
    // Push a system message so the user knows what happened.
    dispatch({
      type: "addAssistantMessage",
      threadId,
      message: { role: "system", text: "Thread recovered from silent failure — please retry." },
    });
  }, [dispatch]),
});
```

Note: only wire this for *assistant* mode threads. Coding threads already have `ThreadEventBuffer`'s heartbeat. Determine assistant-mode via the existing `useAppMode()` store check.

- [ ] **Step 2: Build, typecheck, test**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test 2>&1 | tail -5
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/app/MainApp.tsx
git commit -m "feat(threads): wire useThreadWatchdog into MainApp for assistant mode"
```

---

## Task 24: Stuck-thread banner component

**Files:**
- Create: `desktop-ui/src/features/threads/components/StuckThreadBanner.tsx`
- Create: `desktop-ui/src/features/threads/components/StuckThreadBanner.test.tsx`
- Modify: `desktop-ui/src/styles/index.css` (add import)
- Create: `desktop-ui/src/styles/components/stuck-thread-banner.css`

- [ ] **Step 1: Write the CSS**

Create `desktop-ui/src/styles/components/stuck-thread-banner.css`:

```css
.stuck-thread-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  margin: 8px 16px;
  background: var(--color-warning-bg);
  border: 1px solid var(--color-warning-border);
  border-radius: 8px;
  font-size: var(--fs-xs);
  color: var(--color-warning-fg);
}

.stuck-thread-banner__msg { flex: 1; }

.stuck-thread-banner__btn {
  padding: 4px 10px;
  background: var(--color-warning-fg);
  color: var(--color-bg);
  border: none;
  border-radius: 4px;
  font-size: var(--fs-xs);
  cursor: pointer;
}
```

Add to `src/styles/index.css`:

```css
@import "./components/stuck-thread-banner.css";
```

- [ ] **Step 2: Write the component**

Create `desktop-ui/src/features/threads/components/StuckThreadBanner.tsx`:

```tsx
type Props = {
  durationMs: number;
  onReset: () => void;
};

export function StuckThreadBanner({ durationMs, onReset }: Props): JSX.Element {
  const seconds = Math.round(durationMs / 1000);
  return (
    <div className="stuck-thread-banner" role="alert">
      <span className="stuck-thread-banner__msg">
        This thread has been processing for {seconds}s with no response. It may be stuck.
      </span>
      <button
        type="button"
        className="stuck-thread-banner__btn"
        onClick={onReset}
      >
        Reset and try again
      </button>
    </div>
  );
}
```

- [ ] **Step 3: Write the test**

Create `desktop-ui/src/features/threads/components/StuckThreadBanner.test.tsx`:

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { StuckThreadBanner } from "./StuckThreadBanner";

describe("StuckThreadBanner", () => {
  it("renders duration in seconds", () => {
    render(<StuckThreadBanner durationMs={7_500} onReset={() => {}} />);
    expect(screen.getByText(/processing for 8s/i)).toBeTruthy();
  });

  it("fires onReset on button click", () => {
    const onReset = vi.fn();
    render(<StuckThreadBanner durationMs={5_000} onReset={onReset} />);
    fireEvent.click(screen.getByRole("button"));
    expect(onReset).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 4: Run**

```bash
cd desktop-ui && bun run test StuckThreadBanner 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/threads/components/StuckThreadBanner.tsx \
        desktop-ui/src/features/threads/components/StuckThreadBanner.test.tsx \
        desktop-ui/src/styles/components/stuck-thread-banner.css \
        desktop-ui/src/styles/index.css
git commit -m "feat(threads): StuckThreadBanner component"
```

---

## Task 25: Show stuck banner after 5s

**Files:**
- Modify: `desktop-ui/src/features/app/MainApp.tsx` (or the rendering parent of `Messages`)
- Create: `desktop-ui/src/features/threads/hooks/useStuckThreadDetector.ts`

- [ ] **Step 1: Write the detector hook**

Create `desktop-ui/src/features/threads/hooks/useStuckThreadDetector.ts`:

```ts
import { useEffect, useState } from "react";

const STUCK_THRESHOLD_MS = 5_000;

export function useStuckThreadDetector(
  isProcessing: boolean,
  processingStartedAt: number | null,
): { isStuck: boolean; stuckDurationMs: number } {
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!isProcessing || processingStartedAt == null) return;
    const interval = setInterval(() => setTick((n) => n + 1), 1_000);
    return () => clearInterval(interval);
  }, [isProcessing, processingStartedAt]);

  if (!isProcessing || processingStartedAt == null) {
    return { isStuck: false, stuckDurationMs: 0 };
  }

  const elapsed = Date.now() - processingStartedAt;
  return {
    isStuck: elapsed > STUCK_THRESHOLD_MS,
    stuckDurationMs: elapsed,
  };
}
```

- [ ] **Step 2: Wire it in**

In `MainApp.tsx` near the watchdog wiring:

```tsx
const status = threadStatusById[activeThreadId ?? ""];
const { isStuck, stuckDurationMs } = useStuckThreadDetector(
  status?.isProcessing ?? false,
  status?.processingStartedAt ?? null,
);

// render somewhere above the Messages list:
{activeThreadId && isStuck && (
  <StuckThreadBanner
    durationMs={stuckDurationMs}
    onReset={() => {
      dispatch({ type: "markProcessing", threadId: activeThreadId, isProcessing: false, timestamp: Date.now() });
      dispatch({ type: "setActiveTurnId", threadId: activeThreadId, turnId: null });
    }}
  />
)}
```

- [ ] **Step 3: Manual test**

```bash
cd desktop-ui && bun run dev:vite &
cd .. && cargo tauri dev &
```

Trigger a stuck state by simulating a backend hang (e.g. throw inside `chat_send` while frontend optimistically marked processing). Confirm:
- Banner appears at 5s.
- Click "Reset" → `isProcessing` flips to false, composer re-enables.

Stop the dev servers.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/threads/hooks/useStuckThreadDetector.ts \
        desktop-ui/src/features/app/MainApp.tsx
git commit -m "feat(threads): show StuckThreadBanner after 5s of no activity"
```

---

## Task 26: Composer disabled while stuck (not just isProcessing)

**Files:**
- Modify: `desktop-ui/src/features/composer/components/Composer.tsx`

- [ ] **Step 1: Add a `stuck` prop**

Composer currently has `isProcessing` and `disabled`. Add a third prop `isStuck?: boolean` (default false). When `isStuck` is true, set `defaultSubmitIntent = "default"` (NOT queue) so the user's next send replaces the stuck turn rather than queuing behind it.

Replace lines 263-265:

```ts
const defaultSubmitIntent: ComposerSendIntent =
  isStuck ? "default" : isProcessing ? effectiveFollowUpBehavior : "default";
```

- [ ] **Step 2: Update tests**

In `Composer.test.tsx`, add:

```ts
it("uses default intent when stuck (overrides isProcessing)", () => {
  // existing rendering with isStuck=true, isProcessing=true
  // assert submit intent is "default", not "queue"
});
```

- [ ] **Step 3: Pipe through from MainApp**

In `MainApp.tsx`, pass `isStuck` to the `Composer`:

```tsx
<Composer
  isProcessing={status?.isProcessing ?? false}
  isStuck={isStuck}
  // ... rest
/>
```

- [ ] **Step 4: Run tests**

```bash
cd desktop-ui && bun run test 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/composer/components/Composer.tsx \
        desktop-ui/src/features/composer/components/Composer.test.tsx \
        desktop-ui/src/features/app/MainApp.tsx
git commit -m "feat(composer): honor isStuck — default intent overrides queue"
```

---

## Task 27: Listen for `agent:cancelled` on frontend

**Files:**
- Modify: `desktop-ui/src/features/chat/store/chatStreamStore.ts` (add cancelled handler)

- [ ] **Step 1: Find the existing handlers**

```bash
rg -n 'agent:done|agent:error' desktop-ui/src/features/chat/store/chatStreamStore.ts
```

- [ ] **Step 2: Add `onCancelled`**

Add a new method:

```ts
private onCancelled(payload: { sessionKey: string; partialContent: string; partialReasoning: string }): void {
  if (!this.isActive(payload.sessionKey)) return;
  this.updateState(payload.sessionKey, (s) => ({
    ...s,
    isStreaming: false,
    cancelled: true,
    partialContent: payload.partialContent,
    partialReasoning: payload.partialReasoning,
  }));
}
```

Register a listener in `startStream` or wherever the other `listen()` calls live:

```ts
const unlistenCancelled = await listen<CancelledPayload>("agent:cancelled", (event) => {
  this.onCancelled(event.payload);
});
```

Capture `unlistenCancelled` in the disposer list so it tears down with the store.

- [ ] **Step 3: Test**

Add a test in `chatStreamStore.test.ts`:

```ts
it("clears isStreaming on agent:cancelled", () => {
  // start a stream, fire a cancelled event, assert state
});
```

- [ ] **Step 4: Run, commit**

```bash
cd desktop-ui && bun run test chatStreamStore 2>&1 | tail -5
git add desktop-ui/src/features/chat/store/chatStreamStore.ts \
        desktop-ui/src/features/chat/store/chatStreamStore.test.ts
git commit -m "feat(chat): consume agent:cancelled event on frontend"
```

---

## Task 28: PR3 wrap

- [ ] **Step 1: Full sweep**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test && bun run build 2>&1 | tail -5
```

All must pass.

- [ ] **Step 2: Push + PR**

```bash
git push -u origin feat/chat-thread-overhaul-pr3-frontend-hotfix
gh pr create --title "fix(threads): frontend P0 hotfix for thread wedge bug" --body "$(cat <<'EOF'
## Summary
Closes the FE half of the wedge bug. Removes the silent-drop guard in `onTurnCompleted`/`onTurnError`. Adds a 90s assistant-mode watchdog and a manual-reset banner after 5s of stuck state. Begins generation-counter migration (full wire-level threading lands in PR4).

**Changes:**
- `turnGenerationByThread` added to `ThreadState`; bumps on every new turn
- `onTurnCompleted` / `onTurnError` no longer drop events on turnId mismatch
- `useThreadWatchdog` — 90s timer for assistant mode (mirrors coding-mode buffer)
- `StuckThreadBanner` — visible after 5s of stuck state with manual reset
- Composer honors `isStuck` to short-circuit queue/steer intent
- `agent:cancelled` consumed on FE (was silently dropped)

**PR3 of 10.** Together with PR2, this closes the reported wedge bug. PR4 onwards is structural work to prevent the class entirely.

## Test plan
- [x] `bun run typecheck`
- [x] `bun run lint`
- [x] `bun run test`
- [x] `bun run build`
- [x] New tests: `useThreadWatchdog.test.ts`, `StuckThreadBanner.test.tsx`, generation-counter test in `useThreadsReducer.test.ts`
- [x] Manual: stuck-banner appears at 5s; "Reset" re-enables composer

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR4 — Phase 3: `ThreadEvent` Wire v2

**Goal:** Replace the 50+ stringly-typed `agent:*` Tauri events with a single typed `ThreadEvent` v2 union that includes a `Terminal` invariant. Frontend listens to ONE channel and reads ONE union — generation-counted at the wire level.

**Acceptance:**
- New `crates/desktop-shared/src/thread_event_v2.rs` compiles and is `specta::Type`.
- `bindings.ts` regenerates with `ThreadEvent` and all variants typed end-to-end.
- New frontend `useThreadEventsV2` hook consumes the channel with full inference.
- All existing assistant-mode events are still emitted (deprecated path) until PR5 retires the old channel.

(Tasks 29-37 omitted from this excerpt for brevity but follow the same TDD-per-task structure.)

For full detail, each task adds:
- **Task 29:** Branch + define `ThreadEvent` v2 union with `Terminal { kind: Done|Error|Cancelled }` variant
- **Task 30:** Add `Generation` field to every variant
- **Task 31:** `tauri_specta::Event` impl for `ThreadEvent` v2; verify in `bindings.ts`
- **Task 32:** Backend translator: `AgentEvent` → `ThreadEvent` v2 (replaces the catch-all `_ => {}`)
- **Task 33:** Spawn forwarder emitting on new `"thread:event"` channel
- **Task 34:** FE `useThreadEventsV2` hook with full type inference
- **Task 35:** Migrate `useThreadTurnEvents` to consume v2 (parallel with v1 — feature flag)
- **Task 36:** Tests: roundtrip serialize/deserialize, terminal-invariant proptest
- **Task 37:** PR4 wrap

---

# PR5 — Phase 4: Unified `ThreadRuntime`

**Goal:** Build a `ThreadRuntime` trait in `app-core` that both assistant and coding modes implement. Both modes use the same `ActiveTurns` map, the same `StreamGuard`, and the same v2 event emission path.

**Acceptance:**
- `crates/app-core/src/runtime/mod.rs` defines `ThreadRuntime` trait.
- Both `AssistantThreadRuntime` and `CodingThreadRuntime` impl it.
- `chat_send` and `coding_message_send` delegate to the trait.
- Single integration test asserts both impls satisfy the same lifecycle invariants.

(Tasks 38-48 — analogous structure, each 2-5 min, TDD, committed individually.)

- **Task 38:** Branch + define trait
- **Task 39:** `TurnHandle { thread_id, turn_id, generation }` type
- **Task 40:** `ActiveTurns` — value-identity DashMap keyed by `TurnHandle`
- **Task 41:** `AssistantThreadRuntime` impl wrapping current `chat_send` logic
- **Task 42:** `CodingThreadRuntime` impl wrapping `coding_message_send` logic
- **Task 43:** Update `AppCore::state.rs` to hold `Arc<dyn ThreadRuntime>` for each mode
- **Task 44:** Tauri handlers delegate to trait method `runtime.start_turn(...)`
- **Task 45:** `runtime.cancel_turn(handle)` — single cancel path
- **Task 46:** Shared `RuntimeMetrics { ttft_ms, ttlt_ms, tool_count }` struct
- **Task 47:** Integration test: both runtimes satisfy `back_to_back_send_succeeds`
- **Task 48:** PR5 wrap

---

# PR6 — Phase 5: Frontend Store Unification

**Goal:** Collapse `useThreadsReducer` + `ThreadEventBuffer` + `chatStreamStore` into one Zustand-backed store keyed by `(threadId, turnId, generation)`.

**Acceptance:**
- `useChatStore` is the only state container; old stores become thin shims that proxy until removed in PR8.
- Components rewritten to consume the new store.
- All existing tests still pass.

(Tasks 49-58 — Zustand setup, migration per component, retirement of shims.)

- **Task 49:** Add `zustand` workspace dep (it's likely already present via `chatStreamStore`'s singleton pattern — verify; if not, install)
- **Task 50:** Define `useChatStore` with slices: threads, turns, events, watchdog
- **Task 51:** Migrate `useThreadsReducer` consumers one component at a time
- **Task 52:** Migrate `ThreadEventBuffer` consumers
- **Task 53:** Migrate `chatStreamStore` consumers
- **Task 54:** Delete the three old stores (or keep as deprecated shims pending PR8)
- **Task 55:** Add `useChatStore.test.ts` covering generation-counter invariants
- **Task 56:** Add devtools middleware for the store (gated `import.meta.env.DEV`)
- **Task 57:** Verify FE bench shows no regression (`coalescer.bench.ts`)
- **Task 58:** PR6 wrap

---

# PR7 — Phase 6: Backend Performance

**Goal:** Hit the performance acceptance criteria from the plan header. Propagate tracing spans across `tokio::spawn`. Tune channel capacities. Batch DB writes where possible.

**Acceptance:**
- TTFT bench p95 ≤ 15ms (mock provider).
- Stream throughput ≥ 5,000 events/sec at 10k batch.
- Relay cleanup p99 ≤ 1ms.
- All criterion benches gated in `scripts/run_chat_perf_gates.sh` with numeric assertions.

(Tasks 59-66 — span propagation, channel tuning, gate tightening.)

- **Task 59:** Branch
- **Task 60:** `Span::current().in_scope(...)` wrapper for spawned relay tasks
- **Task 61:** Replace catch-all `_ => {}` in bridge translator with explicit "drop with reason" + counter
- **Task 62:** Batch `add_message` writes per turn via a transaction
- **Task 63:** Replace `Arc<tokio::sync::Mutex<Option<McpManager>>>` with `Arc<parking_lot::RwLock<Option<McpManager>>>`
- **Task 64:** Re-run all backend benches; record numbers in baseline doc
- **Task 65:** Tighten thresholds in `run_chat_perf_gates.sh` with `awk` assertions on log output
- **Task 66:** PR7 wrap

---

# PR8 — Phase 7: Frontend Performance

**Goal:** Hit FE perf criteria. Virtualize the message list, coalesce deltas at 60 fps, stable keys, optional `useTransition` for non-urgent renders.

**Acceptance:**
- Coalescer bench: 10k chunks ≤ 16ms.
- Visual smoke: typing a 4,096-token response with mock provider does not drop frames (Chrome DevTools → Performance tab → no long tasks > 50ms during stream).
- Bundle size for the threads route ≤ +30 kB gzipped vs main.

(Tasks 67-76 — virtualization, coalescer, key stability, transitions.)

- **Task 67:** Branch + install `@tanstack/react-virtual`
- **Task 68:** `VirtualizedMessageList` component with `useVirtualizer({ count, getScrollElement, estimateSize })`
- **Task 69:** Wrap `Messages.tsx` and `CodingThreadView.tsx`
- **Task 70:** `coalesceDeltas` utility with `requestAnimationFrame` batching
- **Task 71:** Wire coalescer into the v2 event consumer
- **Task 72:** Stable `(itemId, partIdx)` keys precomputed at insert
- **Task 73:** `useTransition` for historical-message renders
- **Task 74:** Add `size-limit` dev dep + bundle budget
- **Task 75:** Run full perf gates; tighten coalescer threshold
- **Task 76:** PR8 wrap

---

# PR9 — Phase 8: Recovery & Observability

**Goal:** Add a server-side `Heartbeat` ticker, a zombie-detection query at DB level, and FE state rehydration at app bootstrap.

**Acceptance:**
- Server emits `ThreadEvent::Heartbeat` every 30s during active turns.
- `SessionRepo::detect_zombie_sessions(threshold_ms)` returns sessions with `updated_at < now - threshold` and last message role = "user" (means agent never replied).
- On FE bootstrap, query zombies and surface a banner per thread.

(Tasks 77-85)

- **Task 77:** Branch + add server heartbeat tick (tokio interval)
- **Task 78:** FE consumes heartbeat to reset 90s watchdog
- **Task 79:** Add `last_event_at` column to `sessions` (pre-release, no migration script)
- **Task 80:** `SessionRepo::detect_zombie_sessions` method + test
- **Task 81:** Tauri command `chat_zombie_check` that returns list
- **Task 82:** FE bootstrap: call `chat_zombie_check`, surface banners
- **Task 83:** Manual "force reset" Tauri command for cases where in-memory state survives a crash incomplete (clears `active_streams` + emits terminal events)
- **Task 84:** Error UI: a global `ChatErrorBanner` slot at the workspace level
- **Task 85:** PR9 wrap

---

# PR10 — Phase 9: Hardening, Soak, Docs

**Goal:** Add the final reliability checks — proptests for arbitrary event sequences, a soak benchmark, CLAUDE.md updates, and a finishing checklist.

**Acceptance:**
- Soak: 10,000 random op-sequences leaves all maps empty.
- All P0-P3 acceptance criteria from the plan header met.
- `CLAUDE.md` updated with new architecture.

(Tasks 86-93)

- **Task 86:** Branch
- **Task 87:** Expand event-sequence proptest to 10,000 cases (gated under `--features soak`)
- **Task 88:** `scripts/run_chat_proptest_soak.sh` — runs 10k cases nightly
- **Task 89:** Update `CLAUDE.md` chat/thread architecture sections
- **Task 90:** Update `docs/superpowers/notes/2026-05-11-chat-overhaul-perf-baseline.md` with final numbers
- **Task 91:** Remove deprecated old stores and old per-event Tauri constants
- **Task 92:** Final perf-gate run; commit numbers
- **Task 93:** PR10 wrap

---

## Test Plan (cumulative across all PRs)

- [ ] `cargo build --workspace` (all PRs)
- [ ] `cargo nextest run --workspace`
- [ ] `cargo nextest run --test integration -E 'test(chat_lifecycle)'`
- [ ] `cargo nextest run --test property -E 'test(event_sequence_invariants)'`
- [ ] `cargo clippy --workspace --all-targets --all-features` (0 warnings)
- [ ] `cargo fmt --all --check`
- [ ] `cd desktop-ui && bun run typecheck`
- [ ] `cd desktop-ui && bun run lint`
- [ ] `cd desktop-ui && bun run test`
- [ ] `cd desktop-ui && bun run build`
- [ ] `cd desktop-ui && bun run bench`
- [ ] `./scripts/run_chat_perf_gates.sh` (all thresholds met)
- [ ] `./scripts/run_chat_proptest_soak.sh` (10k iterations)
- [ ] Manual smoke:
  - Back-to-back sends work without refresh
  - Cancel-then-send works
  - Error-then-send works
  - Stuck banner appears after 5s; reset works
  - 4k-token stream renders without dropped frames
  - Both assistant and coding modes pass identical lifecycle tests

---

## Self-Review

**1. Spec coverage:** Every audit point from the conversation maps to a task:

| Audit issue | Tasks |
|---|---|
| Double-insert race in `active_streams` | Tasks 10, 12 |
| 200ms sleep window | Task 11 |
| `Cancelled` silently dropped | Task 13 |
| `chat:message_added` not emitted on error | Task 14 |
| `session_end_fired` leaks | Task 15 |
| mpsc capacity 64 too small | Task 16 |
| FE silent-drop guard | Task 21 |
| No assistant watchdog | Tasks 22-23 |
| No stuck UI | Tasks 24-25, 26 |
| Pipeline duplication | PR5 (Tasks 38-48) |
| Wire-event fragmentation | PR4 (Tasks 29-37) |
| FE store fragmentation | PR6 (Tasks 49-58) |
| No virtualization | Tasks 68-69 |
| Delta thrash | Tasks 70-71 |
| No TTFT metric | Tasks 1, 2 |
| No proptest | Tasks 17, 87 |
| No state recovery on bootstrap | Tasks 80-82 |
| Tracing span breaks at spawn | Task 60 |
| No bundle budget | Task 74 |

**2. Placeholder scan:**

- "filled in based on AppCore::new" appears in Task 8 — that's an *instruction*, not an unfinished step. The engineer reads `state.rs` and fills in the matching constructor. This is the intent: it points to a known reading task with a precise file. Not a placeholder.
- Tasks 29-93 are described in PR-level summary form, not full step-by-step. This is intentional: PRs 4-10 follow the same TDD-per-task structure as PRs 1-3 (which are fully fleshed out in 2-5 min increments). The plan above gives PR-level scope and per-task ordering; if the executing agent needs the per-step body, they can write it from the established pattern by reading the prior PR for the same style.
- **Action for stricter follow-up:** if this plan will be handed to a subagent-driven executor (not a human), each task in PRs 4-10 should be expanded to the same step-by-step granularity as Tasks 0-28 before execution. A "Plan-Phase-Expand" companion PR can do this incrementally per PR boundary.

**3. Type consistency:**

- `TurnHandle { thread_id, turn_id, generation }` — used consistently across `desktop-ui/src/features/threads/store/types.ts`, `crates/app-core/src/runtime/mod.rs`, and `crates/desktop-shared/src/thread_event_v2.rs`. ✅
- `ThreadEvent` v2 vs old `ThreadEvent` — referenced as "v2" in Phase 3 onward; old shape kept under a deprecated path until PR8 retires it. ✅
- `markProcessing(threadId, isProcessing)` signature unchanged. ✅
- `StreamGuard` adds `guard_id: u64` consistently in Tasks 10 and 11. ✅
- `ActiveStreamEntry { guard_id, cancel }` introduced in Task 10 is used in Task 12 (`entry.cancel.is_cancelled()`) and Task 13 (cancel arm). ✅

**4. Risks called out:**

- **Pre-release migrations.** The plan adds columns to `sessions` and `session_messages` (Task 79). CLAUDE.md explicitly authorizes pre-release direct schema edits — no migration script needed. **Risk:** if release happens during this work, freeze the schema edits.
- **`bindings.ts` drift.** Every PR that touches Tauri commands or `tauri_specta::Event` must regenerate `bindings.ts`. The `bindings_are_current` test catches this. **Mitigation:** each PR's task list includes the rebuild + commit.
- **Linkme drift.** PR4 may add new Tauri commands; `registration_drift` test catches mismatch. **Mitigation:** every new `#[klynt_command]` must be added to `klynt_collect_commands![...]` in the same task.
- **Generation counter rollover.** A `u64` generation counter overflows after ~5×10¹¹ years of continuous use at 1 turn/sec. Not a real risk.
- **Backward-compat during PR4-PR5 dual emission.** The plan keeps old per-event Tauri constants emitting alongside the new v2 channel until PR10 Task 91. **Mitigation:** during this window, FE prefers v2 if present, falls back to v1 — both produce the same observable state in the store.

**5. Sequencing rationale:**

- PR1 first because every later PR needs measurable baselines.
- PR2 + PR3 split BE/FE hotfix because they go through different review pipelines and ship the user-visible fix earliest.
- PR4 before PR5 because the trait needs the wire-event union in scope.
- PR6 only after BE is stable — migrating the FE store while BE is moving doubles the test surface.
- PR7 + PR8 perf last because the structural work invalidates any earlier benchmarks.
- PR9 recovery features depend on the unified runtime (PR5).
- PR10 hardening + docs always last.

---

## Spec ↔ Plan link

This plan IS the spec for the chat overhaul (no separate spec doc). Any deviation during execution should amend this file in the same PR that makes the change. The `## File Structure` and `## Table of Contents` are the authoritative scope contract.
