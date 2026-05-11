# Chat overhaul — perf baseline (PR1 end)

**Date:** 2026-05-11
**Branch:** feat/chat-thread-overhaul-pr1-baseline
**Machine:** Apple M2 Pro

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

## PR7 changes (2026-05-11)

**Structural improvements landed:**
- Span propagation across all 4 `tokio::spawn` sites in `streaming.rs`
- Explicit drop arms for unhandled `AgentEvent` variants (no more silent `_ => {}`)
- `add_message` wrapped in SQLite transaction (atomicity + fewer fsyncs)
- `McpManager` locking: `tokio::sync::Mutex` → `tokio::sync::RwLock` (read concurrency for health-check reads)
- `scripts/run_chat_perf_gates.sh` now has numeric `awk` assertions on throughput and cleanup latency

**Benches:** Full criterion runs timed out in CI due to release-profile compilation (~5 min per bench). Local run recommended for final numbers.
