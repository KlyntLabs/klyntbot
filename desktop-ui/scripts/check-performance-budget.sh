#!/usr/bin/env bash
set -euo pipefail

# Performance budget gate for the Tauri desktop UI.
# Run after `bun run build` (or via `bun run check:performance`).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${UI_DIR}/dist"
ASSETS_DIR="${DIST_DIR}/assets"
INDEX_HTML="${DIST_DIR}/index.html"

if [ ! -d "${DIST_DIR}" ] || [ ! -f "${INDEX_HTML}" ]; then
  echo "❌ Performance budget check requires a production build first."
  echo "   Run: cd desktop-ui && bun run build"
  exit 1
fi

# Soft budgets for a local Tauri webview (not a public CDN).
# Calibrated 2026-09-02 from current main chunk sizes (~44 kB JS / ~23 kB CSS gzip);
# headroom left so route/feature growth does not trip the gate immediately.
MAX_ENTRY_JS_BYTES=819200    # 800 KB gzip — entry <script> tags in index.html
MAX_MAIN_CSS_BYTES=81920     # 80 KB gzip — main-*.css (or largest CSS fallback)

format_kb() {
  awk "BEGIN { printf \"%.1f kB\", $1 / 1024 }"
}

gzipped_size() {
  gzip -9 -c "$1" | wc -c | tr -d ' '
}

# ---------------------------------------------------------------------------
# Entry JS: sum of <script src="..."> assets referenced from index.html.
# Falls back to dist/assets/main-*.js if HTML parsing yields nothing.
# ---------------------------------------------------------------------------
ENTRY_JS_BYTES=0
ENTRY_JS_FILES=()

while IFS= read -r asset; do
  [ -z "${asset}" ] && continue
  path="${DIST_DIR}${asset}"
  if [ -f "${path}" ]; then
    ENTRY_JS_FILES+=("${path}")
    ENTRY_JS_BYTES=$((ENTRY_JS_BYTES + $(gzipped_size "${path}")))
  fi
done < <(grep -oE '<script[^>]+src=["'"'"'](/assets/[^"'"'"']+\.js)["'"'"']' "${INDEX_HTML}" \
  | grep -oE '/assets/[^"'"'"']+\.js' \
  | sort -u)

if [ "${#ENTRY_JS_FILES[@]}" -eq 0 ]; then
  shopt -s nullglob
  for path in "${ASSETS_DIR}"/main-*.js; do
    ENTRY_JS_FILES+=("${path}")
    ENTRY_JS_BYTES=$((ENTRY_JS_BYTES + $(gzipped_size "${path}")))
  done
  shopt -u nullglob
fi

# ---------------------------------------------------------------------------
# Main CSS: prefer main-*.css; else largest *.css under assets/.
# ---------------------------------------------------------------------------
MAIN_CSS_PATH=""
MAIN_CSS_BYTES=0

shopt -s nullglob
MAIN_CSS_CANDIDATES=("${ASSETS_DIR}"/main-*.css)
shopt -u nullglob

if [ "${#MAIN_CSS_CANDIDATES[@]}" -gt 0 ]; then
  # If multiple main-*.css exist, pick the largest gzipped.
  LARGEST=0
  for path in "${MAIN_CSS_CANDIDATES[@]}"; do
    size=$(gzipped_size "${path}")
    if [ "${size}" -gt "${LARGEST}" ]; then
      LARGEST="${size}"
      MAIN_CSS_PATH="${path}"
      MAIN_CSS_BYTES="${size}"
    fi
  done
else
  while IFS= read -r line; do
    [ -z "${line}" ] && continue
    MAIN_CSS_BYTES=$(echo "${line}" | awk '{print $1}')
    MAIN_CSS_PATH=$(echo "${line}" | cut -d' ' -f2-)
    break
  done < <(
    shopt -s nullglob
    for path in "${ASSETS_DIR}"/*.css; do
      printf "%s %s\n" "$(gzipped_size "${path}")" "${path}"
    done | sort -rn
  )
fi

echo "Performance budget check (desktop-ui / Tauri)"
echo "---------------------------------------------"
echo "Entry JS (gzipped): $(format_kb ${ENTRY_JS_BYTES}) / $(format_kb ${MAX_ENTRY_JS_BYTES})"
if [ "${#ENTRY_JS_FILES[@]}" -gt 0 ]; then
  for path in "${ENTRY_JS_FILES[@]}"; do
    echo "  - $(basename "${path}"): $(format_kb $(gzipped_size "${path}"))"
  done
else
  echo "  (no entry JS assets found)"
fi

if [ -n "${MAIN_CSS_PATH}" ]; then
  echo "Main CSS (gzipped): $(format_kb ${MAIN_CSS_BYTES}) / $(format_kb ${MAX_MAIN_CSS_BYTES})"
  echo "  - $(basename "${MAIN_CSS_PATH}")"
else
  echo "Main CSS (gzipped): (none found) / $(format_kb ${MAX_MAIN_CSS_BYTES})"
fi

echo ""
echo "Top 10 JS chunks by gzip size (visibility only — not individually gated):"
echo "------------------------------------------------------------------------"
printf "%10s  %s\n" "gzip" "file"
shopt -s nullglob
{
  for path in "${ASSETS_DIR}"/*.js; do
    printf "%10s  %s\n" "$(gzipped_size "${path}")" "$(basename "${path}")"
  done
} | sort -rn | head -n 10 | while IFS= read -r line; do
  size=$(echo "${line}" | awk '{print $1}')
  name=$(echo "${line}" | awk '{print $2}')
  printf "%10s  %s\n" "$(format_kb "${size}")" "${name}"
done
shopt -u nullglob

FAIL=0

if [ "${ENTRY_JS_BYTES}" -gt "${MAX_ENTRY_JS_BYTES}" ]; then
  echo ""
  echo "❌ Entry JS budget exceeded. Split the shell entry or drop weight before raising the budget."
  FAIL=1
fi

if [ -z "${MAIN_CSS_PATH}" ]; then
  echo ""
  echo "❌ No main CSS asset found under dist/assets/."
  FAIL=1
elif [ "${MAIN_CSS_BYTES}" -gt "${MAX_MAIN_CSS_BYTES}" ]; then
  echo ""
  echo "❌ Main CSS budget exceeded."
  FAIL=1
fi

echo ""
if [ "${FAIL}" -eq 0 ]; then
  echo "✅ Performance budget passed."
  exit 0
else
  echo "💡 Tip: inspect dist/assets/ chunk sizes after bun run build; heavy deps belong in manualChunks / React.lazy."
  exit 1
fi
