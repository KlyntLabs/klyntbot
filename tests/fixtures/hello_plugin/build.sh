#!/usr/bin/env bash
# Build the hello plugin for integration tests.
# Requires: cargo + wasm32-wasip1 target
# Usage: ./build.sh
set -e
rustup target add wasm32-wasip1 2>/dev/null || true
cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/hello_plugin.wasm ./plugin.wasm
echo "Built: plugin.wasm"
