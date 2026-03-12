#!/usr/bin/env bash
# workspace/scripts/verify.sh
# Run after each refactor phase to verify correctness.
# Usage: ./workspace/scripts/verify.sh [phase_number]
#
# Example:
#   ./workspace/scripts/verify.sh 1     # Verify after Phase 1 (common restructure)
#   ./workspace/scripts/verify.sh all   # Full verification

set -euo pipefail

PHASE=${1:-"all"}
CRATE=${2:-""}
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
RESULTS_DIR="$ROOT/workspace/scripts/results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
LOG="$RESULTS_DIR/verify-phase${PHASE}-${TIMESTAMP}.log"

echo "═══════════════════════════════════════════════════════"
echo "  Klyntbot Refactor Verification — Phase $PHASE"
echo "  $(date)"
echo "═══════════════════════════════════════════════════════"
echo ""

ERRORS=0

# ─── 1. Build ────────────────────────────────────────────────────────────────
echo "→ [1/6] Building workspace..."
if cargo build --workspace 2>&1 | tee -a "$LOG" | tail -3; then
  echo "  ✓ Build passed"
else
  echo "  ✗ BUILD FAILED"
  ERRORS=$((ERRORS + 1))
fi

# ─── 2. Tests ────────────────────────────────────────────────────────────────
echo ""
echo "→ [2/6] Running test suite..."
if [ -n "$CRATE" ]; then
  TEST_CMD="cargo nextest run -p $CRATE"
else
  TEST_CMD="cargo nextest run --workspace"
fi

if $TEST_CMD 2>&1 | tee -a "$LOG" | tail -5; then
  PASSED=$(grep -o "[0-9]* passed" "$LOG" | tail -1 || echo "? passed")
  FAILED=$(grep -o "[0-9]* failed" "$LOG" | tail -1 || echo "0 failed")
  echo "  ✓ Tests: $PASSED, $FAILED"
else
  echo "  ✗ TESTS FAILED"
  ERRORS=$((ERRORS + 1))
fi

# ─── 3. Clippy ───────────────────────────────────────────────────────────────
echo ""
echo "→ [3/6] Running clippy (zero-warnings policy)..."
CLIPPY_OUT=$(cargo clippy --workspace --all-targets --all-features 2>&1)
echo "$CLIPPY_OUT" >> "$LOG"
CLIPPY_ERRORS=$(echo "$CLIPPY_OUT" | grep -c "^error" || true)
CLIPPY_WARNINGS=$(echo "$CLIPPY_OUT" | grep -c "^warning" || true)

if [ "$CLIPPY_ERRORS" -gt "0" ]; then
  echo "  ✗ Clippy: $CLIPPY_ERRORS errors, $CLIPPY_WARNINGS warnings"
  echo "$CLIPPY_OUT" | grep "^error" | head -10
  ERRORS=$((ERRORS + 1))
else
  echo "  ✓ Clippy: 0 errors, $CLIPPY_WARNINGS warnings"
fi

# ─── 4. Format ───────────────────────────────────────────────────────────────
echo ""
echo "→ [4/6] Checking formatting..."
if cargo fmt --all --check 2>&1 | tee -a "$LOG"; then
  echo "  ✓ Format clean"
else
  echo "  ✗ Format issues (run: cargo fmt --all)"
  ERRORS=$((ERRORS + 1))
fi

# ─── 5. Doc Tests ────────────────────────────────────────────────────────────
echo ""
echo "→ [5/6] Running doc tests..."
if cargo test --workspace --doc 2>&1 | tee -a "$LOG" | tail -3; then
  echo "  ✓ Doc tests passed"
else
  echo "  ✗ DOC TESTS FAILED"
  ERRORS=$((ERRORS + 1))
fi

# ─── 6. Large File Check ─────────────────────────────────────────────────────
echo ""
echo "→ [6/6] Files > 400 lines (should decrease each phase)..."
LARGE_FILES=$(find "$ROOT/crates" -name "*.rs" | xargs wc -l 2>/dev/null | \
  awk '$1 > 400 && $2 != "total"' | sort -rn)
FILE_COUNT=$(echo "$LARGE_FILES" | grep -c "." || true)
echo "$LARGE_FILES" | head -20
echo "  → $FILE_COUNT files over 400 lines"
echo "$LARGE_FILES" >> "$LOG"

# ─── Summary ─────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════"
if [ "$ERRORS" -eq "0" ]; then
  echo "  ✅ Phase $PHASE verification PASSED ($FILE_COUNT large files remaining)"
else
  echo "  ❌ Phase $PHASE verification FAILED ($ERRORS checks failed)"
fi
echo "  Log: $LOG"
echo "═══════════════════════════════════════════════════════"

exit $ERRORS
