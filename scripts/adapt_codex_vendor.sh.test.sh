#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADAPT="$SCRIPT_DIR/adapt_codex_vendor.sh"

TMP=$(mktemp -d); trap "rm -rf $TMP" EXIT
mkdir -p "$TMP/codex-rs/protocol/src"
cat > "$TMP/codex-rs/protocol/Cargo.toml" <<EOF
[package]
name = "codex-protocol"
version = "0.1.0"
edition = "2021"
EOF
echo 'pub mod op;' > "$TMP/codex-rs/protocol/src/lib.rs"
echo 'pub use codex_protocol::Submission;' > "$TMP/codex-rs/protocol/src/op.rs"
( cd "$TMP" && tar czf codex.tgz codex-rs )

DEST=$(mktemp -d); trap "rm -rf $TMP $DEST" EXIT
"$ADAPT" --from-tar "$TMP/codex.tgz" --source codex-rs/protocol \
  --dest "$DEST/klynt-protocol-staging" \
  --rename codex-protocol=klynt-protocol \
  --rename codex_protocol=klynt_protocol

test -f "$DEST/klynt-protocol-staging/Cargo.toml"
grep -q 'name = "klynt-protocol"' "$DEST/klynt-protocol-staging/Cargo.toml"
grep -q 'klynt_protocol::Submission' "$DEST/klynt-protocol-staging/src/op.rs"
! grep -q 'codex_protocol' "$DEST/klynt-protocol-staging/src/op.rs"
echo OK
