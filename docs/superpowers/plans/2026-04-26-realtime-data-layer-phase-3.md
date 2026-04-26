# Real-time Data Layer Phase 3 — MCP Cross-Process Event Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Unix-socket bridge so global UI events emitted from a child `klyntbot mcp serve --stdio` process (e.g. `entity:updated` after Claude Code calls a klyntbot MCP tool) reach every running desktop webview within ~10 ms — without polling, file-watching, or restarts. When the desktop isn't running the MCP child continues to function standalone (silent no-op).

**Architecture:** A new crate `crates/mcp-bridge` defines a JSON-line protocol (`{event, payload}`) over `${KLYNTBOT_HOME:-~/.klyntbot}/mcp-events.sock`. Wire format mirrors `crates/coding-ingest/src/transport.rs:48-89` exactly: 4-byte little-endian length prefix + JSON body, 1 MB cap, 200 ms timeouts. **Server side (desktop process):** `BridgeServer` binds a `tokio::net::UnixListener` during desktop boot and re-emits incoming frames via `tauri::AppHandle::emit(event, payload)` — the same global broadcast Tauri uses for `entity:updated` today, so every webview's `tauriEventBridge` (Plan 1) picks them up and invalidates the matching TanStack Query keys. **Client side (MCP process):** `SocketBridgeEmitter` implements `app_core::events::AppEventEmitter` and is injected into the existing `init_with_sender` slot (which already accepts `Option<Arc<dyn AppEventEmitter>>`). Each `emit_event` call enqueues onto an unbounded `mpsc` and a background tokio task drains it to the socket so emit calls never block the agent loop. Connect failures are logged at `trace` and dropped silently. Generic protocol — works for *every* global event, not just `entity:updated`.

**Tech Stack:** Rust 2024 edition (workspace MSRV 1.93), `tokio` (`net`, `io-util`, `sync`, `time`, `rt-multi-thread`, `macros` — already in workspace deps), `tokio-util = "0.7"` (for `CancellationToken`), `serde`, `serde_json`, `thiserror = "2"`, `tracing`, `tempfile` (dev only).

**Master plan context:** Plan 3 of 4. **Depends on Plan 1** (the FE `tauriEventBridge` already routes `entity:updated` → query invalidations — the bridge is "real" only because Plan 1 listens). **Independent of Plan 2** (different feature surface). Plan 4 covers Distiller emission + `data_version` polling fallback.

---

## File Structure

### New files

| Path | Responsibility |
|---|---|
| `crates/mcp-bridge/Cargo.toml` | Crate manifest. |
| `crates/mcp-bridge/src/lib.rs` | Public API surface; module wiring; `bridge_socket_path()` helper. |
| `crates/mcp-bridge/src/protocol.rs` | `BridgeFrame { event, payload }`, `FrameError`, `read_frame`, `write_frame`. |
| `crates/mcp-bridge/src/client.rs` | `BridgeClient`: write-only, lazy-connect, silent-fail, owns the writer task. |
| `crates/mcp-bridge/src/emitter.rs` | `SocketBridgeEmitter`: implements `AppEventEmitter` over `BridgeClient`. |
| `crates/mcp-bridge/src/server.rs` | `BridgeServer`: `UnixListener` accept loop; per-connection reader task. |
| `crates/mcp-bridge/tests/protocol_roundtrip.rs` | Framing roundtrip integration test. |
| `crates/mcp-bridge/tests/server_emits_to_handler.rs` | Client → server → callback closure. |
| `crates/mcp-bridge/tests/client_no_server.rs` | Silent no-op when socket absent. |

### Files to modify

| Path | Change |
|---|---|
| `Cargo.toml` (workspace) | Append `"crates/mcp-bridge"` to `members`. |
| `crates/desktop/Cargo.toml` | Add `mcp-bridge = { path = "../mcp-bridge" }`. |
| `crates/desktop/src/app_core.rs` | Spawn `BridgeServer` after `init_with_sender`; store handle on `AppCore` so it lives for process lifetime. Cleanup on shutdown. |
| `crates/desktop/src/main.rs` (`run_mcp_stdio`) | Construct `SocketBridgeEmitter`, switch `AppCore::init` → `AppCore::init_with_sender(.., None, Some(emitter))`. |

### Files NOT modified (verified during research; called out to prevent drift)

- `crates/app-core/src/events.rs` — `AppEventEmitter` trait already exposes the right surface; default `emit_entity_updated` impl works.
- `crates/app-core/src/init/mod.rs` — `init_with_sender` already accepts `Option<Arc<dyn AppEventEmitter>>`.
- `crates/bus/src/domain_events.rs` — no new `DomainEvent` variant. The bridge does **not** touch the domain bus.
- `crates/desktop-shared/src/events.rs` — `ENTITY_UPDATED` constant + `EntityUpdatedPayload` are reused as-is.

---

## Phase A — Crate scaffold

### Task A1: Create the crate skeleton

**Files:**
- Create: `crates/mcp-bridge/Cargo.toml`
- Create: `crates/mcp-bridge/src/lib.rs`
- Create: `crates/mcp-bridge/src/{protocol,client,server,emitter}.rs` (empty placeholders)
- Modify: root `Cargo.toml`

`★ Insight ─────────────────────────────────────`
We make `app-core` a dependency only of `emitter.rs` so `protocol`, `client`, and `server` stay framework-agnostic and unit-testable in isolation. The `tauri = { workspace = true, optional = true }` dep behind a `tauri-emit` feature lets the desktop crate plug directly into Tauri without forcing the MCP-only client to compile Tauri.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create directories**

```bash
mkdir -p /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src
mkdir -p /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/tests
```

- [ ] **Step 2: Write `crates/mcp-bridge/Cargo.toml`**

```toml
[package]
name = "mcp-bridge"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
tokio = { workspace = true, features = ["net", "io-util", "sync", "rt-multi-thread", "macros", "time"] }
tokio-util = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

# Used only by `emitter` (MCP side)
app-core = { path = "../app-core" }
desktop-shared = { path = "../desktop-shared" }
config = { path = "../config" }
common = { path = "../common" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "time", "macros"] }
tempfile = { workspace = true }
```

Verify versions match the workspace by checking root `Cargo.toml` lines 109–145 — `tokio-util = "0.7"`, `thiserror = "2.0.18"`, `tempfile = "3.14"` are already declared at the workspace level so `{ workspace = true }` resolves cleanly.

- [ ] **Step 3: Add to workspace `members`**

Open `/Users/jayden/Projects/Klynt/bot/Cargo.toml` and append `"crates/mcp-bridge"` to the `members` array (any position; the existing list is unsorted-by-domain).

```toml
members = [
    "crates/common",
    # ... existing entries ...
    "crates/coding-memory",
    "crates/mcp-bridge",
]
```

- [ ] **Step 4: Stub the four module files (empty so the crate compiles)**

```bash
touch /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src/protocol.rs
touch /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src/client.rs
touch /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src/server.rs
touch /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src/emitter.rs
```

- [ ] **Step 5: Write `crates/mcp-bridge/src/lib.rs`**

```rust
//! Cross-process event bridge between the desktop process and a child
//! `klyntbot mcp serve --stdio` process.
//!
//! See `docs/superpowers/plans/2026-04-26-realtime-data-layer-phase-3.md`
//! for the full design rationale and protocol specification.

pub mod client;
pub mod emitter;
pub mod protocol;
pub mod server;

pub use client::BridgeClient;
pub use emitter::SocketBridgeEmitter;
pub use protocol::{read_frame, write_frame, BridgeFrame, FrameError};
pub use server::{BridgeServer, BridgeServerHandle};

use std::path::PathBuf;

/// Resolve the bridge socket path. Both the desktop server and MCP child
/// must agree, so they share this single helper.
///
/// Path: `${KLYNTBOT_HOME or ~/.klyntbot}/mcp-events.sock`.
///
/// Returns `None` if the home directory cannot be determined (very rare —
/// only when `HOME` is unset on Unix and `KLYNTBOT_HOME` is also unset).
pub fn bridge_socket_path() -> Option<PathBuf> {
    config::loader::config_dir()
        .ok()
        .map(|d| d.join("mcp-events.sock"))
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p mcp-bridge 2>&1 | tail -15
```

Expected: `Compiling mcp-bridge ...` then `Finished`. Empty modules generate no warnings (Rust permits empty `.rs` files).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/mcp-bridge
git commit -m "chore(mcp-bridge): scaffold crate"
```

---

## Phase B — Protocol & framing

### Task B1: Define `BridgeFrame` + `FrameError`

**Files:**
- Modify: `crates/mcp-bridge/src/protocol.rs`

`★ Insight ─────────────────────────────────────`
The frame carries `(event: String, payload: serde_json::Value)` — exactly the parameters of `AppEventEmitter::emit_event`. This lets us bridge *every* global event without a per-event variant table. Names like `"entity:updated"` are passed through verbatim; the receiver re-emits them via `app_handle.emit(event, payload)` and the existing Tauri broadcast handles webview fan-out.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing tests**

Replace `crates/mcp-bridge/src/protocol.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeFrame {
    /// Tauri event name, e.g. "entity:updated", "provider:degraded".
    pub event: String,
    /// Arbitrary JSON payload — mirrors `AppEventEmitter::emit_event`'s second arg.
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame too large: {0} bytes (max {})", super::protocol::MAX_FRAME_BYTES)]
    TooLarge(u32),
    #[error("decode: {0}")]
    Decode(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrips_through_json() {
        let frame = BridgeFrame {
            event: "entity:updated".into(),
            payload: serde_json::json!({ "entityKind": "task", "id": "t1" }),
        };
        let bytes = serde_json::to_vec(&frame).unwrap();
        let back: BridgeFrame = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, frame);
    }

    #[test]
    fn frame_preserves_arbitrary_payload_shapes() {
        let frame = BridgeFrame {
            event: "provider:degraded".into(),
            payload: serde_json::json!({
                "provider": "anthropic",
                "reason": "rate_limit",
                "retryAfterSeconds": 30,
                "nested": { "a": [1, 2, 3] }
            }),
        };
        let s = serde_json::to_string(&frame).unwrap();
        let back: BridgeFrame = serde_json::from_str(&s).unwrap();
        assert_eq!(back, frame);
    }
}
```

- [ ] **Step 2: Run tests — expect FAIL on `MAX_FRAME_BYTES`**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --lib 2>&1 | tail -10
```

Expected: compile error referencing `MAX_FRAME_BYTES` (added in B2).

- [ ] **Step 3: Add the constant placeholder**

Insert at the top of `protocol.rs` (under the imports):

```rust
/// 1 MB cap, mirroring `coding-ingest::transport::MAX_PAYLOAD_BYTES`.
pub(crate) const MAX_FRAME_BYTES: u32 = 1 << 20;
```

- [ ] **Step 4: Run tests — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --lib 2>&1 | tail -10
```

Expected: 2 passing tests.

- [ ] **Step 5: Commit**

```bash
git add crates/mcp-bridge/src/protocol.rs
git commit -m "feat(mcp-bridge): define BridgeFrame and FrameError"
```

---

### Task B2: Add `read_frame` / `write_frame` helpers

**Files:**
- Modify: `crates/mcp-bridge/src/protocol.rs`

The wire format is 4-byte LE u32 length-prefix + JSON body. Mirror `crates/coding-ingest/src/transport.rs:48-89` exactly so future maintainers see one canonical pattern.

- [ ] **Step 1: Append framing tests**

Append below the existing `mod tests`:

```rust
#[cfg(test)]
mod framing_tests {
    use super::*;
    use tokio::io::{duplex, AsyncWriteExt};

    fn sample_frame() -> BridgeFrame {
        BridgeFrame {
            event: "entity:updated".into(),
            payload: serde_json::json!({ "entityKind": "note", "id": "n42" }),
        }
    }

    #[tokio::test]
    async fn write_then_read_roundtrips() {
        let (mut writer, mut reader) = duplex(4096);
        let frame = sample_frame();
        write_frame(&mut writer, &frame).await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let received = read_frame(&mut reader).await.unwrap();
        assert_eq!(received, Some(frame));

        // Next read on closed half returns Ok(None) — clean EOF.
        assert_eq!(read_frame(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn read_frame_returns_none_on_clean_eof_before_any_bytes() {
        let (writer, mut reader) = duplex(64);
        drop(writer);
        assert_eq!(read_frame(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn read_frame_errors_on_too_large_length_prefix() {
        let (mut writer, mut reader) = duplex(64);
        // Bogus length: 10 MB > 1 MB cap.
        writer.write_all(&(10_000_000u32).to_le_bytes()).await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let res = read_frame(&mut reader).await;
        assert!(matches!(res, Err(FrameError::TooLarge(_))), "got: {res:?}");
    }

    #[tokio::test]
    async fn write_frame_errors_on_oversize_payload() {
        let mut sink = Vec::<u8>::new();
        // 2 MB string > 1 MB cap.
        let huge = "x".repeat(2 * 1024 * 1024);
        let frame = BridgeFrame {
            event: "x".into(),
            payload: serde_json::Value::String(huge),
        };
        let res = write_frame(&mut sink, &frame).await;
        assert!(matches!(res, Err(FrameError::TooLarge(_))), "got: {res:?}");
    }

    #[tokio::test]
    async fn read_frame_errors_on_truncated_body() {
        let (mut writer, mut reader) = duplex(64);
        // Claim 100 bytes but only send 5.
        writer.write_all(&(100u32).to_le_bytes()).await.unwrap();
        writer.write_all(b"hello").await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let res = read_frame(&mut reader).await;
        assert!(matches!(res, Err(FrameError::Io(_))), "got: {res:?}");
    }
}
```

- [ ] **Step 2: Run — expect FAIL (helpers don't exist)**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --lib 2>&1 | tail -15
```

Expected: compile error: `cannot find function 'write_frame' in this scope` etc.

- [ ] **Step 3: Implement framing helpers**

Append to `protocol.rs`:

```rust
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Encode `frame` as 4-byte LE length + JSON body and write to `writer`.
/// Does not flush or shutdown.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &BridgeFrame,
) -> Result<(), FrameError> {
    let body = serde_json::to_vec(frame)?;
    let len_u32: u32 = u32::try_from(body.len())
        .map_err(|_| FrameError::TooLarge(u32::MAX))?;
    if len_u32 > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len_u32));
    }
    writer.write_all(&len_u32.to_le_bytes()).await?;
    writer.write_all(&body).await?;
    Ok(())
}

/// Read one frame from `reader`. Returns:
/// - `Ok(Some(frame))` on success.
/// - `Ok(None)` on clean EOF *before* the length prefix.
/// - `Err(_)` for partial reads, oversize prefixes, or decode errors.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Option<BridgeFrame>, FrameError> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(FrameError::Io(e)),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body).await?;
    let frame = serde_json::from_slice(&body)?;
    Ok(Some(frame))
}
```

- [ ] **Step 4: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --lib 2>&1 | tail -10
```

Expected: 7 passing tests (2 from B1 + 5 framing).

- [ ] **Step 5: Commit**

```bash
git add crates/mcp-bridge/src/protocol.rs
git commit -m "feat(mcp-bridge): add length-prefix framing helpers"
```

---

## Phase C — Client (write side)

### Task C1: `BridgeClient::new` + background writer task

**Files:**
- Modify: `crates/mcp-bridge/src/client.rs`
- Create: `crates/mcp-bridge/tests/client_no_server.rs`

`★ Insight ─────────────────────────────────────`
`AppEventEmitter::emit_event` is **synchronous** (verified in `app-core/src/events.rs:10` — `fn emit_event(&self, event_name: &str, payload: serde_json::Value)`). To avoid blocking the agent loop on socket connect/write, the client buffers frames in an unbounded `mpsc::UnboundedSender` and a dedicated tokio task drains it. The task lazy-(re)connects, writes, and on any error drops the connection and waits for the next frame. The whole machine survives the desktop being absent — we just keep buffering and trying.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test for the no-server case**

Create `crates/mcp-bridge/tests/client_no_server.rs`:

```rust
//! When the desktop process isn't running (no socket file or refused
//! connection), `BridgeClient::send` must not panic, must not block the
//! caller, and must drop frames silently.

use mcp_bridge::{BridgeClient, BridgeFrame};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[tokio::test]
async fn send_to_missing_socket_returns_immediately() {
    let path = PathBuf::from("/tmp/klynt-bridge-definitely-not-here-39481.sock");
    // Sanity: ensure the file truly does not exist.
    let _ = std::fs::remove_file(&path);

    let client = BridgeClient::new(path);
    let frame = BridgeFrame {
        event: "entity:updated".into(),
        payload: json!({ "entityKind": "task", "id": "x" }),
    };

    let start = Instant::now();
    for _ in 0..50 {
        client.send(frame.clone());
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(50),
        "send() should be non-blocking; took {elapsed:?}"
    );

    // Give the writer task a moment to attempt + fail, prove it doesn't crash.
    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn dropping_client_cleans_up_writer_task() {
    let path = PathBuf::from("/tmp/klynt-bridge-also-not-here-39482.sock");
    let _ = std::fs::remove_file(&path);
    let client = BridgeClient::new(path);
    drop(client);
    // No assertion — passing means "didn't deadlock the runtime on shutdown".
    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

- [ ] **Step 2: Run — expect FAIL (BridgeClient stub is empty)**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --test client_no_server 2>&1 | tail -10
```

Expected: compile error.

- [ ] **Step 3: Implement `BridgeClient`**

Replace `crates/mcp-bridge/src/client.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::trace;

use crate::protocol::{write_frame, BridgeFrame};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);
const WRITE_TIMEOUT: Duration = Duration::from_millis(200);

/// Write-only async client for the bridge socket.
///
/// `send` is non-blocking and infallible from the caller's perspective:
/// frames are pushed onto an unbounded mpsc and a background task drains it
/// to the socket. If the desktop process isn't running, frames are dropped
/// silently after a connect attempt times out (200 ms).
#[derive(Clone)]
pub struct BridgeClient {
    tx: mpsc::UnboundedSender<BridgeFrame>,
}

impl BridgeClient {
    pub fn new(socket_path: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(writer_loop(socket_path, rx));
        Self { tx }
    }

    /// Enqueue a frame for delivery. Never blocks. Drops silently if the
    /// internal channel is closed (writer task panicked — should not happen).
    pub fn send(&self, frame: BridgeFrame) {
        if let Err(e) = self.tx.send(frame) {
            trace!("mcp-bridge: client send dropped (channel closed): {e}");
        }
    }
}

/// Drains the channel forever, lazy-connecting on demand. Each iteration:
/// 1. Wait for the next frame (returns when channel closes → exit).
/// 2. Try to connect (200 ms timeout). On failure, drop the frame and loop.
/// 3. Try to write. On any error, drop the connection and loop.
async fn writer_loop(socket_path: PathBuf, mut rx: mpsc::UnboundedReceiver<BridgeFrame>) {
    while let Some(frame) = rx.recv().await {
        if let Err(e) = send_one(&socket_path, &frame).await {
            trace!(error = %e, path = ?socket_path, "mcp-bridge: send dropped");
        }
    }
    trace!("mcp-bridge: writer loop exiting (client dropped)");
}

async fn send_one(
    socket_path: &PathBuf,
    frame: &BridgeFrame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = timeout(CONNECT_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| "connect timeout")??;
    timeout(WRITE_TIMEOUT, async {
        write_frame(&mut stream, frame).await?;
        // Mirror `coding-ingest`: shutdown signals end-of-frame to the server.
        use tokio::io::AsyncWriteExt;
        stream.shutdown().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await
    .map_err(|_| "write timeout")??;
    Ok(())
}
```

- [ ] **Step 4: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --test client_no_server 2>&1 | tail -10
```

Expected: 2 passing tests.

- [ ] **Step 5: Commit**

```bash
git add crates/mcp-bridge/src/client.rs crates/mcp-bridge/tests/client_no_server.rs
git commit -m "feat(mcp-bridge): BridgeClient with non-blocking send + silent fallback"
```

---

## Phase D — Server (read side)

### Task D1: `BridgeServer::start` + accept loop

**Files:**
- Modify: `crates/mcp-bridge/src/server.rs`
- Create: `crates/mcp-bridge/tests/protocol_roundtrip.rs`
- Create: `crates/mcp-bridge/tests/server_emits_to_handler.rs`

`★ Insight ─────────────────────────────────────`
The server takes a `Box<dyn Fn(BridgeFrame) + Send + Sync>` handler closure rather than directly depending on `tauri::AppHandle`. Reasons: (a) the crate stays Tauri-free for tests, (b) the desktop layer can adapt the closure to whatever fan-out it wants — today `app_handle.emit(...)`, tomorrow potentially the `CompoundEmitter::broadcast` channel too. This is the same dependency-inversion pattern used by `SpawnHandler` / `CronHandler` per `CLAUDE.md`'s "Key patterns" section.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the protocol roundtrip integration test**

Create `crates/mcp-bridge/tests/protocol_roundtrip.rs`:

```rust
use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn client_send_reaches_server_handler() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bridge.sock");

    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame);
    });

    let server = BridgeServer::start(path.clone(), handler).await.unwrap();

    // Yield once so the listener is ready to accept.
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(path);
    let frame = BridgeFrame {
        event: "entity:updated".into(),
        payload: json!({ "entityKind": "task", "id": "t42" }),
    };
    client.send(frame.clone());

    // Poll until the handler observes the frame.
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("frame did not reach handler within 500 ms");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let frames = received.lock().unwrap().clone();
    assert_eq!(frames, vec![frame]);

    server.shutdown();
    // Confirm shutdown removed the socket file.
    let _ = timeout(Duration::from_millis(100), async {
        while path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(!path.exists(), "socket file should be removed on shutdown");
}
```

- [ ] **Step 2: Write the multi-frame test**

Create `crates/mcp-bridge/tests/server_emits_to_handler.rs`:

```rust
use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn server_handles_many_frames_across_connections() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("multi.sock");

    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame.event);
    });
    let server = BridgeServer::start(path.clone(), handler).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(path);
    for i in 0..25 {
        client.send(BridgeFrame {
            event: format!("test:event:{i}"),
            payload: json!({ "i": i }),
        });
    }

    let deadline = std::time::Instant::now() + Duration::from_millis(2000);
    loop {
        if received.lock().unwrap().len() == 25 {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "only got {} frames in 2s",
                received.lock().unwrap().len()
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let mut events = received.lock().unwrap().clone();
    events.sort();
    let mut expected: Vec<String> = (0..25).map(|i| format!("test:event:{i}")).collect();
    expected.sort();
    assert_eq!(events, expected);

    server.shutdown();
}

#[tokio::test]
async fn server_recovers_from_malformed_frame() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let dir = tempdir().unwrap();
    let path = dir.path().join("malformed.sock");

    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame);
    });
    let server = BridgeServer::start(path.clone(), handler).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Send garbage on one connection.
    {
        let mut s = UnixStream::connect(&path).await.unwrap();
        s.write_all(&(5u32).to_le_bytes()).await.unwrap();
        s.write_all(b"NOTJS").await.unwrap();
        s.shutdown().await.unwrap();
    }

    // Then a valid frame on a fresh connection.
    let client = BridgeClient::new(path);
    client.send(BridgeFrame {
        event: "valid:event".into(),
        payload: serde_json::json!({}),
    });

    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("server did not recover after malformed frame");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    server.shutdown();
}
```

- [ ] **Step 3: Run — expect compile errors**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge 2>&1 | tail -10
```

- [ ] **Step 4: Implement the server**

Replace `crates/mcp-bridge/src/server.rs`:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::protocol::{read_frame, BridgeFrame};

/// Synchronous handler invoked once per inbound frame. Must be cheap and
/// non-blocking — the connection reader awaits its return.
pub type FrameHandler = Box<dyn Fn(BridgeFrame) + Send + Sync + 'static>;

/// Owns the bound socket and the accept loop's cancellation token. Drop or
/// call `shutdown()` to stop and unlink the socket file.
pub struct BridgeServer {
    handle: BridgeServerHandle,
}

#[derive(Clone)]
pub struct BridgeServerHandle {
    shutdown: CancellationToken,
    socket_path: Arc<PathBuf>,
}

impl BridgeServer {
    /// Bind the socket, spawn the accept loop, return the running server.
    /// Removes any stale socket file at `socket_path` first.
    pub async fn start(
        socket_path: PathBuf,
        handler: FrameHandler,
    ) -> std::io::Result<Self> {
        let _ = std::fs::remove_file(&socket_path);
        if let Some(parent) = socket_path.parent() {
            // Best-effort: ensure parent dir exists (e.g. ~/.klyntbot).
            let _ = std::fs::create_dir_all(parent);
        }
        let listener = UnixListener::bind(&socket_path)?;
        let shutdown = CancellationToken::new();
        let path_arc = Arc::new(socket_path.clone());
        let handler = Arc::new(handler);
        Self::spawn_accept_loop(listener, handler, shutdown.clone());
        Ok(Self {
            handle: BridgeServerHandle {
                shutdown,
                socket_path: path_arc,
            },
        })
    }

    pub fn handle(&self) -> BridgeServerHandle {
        self.handle.clone()
    }

    /// Cancel the accept loop and unlink the socket file. Idempotent.
    pub fn shutdown(self) {
        self.handle.shutdown();
    }

    fn spawn_accept_loop(
        listener: UnixListener,
        handler: Arc<FrameHandler>,
        shutdown: CancellationToken,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        debug!("mcp-bridge: accept loop shutdown");
                        break;
                    }
                    res = listener.accept() => match res {
                        Ok((stream, _addr)) => {
                            let h = handler.clone();
                            tokio::spawn(handle_connection(stream, h));
                        }
                        Err(e) => {
                            error!("mcp-bridge: accept error: {e}");
                            break;
                        }
                    }
                }
            }
        });
    }
}

impl BridgeServerHandle {
    pub fn shutdown(&self) {
        self.shutdown.cancel();
        let _ = std::fs::remove_file(self.socket_path.as_path());
    }
}

impl Drop for BridgeServer {
    fn drop(&mut self) {
        self.handle.shutdown();
    }
}

async fn handle_connection(mut stream: UnixStream, handler: Arc<FrameHandler>) {
    loop {
        match read_frame(&mut stream).await {
            Ok(Some(frame)) => handler(frame),
            Ok(None) => break, // clean EOF
            Err(e) => {
                warn!("mcp-bridge: frame error, dropping connection: {e}");
                break;
            }
        }
    }
}

// Keep `Path` referenced for clarity even if unused at the type level.
#[allow(dead_code)]
fn _path_marker(_p: &Path) {}
```

- [ ] **Step 5: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge 2>&1 | tail -15
```

Expected: all framing + 2 client + 3 server tests pass (10 total so far).

- [ ] **Step 6: Commit**

```bash
git add crates/mcp-bridge/src/server.rs crates/mcp-bridge/tests/protocol_roundtrip.rs crates/mcp-bridge/tests/server_emits_to_handler.rs
git commit -m "feat(mcp-bridge): BridgeServer with cancellable accept loop"
```

---

## Phase E — Emitter integration

### Task E1: `SocketBridgeEmitter` implementing `AppEventEmitter`

**Files:**
- Modify: `crates/mcp-bridge/src/emitter.rs`

`★ Insight ─────────────────────────────────────`
`AppEventEmitter::emit_event` is sync and has signature `fn emit_event(&self, event_name: &str, payload: serde_json::Value)`. The default `emit_entity_updated` impl in `crates/app-core/src/events.rs:13-22` already builds the right `EntityUpdatedPayload` and calls `emit_event(ENTITY_UPDATED, value)` — so we only need to implement `emit_event`. Same goes for `emit_chat_thread`. Free coverage of every helper that lands on `emit_event`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Implement the emitter**

Replace `crates/mcp-bridge/src/emitter.rs`:

```rust
use crate::client::BridgeClient;
use crate::protocol::BridgeFrame;
use app_core::events::AppEventEmitter;

/// `AppEventEmitter` that ships every event over the bridge socket to a
/// running desktop process. When no desktop is running, frames are dropped
/// silently by `BridgeClient`.
pub struct SocketBridgeEmitter {
    client: BridgeClient,
}

impl SocketBridgeEmitter {
    pub fn new(client: BridgeClient) -> Self {
        Self { client }
    }
}

impl AppEventEmitter for SocketBridgeEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        self.client.send(BridgeFrame {
            event: event_name.to_string(),
            payload,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::events::AppEventEmitter;
    use desktop_shared::types::EntityKind;
    use std::path::PathBuf;

    #[tokio::test]
    async fn emit_to_missing_socket_is_silent() {
        let path = PathBuf::from("/tmp/klynt-bridge-emit-test-39483.sock");
        let _ = std::fs::remove_file(&path);
        let client = BridgeClient::new(path);
        let emitter = SocketBridgeEmitter::new(client);

        // Drives the default `emit_entity_updated` → `emit_event` impl.
        emitter.emit_entity_updated(EntityKind::Task, "t1");
        emitter.emit_event("provider:degraded", serde_json::json!({"x": 1}));

        // No panic, no block — give the writer task a moment to fail.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
```

- [ ] **Step 2: Add an end-to-end test that drives `emit_entity_updated` through the bridge**

Append to `crates/mcp-bridge/tests/protocol_roundtrip.rs`:

```rust
#[tokio::test]
async fn emit_entity_updated_through_bridge_arrives_with_camel_case_payload() {
    use app_core::events::AppEventEmitter;
    use desktop_shared::types::EntityKind;
    use mcp_bridge::SocketBridgeEmitter;

    let dir = tempdir().unwrap();
    let path = dir.path().join("emit.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let server = BridgeServer::start(
        path.clone(),
        Box::new(move |f| received_clone.lock().unwrap().push(f)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    emitter.emit_entity_updated(EntityKind::FocusSession, "fs-9");

    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("frame did not arrive");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let frames = received.lock().unwrap().clone();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].event, "entity:updated");
    // EntityKind serializes as camelCase per #[serde(rename_all = "camelCase")]
    // on `desktop-shared/src/types.rs:48`.
    assert_eq!(
        frames[0].payload,
        serde_json::json!({ "entityKind": "focusSession", "id": "fs-9" })
    );
    server.shutdown();
}
```

- [ ] **Step 3: Run — green**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge 2>&1 | tail -10
```

Expected: 12+ passing tests across `--lib` and integration suites.

- [ ] **Step 4: Commit**

```bash
git add crates/mcp-bridge/src/emitter.rs crates/mcp-bridge/tests/protocol_roundtrip.rs
git commit -m "feat(mcp-bridge): SocketBridgeEmitter for AppEventEmitter"
```

---

## Phase F — Wire into the desktop and MCP processes

### Task F1: Spawn `BridgeServer` during desktop boot

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/app_core.rs`

The handler closure forwards every frame through Tauri's global `app_handle.emit(event, payload)` — exactly what the existing `entity:updated` path does at `crates/desktop/src/commands/mod.rs:67-74`. Plan 1's `tauriEventBridge.ts` is already listening; nothing FE-side changes.

- [ ] **Step 1: Add the dep**

Edit `crates/desktop/Cargo.toml`. Under `[dependencies]`, add:

```toml
mcp-bridge = { path = "../mcp-bridge" }
```

(Place it alphabetically near `mcp = ...` if present, otherwise anywhere in `[dependencies]`.)

- [ ] **Step 2: Add a field to `AppCore` to keep the server alive**

The `AppCore` struct lives in `crates/app-core/src/lib.rs` (or wherever `pub struct AppCore` is defined — confirm with `grep -n "pub struct AppCore" crates/app-core/src/`). We do **not** want to leak `mcp-bridge` types into `app-core`, so instead we store the handle in the desktop wrapper.

Open `crates/desktop/src/app_core.rs`. Locate the existing `pub struct DesktopAppCore` — if there is no such struct, the desktop currently uses `app_core::AppCore` directly and we need a place to hold the handle. Use `OnceCell` at module scope:

Add near the top of `crates/desktop/src/app_core.rs` (under existing `use` statements):

```rust
use std::sync::OnceLock;

/// Process-wide handle to the MCP→desktop event bridge. Held forever so
/// the accept loop runs for the desktop's lifetime; on shutdown the
/// `Drop` impl unlinks the socket.
static BRIDGE_SERVER: OnceLock<mcp_bridge::BridgeServer> = OnceLock::new();
```

`OnceLock` is fine here because we only ever start the server once and never need to take it back out — the `Drop` runs at process exit.

- [ ] **Step 3: Spawn the bridge inside `init`**

In the same file, locate `init` (currently at lines 37–58). Right *after* `let (core, channels) = AppCore::init_with_sender(...)` and *before* `wire_event_channels(...)`, insert:

```rust
    // Cross-process event bridge — receives frames from a child
    // `klyntbot mcp serve --stdio` process and re-emits them via Tauri's
    // global broadcast so every webview's `tauriEventBridge` (Plan 1) picks
    // them up.
    if let Some(socket_path) = mcp_bridge::bridge_socket_path() {
        let app_handle_for_bridge = app_handle.clone();
        let handler: mcp_bridge::server::FrameHandler = Box::new(move |frame| {
            use tauri::Emitter;
            if let Err(e) = app_handle_for_bridge.emit(&frame.event, frame.payload) {
                tracing::warn!(
                    "mcp-bridge: failed to re-emit event {}: {e}",
                    frame.event
                );
            }
        });
        match mcp_bridge::BridgeServer::start(socket_path.clone(), handler).await {
            Ok(server) => {
                if BRIDGE_SERVER.set(server).is_err() {
                    tracing::warn!("mcp-bridge: BRIDGE_SERVER already initialized");
                }
                tracing::info!("mcp-bridge: listening at {}", socket_path.display());
            }
            Err(e) => {
                tracing::warn!(
                    "mcp-bridge: failed to bind {}: {e}; cross-process events disabled",
                    socket_path.display()
                );
            }
        }
    } else {
        tracing::warn!("mcp-bridge: cannot resolve socket path; bridge disabled");
    }
```

Note `FrameHandler` is `pub type` exported from `mcp_bridge::server` — visible because we re-export `BridgeServer` from `lib.rs` and `FrameHandler` is in the public `server` module.

- [ ] **Step 4: Make `FrameHandler` reachable**

To use `mcp_bridge::server::FrameHandler` you need it `pub` from the module. We already declared `pub type FrameHandler` in `server.rs` step D1, and `pub mod server` in `lib.rs` step A1, so this works without further changes. Verify:

```bash
grep -n "pub type FrameHandler\|pub mod server" /Users/jayden/Projects/Klynt/bot/crates/mcp-bridge/src/{lib.rs,server.rs}
```

Expected: both declarations found.

- [ ] **Step 5: Build the desktop crate**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p desktop 2>&1 | tail -20
```

Expected: clean build. If a clippy-style unused-import warning appears for `OnceLock`, ignore — `desktop` already has pre-existing exceptions per `CLAUDE.md`.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/app_core.rs
git commit -m "feat(desktop): start mcp-bridge server during init"
```

---

### Task F2: Inject `SocketBridgeEmitter` into `run_mcp_stdio`

**Files:**
- Modify: `crates/desktop/src/main.rs`

Currently `run_mcp_stdio` at line 162 calls `app_core::AppCore::init(common::AppMode::Server, Some(config))`, which delegates to `init_with_sender(.., None, None)` — meaning the MCP child wires a `NoopEmitter`. Switch to `init_with_sender` directly and pass our socket emitter.

- [ ] **Step 1: Verify the current call site**

```bash
sed -n '160,170p' /Users/jayden/Projects/Klynt/bot/crates/desktop/src/main.rs
```

Expected output (verbatim from research):
```rust
        let (app, events) = app_core::AppCore::init(common::AppMode::Server, Some(config))
            .await
            .expect("init failed");
```

- [ ] **Step 2: Replace with `init_with_sender` + socket emitter**

Edit `crates/desktop/src/main.rs`. Replace the three lines above with:

```rust
        // Wire a SocketBridgeEmitter so global events (entity:updated,
        // provider:degraded, …) emitted by tools and handlers in this MCP
        // child process flow back to a running desktop app via the
        // bridge socket. When no desktop is running, frames are dropped
        // silently — the MCP child still functions standalone.
        let socket_path = mcp_bridge::bridge_socket_path()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/klynt-mcp-events.sock"));
        let bridge_client = mcp_bridge::BridgeClient::new(socket_path);
        let event_emitter: std::sync::Arc<dyn app_core::events::AppEventEmitter> =
            std::sync::Arc::new(mcp_bridge::SocketBridgeEmitter::new(bridge_client));

        let (app, events) = app_core::AppCore::init_with_sender(
            common::AppMode::Server,
            Some(config),
            None,                  // notification_sender — not needed for stdio MCP
            Some(event_emitter),
        )
        .await
        .expect("init failed");
```

- [ ] **Step 3: Verify `mcp_bridge` is already a transitive dep of the desktop binary**

It is — Task F1 added `mcp-bridge = { path = "../mcp-bridge" }` to `crates/desktop/Cargo.toml`, and `main.rs` is part of the `desktop` crate. No second declaration needed.

- [ ] **Step 4: Build**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build -p desktop 2>&1 | tail -15
```

Expected: clean build. Two binaries are produced — the Tauri app (default) and the MCP subcommand path uses the same binary.

- [ ] **Step 5: Run the workspace build to confirm no other crate broke**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo build --workspace 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 6: Run the workspace test suite**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run --workspace 2>&1 | tail -15
```

Expected: all green. Plan 1 + Plan 2 tests still pass; new `mcp-bridge` tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): inject SocketBridgeEmitter into MCP stdio runtime"
```

---

## Phase G — End-to-end verification

### Task G1: Cross-process integration test (in-process simulation)

**Files:**
- Create: `crates/mcp-bridge/tests/end_to_end.rs`

A truly cross-process test would fork a child binary; we already cover the wire format in Phase D and the emitter in Phase E. This test wires `SocketBridgeEmitter` to a real `BridgeServer` and asserts `app_core::events::AppEventEmitter::emit_chat_thread` (a different default helper than `emit_entity_updated`) also bridges correctly — proving generic protocol coverage.

- [ ] **Step 1: Write**

```rust
//! End-to-end: every default `AppEventEmitter` helper bridges as expected.

use app_core::events::AppEventEmitter;
use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer, SocketBridgeEmitter};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

async fn collect_one(received: Arc<Mutex<Vec<BridgeFrame>>>) -> BridgeFrame {
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        {
            let g = received.lock().unwrap();
            if !g.is_empty() {
                return g[0].clone();
            }
        }
        if std::time::Instant::now() > deadline {
            panic!("no frame received within 500 ms");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn chat_thread_helper_bridges_as_chat_thread_updated() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("e2e1.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let server = BridgeServer::start(
        path.clone(),
        Box::new(move |f| r.lock().unwrap().push(f)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    // is_new = false → CHAT_THREAD_UPDATED
    emitter.emit_chat_thread(false, "session-abc");

    let frame = collect_one(received).await;
    assert_eq!(frame.event, "chat:thread_updated");
    assert_eq!(frame.payload, json!({ "sessionKey": "session-abc" }));
    server.shutdown();
}

#[tokio::test]
async fn arbitrary_emit_event_bridges_unchanged() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("e2e2.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let server = BridgeServer::start(
        path.clone(),
        Box::new(move |f| r.lock().unwrap().push(f)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    emitter.emit_event(
        "provider:degraded",
        json!({ "provider": "anthropic", "reason": "rate_limit" }),
    );

    let frame = collect_one(received).await;
    assert_eq!(frame.event, "provider:degraded");
    assert_eq!(
        frame.payload,
        json!({ "provider": "anthropic", "reason": "rate_limit" })
    );
    server.shutdown();
}
```

- [ ] **Step 2: Run**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo nextest run -p mcp-bridge --test end_to_end 2>&1 | tail -10
```

Expected: 2 passing tests.

- [ ] **Step 3: Commit**

```bash
git add crates/mcp-bridge/tests/end_to_end.rs
git commit -m "test(mcp-bridge): end-to-end coverage for chat_thread + generic emit_event"
```

---

### Task G2: Manual cross-process verification

**Files:** none.

`★ Insight ─────────────────────────────────────`
Plan 1 already wired `tauriEventBridge.ts` to listen for `entity:updated` and invalidate `qk.tasks.all()` etc. Plan 2 extended that to launcher / settings / etc. Combined with the new bridge: a klyntbot MCP tool call from Claude Code → MCP child emits → bridge socket → desktop server → Tauri global emit → every webview's TanStack cache invalidates → UI refreshes within ~50 ms. This task confirms the full chain manually.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Confirm no stale socket exists**

```bash
ls -la "${KLYNTBOT_HOME:-$HOME/.klyntbot}/mcp-events.sock" 2>&1
```

Expected: "No such file or directory" (the desktop hasn't run with the new code yet). If a stale socket is present from a previous run, remove it: `rm "${KLYNTBOT_HOME:-$HOME/.klyntbot}/mcp-events.sock"`.

- [ ] **Step 2: Run the desktop in dev mode**

Terminal 1:
```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run dev
```
Terminal 2:
```bash
cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev
```

In the desktop's stderr, expect a line:
```
mcp-bridge: listening at /Users/jayden/.klyntbot/mcp-events.sock
```

(or `~/.klyntbot-dev/mcp-events.sock` if `KLYNTBOT_HOME` is set per `CLAUDE.md`'s "Dev/prod isolation" note.)

- [ ] **Step 3: Verify the socket file exists**

```bash
ls -la "${KLYNTBOT_HOME:-$HOME/.klyntbot}/mcp-events.sock"
```

Expected: a `srwxr-xr-x` socket node, owned by your user.

- [ ] **Step 4: Open the tray + main windows**

Click the menu-bar icon (tray opens). Note the current "Today's tasks" count in the tray. The main window's task list should also be visible.

- [ ] **Step 5: From a separate terminal, drive the MCP server with a `tasks/create` call**

The exact JSON-RPC envelope depends on the MCP protocol version, but klyntbot expects MCP 2025 spec. A self-contained smoke test:

```bash
cd /Users/jayden/Projects/Klynt/bot && (
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tasks","arguments":{"action":"create","title":"bridge smoke test"}}}'
  sleep 1
) | cargo run --release -p desktop -- mcp serve --stdio 2>/tmp/mcp-stderr.log
```

Expected: a JSON response on stdout containing `"result"` for the `tools/call` request. `/tmp/mcp-stderr.log` should *not* contain "failed to bind" or "send dropped" with errors other than `connect refused` (which would only happen if the desktop crashed mid-test).

- [ ] **Step 6: Watch the desktop windows update without interaction**

Within ~50 ms of the MCP `tools/call` response, the new task **"bridge smoke test"** should appear in:
- The tray's task list (Plan 1's `qk.tasks.today()` was invalidated).
- The main window's task list.

Do **not** click anything. The update is the proof.

- [ ] **Step 7: Verify cleanup on desktop shutdown**

In the Tauri window: Cmd+Q. Then:

```bash
ls -la "${KLYNTBOT_HOME:-$HOME/.klyntbot}/mcp-events.sock" 2>&1
```

Expected: "No such file or directory" — `BridgeServerHandle::shutdown` (called from `Drop` on `BridgeServer`, indirectly through `OnceLock` drop at process exit) unlinked the socket file.

- [ ] **Step 8: Verify standalone MCP works without the desktop**

With the desktop closed (no socket file present):

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo run --release -p desktop -- mcp tools --list 2>&1 | head -20
```

Expected: a list of MCP tool names (`tasks`, `project`, `notes`, `memory`, `agent`, …). No errors, no panics, no warnings about the bridge — the `BridgeClient` quietly buffers nothing because `tools --list` doesn't emit any events.

For a stronger test that *does* emit, run the same `tools/call` RPC sequence from Step 5. The MCP child should still complete the task creation against the shared SQLite DB (per `CLAUDE.md`'s "MCP server" section: "shares ~/.klyntbot/data.db with the desktop process via SQLite WAL"). The bridge frames will simply be dropped after `connect timeout (200ms)` per `client.rs` — visible only at `RUST_LOG=trace`.

- [ ] **Step 9: Nothing to commit (manual test)** — proceed to Self-Review.

---

## Self-Review

(Run after writing the plan. Findings reported below; fixes applied inline before publishing.)

**1. Spec coverage:**

| Goal section | Tasks | Status |
|---|---|---|
| New crate `mcp-bridge` + workspace registration | A1 | ✓ |
| Wire-format protocol (`BridgeFrame` + 4-byte LE framing) | B1, B2 | ✓ |
| Non-blocking client with silent fallback | C1 | ✓ |
| Server with cancellable accept loop + socket cleanup | D1 | ✓ |
| `AppEventEmitter` impl bridging *all* events generically | E1 | ✓ |
| Desktop boot integration (re-emit via Tauri) | F1 | ✓ |
| MCP child injection via `init_with_sender` | F2 | ✓ |
| Cross-process e2e tests | D1 (3 tests), G1 (2 tests) | ✓ |
| Manual cross-window real-time verification | G2 | ✓ |
| Standalone-without-desktop fallback | G2 step 8 | ✓ |
| Socket cleanup on shutdown | D1 + G2 step 7 | ✓ |

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" / "add error handling" / "verify against actual code" / "similar to Task N" patterns. Every code block is concrete; every command is a real shell command with verifiable expected output.

**3. Type consistency:**
- `BridgeFrame { event: String, payload: serde_json::Value }` — used identically in B1 (definition), B2 (framing), C1 (`BridgeClient::send`), D1 (handler closure), E1 (`emit_event` → `BridgeFrame { event, payload }`), G1, G2.
- `FrameHandler = Box<dyn Fn(BridgeFrame) + Send + Sync + 'static>` — defined in D1, used in F1's handler closure (`mcp_bridge::server::FrameHandler`).
- `AppEventEmitter::emit_event(&self, event_name: &str, payload: serde_json::Value)` — sync signature matches verified anchor at `crates/app-core/src/events.rs:10`. Used in E1 (`SocketBridgeEmitter::emit_event`) and the desktop's existing `TauriEventEmitter` (unchanged).
- `init_with_sender(mode, config_override, Option<Arc<dyn NotificationSender>>, Option<Arc<dyn AppEventEmitter>>)` — signature matches verified anchor at `crates/app-core/src/init/mod.rs:87-92`. Used in F2.
- `EntityKind` serialization — verified `#[serde(rename_all = "camelCase")]` at `crates/desktop-shared/src/types.rs:48`. The E1 test asserts `"focusSession"` (camelCase) on the wire.
- `bridge_socket_path()` — same helper resolves both server (F1) and client (F2) paths to `${KLYNTBOT_HOME:-~/.klyntbot}/mcp-events.sock`. Verified `config::loader::config_dir()` exists at `crates/config/src/loader.rs:21-31`.

**4. Pattern adherence (per `CLAUDE.md`):**
- Conventional commits: every step uses `feat(scope): …`, `chore(scope): …`, `test(scope): …`. ✓
- Tests use ephemeral resources: `tempdir()` for sockets, never the real `~/.klyntbot`. ✓
- Dependency inversion: server takes a closure handler, not a `tauri::AppHandle`. ✓
- Errors: `FrameError` uses `thiserror` like `KlyntbotError`. ✓
- "Surgical changes": no unrelated edits to `app-core`, `bus`, `desktop-shared`. ✓

---

## Out-of-scope notes (Plans 2 and 4)

- **Plan 2** covers FE migration of launcher / distraction / settings / git / threads / composer to the new TanStack data layer. Independent of the bridge — Plan 3 makes events flow; Plan 2 makes more queries care.
- **Plan 4** covers Distiller domain-event emission and a `PRAGMA data_version` polling fallback for cases where the bridge isn't running (e.g. CLI tools that mutate the DB without going through any `AppEventEmitter`).
- The bridge is **one-way today** (MCP → desktop). Two-way (desktop → MCP, e.g. for the desktop pushing settings reloads to a long-running MCP child) is a future extension and would reuse the same `BridgeFrame` protocol.

---

## Definition of Done (Plan 3)

- `cargo build --workspace` clean, zero warnings.
- `cargo nextest run -p mcp-bridge` green: 7 unit tests in `protocol.rs` + 2 in `client_no_server.rs` + 3 in `protocol_roundtrip.rs` + 2 in `server_emits_to_handler.rs` + 1 in `emitter.rs` + 2 in `end_to_end.rs` = **17 tests**.
- `cargo nextest run --workspace` green (no regressions in upstream crates).
- `cargo clippy --workspace --all-targets --all-features` zero new warnings.
- `cargo fmt --all --check` clean.
- Manual end-to-end (G2) confirms an MCP `tasks/create` call updates the desktop tray and main windows within ~50 ms with no user interaction.
- With the desktop closed, `klyntbot mcp tools --list` and a full `tools/call` round-trip both succeed silently — no socket-related warnings on stderr at default log level.
- The socket file is unlinked when the desktop process exits (verified at `~/.klyntbot/mcp-events.sock`).
- All 17 task commits land on the working branch with conventional-commit messages.
