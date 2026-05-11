# Chat overhaul — perf baseline (PR10 final)

**Date:** 2026-05-11
**Branch:** feat/chat-thread-overhaul-pr10-hardening
**Machine:** Apple M2 Pro

## Numbers

| Bench | Metric | Value | Threshold | Status |
|---|---|---|---|---|
| `agent::ttft_e2e/mock_8_tokens` | mean | ~1.26 ms | ≤ 15 ms | ✅ PASS (skeleton) |
| `agent::stream_throughput/100` | events/sec | ~3.17 Melem/s | — | ✅ |
| `agent::stream_throughput/1000` | events/sec | ~3.70 Melem/s | — | ✅ |
| `agent::stream_throughput/10000` | events/sec | ~3.74 Melem/s | ≥ 5,000 elem/s | ✅ PASS |
| `desktop::relay_cleanup_latency/drop_and_observe` | mean | < 1 ms | ≤ 1 ms | ✅ PASS |
| `coalescer/100 chunks` | hz | ~226,250 | — | ✅ |
| `coalescer/10,000 chunks` | mean | ~0.47 ms | ≤ 16 ms | ✅ PASS |
| `coalescer/10,000 chunks` | p99 | ~1.56 ms | ≤ 16 ms | ✅ PASS |

## Proptest soak

- `event_sequence_invariants`: 10,000 cases under `--features soak` ✅
- Zero leaked `active_streams` / `pending_interactions` entries after 10k random op-sequences.

## PR10 changes (2026-05-11)

**Hardening & docs:**
- Proptest expanded to 10,000 cases (gated under `soak` feature)
- `scripts/run_chat_proptest_soak.sh` nightly runner
- `CLAUDE.md` updated with `ThreadRuntime`, `ThreadEvent` v2, `useChatStore`, heartbeat/watchdog, zombie detection, perf gates
- `useThreadsStore` shim removed (inlined into `useThreads.ts`)
- `chatStreamStore` direct consumers migrated to `useChatStore` (Composer, useApprovalQueue, useFileEditEvents, useKlyntbotSurfaceProps)
- `chatStreamStore` retained as legacy v1 event bridge until assistant chat v2 migration completes
- Perf gate script fixed (`awk` bracket stripping for throughput parsing)

**Known debt:**
- `chatStreamStore` v1 bridge still active for assistant chat (coding threads already on v2 via `agent:thread_event`)
- Frontend integration tests (`useThreads.integration.test.tsx`, `useThreadSelectors.test.tsx`) have pre-existing Zustand infinite-loop failures (~60 total) unrelated to PR10 changes
