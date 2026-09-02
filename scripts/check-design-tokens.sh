#!/usr/bin/env bash
# Design-system gates for desktop-ui (fast soft/hard modes).
#
# Raw-literal carve-outs (intentional, do not "fix" by inlining tokens):
#   - ThemeSwitcher previews (theme swatches)
#   - tagColor.ts (deterministic tag palette)
#   - productivity.tsx app brand map (external app icon colors)
#   - ProgressRing decorative gradients + tests (shared/ui + composites)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UI_SRC="$ROOT/desktop-ui/src"
FAIL=0

echo "==> Legacy utility class names"
LEGACY_PAT='text-muted-foreground|text-foreground\b|bg-background\b|bg-card\b|border-border\b|bg-muted\b|focus:border-brand|bg-surface-'
legacy="$(rg -n --pcre2 "$LEGACY_PAT" "$UI_SRC" --glob '*.{tsx,ts,css}' || true)"
count="$(printf '%s\n' "$legacy" | rg -c '.' || true)"
count="${count:-0}"
if [[ "$count" -gt 0 ]]; then
  echo "Found $count legacy references:"
  printf '%s\n' "$legacy" | head -40
  FAIL=1
else
  echo "OK: no legacy utility class names."
fi

echo
echo "==> Legacy runtime CSS vars"
VARS_PAT='var\(--brand\)|var\(--destructive\)|var\(--success\)|var\(--text-muted-foreground\)|var\(--surface-highest\)'
vars="$(rg -n --pcre2 "$VARS_PAT" "$UI_SRC" --glob '*.{tsx,ts,css}' || true)"
vcount="$(printf '%s\n' "$vars" | rg -c '.' || true)"
vcount="${vcount:-0}"
if [[ "$vcount" -gt 0 ]]; then
  echo "Found $vcount legacy var() refs:"
  printf '%s\n' "$vars" | head -40
  FAIL=1
else
  echo "OK: no legacy var(--brand/--destructive/...) refs."
fi

echo
echo "==> Deep imports of @klyntbot/design-system"
deep="$(rg -n '@klyntbot/design-system/' "$UI_SRC" --glob '*.{tsx,ts}' || true)"
deep_filtered="$(printf '%s\n' "$deep" | rg -v 'design-system/styles/' || true)"
if [[ -n "${deep_filtered// }" ]]; then
  echo "FAIL: deep imports (barrel only, except styles/index.css):"
  echo "$deep_filtered"
  FAIL=1
else
  echo "OK: barrel-only TS imports."
fi

echo
echo "==> Raw color literals in styles/ + shared/ (chrome)"
hits="$(
  rg -n --pcre2 '(#[0-9a-fA-F]{3,8}\b|rgba?\()' \
    "$UI_SRC/styles" "$UI_SRC/shared" \
    --glob '!**/ThemeSwitcher.tsx' \
    --glob '!**/tagColor.ts' \
    --glob '!**/productivity.tsx' \
    --glob '!**/ProgressRing.tsx' \
    --glob '!**/ProgressRing.test.tsx' \
    || true
)"
filtered="$(printf '%s\n' "$hits" | rg -v 'var\(--|^\s*/\*|^\s*\*|^\s*\*/|color-mix\(' || true)"
if [[ -n "${filtered// }" ]]; then
  echo "Found raw literals in styles/shared:"
  echo "$filtered" | head -40
  if [[ "${HARD:-0}" == "1" ]]; then
    FAIL=1
  else
    echo "(soft) Set HARD=1 to fail on these."
  fi
else
  echo "OK: no raw chrome literals in styles/shared (excl. documented carve-outs)."
fi

exit "$FAIL"
