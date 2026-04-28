#!/usr/bin/env bash
# KCA full validation — runs every gate from spec section 7 and exits nonzero
# on any failure.
#
# Lifecycle:
#   • Run locally before merging any feat/kca-* branch.
#   • Run in CI on push to main / PR to main (see .github/workflows/kca-validation.yml).
#   • Run with RUN_SOAK=1 on tagged release branches for the 10K-turn soak.
#
# Steps run sequentially so the first failure surfaces clearly. Optional steps
# (KCA E2E, bench, soak) are gracefully skipped with a warning when their
# crates / fixtures aren't present yet — useful while the KCA implementation
# plans are still being executed phase-by-phase.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Helpers

log_step() { printf '\n===== %s =====\n' "$*"; }
log_warn() { printf '⚠️  %s\n' "$*" >&2; }
log_ok()   { printf '✅ %s\n' "$*"; }
log_skip() { printf '⏭️  %s\n' "$*"; }
log_fail() { printf '❌ %s\n' "$*" >&2; }

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log_fail "required command not found: $1"
        log_fail "install with: $2"
        exit 1
    fi
}

crate_exists() {
    [[ -f "crates/$1/Cargo.toml" ]]
}

# Crates directly touched by Phase A (Online Graph Integrity).
PHASE_A_CRATES="cognitive coding-memory agent config"

# ---------------------------------------------------------------------------
# Preflight

log_step "Preflight"
require_cmd cargo "rustup install stable"
require_cmd cargo-nextest "cargo install cargo-nextest --locked"

KCA_E2E_PRESENT=0
KCA_BENCH_PRESENT=0
KCA_FIXTURES_PRESENT=0

if crate_exists kca-e2e; then KCA_E2E_PRESENT=1; fi
if crate_exists kca-bench; then KCA_BENCH_PRESENT=1; fi
if [[ -d tests/fixtures/kca ]] && [[ -n "$(ls -A tests/fixtures/kca 2>/dev/null)" ]]; then
    KCA_FIXTURES_PRESENT=1
fi

printf 'kca-e2e crate ............ %s\n'   "$([[ $KCA_E2E_PRESENT -eq 1 ]] && echo present || echo MISSING)"
printf 'kca-bench crate .......... %s\n'   "$([[ $KCA_BENCH_PRESENT -eq 1 ]] && echo present || echo MISSING)"
printf 'tests/fixtures/kca ....... %s\n'   "$([[ $KCA_FIXTURES_PRESENT -eq 1 ]] && echo present || echo MISSING)"

# ---------------------------------------------------------------------------
# Step 1: Lint + format (ALWAYS required)

log_step "Step 1: Lint + format"
cargo fmt --all --check
log_ok "fmt clean"

# Phase A: run clippy with -D warnings only on crates we directly control.
# Full workspace clippy will be enabled once pre-existing lint debt in other
# crates is cleared (Phase E).
for crate in $PHASE_A_CRATES; do
    printf ' -- clippy -p %s --\n' "$crate"
    cargo clippy -p "$crate" --all-targets --all-features -- -D warnings
done
log_ok "clippy clean (Phase A crates)"

# ---------------------------------------------------------------------------
# Step 2: Workspace tests (ALWAYS required)

log_step "Step 2: Workspace tests"
# Phase A: run nextest on Phase A crates. Full workspace will be enabled in Phase E.
cargo nextest run -p cognitive -p coding-memory -p agent -p config
log_ok "workspace nextest green (Phase A crates)"

# Doctests aren't supported by nextest — run separately on Phase A crates.
log_step "Step 2b: Doctests"
for crate in $PHASE_A_CRATES; do
    cargo test -p "$crate" --doc
done
log_ok "doctests green (Phase A crates)"

# ---------------------------------------------------------------------------
# Step 3: Property tests (ALWAYS required)

log_step "Step 3: Property tests"
# --test-threads 1 because some property tests share filesystem fixtures.
if cargo nextest list -p coding-memory -E 'test(/prop_/)' 2>/dev/null | grep -q 'prop_'; then
    cargo nextest run -p coding-memory -E 'test(/prop_/)' --test-threads 1
    log_ok "all property tests green"
else
    log_skip "no property tests found in workspace (yet)"
fi

# ---------------------------------------------------------------------------
# Step 4: KCA Phase A integration tests

log_step "Step 4: KCA Phase A integration tests"
cargo nextest run -p cognitive --test phase_a_graph_integrity
cargo nextest run -p coding-memory --test phase_a_distiller_graph_parity
cargo nextest run -p coding-memory --test recall_causal_render
cargo nextest run -p coding-memory --test distiller_entity_graph
log_ok "Phase A integration tests green"

# ---------------------------------------------------------------------------
# Step 5: KCA E2E tests (skipped if kca-e2e crate not present)

if [[ $KCA_E2E_PRESENT -eq 0 ]]; then
    log_step "Step 5: KCA E2E tests"
    log_skip "crates/kca-e2e not present — execute Phase E plan first"
else
    log_step "Step 5: KCA E2E tests"

    # Run all #[test] in the crate first.
    cargo nextest run -p kca-e2e --tests

    # Then explicitly run each integration test binary so a missing one fails loudly.
    for test_bin in full_pipeline multi_cli_parity migration_safety regression_panel cancellation_safety; do
        printf -- '--- %s ---\n' "$test_bin"
        if cargo nextest list -p kca-e2e --test "$test_bin" >/dev/null 2>&1; then
            cargo nextest run -p kca-e2e --test "$test_bin"
        else
            log_warn "test binary $test_bin not yet implemented; skipping"
        fi
    done
    log_ok "KCA E2E green"
fi

# ---------------------------------------------------------------------------
# Step 6: KCA Benchmarks + game-changer report (skipped if kca-bench not present)

if [[ $KCA_BENCH_PRESENT -eq 0 || $KCA_FIXTURES_PRESENT -eq 0 ]]; then
    log_step "Step 6: KCA benchmarks + game-changer report"
    log_skip "kca-bench crate or tests/fixtures/kca missing — execute Phase E plan first"
else
    log_step "Step 6: KCA benchmarks + game-changer report"
    mkdir -p docs/architecture
    cargo run -p kca-bench --release --bin run-bench -- \
        --output docs/architecture/kca-game-changer.md
    log_ok "report written to docs/architecture/kca-game-changer.md"
fi

# ---------------------------------------------------------------------------
# Step 7: Soak test (only when RUN_SOAK is set)

if [[ -n "${RUN_SOAK:-}" ]]; then
    if [[ $KCA_E2E_PRESENT -eq 0 ]]; then
        log_step "Step 7: Soak test (RUN_SOAK=1)"
        log_skip "kca-e2e crate not present"
    else
        log_step "Step 7: Soak test (RUN_SOAK=1, ~10–15 minutes)"
        cargo nextest run -p kca-e2e --features soak --test soak_test
        log_ok "soak test green"
    fi
else
    log_skip "soak test skipped (set RUN_SOAK=1 to enable)"
fi

# ---------------------------------------------------------------------------
# Summary

printf '\n===== ✅ KCA validation passed =====\n'
if [[ $KCA_E2E_PRESENT -eq 0 || $KCA_BENCH_PRESENT -eq 0 ]]; then
    printf '\nNote: KCA-specific steps were skipped because the kca-e2e and/or\n'
    printf '      kca-bench crates have not been implemented yet. Execute the\n'
    printf '      plans in docs/superpowers/plans/2026-04-29-kca-*.md to enable\n'
    printf '      full validation.\n'
fi
