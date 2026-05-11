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
