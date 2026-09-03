#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

BASE="3b3b1bab8d2ec80deef263df61960d89f2cfa40a"
# First column of `verify:frontend --list` (name mode profiles cwd command).
CHECKS=()
while IFS= read -r line; do
  [[ -z "$line" ]] && continue
  CHECKS+=("${line%% *}")
done < <(bun run verify:frontend --list)
test "${#CHECKS[@]}" -gt 0

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

log_name() {
  printf '%s' "${1//:/_}"
}

echo "=== Acceptance: per-check exit parity ==="
for name in "${CHECKS[@]}"; do
  safe="$(log_name "$name")"
  direct=0
  (cd desktop-ui && bun run "$name") >/dev/null 2>&1 || direct=$?
  matrix=0
  bun run verify:frontend "$name" >"$TMP/matrix-${safe}.log" 2>&1 || matrix=$?
  echo "parity  ${name}  direct=${direct}  matrix=${matrix}"
  test "$direct" -eq "$matrix"
done

echo "=== Acceptance: twice-run summary ==="
run1=0
bun run verify:frontend >"$TMP/run1.log" 2>&1 || run1=$?
run2=0
bun run verify:frontend >"$TMP/run2.log" 2>&1 || run2=$?
echo "full-matrix exit  run1=${run1}  run2=${run2}"
test "$run1" -eq "$run2"

extract_summary() {
  local n="${#CHECKS[@]}"
  awk -v n="$n" '
    /^name[[:space:]]+mode[[:space:]]+result[[:space:]]+exit[[:space:]]+seconds/ {
      p = 1
      print
      next
    }
    p {
      print
      if (++rows == n) exit
    }
  ' "$1"
}

# Wall-clock seconds jitter across runs; compare name/mode/result/exit only.
normalize_summary() {
  awk '{ print $1, $2, $3, $4 }'
}

extract_summary "$TMP/run1.log" | tee "$TMP/sum1.txt" | normalize_summary >"$TMP/sum1.norm"
extract_summary "$TMP/run2.log" | tee "$TMP/sum2.txt" | normalize_summary >"$TMP/sum2.norm"

echo "--- summary run1 ---"
cat "$TMP/sum1.txt"
echo "--- summary run2 ---"
cat "$TMP/sum2.txt"
echo "--- summary diff (name mode result exit) ---"
diff -u "$TMP/sum1.norm" "$TMP/sum2.norm"

echo "=== Acceptance: quick-check duration ceiling ==="
quick_sum=$(awk '
  $1 == "typecheck" || $1 == "lint" || $1 == "test" || $1 == "check:tokens" {
    s += $5
  }
  END { printf "%.1f", s + 0 }
' "$TMP/sum1.txt")
echo "quick duration sum=${quick_sum}s (ceiling 120)"
awk -v s="$quick_sum" 'BEGIN {
  if ((s + 0) > 120) {
    printf "FAIL: quick sum %.1f exceeds 120\n", s + 0
    exit 1
  }
  print "quick duration OK"
}'

echo "=== Acceptance: relayed observation markers ==="
grep -F '==> Raw color literals' "$TMP/run1.log" >/dev/null
echo "found: ==> Raw color literals"
grep -E '1 passed' "$TMP/run1.log" >/dev/null
echo "found: 1 passed"
grep -E '[[:space:]]gzip[[:space:]]+file' "$TMP/run1.log" >/dev/null
echo "found: gzip column header"

echo "=== Acceptance: change-boundary diffs vs ${BASE} ==="
git diff --exit-code "$BASE" -- \
  packages/design-system/src/tokens/ \
  scripts/check-design-tokens.sh \
  desktop-ui/playwright.config.ts \
  desktop-ui/tests/ \
  desktop-ui/scripts/check-performance-budget.sh
echo "boundary tokens/playwright/tests/perf: OK"

git diff --exit-code "$BASE" -- desktop-ui/package.json
echo "boundary desktop-ui/package.json: OK"

python3 - "$BASE" <<'PY'
import subprocess
import sys

import yaml

base = sys.argv[1]
base_doc = yaml.safe_load(
    subprocess.check_output(
        ["git", "show", f"{base}:.github/workflows/ci.yml"],
        text=True,
    )
)
with open(".github/workflows/ci.yml", encoding="utf-8") as fh:
    head_doc = yaml.safe_load(fh)
for name in ("rust-quality", "desktop-build-check"):
    if base_doc["jobs"][name] != head_doc["jobs"][name]:
        print(f"DIFF in job {name}", file=sys.stderr)
        sys.exit(1)
    print(f"boundary job {name}: OK")
PY

echo "=== ACCEPTANCE PASS ==="
