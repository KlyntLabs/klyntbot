#!/usr/bin/env bash
# Smoke test for adapt_codex_vendor.sh
set -euo pipefail
output=$(bash "$(dirname "$0")/adapt_codex_vendor.sh" --help 2>&1)
echo "$output" | grep -q "Usage:" || { echo "FAIL: usage line missing"; exit 1; }
echo "PASS: usage line present"
