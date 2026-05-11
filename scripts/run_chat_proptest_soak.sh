#!/usr/bin/env bash
# Soak test: run the event-sequence proptest with 10,000 cases.
# Usage: ./scripts/run_chat_proptest_soak.sh

set -euo pipefail

echo "[soak] Running event_sequence_invariants with 10,000 cases..."
cargo nextest run --test property -E 'test(active_streams_drains_soaked)' --features soak 2>&1 | tee /tmp/soak.log

echo "[soak] Done. Log: /tmp/soak.log"
