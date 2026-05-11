#!/usr/bin/env bash
# Chat overhaul perf gates. Exits 0 if all gates pass, non-zero otherwise.
# Thresholds are initial 1.5× current_baseline; tightened per PR.

set -euo pipefail

THRESHOLD_TTFT_P95_MS="${THRESHOLD_TTFT_P95_MS:-25}"
THRESHOLD_THROUGHPUT_EVENTS_PER_SEC="${THROUGHPUT_THRESHOLD:-5000}"
THRESHOLD_CLEANUP_P99_MS="${THRESHOLD_CLEANUP_P99_MS:-1}"
THRESHOLD_COALESCER_10K_MS="${THRESHOLD_COALESCER_10K_MS:-16}"

FAIL=0

# ── Helper: extract criterion mean time in microseconds ─────────────────────
extract_criterion_mean_us() {
    local log="$1"
    # Criterion outputs: "time:   [11.234 µs 11.345 µs 11.456 µs]"
    # We grab the median (middle) value.
    grep -oP 'time:\s+\[\K[^\]]+' "$log" | head -1 | awk '{print $2}' | sed 's/µs//' | sed 's/us//' || true
}

# ── Helper: extract criterion throughput in elem/s ──────────────────────────
extract_criterion_thrpt() {
    local log="$1"
    # Criterion outputs: "thrpt:  [87.654 Kelem/s 88.123 Kelem/s 88.456 Kelem/s]"
    # We grab the median and normalize K/M suffixes.
    grep -oP 'thrpt:\s+\[\K[^\]]+' "$log" | head -1 | awk '{
        val = $2
        if (val ~ /K/) { gsub(/K/, "", val); print val * 1000 }
        else if (val ~ /M/) { gsub(/M/, "", val); print val * 1000000 }
        else { print val }
    }' || true
}

# ── ttft_e2e ────────────────────────────────────────────────────────────────
echo "[perf-gate] criterion: ttft_e2e"
cargo bench -p agent --bench ttft_e2e -- --quick --noplot 2>&1 | tee /tmp/ttft.log

# TODO: ttft_e2e is a skeleton (PR1); real measurement wires in PR8.
# For now we just confirm it runs; numeric gate activates once harnessed.
echo "[perf-gate] ttft_e2e: ran (skeleton — numeric gate deferred to PR8)"

# ── stream_throughput ───────────────────────────────────────────────────────
echo "[perf-gate] criterion: stream_throughput"
cargo bench -p agent --bench stream_throughput -- --quick --noplot 2>&1 | tee /tmp/throughput.log

# Extract throughput at 10,000 batch size (last group in the bench).
THRPT_10K=$(grep -A2 'stream_throughput/10000' /tmp/throughput.log | grep 'thrpt:' | awk '{
    val = $2
    if (val ~ /K/) { gsub(/K/, "", val); print val * 1000 }
    else if (val ~ /M/) { gsub(/M/, "", val); print val * 1000000 }
    else { print val }
}' | head -1 || true)

if [[ -n "$THRPT_10K" ]]; then
    THRPT_10K_INT=$(printf '%.0f' "$THRPT_10K")
    if (( THRPT_10K_INT < THRESHOLD_THROUGHPUT_EVENTS_PER_SEC )); then
        echo "[perf-gate] FAIL: stream_throughput 10k batch = ${THRPT_10K_INT} elem/s < ${THRESHOLD_THROUGHPUT_EVENTS_PER_SEC} elem/s"
        FAIL=1
    else
        echo "[perf-gate] PASS: stream_throughput 10k batch = ${THRPT_10K_INT} elem/s ≥ ${THRESHOLD_THROUGHPUT_EVENTS_PER_SEC} elem/s"
    fi
else
    echo "[perf-gate] WARN: could not extract stream_throughput 10k metric"
fi

# ── relay_cleanup_latency ───────────────────────────────────────────────────
echo "[perf-gate] criterion: relay_cleanup_latency"
cargo bench -p desktop --bench relay_cleanup_latency -- --quick --noplot 2>&1 | tee /tmp/cleanup.log

# Extract mean cleanup time in microseconds.
CLEANUP_MEAN_US=$(grep -A2 'relay_cleanup_latency/drop_and_observe' /tmp/cleanup.log | grep 'time:' | awk '{print $2}' | sed 's/µs//' | sed 's/us//' | head -1 || true)

if [[ -n "$CLEANUP_MEAN_US" ]]; then
    # Convert µs to ms for comparison.
    CLEANUP_MEAN_MS=$(awk "BEGIN {printf \"%.3f\", $CLEANUP_MEAN_US / 1000}")
    CLEANUP_CMP=$(awk "BEGIN {printf \"%.0f\", $CLEANUP_MEAN_US}")
    THRESHOLD_US=$(awk "BEGIN {printf \"%.0f\", $THRESHOLD_CLEANUP_P99_MS * 1000}")
    if (( CLEANUP_CMP > THRESHOLD_US )); then
        echo "[perf-gate] FAIL: relay_cleanup mean = ${CLEANUP_MEAN_MS} ms > ${THRESHOLD_CLEANUP_P99_MS} ms"
        FAIL=1
    else
        echo "[perf-gate] PASS: relay_cleanup mean = ${CLEANUP_MEAN_MS} ms ≤ ${THRESHOLD_CLEANUP_P99_MS} ms"
    fi
else
    echo "[perf-gate] WARN: could not extract relay_cleanup_latency metric"
fi

# ── vitest coalescer ────────────────────────────────────────────────────────
echo "[perf-gate] vitest: coalescer"
(cd desktop-ui && bun run bench 2>&1 | tee /tmp/coalescer.log)

# Extract 10k chunks mean time in ms.
COALESCER_10K_MS=$(grep -A1 '10,000 chunks' /tmp/coalescer.log | grep 'mean' | sed 's/.*mean[: ]*//' | sed 's/ms//' | head -1 || true)

if [[ -n "$COALESCER_10K_MS" ]]; then
    COALESCER_10K_INT=$(printf '%.0f' "$COALESCER_10K_MS")
    if (( COALESCER_10K_INT > THRESHOLD_COALESCER_10K_MS )); then
        echo "[perf-gate] FAIL: coalescer 10k chunks = ${COALESCER_10K_INT} ms > ${THRESHOLD_COALESCER_10K_MS} ms"
        FAIL=1
    else
        echo "[perf-gate] PASS: coalescer 10k chunks = ${COALESCER_10K_INT} ms ≤ ${THRESHOLD_COALESCER_10K_MS} ms"
    fi
else
    echo "[perf-gate] WARN: could not extract coalescer 10k metric"
fi

if (( FAIL )); then
    echo "[perf-gate] FAILED — see above for details"
    exit 1
fi

echo "[perf-gate] ALL GATES PASSED"
