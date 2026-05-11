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
