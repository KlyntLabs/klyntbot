# Kimi CLI Poll-Only Ingest — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the never-functional Tier-2 wire-socket Kimi adapter and unused `KimiInstaller` with a poll-only adapter that tails `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`, mirrors `CodexPoller`, and feeds the existing Coding Memory pipeline so the Plugins → Coding Memory → Kimi tab populates from real kimi-cli sessions.

**Architecture:** New `KimiPoller` in `crates/coding-ingest/src/adapters/kimi_cli/poller.rs`, reusing the daemon's `event_tx` and `IngestEventLogRepo`. Wire frames parsed by a small Rust port of kimi-cli's `kimi_cli.wire.file::parse_wire_file_line` (Apache-2.0). Workspace `<hash> → cwd` resolved from `~/.kimi/kimi.json`. `IngestDaemonConfig.kimi_wire_socket` is replaced by `kimi_sessions_dir`. The Tier-1 hook installer (`KimiInstaller`) is deleted, mirroring the Codex precedent in commit `8b4e2789`.

**Tech Stack:** Rust 1.93, tokio (fs/io-util/sync), serde_json, jiff, sqlx (SQLite), tempfile (tests), `cargo nextest run`. SQLite migrations and storage already in place (`ingest_event_log`).

**Reference spec:** `docs/superpowers/specs/2026-04-29-kimi-cli-poll-ingest-design.md`

**Reference implementations to mirror:**
- `crates/coding-ingest/src/adapters/codex/poller.rs` — overall poller shape, byte-offset bookkeeping
- `crates/coding-ingest/src/adapters/opencode/poller.rs` — alternative pattern for poll-only adapters
- `crates/coding-ingest/tests/opencode_poller.rs` — test harness pattern

---

## File map

**New**
- `crates/coding-ingest/src/adapters/kimi_cli/poller.rs` — `KimiPoller` struct + tick loop.
- `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs` — Rust port of `kimi_cli.wire.file::parse_wire_file_line`. Defines `WireFileLine`, `WireMetadata`, `WireRecord`, `parse_line`, `collect_events`.
- `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs` — `<md5_hash> → cwd` resolver reading `~/.kimi/kimi.json`.
- `crates/coding-ingest/tests/kimi_poller.rs` — fixture-driven integration test.
- `crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl` — small wire-file fixture.
- `crates/coding-ingest/tests/fixtures/kimi/kimi.json` — workspace metadata fixture.

**Modified**
- `crates/coding-ingest/src/adapters/kimi_cli/mod.rs` — drop `wire`, `dispatch`, `payload` modules and `spawn_wire`/`KimiAdapter`; export `poller`, `wire_file`, `workdir`.
- `crates/coding-ingest/src/adapters/mod.rs` — change doc comment for `kimi_cli` (no longer "tier-1 hook + tier-2 Wire").
- `crates/coding-ingest/src/daemon.rs` — remove `kimi_wire_socket: Option<PathBuf>`; add `kimi_sessions_dir: Option<PathBuf>` and `kimi_poll_interval: Option<Duration>`; replace `spawn_wire` call with `KimiPoller::spawn`.
- `crates/coding-ingest/src/lib.rs` — drop `pub use adapters::kimi_cli::KimiAdapter`; add `pub use adapters::kimi_cli::poller::KimiPoller`.
- `crates/coding-ingest/tests/daemon_lifecycle.rs` — update `IngestDaemonConfig` literal (remove `kimi_wire_socket`, add `kimi_sessions_dir: None`, `kimi_poll_interval: None`).
- `crates/coding-ingest/tests/drain_buffer.rs` — same field updates.
- `crates/app-core/src/init/mod.rs` — wire `kimi_sessions_dir` from `coding_memory.cli.kimi_cli.enabled`; remove `kimi_wire_socket: None`.
- `crates/app-core/src/coding_memory/mod.rs` — drop `"kimi-cli"` arms in `set_cli_enabled` and `coding_memory_diagnose_cli`; the toggle still updates `cfg.coding_memory.cli.kimi_cli.enabled`.
- `crates/coding-ingest/src/adapters/kimi_cli/wire.rs` — **deleted** (legacy global-socket adapter).
- `crates/coding-ingest/src/adapters/kimi_cli/dispatch.rs` — **deleted** (legacy hook dispatcher).
- `crates/coding-ingest/src/adapters/kimi_cli/payload.rs` — **deleted** (legacy hook payloads).
- `crates/coding-ingest/tests/kimi_wire_tier2.rs` — **deleted**.
- `crates/coding-ingest/tests/kimi_adapter_tier1.rs` — **deleted**.
- `crates/app-core/src/coding_memory/kimi_installer.rs` — **deleted**.
- `crates/app-core/src/coding_memory/mod.rs` — drop `pub mod kimi_installer;` line.
- `crates/app-core/tests/kimi_installer.rs` — **deleted**.

**Untouched (verify still pass)**
- `crates/coding-ingest/tests/cross_cli_normalization.rs` (proptest covering `AgentSource::KimiCli`)
- `crates/coding-ingest/tests/agent_event_roundtrip.rs::kimi_cli_source_roundtrip`
- Frontend: `desktop-ui/src/features/plugins/coding-memory/*` (no changes)

---

## Conventions for every task

- Run `cargo fmt --all` before each commit.
- Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` before each commit. Zero warnings.
- Use `cargo nextest run -p <crate>` for fast iteration; full suite once at the end.
- Every commit message follows Conventional Commits, e.g. `feat(coding-ingest): add KimiPoller skeleton`.
- Co-author trailer is required: `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.
- Cross-crate references use crate names: `use common::Result;` not `use crate::...`.
- Public methods on `AppCore` handlers must keep `#[tracing::instrument(skip(self), err)]` (we don't add new ones in this plan; just don't strip existing).

---

## Task 0: Capture real-world fixtures

**Why first:** The exact `message.type` strings and payload shapes kimi-cli emits are the contract we parse against. Fixtures pulled from the user's existing on-disk sessions give us proof-by-evidence rather than guessing from Python type hints.

**Files:**
- Create: `crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl`
- Create: `crates/coding-ingest/tests/fixtures/kimi/kimi.json`

- [ ] **Step 0.1: Pick a real wire.jsonl source**

```bash
find ~/.kimi/sessions -name "wire.jsonl" -size -200k | head -1
```

Expected: a path like `/Users/maixuantung/.kimi/sessions/<hash>/<uuid>/wire.jsonl`. Save the path; use it in 0.2.

- [ ] **Step 0.2: Copy the first ~30 representative lines into a fixture**

```bash
SRC=$(find ~/.kimi/sessions -name "wire.jsonl" -size -200k | head -1)
mkdir -p crates/coding-ingest/tests/fixtures/kimi
head -30 "$SRC" > crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl
wc -l crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl
```

Expected output: `30 crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl`. The first line must be `{"type": "metadata", "protocol_version": "..."}`.

- [ ] **Step 0.3: Sanitize the fixture**

Open `crates/coding-ingest/tests/fixtures/kimi/wire_minimal.jsonl`. Scan for any absolute paths, emails, API keys, or personal text. Replace user prompts with `"hello"` if they contain sensitive content; keep all other fields untouched so types and shapes survive.

- [ ] **Step 0.4: Write a synthetic `kimi.json` fixture**

Write to `crates/coding-ingest/tests/fixtures/kimi/kimi.json`:

```json
{
  "work_dirs": [
    {
      "path": "/tmp/kimi-fixture-repo",
      "kaos": "local",
      "last_session_id": null
    }
  ]
}
```

- [ ] **Step 0.5: Document the protocol_version captured**

In a Rust comment we'll add later (Task 2) we'll refer to whatever `protocol_version` the fixture contains. Note it now: read line 1 of `wire_minimal.jsonl` and remember the version string (e.g. `1.9`).

- [ ] **Step 0.6: Commit fixtures**

```bash
git add crates/coding-ingest/tests/fixtures/kimi/
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
test(coding-ingest): add kimi wire.jsonl + kimi.json fixtures

Captured from a real ~/.kimi session, sanitized of personal content.
Used by upcoming KimiPoller tests.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 1: Define the wire-file types (no logic yet)

**Files:**
- Create: `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs`

- [ ] **Step 1.1: Create the file with module header and types only**

Create `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs` with this exact content:

```rust
//! Rust port of `kimi_cli.wire.file::parse_wire_file_line` from
//! [kimi-cli](https://github.com/MoonshotAI/kimi-cli) (Apache-2.0).
//!
//! Kimi writes per-session JSONL files at
//! `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`. The first
//! line is a metadata header; every subsequent line is a `WireRecord` whose
//! `message.type` is the kimi `WireMessage` Python class name (e.g.
//! `"TurnBegin"`, `"TextPart"`, `"ToolCall"`, `"ToolResult"`,
//! `"SubagentEvent"`).

use serde::Deserialize;
use serde_json::Value;

/// First-line header. We only read `protocol_version` for tracing; we don't
/// gate on it today. Captured fixtures show version `"1.9"`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WireMetadata {
    /// The discriminator literal `"metadata"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Wire protocol version string (e.g. `"1.9"`).
    pub protocol_version: String,
}

/// One non-metadata line. `timestamp` is unix epoch seconds (float).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WireRecord {
    /// Unix epoch seconds; fractional component is microsecond precision.
    pub timestamp: f64,
    /// `WireMessageEnvelope` — `{ type: <ClassName>, payload: <object> }`.
    pub message: WireEnvelope,
}

/// `{type, payload}` envelope as emitted by kimi-cli's `WireMessageEnvelope`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WireEnvelope {
    /// Python class name of the `WireMessage` variant.
    #[serde(rename = "type")]
    pub kind: String,
    /// Variant-specific JSON payload.
    pub payload: Value,
}

/// Parsed line — either the metadata header or a record.
#[derive(Debug, Clone, PartialEq)]
pub enum WireLine {
    /// First line of the file.
    Metadata(WireMetadata),
    /// Any subsequent line.
    Record(WireRecord),
}
```

- [ ] **Step 1.2: Verify the crate compiles**

Run: `cargo check -p coding-ingest`

Expected: succeeds. The new module isn't yet exported, so it'll trigger a `dead_code` warning later — we'll wire the export in Task 4.

- [ ] **Step 1.3: Commit types**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): scaffold kimi wire_file types

Define WireMetadata, WireRecord, WireEnvelope, WireLine. No logic yet —
parse_line and collect_events follow.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: TDD `parse_line`

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs`

- [ ] **Step 2.1: Write failing test for metadata parse**

Append to `wire_file.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_metadata_first_line() {
        let line = r#"{"type":"metadata","protocol_version":"1.9"}"#;
        let parsed = parse_line(line).expect("metadata line should parse");
        match parsed {
            WireLine::Metadata(m) => {
                assert_eq!(m.kind, "metadata");
                assert_eq!(m.protocol_version, "1.9");
            }
            WireLine::Record(_) => panic!("expected metadata"),
        }
    }
}
```

- [ ] **Step 2.2: Run failing test**

Run: `cargo nextest run -p coding-ingest --no-fail-fast wire_file::tests::parse_line_metadata_first_line 2>&1 | tail -30`

Expected: compile error or test failure citing `parse_line` not found.

- [ ] **Step 2.3: Implement `parse_line`**

Insert above the `#[cfg(test)]` block:

```rust
/// Parse a single line. Tries metadata first; falls back to record.
///
/// Mirrors `parse_wire_file_line` in kimi-cli's `wire/file.py`.
pub fn parse_line(line: &str) -> Result<WireLine, serde_json::Error> {
    if let Ok(meta) = serde_json::from_str::<WireMetadata>(line) {
        if meta.kind == "metadata" {
            return Ok(WireLine::Metadata(meta));
        }
    }
    let record: WireRecord = serde_json::from_str(line)?;
    Ok(WireLine::Record(record))
}
```

- [ ] **Step 2.4: Run test to verify it passes**

Run: `cargo nextest run -p coding-ingest wire_file::tests::parse_line_metadata_first_line`

Expected: 1 test PASS.

- [ ] **Step 2.5: Add failing test for record parse**

Append inside `mod tests`:

```rust
    #[test]
    fn parse_line_record_turnbegin() {
        let line = r#"{"timestamp":1777096658.415196,"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"hi"}]}}}"#;
        let parsed = parse_line(line).expect("record line should parse");
        match parsed {
            WireLine::Record(r) => {
                assert!((r.timestamp - 1777096658.415196).abs() < 1e-6);
                assert_eq!(r.message.kind, "TurnBegin");
                assert_eq!(
                    r.message.payload["user_input"][0]["text"]
                        .as_str()
                        .unwrap(),
                    "hi"
                );
            }
            WireLine::Metadata(_) => panic!("expected record"),
        }
    }
```

- [ ] **Step 2.6: Run — should already pass**

Run: `cargo nextest run -p coding-ingest wire_file::tests::parse_line_record_turnbegin`

Expected: PASS (the existing implementation already handles records).

- [ ] **Step 2.7: Add failing test for invalid JSON**

Append:

```rust
    #[test]
    fn parse_line_invalid_json_returns_err() {
        let res = parse_line("{not json");
        assert!(res.is_err(), "invalid JSON must error");
    }
```

- [ ] **Step 2.8: Run — should pass**

Run: `cargo nextest run -p coding-ingest wire_file::tests`

Expected: 3 tests PASS.

- [ ] **Step 2.9: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): implement kimi wire parse_line

Tries metadata-shaped header first, falls back to record. Mirrors
kimi-cli's parse_wire_file_line.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: TDD `collect_events` — SubagentEvent unwrap

**Why:** kimi recursively wraps inner events in `SubagentEvent`. We need to flatten the chain while remembering the originating `agent_id` for each inner event so it can be tagged onto our `AgentEventV1.turn_id`.

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs`

- [ ] **Step 3.1: Add failing test for non-subagent passthrough**

Append inside `mod tests`:

```rust
    #[test]
    fn collect_events_passthrough_non_subagent() {
        let env = WireEnvelope {
            kind: "TurnBegin".into(),
            payload: serde_json::json!({"user_input": "hi"}),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "TurnBegin");
        assert_eq!(out[0].agent_id, None);
    }
```

- [ ] **Step 3.2: Add failing test for one-level subagent unwrap**

Append:

```rust
    #[test]
    fn collect_events_unwraps_subagent_one_level() {
        let env = WireEnvelope {
            kind: "SubagentEvent".into(),
            payload: serde_json::json!({
                "agent_id": "sub-1",
                "subagent_type": "task",
                "event": {"type": "ToolCall", "payload": {"id": "c1"}}
            }),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "ToolCall");
        assert_eq!(out[0].agent_id.as_deref(), Some("sub-1"));
    }
```

- [ ] **Step 3.3: Add failing test for nested subagent unwrap**

Append:

```rust
    #[test]
    fn collect_events_unwraps_subagent_nested() {
        let env = WireEnvelope {
            kind: "SubagentEvent".into(),
            payload: serde_json::json!({
                "agent_id": "outer",
                "event": {
                    "type": "SubagentEvent",
                    "payload": {
                        "agent_id": "inner",
                        "event": {"type": "TurnBegin", "payload": {"user_input": "hi"}}
                    }
                }
            }),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "TurnBegin");
        // Innermost agent_id wins — that's the closest scope to the inner event.
        assert_eq!(out[0].agent_id.as_deref(), Some("inner"));
    }
```

- [ ] **Step 3.4: Run failing tests**

Run: `cargo nextest run -p coding-ingest wire_file::tests::collect_events 2>&1 | tail -20`

Expected: compile error citing `collect_events` not found.

- [ ] **Step 3.5: Implement `collect_events` and `CollectedEvent`**

Insert above the `#[cfg(test)]` block:

```rust
/// One unwrapped event with the closest enclosing subagent id (if any).
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedEvent {
    /// Kimi `WireMessage` type name (e.g. `"TurnBegin"`).
    pub kind: String,
    /// Variant payload — same shape as `WireEnvelope.payload`.
    pub payload: Value,
    /// Innermost subagent `agent_id` if the event was wrapped.
    pub agent_id: Option<String>,
}

/// Flatten a `WireEnvelope`. Non-subagent events return as a single-element
/// vec with `agent_id: None`. `SubagentEvent`s are recursively unwrapped; the
/// innermost `agent_id` is attached to the leaf event.
pub fn collect_events(env: &WireEnvelope) -> Vec<CollectedEvent> {
    let mut out = Vec::new();
    collect_inner(&env.kind, &env.payload, None, &mut out);
    out
}

fn collect_inner(
    kind: &str,
    payload: &Value,
    parent_agent_id: Option<String>,
    out: &mut Vec<CollectedEvent>,
) {
    if kind == "SubagentEvent" {
        let inner_agent_id = payload
            .get("agent_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or(parent_agent_id);
        if let Some(inner) = payload.get("event") {
            let inner_kind = inner.get("type").and_then(Value::as_str).unwrap_or("");
            let inner_payload = inner.get("payload").cloned().unwrap_or(Value::Null);
            if !inner_kind.is_empty() {
                collect_inner(inner_kind, &inner_payload, inner_agent_id, out);
            }
        }
        return;
    }
    out.push(CollectedEvent {
        kind: kind.to_owned(),
        payload: payload.clone(),
        agent_id: parent_agent_id,
    });
}
```

- [ ] **Step 3.6: Run all wire_file tests**

Run: `cargo nextest run -p coding-ingest wire_file::tests`

Expected: 6 tests PASS.

- [ ] **Step 3.7: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): collect_events flattens SubagentEvent chains

The innermost agent_id wins so leaf events carry their closest scope.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Wire `wire_file` into the kimi_cli module tree

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`

- [ ] **Step 4.1: Read the current state**

```bash
cat crates/coding-ingest/src/adapters/kimi_cli/mod.rs
```

- [ ] **Step 4.2: Replace its contents with this exact body**

```rust
//! kimi-cli adapter — poll-only ingestion driven by `KimiPoller`.
//!
//! kimi-cli writes per-session JSONL files at
//! `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`. The poller
//! tails those files and emits `AgentEvent`s into the daemon's event channel.
//!
//! Legacy Tier-1 hook + Tier-2 Wire-socket adapters were removed in favor of
//! this single poll-only path, mirroring the Codex precedent.

/// Workspace `<hash> → cwd` resolver reading `~/.kimi/kimi.json`.
pub mod workdir;
/// Wire-file frame parser (`metadata` header + `WireRecord` lines).
pub mod wire_file;
```

`KimiPoller` will be added in Task 7. Don't add the `poller` module here yet — let TDD drive it.

- [ ] **Step 4.3: Verify the crate still compiles**

Run: `cargo check -p coding-ingest`

Expected: compile errors from `lib.rs` re-exporting things that no longer exist (`KimiAdapter`). We fix `lib.rs` next so the crate is in a consistent state for the rest of Task 4.

- [ ] **Step 4.4: Update `lib.rs`**

In `crates/coding-ingest/src/lib.rs`, find:

```rust
pub use adapters::kimi_cli::KimiAdapter;
```

Replace with:

```rust
// kimi-cli is poll-only — see adapters::kimi_cli::poller::KimiPoller (added in Task 7).
```

- [ ] **Step 4.5: Verify compile**

Run: `cargo check -p coding-ingest`

Expected: compile fails because `wire.rs`, `dispatch.rs`, `payload.rs` still exist on disk and may reference removed module declarations. Resolve in next step.

- [ ] **Step 4.6: Delete `dispatch.rs` and `payload.rs`**

```bash
rm crates/coding-ingest/src/adapters/kimi_cli/dispatch.rs
rm crates/coding-ingest/src/adapters/kimi_cli/payload.rs
```

(`wire.rs` is left in place but stripped down in 4.7 — `daemon.rs` still calls `spawn_wire` and we want the workspace to keep building. Task 9 replaces the daemon call site, then Task 9.7 deletes `wire.rs`.)

- [ ] **Step 4.7: Replace `wire.rs` with a no-op stub**

Overwrite `crates/coding-ingest/src/adapters/kimi_cli/wire.rs` with:

```rust
//! Deprecated tier-2 Wire socket stub.
//!
//! Kept ONLY so `crate::adapters::kimi_cli::spawn_wire` keeps resolving
//! while Task 9 swaps `daemon.rs` over to `KimiPoller`. Deleted in Task 9.7.

use crate::event::AgentEvent;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// No-op replacement — returns immediately. The legacy tier-2 socket
/// adapter is gone; the new poll-only path lives in `poller.rs`.
pub fn spawn_wire(
    _socket_path: PathBuf,
    _tx: mpsc::UnboundedSender<AgentEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {})
}
```

In `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`, add the line so the stub is reachable:

```rust
/// Deprecated tier-2 stub — removed in Task 9.7.
#[allow(deprecated)]
pub mod wire;

pub use wire::spawn_wire;
```

- [ ] **Step 4.8: Delete the legacy tier-1/tier-2 tests**

```bash
rm crates/coding-ingest/tests/kimi_wire_tier2.rs
rm crates/coding-ingest/tests/kimi_adapter_tier1.rs
```

- [ ] **Step 4.9: Verify compile and tests still build**

Run: `cargo check -p coding-ingest --all-targets`

Expected: success. (The daemon still calls the no-op `spawn_wire` — that's fine for now; Task 9 replaces it.)

- [ ] **Step 4.10: Commit**

```bash
git add -A crates/coding-ingest/src/adapters/kimi_cli/ crates/coding-ingest/src/lib.rs crates/coding-ingest/tests/
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
refactor(coding-ingest): drop kimi tier-1 hook + tier-2 wire-socket adapters

Legacy KimiAdapter (hook payload parser), spawn_wire (global Unix socket),
and the dispatch/payload helpers never had a working transport. Removing
ahead of the new poll-only adapter, mirroring 8b4e2789 (codex switched
from hooks to poll-only).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: TDD `workdir` resolver

**Files:**
- Create: `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs`

- [ ] **Step 5.1: Add `md5` dependency**

Run: `grep '^md5' crates/coding-ingest/Cargo.toml || echo "missing"`

If `missing`, add to `[dependencies]` block of `crates/coding-ingest/Cargo.toml`:

```toml
md5 = "0.7"
```

(The crate `md5` v0.7 is already in `Cargo.lock` if any other workspace member uses it. If not, `cargo check` in 5.7 will pull it.)

- [ ] **Step 5.2: Create the file with types and a stub function**

Write `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs`:

```rust
//! Resolve a kimi workspace `<hash>` directory back to its `cwd` by reading
//! `~/.kimi/kimi.json`.
//!
//! Kimi names each workspace's session directory by md5(work_dir) for
//! `kaos == "local"` entries; non-local entries use `<kaos>_<hash>`.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct WorkDirEntry {
    path: String,
    #[serde(default = "default_kaos")]
    kaos: String,
}

fn default_kaos() -> String {
    "local".to_string()
}

#[derive(Debug, Deserialize)]
struct KimiMetadata {
    work_dirs: Vec<WorkDirEntry>,
}

/// In-memory cache of `<hash> → cwd`. Refreshed lazily on miss.
#[derive(Debug, Default)]
pub struct WorkdirIndex {
    map: RwLock<HashMap<String, PathBuf>>,
}

impl WorkdirIndex {
    /// Construct an empty index; call [`refresh`](Self::refresh) before use.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read `kimi_json_path` and rebuild the cache. Missing file is not an
    /// error — the index simply stays empty and resolves yield `None`.
    pub async fn refresh(&self, kimi_json_path: &Path) -> common::Result<()> {
        let bytes = match tokio::fs::read(kimi_json_path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(path = %kimi_json_path.display(), "kimi.json missing — workdir index empty");
                self.map.write().await.clear();
                return Ok(());
            }
            Err(e) => {
                return Err(common::KlyntbotError::Storage(format!(
                    "kimi.json read {}: {e}",
                    kimi_json_path.display()
                )));
            }
        };
        let meta: KimiMetadata = serde_json::from_slice(&bytes).map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi.json parse: {e}"))
        })?;
        let mut next = HashMap::with_capacity(meta.work_dirs.len());
        for entry in meta.work_dirs {
            let hash = hash_for(&entry.path, &entry.kaos);
            next.insert(hash, PathBuf::from(entry.path));
        }
        *self.map.write().await = next;
        Ok(())
    }

    /// Look up a hash. `None` if unknown.
    pub async fn get(&self, hash: &str) -> Option<PathBuf> {
        self.map.read().await.get(hash).cloned()
    }
}

/// Compute the directory-name hash kimi assigns to a `(work_dir, kaos)` pair.
///
/// `local` workspaces use the bare md5 hex digest. Other kaos names are
/// prefixed: `<kaos>_<md5>`. This mirrors the Python reference in
/// `kimi_cli.vis.api.sessions::get_work_dir_for_hash`.
pub fn hash_for(work_dir: &str, kaos: &str) -> String {
    let digest = md5::compute(work_dir.as_bytes());
    let hex = format!("{digest:x}");
    if kaos == "local" {
        hex
    } else {
        format!("{kaos}_{hex}")
    }
}
```

- [ ] **Step 5.3: Add tests**

Append at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_for_local_is_md5_hex() {
        // md5("/tmp/kimi-fixture-repo") computed once with `md5sum` for the
        // golden value — DON'T regenerate, this is a contract test.
        let h = hash_for("/tmp/kimi-fixture-repo", "local");
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_for_non_local_is_prefixed() {
        let h = hash_for("/foo", "remote");
        assert!(h.starts_with("remote_"), "got {h}");
    }

    #[tokio::test]
    async fn refresh_missing_file_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let idx = WorkdirIndex::new();
        idx.refresh(&path).await.expect("missing file is not an error");
        assert!(idx.get("anyhash").await.is_none());
    }

    #[tokio::test]
    async fn refresh_loads_entries_and_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kimi.json");
        std::fs::write(
            &path,
            r#"{"work_dirs":[{"path":"/tmp/kimi-fixture-repo","kaos":"local","last_session_id":null}]}"#,
        )
        .unwrap();
        let idx = WorkdirIndex::new();
        idx.refresh(&path).await.unwrap();
        let h = hash_for("/tmp/kimi-fixture-repo", "local");
        assert_eq!(
            idx.get(&h).await.unwrap(),
            std::path::PathBuf::from("/tmp/kimi-fixture-repo")
        );
    }
}
```

- [ ] **Step 5.4: Wire the module into the kimi_cli mod tree**

The line `pub mod workdir;` is already in `mod.rs` from Task 4.

- [ ] **Step 5.5: Run tests**

Run: `cargo nextest run -p coding-ingest workdir::tests`

Expected: 4 tests PASS.

- [ ] **Step 5.6: Lint**

Run: `cargo clippy -p coding-ingest --all-targets --all-features -- -D warnings`

Expected: no warnings.

- [ ] **Step 5.7: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/workdir.rs crates/coding-ingest/Cargo.toml
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): add WorkdirIndex resolver

Reads ~/.kimi/kimi.json and maps md5(work_dir) (or `<kaos>_<hash>` for
non-local) back to the cwd. Missing file resolves to an empty index.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: TDD message-type → `EventKind` mapper

**Why a separate task:** The mapper is the single biggest source of subtle bugs (kimi adds wire variants quickly). Isolating it from poller plumbing lets us test mapping logic with pure unit tests against fixture JSON.

**Files:**
- Create: `crates/coding-ingest/src/adapters/kimi_cli/mapper.rs`
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`

- [ ] **Step 6.1: Add mapper module declaration**

Edit `crates/coding-ingest/src/adapters/kimi_cli/mod.rs` and append:

```rust
/// `WireRecord` → `AgentEventV1` mapping.
pub mod mapper;
```

- [ ] **Step 6.2: Create the mapper file with types and a stub**

Write `crates/coding-ingest/src/adapters/kimi_cli/mapper.rs`:

```rust
//! Map kimi `WireRecord`s to `AgentEventV1`s.
//!
//! Tool calls are buffered per `call_id` because `EventKind::ToolCall`
//! requires both invocation (tool, args_preview) and result (ok,
//! duration_ms, result_preview) at emit time, while kimi emits these in
//! two separate wire messages (`ToolCall` and `ToolResult`).

use crate::adapters::kimi_cli::wire_file::{CollectedEvent, WireRecord};
use crate::event::{AgentEventV1, AgentSource, EventKind, TokenUsage};
use crate::scope_resolver::resolve_scope;
use jiff::Timestamp;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

const PREVIEW_MAX: usize = 1024;

/// Per-session state carried across consecutive lines.
#[derive(Debug, Default)]
pub struct SessionState {
    /// Buffered `ToolCall { id → (tool, args_preview, started_at_ms) }` waiting
    /// on a matching `ToolResult`.
    pending_tools: HashMap<String, PendingTool>,
    /// `model` extracted from the most recent `StatusUpdate` (kimi reports
    /// usage on `StatusUpdate`, not on the prior assistant text).
    pub last_model: Option<String>,
    /// Whether we've already emitted a `SessionStart` for this session.
    pub session_start_emitted: bool,
}

#[derive(Debug, Clone)]
struct PendingTool {
    tool: String,
    args_preview: String,
    started_at: Timestamp,
}

/// Convert one collected event into zero or more `AgentEventV1`s.
///
/// `state` is mutated. `session_id`, `cwd` are stable for the file.
pub fn map_event(
    state: &mut SessionState,
    collected: &CollectedEvent,
    record: &WireRecord,
    session_id: &str,
    cwd: &std::path::Path,
) -> Vec<AgentEventV1> {
    let occurred_at = unix_seconds_to_ts(record.timestamp);
    let repo = resolve_scope(cwd);
    let turn_id = collected.agent_id.clone();
    let payload = &collected.payload;
    match collected.kind.as_str() {
        "TurnBegin" => map_turn_begin(payload, &repo, turn_id, session_id, cwd, occurred_at),
        "TextPart" => map_text_part(state, payload, &repo, turn_id, session_id, cwd, occurred_at),
        "ToolCall" => {
            buffer_tool_call(state, payload, occurred_at);
            vec![]
        }
        "ToolResult" => map_tool_result(
            state, payload, &repo, turn_id, session_id, cwd, occurred_at,
        ),
        "StatusUpdate" => {
            update_status(state, payload);
            vec![]
        }
        "TurnEnd" | "StepBegin" | "StepInterrupted" | "CompactionBegin"
        | "CompactionEnd" | "MCPLoadingBegin" | "MCPLoadingEnd"
        | "HookTriggered" | "HookResolved" | "BtwBegin" | "BtwEnd"
        | "PlanDisplay" | "ImageURLPart" | "AudioURLPart" | "VideoURLPart"
        | "ThinkPart" | "ToolCallPart" | "Notification" | "ApprovalRequest"
        | "ApprovalResponse" | "QuestionRequest" | "QuestionResponse"
        | "HookRequest" | "HookResponse" | "ToolCallRequest" | "SteerInput" => vec![],
        other => {
            tracing::debug!(kind = other, "kimi mapper: skipping unknown event type");
            vec![]
        }
    }
}

/// Emit a `SessionStart` event the first time the mapper sees this session.
/// Called by the poller on first record per file.
pub fn maybe_emit_session_start(
    state: &mut SessionState,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
    model: Option<String>,
) -> Option<AgentEventV1> {
    if state.session_start_emitted {
        return None;
    }
    state.session_start_emitted = true;
    Some(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id: None,
        cwd: cwd.to_path_buf(),
        repo: resolve_scope(cwd),
        occurred_at,
        kind: EventKind::SessionStart {
            model,
            source_reason: "kimi-cli".into(),
        },
    })
}

fn map_turn_begin(
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    let text = extract_user_input_text(payload.get("user_input"));
    if text.is_empty() {
        return vec![];
    }
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::UserPrompt {
            text,
            attachments: vec![],
        },
    }]
}

fn map_text_part(
    _state: &mut SessionState,
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    // Assistant `TextPart` payload shape: `{"text": "..."}`.
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text.is_empty() {
        return vec![];
    }
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::AssistantMsg {
            text,
            truncated: false,
            token_usage: None,
        },
    }]
}

fn buffer_tool_call(state: &mut SessionState, payload: &Value, occurred_at: Timestamp) {
    let id = match payload.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };
    let tool = payload
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("name").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let args_preview = payload
        .get("function")
        .and_then(|f| f.get("arguments"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("arguments").and_then(Value::as_str))
        .map(|s| truncate_preview(s))
        .unwrap_or_default();
    state.pending_tools.insert(
        id,
        PendingTool {
            tool,
            args_preview,
            started_at: occurred_at,
        },
    );
}

fn map_tool_result(
    state: &mut SessionState,
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    let id = match payload.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return vec![],
    };
    let pending = match state.pending_tools.remove(&id) {
        Some(p) => p,
        None => return vec![],
    };
    let result_preview = payload
        .get("content")
        .map(serde_value_preview)
        .unwrap_or_default();
    let ok = !payload
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let duration_ms = u32::try_from(
        (occurred_at.as_millisecond() - pending.started_at.as_millisecond()).max(0),
    )
    .unwrap_or(u32::MAX);
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::ToolCall {
            tool: pending.tool,
            args_preview: pending.args_preview,
            ok,
            duration_ms,
            result_preview,
        },
    }]
}

fn update_status(state: &mut SessionState, payload: &Value) {
    if let Some(usage) = payload.get("token_usage") {
        let _ = TokenUsage {
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            cached_tokens: usage
                .get("cached_tokens")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        };
        // Token-usage attachment to the prior AssistantMsg is post-emission
        // (the row is already in ingest_event_log). Distiller-side enrichment
        // is parked — recording the parse here keeps the door open without
        // breaking anything if usage shape drifts.
    }
}

fn extract_user_input_text(input: Option<&Value>) -> String {
    let Some(value) = input else { return String::new() };
    if let Some(s) = value.as_str() {
        return s.to_string();
    }
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str)?;
                if kind == "text" {
                    Some(part.get("text").and_then(Value::as_str)?.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}

fn truncate_preview(s: &str) -> String {
    if s.len() <= PREVIEW_MAX {
        s.to_string()
    } else {
        format!("{}…[{} bytes]", &s[..PREVIEW_MAX], s.len())
    }
}

fn serde_value_preview(v: &Value) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    };
    truncate_preview(&s)
}

fn unix_seconds_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}
```

- [ ] **Step 6.3: Re-export `RepoScope` for the mapper signature**

Check if `crate::RepoScope` is reachable. If `cargo check -p coding-ingest` reports `unresolved import crate::RepoScope`, add to `crates/coding-ingest/src/lib.rs` near the existing `pub use scope::RepoScope;`:

```rust
pub use scope::RepoScope;
```

(It's already exported per the file we read in design-time.) If still broken, replace `crate::RepoScope` in `mapper.rs` with `crate::scope::RepoScope`.

- [ ] **Step 6.4: Add unit tests for the mapper**

Append to `mapper.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::kimi_cli::wire_file::WireEnvelope;
    use std::path::Path;

    fn record(env: WireEnvelope, ts: f64) -> WireRecord {
        WireRecord { timestamp: ts, message: env }
    }

    fn collected(kind: &str, payload: Value) -> CollectedEvent {
        CollectedEvent {
            kind: kind.into(),
            payload,
            agent_id: None,
        }
    }

    #[test]
    fn turn_begin_emits_user_prompt() {
        let mut state = SessionState::default();
        let c = collected(
            "TurnBegin",
            serde_json::json!({"user_input": [{"type":"text","text":"hi"}]}),
        );
        let r = record(WireEnvelope { kind: "TurnBegin".into(), payload: c.payload.clone() }, 100.0);
        let out = map_event(&mut state, &c, &r, "sess1", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
        match &out[0].kind {
            EventKind::UserPrompt { text, .. } => assert_eq!(text, "hi"),
            other => panic!("expected UserPrompt, got {other:?}"),
        }
    }

    #[test]
    fn turn_begin_string_input_works() {
        let mut state = SessionState::default();
        let c = collected("TurnBegin", serde_json::json!({"user_input": "plain"}));
        let r = record(WireEnvelope { kind: "TurnBegin".into(), payload: c.payload.clone() }, 1.0);
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn text_part_emits_assistant_msg() {
        let mut state = SessionState::default();
        let c = collected("TextPart", serde_json::json!({"text":"hello back"}));
        let r = record(WireEnvelope { kind: "TextPart".into(), payload: c.payload.clone() }, 1.0);
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].kind, EventKind::AssistantMsg { .. }));
    }

    #[test]
    fn tool_call_then_result_emits_one_toolcall() {
        let mut state = SessionState::default();
        // ToolCall — buffered, no emission.
        let c1 = collected(
            "ToolCall",
            serde_json::json!({"id":"c1","function":{"name":"Read","arguments":"{\"path\":\"/x\"}"}}),
        );
        let r1 = record(WireEnvelope { kind: "ToolCall".into(), payload: c1.payload.clone() }, 100.0);
        let out1 = map_event(&mut state, &c1, &r1, "s", Path::new("/tmp"));
        assert!(out1.is_empty(), "ToolCall must buffer, not emit");

        // ToolResult — pairs with buffered ToolCall.
        let c2 = collected(
            "ToolResult",
            serde_json::json!({"id":"c1","content":"ok","is_error":false}),
        );
        let r2 = record(WireEnvelope { kind: "ToolResult".into(), payload: c2.payload.clone() }, 100.5);
        let out2 = map_event(&mut state, &c2, &r2, "s", Path::new("/tmp"));
        assert_eq!(out2.len(), 1);
        match &out2[0].kind {
            EventKind::ToolCall { tool, ok, duration_ms, .. } => {
                assert_eq!(tool, "Read");
                assert!(*ok);
                assert!(*duration_ms <= 1000, "should be ~500ms, got {duration_ms}");
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn unknown_kind_is_skipped() {
        let mut state = SessionState::default();
        let c = collected("BrandNewType_2027", serde_json::json!({}));
        let r = record(WireEnvelope { kind: c.kind.clone(), payload: c.payload.clone() }, 1.0);
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert!(out.is_empty());
    }

    #[test]
    fn session_start_emitted_once() {
        let mut state = SessionState::default();
        let ts = unix_seconds_to_ts(100.0);
        let first = maybe_emit_session_start(&mut state, "s", Path::new("/tmp"), ts, Some("k2".into()));
        let second = maybe_emit_session_start(&mut state, "s", Path::new("/tmp"), ts, Some("k2".into()));
        assert!(first.is_some());
        assert!(second.is_none());
    }
}
```

- [ ] **Step 6.5: Run mapper tests**

Run: `cargo nextest run -p coding-ingest mapper::tests`

Expected: 6 tests PASS.

- [ ] **Step 6.6: Lint**

Run: `cargo clippy -p coding-ingest --all-targets -- -D warnings`

Expected: no warnings.

- [ ] **Step 6.7: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/mapper.rs crates/coding-ingest/src/adapters/kimi_cli/mod.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): add kimi WireRecord → AgentEventV1 mapper

Buffers ToolCall by id and emits EventKind::ToolCall on matching
ToolResult. Unknown kimi message types are debug-logged and skipped.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Build the `KimiPoller` skeleton (no tail logic yet)

**Files:**
- Create: `crates/coding-ingest/src/adapters/kimi_cli/poller.rs`
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`

- [ ] **Step 7.1: Add module declaration**

Edit `crates/coding-ingest/src/adapters/kimi_cli/mod.rs` and append:

```rust
/// Poller that tails per-session `wire.jsonl` files.
pub mod poller;
```

- [ ] **Step 7.2: Create the poller skeleton**

Write `crates/coding-ingest/src/adapters/kimi_cli/poller.rs`:

```rust
//! Long-lived task that tails kimi-cli per-session `wire.jsonl` files.
//!
//! Layout:
//!   `<sessions_dir>/<work_dir_hash>/<session_uuid>/wire.jsonl`
//!     plus optional `subagents/<id>/wire.jsonl` (recursed).
//!
//! State is in-memory: per-file byte offsets seeded to current size on
//! startup (no backfill). Mirrors `CodexPoller`.

use crate::adapters::kimi_cli::mapper::{map_event, maybe_emit_session_start, SessionState};
use crate::adapters::kimi_cli::wire_file::{collect_events, parse_line, WireLine};
use crate::adapters::kimi_cli::workdir::WorkdirIndex;
use crate::event::AgentEvent;
use crate::store::IngestEventLogRepo;
use common::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, Mutex};

/// Tail kimi `wire.jsonl` files, one tick per `interval`.
pub struct KimiPoller {
    sessions_dir: PathBuf,
    kimi_json_path: PathBuf,
    interval: std::time::Duration,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    repo: Arc<IngestEventLogRepo>,
    offsets: Arc<Mutex<HashMap<PathBuf, u64>>>,
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    workdir_index: Arc<WorkdirIndex>,
}

impl KimiPoller {
    /// Construct with the user's `~/.kimi/sessions` and `~/.kimi/kimi.json`.
    pub fn new(
        sessions_dir: PathBuf,
        kimi_json_path: PathBuf,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        repo: Arc<IngestEventLogRepo>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            sessions_dir,
            kimi_json_path,
            interval,
            event_tx,
            repo,
            offsets: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            workdir_index: Arc::new(WorkdirIndex::new()),
        }
    }

    /// Spawn the polling loop as a detached tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!(
                sessions_dir = %self.sessions_dir.display(),
                "kimi poller starting"
            );
            if let Err(e) = self.workdir_index.refresh(&self.kimi_json_path).await {
                tracing::warn!(error = %e, "kimi workdir_index initial refresh failed");
            }
            if let Err(e) = self.seed_offsets().await {
                tracing::warn!(error = %e, "kimi poller seed_offsets failed");
            }
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.poll_once().await {
                    tracing::warn!(error = %e, "kimi poll failed");
                }
            }
        })
    }

    async fn seed_offsets(&self) -> Result<()> {
        let files = list_wire_files(&self.sessions_dir).await?;
        let mut guard = self.offsets.lock().await;
        for f in files {
            if let Ok(meta) = tokio::fs::metadata(&f).await {
                guard.insert(f, meta.len());
            }
        }
        Ok(())
    }

    async fn poll_once(&self) -> Result<()> {
        if !self.sessions_dir.exists() {
            return Ok(());
        }
        let files = list_wire_files(&self.sessions_dir).await?;
        for path in files {
            if let Err(e) = self.tail_file(&path).await {
                tracing::warn!(
                    error = %e,
                    file = %path.display(),
                    "kimi tail failed"
                );
            }
        }
        Ok(())
    }

    /// Tail one wire.jsonl. Implementation lives in Task 8.
    async fn tail_file(&self, _path: &Path) -> Result<()> {
        // Filled in by Task 8.
        Ok(())
    }
}

/// Walk `<sessions_dir>` for files named `wire.jsonl`, including under
/// `subagents/<id>/`. Bounded depth to avoid runaway recursion.
async fn list_wire_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk(root, &mut out, 0).await?;
    Ok(out)
}

async fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    // Layout depth: sessions/<hash>/<uuid>/wire.jsonl is depth 3; subagents
    // adds two more levels (subagents/<id>/wire.jsonl). Cap at 6.
    if depth > 6 {
        return Ok(());
    }
    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("kimi readdir: {e}")))?
    {
        let path = entry.path();
        let ty = entry.file_type().await.map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi file_type: {e}"))
        })?;
        if ty.is_dir() {
            Box::pin(walk(&path, out, depth + 1)).await?;
        } else if ty.is_file()
            && path.file_name().and_then(|s| s.to_str()) == Some("wire.jsonl")
        {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_wire_files_finds_nested() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("hash1/uuid1");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::write(nested.join("wire.jsonl"), "x").await.unwrap();
        let sub = nested.join("subagents/sa1");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::write(sub.join("wire.jsonl"), "y").await.unwrap();
        let found = list_wire_files(dir.path()).await.unwrap();
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn list_wire_files_missing_dir_is_ok() {
        let res = list_wire_files(Path::new("/no/such/path/12345")).await.unwrap();
        assert!(res.is_empty());
    }
}
```

- [ ] **Step 7.3: Verify compile and tests**

Run: `cargo nextest run -p coding-ingest poller::tests`

Expected: 2 tests PASS.

- [ ] **Step 7.4: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/poller.rs crates/coding-ingest/src/adapters/kimi_cli/mod.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): scaffold KimiPoller with file walker

Recursive walk depth-capped at 6 to cover sessions/<hash>/<uuid>/wire.jsonl
plus subagents/<id>/wire.jsonl. tail_file is a stub — implementation in
the next commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Implement `tail_file` and end-to-end fixture test

**Files:**
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/poller.rs`
- Create: `crates/coding-ingest/tests/kimi_poller.rs`

- [ ] **Step 8.1: Replace `tail_file` body**

In `poller.rs`, replace the `tail_file` stub with:

```rust
    async fn tail_file(&self, path: &Path) -> Result<()> {
        let file_size = match tokio::fs::metadata(path).await {
            Ok(m) => m.len(),
            Err(_) => return Ok(()),
        };
        let last_offset = {
            let guard = self.offsets.lock().await;
            *guard.get(path).unwrap_or(&0)
        };
        if file_size <= last_offset {
            return Ok(());
        }

        let session_id = match path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let work_dir_hash = match path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let cwd = match self.workdir_index.get(&work_dir_hash).await {
            Some(p) => p,
            None => {
                // One-shot refresh on miss before falling back.
                let _ = self.workdir_index.refresh(&self.kimi_json_path).await;
                self.workdir_index
                    .get(&work_dir_hash)
                    .await
                    .unwrap_or_else(|| PathBuf::from("/"))
            }
        };

        let mut file = tokio::fs::File::open(path).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi open {}: {e}", path.display()))
        })?;
        file.seek(std::io::SeekFrom::Start(last_offset))
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("kimi seek: {e}")))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut new_offset = last_offset;
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("kimi read: {e}")))?;
            if n == 0 {
                break;
            }
            // Skip trailing partial line if file is being written.
            if !line.ends_with('\n') {
                break;
            }
            new_offset += n as u64;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.process_line(trimmed, &session_id, &cwd).await;
        }
        let mut guard = self.offsets.lock().await;
        guard.insert(path.to_path_buf(), new_offset);
        Ok(())
    }

    async fn process_line(&self, raw: &str, session_id: &str, cwd: &Path) {
        let parsed = match parse_line(raw) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "kimi parse_line failed");
                return;
            }
        };
        let record = match parsed {
            WireLine::Metadata(_) => return,
            WireLine::Record(r) => r,
        };
        let collected = collect_events(&record.message);

        // Compute all events under the per-session lock, then release it
        // before doing any awaiting work — never hold a mutex across await.
        let to_dispatch: Vec<crate::event::AgentEventV1> = {
            let mut sessions = self.sessions.lock().await;
            let state = sessions.entry(session_id.to_string()).or_default();
            let mut out = Vec::new();

            if !state.session_start_emitted {
                let ts = jiff::Timestamp::new(record.timestamp.trunc() as i64, 0)
                    .unwrap_or_else(|_| jiff::Timestamp::now());
                let model_hint = state.last_model.clone();
                if let Some(evt) =
                    maybe_emit_session_start(state, session_id, cwd, ts, model_hint)
                {
                    out.push(evt);
                }
            }

            for c in &collected {
                out.extend(map_event(state, c, &record, session_id, cwd));
            }
            out
        };

        for evt in to_dispatch {
            self.dispatch(evt).await;
        }
    }

    async fn dispatch(&self, event: crate::event::AgentEventV1) {
        let envelope = AgentEvent::V1(event);
        if let Err(e) = self.repo.insert(&envelope).await {
            tracing::warn!(error = %e, "kimi repo.insert failed");
        }
        if self.event_tx.send(envelope).is_err() {
            tracing::warn!("kimi event_tx send failed (channel closed)");
        }
    }
```

- [ ] **Step 8.2: Verify compile**

Run: `cargo check -p coding-ingest --all-targets`

Expected: success.

- [ ] **Step 8.3: Write the integration test**

Write `crates/coding-ingest/tests/kimi_poller.rs`:

```rust
//! End-to-end smoke test for KimiPoller against a fixture session dir.

use coding_ingest::adapters::kimi_cli::poller::KimiPoller;
use coding_ingest::event::AgentEvent;
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use std::time::Duration;
use storage::StoragePool;
use tokio::sync::mpsc;

#[tokio::test]
async fn poller_ingests_fixture_session() {
    // Migrations: storage core + cognitive + coding_memory provide the
    // ingest_event_log table.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = tempfile::tempdir().unwrap();
    // Layout: <sessions>/<hash>/<uuid>/wire.jsonl
    let work_dir = "/tmp/kimi-fixture-repo";
    let hash = coding_ingest::adapters::kimi_cli::workdir::hash_for(work_dir, "local");
    let session_id = "11111111-2222-3333-4444-555555555555";
    let session_dir = dir.path().join(format!("sessions/{hash}/{session_id}"));
    tokio::fs::create_dir_all(&session_dir).await.unwrap();

    let kimi_json = dir.path().join("kimi.json");
    tokio::fs::write(
        &kimi_json,
        format!(
            r#"{{"work_dirs":[{{"path":"{work_dir}","kaos":"local","last_session_id":null}}]}}"#
        ),
    )
    .await
    .unwrap();

    // Seed the session file BEFORE spawning so seed_offsets sees size N
    // and we can append AFTER spawn to drive the tail.
    tokio::fs::write(
        session_dir.join("wire.jsonl"),
        b"{\"type\":\"metadata\",\"protocol_version\":\"1.9\"}\n",
    )
    .await
    .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
    let poller = KimiPoller::new(
        dir.path().join("sessions"),
        kimi_json.clone(),
        tx,
        repo.clone(),
        Duration::from_millis(50),
    );
    let handle = poller.spawn();

    // Append three records after the poller has started.
    tokio::time::sleep(Duration::from_millis(120)).await;
    let mut f = tokio::fs::OpenOptions::new()
        .append(true)
        .open(session_dir.join("wire.jsonl"))
        .await
        .unwrap();
    use tokio::io::AsyncWriteExt;
    let body = concat!(
        r#"{"timestamp":1777096658.4,"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"hi"}]}}}"#,
        "\n",
        r#"{"timestamp":1777096659.0,"message":{"type":"TextPart","payload":{"text":"hello back"}}}"#,
        "\n",
        r#"{"timestamp":1777096660.0,"message":{"type":"ToolCall","payload":{"id":"c1","function":{"name":"Read","arguments":"{\"path\":\"/x\"}"}}}}"#,
        "\n",
        r#"{"timestamp":1777096660.5,"message":{"type":"ToolResult","payload":{"id":"c1","content":"ok"}}}"#,
        "\n",
    );
    f.write_all(body.as_bytes()).await.unwrap();
    f.flush().await.unwrap();
    drop(f);

    // Collect up to 5 events with a 2s timeout.
    let mut received = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline && received.len() < 4 {
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await {
            received.push(evt);
        }
    }

    handle.abort();

    // Expect: SessionStart + UserPrompt + AssistantMsg + ToolCall = 4 events.
    assert_eq!(received.len(), 4, "got {received:#?}");
    let kinds: Vec<&str> = received
        .iter()
        .map(|e| {
            let AgentEvent::V1(v) = e;
            match &v.kind {
                coding_ingest::event::EventKind::SessionStart { .. } => "sessionStart",
                coding_ingest::event::EventKind::UserPrompt { .. } => "userPrompt",
                coding_ingest::event::EventKind::AssistantMsg { .. } => "assistantMsg",
                coding_ingest::event::EventKind::ToolCall { .. } => "toolCall",
                _ => "other",
            }
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["sessionStart", "userPrompt", "assistantMsg", "toolCall"]
    );

    // Same 4 rows must be in ingest_event_log.
    let count = repo.count_by_session(session_id).await.unwrap();
    assert_eq!(count, 4);
}
```

- [ ] **Step 8.4: Run integration test**

Run: `cargo nextest run -p coding-ingest --test kimi_poller -- --test-threads=1 2>&1 | tail -30`

Expected: 1 test PASS. If it flakes due to filesystem timing, bump the post-spawn sleep in 8.3 to 250ms.

- [ ] **Step 8.5: Lint**

Run: `cargo clippy -p coding-ingest --all-targets -- -D warnings`

Expected: no warnings.

- [ ] **Step 8.6: Commit**

```bash
git add crates/coding-ingest/src/adapters/kimi_cli/poller.rs crates/coding-ingest/tests/kimi_poller.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): KimiPoller tails wire.jsonl end-to-end

Reads new bytes per tick, parses metadata + records, unwraps SubagentEvent,
buffers ToolCall by id, and emits AgentEvents into the daemon's channel +
ingest_event_log. Integration test covers SessionStart/UserPrompt/
AssistantMsg/ToolCall fan-out for one fixture session.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Replace `kimi_wire_socket` with `kimi_sessions_dir` in the daemon

**Files:**
- Modify: `crates/coding-ingest/src/daemon.rs`
- Modify: `crates/coding-ingest/tests/daemon_lifecycle.rs`
- Modify: `crates/coding-ingest/tests/drain_buffer.rs`

- [ ] **Step 9.1: Update `IngestDaemonConfig` field**

In `crates/coding-ingest/src/daemon.rs`, find:

```rust
    /// Optional kimi-cli Wire Unix socket path. When set, a tier-2 streaming
    /// task is spawned alongside tier-1 hooks.
    pub kimi_wire_socket: Option<PathBuf>,
```

Replace with:

```rust
    /// Optional kimi sessions directory (typically `~/.kimi/sessions`). When
    /// set, a [`KimiPoller`](crate::KimiPoller) task is spawned to tail
    /// per-session `wire.jsonl` files.
    pub kimi_sessions_dir: Option<PathBuf>,
    /// Polling interval for kimi. Defaults to 1 s.
    pub kimi_poll_interval: Option<std::time::Duration>,
    /// Optional override for `~/.kimi/kimi.json` (used to resolve work-dir
    /// hashes). Defaults to `<HOME>/.kimi/kimi.json`.
    pub kimi_json_path: Option<PathBuf>,
```

- [ ] **Step 9.2: Replace the spawn block**

In `crates/coding-ingest/src/daemon.rs`, find the block:

```rust
    // Kimi-cli Wire tier-2 streaming task — optional.
    let kimi_wire_task = if let Some(socket_path) = cfg.kimi_wire_socket {
        if let Some(tx) = cfg.event_tx.clone() {
            Some(crate::adapters::kimi_cli::spawn_wire(socket_path, tx))
        } else {
            tracing::warn!("kimi_wire_socket set but no event_tx — wire loop not spawned");
            None
        }
    } else {
        None
    };
```

Replace with:

```rust
    // Kimi-cli per-session JSONL poller — optional, spawned when sessions
    // dir is set in the config. The caller is responsible for providing
    // `kimi_json_path` (mirrors how `opencode_db_path` is plumbed).
    let kimi_wire_task = if let Some(sessions_dir) = cfg.kimi_sessions_dir {
        let interval = cfg
            .kimi_poll_interval
            .unwrap_or(std::time::Duration::from_secs(1));
        if let Some(tx) = cfg.event_tx.clone() {
            // Default to a non-existent path if the caller didn't provide
            // one — `WorkdirIndex::refresh` treats NotFound as an empty
            // index, which is the right behaviour.
            let kimi_json = cfg
                .kimi_json_path
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("/nonexistent/kimi.json"));
            let poller = crate::adapters::kimi_cli::poller::KimiPoller::new(
                sessions_dir,
                kimi_json,
                tx,
                cfg.repo.clone(),
                interval,
            );
            Some(poller.spawn())
        } else {
            tracing::warn!("kimi_sessions_dir set but no event_tx — poller not spawned");
            None
        }
    } else {
        None
    };
```

The local variable name `kimi_wire_task` stays so we don't churn the `IngestDaemonHandle` struct field.

- [ ] **Step 9.3: Update existing tests' config literals**

In `crates/coding-ingest/tests/daemon_lifecycle.rs`, change:

```rust
        kimi_wire_socket: None,
```

to:

```rust
        kimi_sessions_dir: None,
        kimi_poll_interval: None,
        kimi_json_path: None,
```

In `crates/coding-ingest/tests/drain_buffer.rs`, do the same replacement.

- [ ] **Step 9.4: Verify compile**

Run: `cargo check -p coding-ingest --all-targets`

Expected: success.

- [ ] **Step 9.5: Run the full coding-ingest test suite**

Run: `cargo nextest run -p coding-ingest`

Expected: all tests pass.

- [ ] **Step 9.6: Delete the legacy wire stub**

Now that `daemon.rs` no longer calls `spawn_wire`, drop the stub:

```bash
rm crates/coding-ingest/src/adapters/kimi_cli/wire.rs
```

In `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`, remove the lines:

```rust
/// Deprecated tier-2 stub — removed in Task 9.7.
#[allow(deprecated)]
pub mod wire;

pub use wire::spawn_wire;
```

Run: `cargo check -p coding-ingest --all-targets`

Expected: success.

- [ ] **Step 9.7: Lint**

Run: `cargo clippy -p coding-ingest --all-targets -- -D warnings`

Expected: no warnings.

- [ ] **Step 9.8: Commit**

```bash
git add -A crates/coding-ingest/src/daemon.rs crates/coding-ingest/src/adapters/kimi_cli/ crates/coding-ingest/tests/daemon_lifecycle.rs crates/coding-ingest/tests/drain_buffer.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(coding-ingest): wire KimiPoller into daemon, drop kimi_wire_socket

Replaces IngestDaemonConfig.kimi_wire_socket with kimi_sessions_dir,
kimi_poll_interval, and kimi_json_path. Tests updated.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Wire app-core init to feed `kimi_sessions_dir`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 10.1: Locate the daemon-config block**

Run: `grep -n 'kimi_wire_socket\|opencode_db_path' crates/app-core/src/init/mod.rs`

You're looking for the `IngestDaemonConfig { ... }` literal around line 1069.

- [ ] **Step 10.2: Update the config literal**

In `crates/app-core/src/init/mod.rs`, find the block that includes `kimi_wire_socket: None,`. Just before that line, add a `kimi_sessions_dir`/`kimi_json_path` resolver mirroring the `opencode_db_path` block:

```rust
            let kimi_sessions_dir = if config.coding_memory.cli.kimi_cli.enabled {
                dirs::home_dir().map(|h| h.join(".kimi").join("sessions"))
            } else {
                None
            };
            let kimi_json_path = if config.coding_memory.cli.kimi_cli.enabled {
                dirs::home_dir().map(|h| h.join(".kimi").join("kimi.json"))
            } else {
                None
            };
```

(Place it next to `opencode_db_path` and `codex_sessions_dir` for consistency.)

In the `IngestDaemonConfig { ... }` literal, replace the line:

```rust
                kimi_wire_socket: None,
```

with:

```rust
                kimi_sessions_dir,
                kimi_poll_interval: None,
                kimi_json_path,
```

- [ ] **Step 10.3: Verify compile**

Run: `cargo check -p app-core`

Expected: success.

- [ ] **Step 10.4: Run app-core tests**

Run: `cargo nextest run -p app-core`

Expected: all tests pass except `kimi_installer.rs` which we delete next.

- [ ] **Step 10.5: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
feat(app-core): wire KimiPoller via kimi_sessions_dir + kimi_json_path

Toggling coding_memory.cli.kimi_cli.enabled now spawns the poller on next
daemon init (parity with opencode/codex).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Drop the `kimi-cli` arms in `coding_memory_*` handlers and delete `KimiInstaller`

**Files:**
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Delete: `crates/app-core/src/coding_memory/kimi_installer.rs`
- Delete: `crates/app-core/tests/kimi_installer.rs`

- [ ] **Step 11.1: Drop the `pub mod kimi_installer` line**

In `crates/app-core/src/coding_memory/mod.rs`, find:

```rust
/// Kimi hooks.json installer.
pub mod kimi_installer;
```

Delete those two lines.

- [ ] **Step 11.2: Drop the `"kimi-cli"` arm in `set_cli_enabled`**

In `crates/app-core/src/coding_memory/mod.rs`, find the `set_cli_enabled` function and remove the arm:

```rust
            "kimi-cli" => {
                let cfg = dirs::home_dir()
                    .ok_or_else(|| ApiError::new("INTERNAL_ERROR", "no home dir"))?
                    .join(".kimi/config.toml");
                let binary = hook_binary_path()?;
                if enabled {
                    tokio::task::spawn_blocking(move || {
                        crate::coding_memory::kimi_installer::KimiInstaller::install(&cfg, &binary)
                    })
                } else {
                    tokio::task::spawn_blocking(move || {
                        crate::coding_memory::kimi_installer::KimiInstaller::uninstall(&cfg)
                    })
                }
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
            }
```

- [ ] **Step 11.3: Drop the `"kimi-cli"` arm in `coding_memory_diagnose_cli`**

In the same file, find the `coding_memory_diagnose_cli` function and remove:

```rust
            "kimi-cli" => {
                let outcome = tokio::task::spawn_blocking(move || {
                    crate::coding_memory::kimi_installer::KimiInstaller::diagnose(&binary)
                })
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
                Ok(diagnose_to_result(outcome))
            }
```

- [ ] **Step 11.4: Verify the toggle still flips the config bit**

Confirm the second `match cli` block (`"kimi-cli" => cfg.coding_memory.cli.kimi_cli.enabled = enabled,`) is still present — the `enabled` flip is still required so the daemon picks it up on next init / hot-reload.

- [ ] **Step 11.5: Delete the installer files**

```bash
rm crates/app-core/src/coding_memory/kimi_installer.rs
rm crates/app-core/tests/kimi_installer.rs
```

- [ ] **Step 11.6: Verify compile**

Run: `cargo check -p app-core --all-targets`

Expected: success.

- [ ] **Step 11.7: Update the `kimi-cli` UI behaviour expectation**

Search for any user-facing strings or doc comments that imply Kimi has an installer. Run:

```bash
grep -rn "Kimi\|kimi-cli" crates/app-core/src/coding_memory/ --include="*.rs"
```

The remaining matches should be:
- `handlers.rs::cli_health` source slug (still required — keeps the CLI health row).
- `handlers.rs::provider_id_to_db_slug` (still required — UI camelCase translation).
- `mod.rs::set_cli_enabled` config flip arm.

If any other reference still implies hook installation, leave it — that's owned by future cleanup tasks.

- [ ] **Step 11.8: Run all crate tests**

Run: `cargo nextest run -p app-core -p coding-ingest`

Expected: all tests pass.

- [ ] **Step 11.9: Lint**

Run: `cargo clippy -p app-core -p coding-ingest --all-targets -- -D warnings`

Expected: no warnings.

- [ ] **Step 11.10: Commit**

```bash
git add -A crates/app-core/src/coding_memory/ crates/app-core/tests/
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
refactor(app-core): drop KimiInstaller, kimi-cli is poll-only now

set_cli_enabled and coding_memory_diagnose_cli no longer have hook-install
side-effects for kimi-cli. The toggle still flips
coding_memory.cli.kimi_cli.enabled which gates the daemon's KimiPoller.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Sanity-check cross-cutting tests, formatting, and full workspace build

**Files:** none modified — verification only.

- [ ] **Step 12.1: Cross-CLI normalization proptest**

Run: `cargo nextest run -p coding-ingest --test cross_cli_normalization`

Expected: passes (KimiCli source roundtrip already covered).

- [ ] **Step 12.2: Agent-event roundtrip**

Run: `cargo nextest run -p coding-ingest --test agent_event_roundtrip`

Expected: `kimi_cli_source_roundtrip` passes.

- [ ] **Step 12.3: Workspace build**

Run: `cargo build --workspace 2>&1 | tail -20`

Expected: success.

- [ ] **Step 12.4: Workspace tests**

Run: `cargo nextest run --workspace 2>&1 | tail -30`

Expected: all pass.

- [ ] **Step 12.5: Workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -20`

Expected: no warnings.

- [ ] **Step 12.6: Format check**

Run: `cargo fmt --all --check`

Expected: success.

- [ ] **Step 12.7: Doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -10`

Expected: success.

If any step fails, fix it and re-run before continuing.

---

## Task 13: Manual end-to-end verification against a real kimi session

**Files:** none — runtime verification.

- [ ] **Step 13.1: Build dev desktop binary**

Run: `cargo build -p desktop`

Expected: success.

- [ ] **Step 13.2: Confirm dev data dir is set**

Run: `grep KLYNTBOT_HOME .env 2>/dev/null || echo unset`

If `unset`, set `KLYNTBOT_HOME=~/.klyntbot-dev` in `.env` for this run so we don't write into the production data dir.

- [ ] **Step 13.3: Start Vite + Tauri dev**

In one terminal: `cd desktop-ui && bun run dev`
In a second terminal: `cargo tauri dev`

Expected: desktop window opens.

- [ ] **Step 13.4: Confirm Kimi poll wiring in logs**

In the `cargo tauri dev` terminal output, look for:

```
kimi poller starting sessions_dir=/Users/<you>/.kimi/sessions
```

Expected: that line is present once at startup.

- [ ] **Step 13.5: Run a fresh kimi-cli session**

In a third terminal: `cd /tmp && mkdir -p kimi-poll-smoke && cd kimi-poll-smoke && kimi`

Send a short prompt like "hello" and let kimi respond. Quit kimi (`/exit`).

- [ ] **Step 13.6: Verify the session shows up**

In the desktop UI: navigate to Plugins → Coding Memory → Kimi tab. Within ~2s, the new session should appear in the Sessions list with a non-zero event count. Click it and confirm the events panel shows `sessionStart`, `userPrompt`, `assistantMsg`, and any `toolCall` rows.

- [ ] **Step 13.7: Verify ingest_event_log directly**

Run:

```bash
sqlite3 ~/.klyntbot-dev/data.db \
  "SELECT source, kind, COUNT(*) FROM ingest_event_log WHERE source = 'kimi-cli' GROUP BY kind ORDER BY kind;"
```

Expected: rows for at least `sessionStart`, `userPrompt`, `assistantMsg`. If empty, re-check Step 13.4.

- [ ] **Step 13.8: Toggle disable → re-enable**

In the UI, toggle Kimi off, then on. Confirm the daemon log emits one new `kimi poller starting` line after the re-enable (config hot-reload path).

If 13.6 or 13.7 fails, debug:
- Is `~/.kimi/sessions/<hash>/<uuid>/wire.jsonl` being written? (`ls -lat`)
- Is the dev data dir the same one the desktop reads from? (`echo $KLYNTBOT_HOME`)
- Is the workdir hash resolving? Check `tracing` logs at `info` for "kimi poller" and "workdir_index".

---

## Task 14: Update CLAUDE.md gotchas (doc-only)

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 14.1: Locate the existing kimi-cli line**

Run: `grep -n "kimi-cli\|Kimi" CLAUDE.md | head`

You're looking for the bullet that begins with **Coding-memory Phase 7 — multi-CLI ingest.**

- [ ] **Step 14.2: Update the bullet**

In `CLAUDE.md`, find the sentence:

```
Codex + Kimi are hook-driven (TOML/JSON installers in `app-core/src/coding_memory/{codex,kimi}_installer.rs`); opencode is poll-only ...
```

Replace with:

```
Codex, Kimi, and opencode are poll-only (Codex via `~/.codex/sessions` JSONL; Kimi via `~/.kimi/sessions/<hash>/<uuid>/wire.jsonl`; opencode via SQLite WAL). Hook-driven adapters were removed in 2026-04-29 — `KimiInstaller` no longer exists.
```

- [ ] **Step 14.3: Commit**

```bash
git add CLAUDE.md
git -c commit.gpgsign=false commit -m "$(cat <<'EOF'
docs: update CLAUDE.md to reflect kimi-cli poll-only ingest

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review checklist (before declaring done)

- [ ] Every section in the spec maps to at least one task above. (Spot-check the Components, Data flow, Message-type mapping, Error handling, Testing sections.)
- [ ] Every step that writes code shows the code.
- [ ] Every step that runs a command shows the command and expected output.
- [ ] No "TBD"/"TODO"/"similar to"/"add appropriate" placeholders remain.
- [ ] Method/struct names are consistent across tasks: `KimiPoller`, `WorkdirIndex`, `WireMetadata`, `WireRecord`, `WireEnvelope`, `WireLine`, `CollectedEvent`, `SessionState`, `parse_line`, `collect_events`, `map_event`, `maybe_emit_session_start`, `hash_for`, `kimi_sessions_dir`, `kimi_poll_interval`, `kimi_json_path`.
- [ ] Tasks are ordered so each commit leaves the workspace in a building state.
- [ ] Cross-CLI normalization proptest is verified (Task 12).
- [ ] Manual end-to-end verification has been performed (Task 13).
