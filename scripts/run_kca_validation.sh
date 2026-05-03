#!/usr/bin/env bash
# KCA full validation — runs every gate and exits nonzero on any failure.
# NOTE: Requires a valid ~/.klyntbot/config.json with API keys for E2E tests.

set -euo pipefail

export RUST_MIN_STACK=8388608

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "===== Step 1: Lint KCA crates ====="
cargo fmt -p kca-e2e -p kca-bench --check
cargo clippy -p kca-e2e -p kca-bench --all-targets --all-features --no-deps -- -D warnings

echo "===== Step 2: KCA unit + integration tests ====="
cargo nextest run -p kca-e2e -p kca-bench --tests

echo "===== Step 3: KCA E2E tests (real API) ====="
cargo nextest run -p kca-e2e --test cancellation_safety
cargo nextest run -p kca-e2e --test migration_safety
cargo nextest run -p kca-e2e --test regression_panel
cargo nextest run -p kca-e2e --test multi_cli_parity
cargo nextest run -p kca-e2e --test full_pipeline

echo "===== Step 4: KCA Benchmark build check ====="
cargo check -p kca-bench

echo "===== Step 5: KCA Benchmark — real LoCoMo (Letta-comparable) ====="
mkdir -p docs/architecture
# Default: 2 conversations × 10 QA = ~20 questions, ~3 min. Override with
# KCA_LOCOMO_LIMIT / KCA_LOCOMO_QA_LIMIT for fuller runs (full = 10 × ~150 ≈ 1500).
# Requires OPENAI_API_KEY for the SimpleQA grader (gpt-4.1).
KCA_LOCOMO_LIMIT="${KCA_LOCOMO_LIMIT:-2}" \
KCA_LOCOMO_QA_LIMIT="${KCA_LOCOMO_QA_LIMIT:-10}" \
  cargo run -p kca-bench --release --bin run-locomo-real

if [[ -n "${RUN_SOAK:-}" ]]; then
    echo "===== Step 6: Soak test (RUN_SOAK=1) ====="
    cargo nextest run -p kca-e2e --features soak --test soak_test
fi

echo ""
echo "===== ✅ KCA validation passed ====="
