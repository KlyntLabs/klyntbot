#!/usr/bin/env bash
set -euo pipefail

# Build Klynt desktop app locally for production use.
# Usage: ./scripts/build-local.sh [--install]

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/klynt.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="klynt2026"

echo "==> Building frontend..."
cd desktop-ui && bun install --frozen-lockfile && bun run build && cd ..

echo "==> Building Tauri app (release)..."
cd crates/desktop
cargo tauri build --target aarch64-apple-darwin --config '{"build":{"beforeBuildCommand":""}}'
cd "$REPO_ROOT"

DMG="target/aarch64-apple-darwin/release/bundle/dmg/Klynt_0.1.0_aarch64.dmg"
APP="target/aarch64-apple-darwin/release/bundle/macos/Klynt.app"

echo ""
echo "==> Build complete!"
echo "    .app: $APP"
echo "    .dmg: $DMG"

if [[ "${1:-}" == "--install" ]]; then
    echo "==> Installing to /Applications..."
    rm -rf /Applications/Klynt.app
    cp -rf "$APP" /Applications/Klynt.app
    echo "    Installed! Launch Klynt from Applications."
fi
