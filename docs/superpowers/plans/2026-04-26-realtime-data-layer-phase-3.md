# Real-time Data Layer Phase 3 — MCP Cross-Process Entity Bridge

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Unix-socket bridge between the desktop process and any child `klyntbot mcp serve --stdio` process so that mutations made by external coding agents (Claude Code, Cursor) reach the desktop UI in real time. After this plan, when Claude Code calls a klyntbot MCP tool that mutates state, the corresponding `entity:updated` Tauri event fires in every desktop webview within ~10 ms.

**Architecture:** A new tiny crate `crates/ipc-bridge` defines a JSON-line protocol over `~/.klyntbot/entity-bridge.sock`. The desktop process boots a `tokio::net::UnixListener` that publishes incoming `BridgeMessage::EntityUpdated` events to its `DomainEventBus`, which `app_core.rs:303` already forwards to the `entity:updated` Tauri event. The MCP child injects a `SocketBridgeEmitter` (implementing `AppEventEmitter`) so its `emit_entity_updated` calls become socket writes instead of no-ops. When the desktop isn't running, the writer fails gracefully (silent no-op) — the MCP process continues to work standalone. Wire format: 4-byte little-endian length + JSON body, mirroring the existing `coding-ingest` protocol.

**Tech Stack:** Rust, `tokio::net::unix`, `serde_json`, existing `app-core::AppEventEmitter` trait, existing `desktop-shared::events::ENTITY_UPDATED`.

**Master plan context:** Plan 3 of 4. Depends on Plan 1 (FE foundation already routes `entity:updated` to invalidations). Independent of Plan 2. Plan 4 covers Distiller emission + `data_version` polling fallback.

---

## File Structure

### New files

| Path | Responsibility |
|---|---|
| `crates/ipc-bridge/Cargo.toml` | Crate manifest. |
| `crates/ipc-bridge/src/lib.rs` | Public API surface; re-exports. |
| `crates/ipc-bridge/src/protocol.rs` | `BridgeMessage` enum (`EntityUpdated`, `Heartbeat`); `read_frame` / `write_frame` framing helpers. |
| `crates/ipc-bridge/src/client.rs` | `BridgeClient` — write-only connection used by the MCP child. Silently no-ops when socket absent. |
| `crates/ipc-bridge/src/server.rs` | `BridgeServer` — `UnixListener` accept loop; publishes received messages to a provided `Arc<DomainEventBus>`. |
| `crates/ipc-bridge/src/emitter.rs` | `SocketBridgeEmitter` — implements `AppEventEmitter`; converts entity-updated calls to `BridgeClient::send`. |
| `crates/ipc-bridge/tests/roundtrip.rs` | Integration test: server + client share a socket; client sends; server publishes to bus; assertion. |

### Files to modify

| Path | Change |
|---|---|
| `Cargo.toml` (workspace) | Add `crates/ipc-bridge` to `members`. |
| `crates/desktop/Cargo.toml` | Add `ipc-bridge = { path = "../ipc-bridge" }`. |
| `crates/klyntbot-server/Cargo.toml` | Add `ipc-bridge = { path = "../ipc-bridge" }`. |
| `crates/desktop/src/main.rs` `run_mcp_stdio` | Inject `SocketBridgeEmitter` into the MCP `AppCore::init` call. |
| `crates/desktop/src/app_core.rs` | Spawn `BridgeServer` during desktop startup. |
| `crates/app-core/src/init/mod.rs` | Accept `Option<Arc<dyn AppEventEmitter>>` in `init_with_sender` (likely already does — verify; pass through). |

---

## Phase A — Crate scaffold

### Task A1: Create the crate

**Files:**
- Create: `crates/ipc-bridge/Cargo.toml`
- Create: `crates/ipc-bridge/src/lib.rs`
- Modify: `Cargo.toml` (workspace members list)

- [ ] **Step 1: Create crate directory + manifest**

```bash
mkdir -p /Users/jayden/Projects/Klynt/bot/crates/ipc-bridge/src
mkdir -p /Users/jayden/Projects/Klynt/bot/crates/ipc-bridge/tests
```

Create `crates/ipc-bridge/Cargo.toml`:

```toml
[package]
name = "ipc-bridge"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
tokio = { workspace = true, features = ["net", "io-util", "sync", "rt-multi-thread", "macros"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tracing = { workspace = true }
common = { path = "../common" }
bus = { path = "../bus" }

[target.'cfg(unix)'.dependencies]
# nothing extra — tokio::net::unix is gated by cfg(unix) automatically

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "time"] }
tempfile = { workspace = true }
```

- [ ] **Step 2: Stub `src/lib.rs`**

Create `crates/ipc-bridge/src/lib.rs`:

```rust
//! Cross-process entity-update bridge.
//!
//! See `docs/superpowers/plans/2026-04-26-realtime-data-layer-phase-3.md`
//! for design rationale and protocol details.

pub mod client;
pub mod emitter;
pub mod protocol;
pub mod server;

pub use client::BridgeClient;
pub use emitter::SocketBridgeEmitter;
pub use protocol::{BridgeMessage, FrameError};
pub use server::BridgeServer;
```

- [ ] **Step 3: Add to workspace members**

Edit root `Cargo.toml` — add `"crates/ipc-bridge"` to the `members` array (alphabetical position is fine).

- [ ] **Step 4: Verify it compiles (with empty modules — they exist as files in next tasks)**

For now create empty placeholder files so the lib compiles:

```bash
touch crates/ipc-bridge/src/{client,emitter,protocol,server}.rs
```

```bash
cargo build -p ipc-bridge 2>&1 | tail -10
```

Expected: ok or "no public items" (acceptable).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ipc-bridge
git commit -m "chore: scaffold ipc-bridge crate"
```

---

## Phase B — Protocol + framing

### Task B1: Define `BridgeMessage`

**Files:**
- Modify: `crates/ipc-bridge/src/protocol.rs`

- [ ] **Step 1: Write the failing test first**

Create `crates/ipc-bridge/src/protocol.rs` with a placeholder + module test:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeMessage {
    EntityUpdated { entity_kind: String, id: String },
    Heartbeat,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame too large: {0}")]
    TooLarge(u32),
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_roundtrips_as_json() {
        let m = BridgeMessage::EntityUpdated {
            entity_kind: "task".into(),
            id: "t1".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains(r#""type":"entity_updated""#));
        let m2: BridgeMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn heartbeat_serializes() {
        let m = BridgeMessage::Heartbeat;
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"type":"heartbeat"}"#);
    }
}
```

Add `thiserror` to `Cargo.toml` if not already in workspace deps.

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p ipc-bridge --lib 2>&1 | tail -20
```

Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ipc-bridge/src/protocol.rs crates/ipc-bridge/Cargo.toml
git commit -m "feat(ipc-bridge): define BridgeMessage type"
```

### Task B2: Add framing helpers

**Files:**
- Modify: `crates/ipc-bridge/src/protocol.rs`

The wire is 4-byte LE length-prefix + JSON body, mirroring `coding-ingest::transport` (`crates/coding-ingest/src/transport.rs:53-88`).

- [ ] **Step 1: Add framing tests**

Append to `protocol.rs`:

```rust
#[cfg(test)]
mod framing_tests {
    use super::*;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn write_then_read_roundtrips() {
        let (mut a, mut b) = duplex(1024);
        let m = BridgeMessage::EntityUpdated {
            entity_kind: "note".into(),
            id: "n42".into(),
        };
        write_frame(&mut a, &m).await.unwrap();
        a.shutdown().await.unwrap();
        let received = read_frame(&mut b).await.unwrap();
        assert_eq!(received, Some(m));
    }

    #[tokio::test]
    async fn read_returns_none_on_clean_eof() {
        let (mut a, mut b) = duplex(1024);
        a.shutdown().await.unwrap();
        drop(a);
        let received = read_frame(&mut b).await.unwrap();
        assert_eq!(received, None);
    }

    #[tokio::test]
    async fn frame_too_large_errors() {
        let (mut a, mut b) = duplex(8);
        // Write a fake length prefix > 1 MB
        let bad = (10_000_000u32).to_le_bytes();
        a.write_all(&bad).await.unwrap();
        let res = read_frame(&mut b).await;
        assert!(matches!(res, Err(FrameError::TooLarge(_))));
    }
}
```

- [ ] **Step 2: Run tests — expect failure (helpers don't exist)**

```bash
cargo nextest run -p ipc-bridge --lib 2>&1 | tail -10
```

- [ ] **Step 3: Implement framing helpers**

Append to `protocol.rs`:

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_FRAME: u32 = 1 << 20; // 1 MB

pub async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    msg: &BridgeMessage,
) -> Result<(), FrameError> {
    let body = serde_json::to_vec(msg)?;
    let len: u32 = body
        .len()
        .try_into()
        .map_err(|_| FrameError::TooLarge(u32::MAX))?;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&body).await?;
    Ok(())
}

pub async fn read_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<BridgeMessage>, FrameError> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(FrameError::Io(e)),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body).await?;
    let msg = serde_json::from_slice(&body)?;
    Ok(Some(msg))
}
```

- [ ] **Step 4: Run tests — green**

```bash
cargo nextest run -p ipc-bridge --lib 2>&1 | tail -10
```

Expected: 5 tests pass total.

- [ ] **Step 5: Commit**

```bash
git add crates/ipc-bridge/src/protocol.rs
git commit -m "feat(ipc-bridge): add length-prefix framing helpers"
```

---

## Phase C — Client

### Task C1: `BridgeClient::send`

**Files:**
- Modify: `crates/ipc-bridge/src/client.rs`

The client is **write-only** and **silently no-ops** when the socket is missing. This preserves "MCP standalone without desktop" semantics.

- [ ] **Step 1: Write the failing test**

Create `crates/ipc-bridge/tests/client_no_socket.rs`:

```rust
use ipc_bridge::{BridgeClient, BridgeMessage};
use std::path::PathBuf;

#[tokio::test]
async fn send_to_missing_socket_is_silent_noop() {
    let path = PathBuf::from("/tmp/klynt-bridge-does-not-exist.sock");
    let client = BridgeClient::new(path);
    // Should NOT panic, should NOT block
    client
        .send(BridgeMessage::EntityUpdated {
            entity_kind: "task".into(),
            id: "x".into(),
        })
        .await;
}
```

- [ ] **Step 2: Run test — fails because BridgeClient doesn't exist**

```bash
cargo nextest run -p ipc-bridge --test client_no_socket 2>&1 | tail -10
```

- [ ] **Step 3: Implement client**

Replace `crates/ipc-bridge/src/client.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;
use tracing::trace;

use crate::protocol::{write_frame, BridgeMessage};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);
const WRITE_TIMEOUT: Duration = Duration::from_millis(200);

/// Write-only client. Designed for the MCP child process: when the desktop
/// isn't running (socket absent) every send is a silent no-op.
#[derive(Clone)]
pub struct BridgeClient {
    socket_path: PathBuf,
}

impl BridgeClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub async fn send(&self, msg: BridgeMessage) {
        if let Err(e) = self.send_inner(msg).await {
            trace!(error = %e, "ipc-bridge: send dropped");
        }
    }

    async fn send_inner(
        &self,
        msg: BridgeMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = timeout(CONNECT_TIMEOUT, UnixStream::connect(&self.socket_path))
            .await
            .map_err(|_| "connect timeout")??;
        timeout(WRITE_TIMEOUT, write_frame(&mut stream, &msg))
            .await
            .map_err(|_| "write timeout")??;
        Ok(())
    }
}
```

- [ ] **Step 4: Run test — green**

```bash
cargo nextest run -p ipc-bridge --test client_no_socket 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/ipc-bridge/src/client.rs crates/ipc-bridge/tests/client_no_socket.rs
git commit -m "feat(ipc-bridge): BridgeClient with silent no-op fallback"
```

---

## Phase D — Server

### Task D1: `BridgeServer::start`

**Files:**
- Modify: `crates/ipc-bridge/src/server.rs`

- [ ] **Step 1: Write the failing roundtrip test**

Create `crates/ipc-bridge/tests/roundtrip.rs`:

```rust
use bus::DomainEventBus;
use ipc_bridge::{BridgeClient, BridgeMessage, BridgeServer};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn end_to_end_roundtrip() {
    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("bridge.sock");
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();

    let server = BridgeServer::start(socket_path.clone(), bus.clone()).await.unwrap();

    // Give listener a tick to bind
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(socket_path);
    client
        .send(BridgeMessage::EntityUpdated {
            entity_kind: "task".into(),
            id: "t42".into(),
        })
        .await;

    let received = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("bus closed");
    // Verify the event maps to a domain event we can use
    assert_eq!(received.domain(), "entity_updated"); // or whatever domain() returns

    server.shutdown();
}
```

(NOTE: the exact `DomainEvent` variant + `domain()` return depends on how `bus::DomainEvent` is structured. The test asserts the rough shape; refine after reading `crates/bus/src/lib.rs`.)

- [ ] **Step 2: Run — fails because server doesn't exist**

```bash
cargo nextest run -p ipc-bridge --test roundtrip 2>&1 | tail -15
```

- [ ] **Step 3: Implement server**

Replace `crates/ipc-bridge/src/server.rs`:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use bus::{DomainEvent, DomainEventBus};
use tokio::net::UnixListener;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::protocol::{read_frame, BridgeMessage};

pub struct BridgeServer {
    shutdown: CancellationToken,
    socket_path: PathBuf,
}

impl BridgeServer {
    /// Bind the socket, spawn the accept loop, return a handle.
    pub async fn start(
        socket_path: PathBuf,
        bus: Arc<DomainEventBus>,
    ) -> std::io::Result<Self> {
        // Remove a stale socket file from a previous run.
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)?;
        let shutdown = CancellationToken::new();
        Self::spawn_accept_loop(listener, bus, shutdown.clone());
        Ok(Self {
            shutdown,
            socket_path,
        })
    }

    fn spawn_accept_loop(
        listener: UnixListener,
        bus: Arc<DomainEventBus>,
        shutdown: CancellationToken,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    res = listener.accept() => {
                        match res {
                            Ok((stream, _addr)) => {
                                let bus = bus.clone();
                                tokio::spawn(handle_connection(stream, bus));
                            }
                            Err(e) => {
                                error!("ipc-bridge: accept error: {e}");
                                break;
                            }
                        }
                    }
                }
            }
            debug!("ipc-bridge: accept loop exited");
        });
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

async fn handle_connection(mut stream: tokio::net::UnixStream, bus: Arc<DomainEventBus>) {
    loop {
        match read_frame(&mut stream).await {
            Ok(Some(msg)) => {
                if let Some(event) = bridge_to_domain(msg) {
                    bus.publish(event);
                }
            }
            Ok(None) => break, // clean EOF
            Err(e) => {
                warn!("ipc-bridge: frame error: {e}");
                break;
            }
        }
    }
}

fn bridge_to_domain(msg: BridgeMessage) -> Option<DomainEvent> {
    match msg {
        BridgeMessage::EntityUpdated { entity_kind, id } => {
            // The DomainEvent enum is defined in `bus`. The exact variant name
            // is "EntityUpdated" — verify against bus::DomainEvent definition.
            Some(DomainEvent::EntityUpdated { entity_kind, id })
        }
        BridgeMessage::Heartbeat => None,
    }
}
```

(NOTE: `DomainEvent::EntityUpdated` may need to be added to the bus crate if it doesn't exist. If so, that's a sub-task here — see Step 4.)

- [ ] **Step 4: Confirm `DomainEvent::EntityUpdated` exists; add if missing**

```bash
grep -nE "EntityUpdated|entity_updated" /Users/jayden/Projects/Klynt/bot/crates/bus/src/*.rs
```

If missing, add the variant. The variant should resemble:

```rust
pub enum DomainEvent {
    // ... existing
    EntityUpdated { entity_kind: String, id: String },
}
```

And `domain()` should return `"entity_updated"`. Update the existing `app_core.rs:303` forwarder to recognize it (likely already does via the `variant_name()` pattern).

- [ ] **Step 5: Run roundtrip test — green**

```bash
cargo nextest run -p ipc-bridge --test roundtrip 2>&1 | tail -10
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ipc-bridge/src/server.rs crates/ipc-bridge/tests/roundtrip.rs crates/bus/src
git commit -m "feat(ipc-bridge): BridgeServer + DomainEvent::EntityUpdated bus variant"
```

---

## Phase E — Emitter integration

### Task E1: `SocketBridgeEmitter`

**Files:**
- Modify: `crates/ipc-bridge/src/emitter.rs`

The emitter implements the existing `app_core::AppEventEmitter` trait so it drops in wherever `NoopEmitter` is currently used.

- [ ] **Step 1: Inspect the trait**

```bash
grep -nE "trait AppEventEmitter|emit_entity_updated|fn emit" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/events.rs | head -20
```

Note the exact method signatures.

- [ ] **Step 2: Implement**

Replace `crates/ipc-bridge/src/emitter.rs`:

```rust
use std::sync::Arc;
use app_core::events::AppEventEmitter;
use common::types::EntityKind;

use crate::client::BridgeClient;
use crate::protocol::BridgeMessage;

/// AppEventEmitter that forwards entity-update events to the desktop process
/// via the bridge socket. Used in `klyntbot mcp serve --stdio` so external
/// MCP clients' mutations propagate to running webviews.
pub struct SocketBridgeEmitter {
    client: BridgeClient,
}

impl SocketBridgeEmitter {
    pub fn new(client: BridgeClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl AppEventEmitter for SocketBridgeEmitter {
    async fn emit_entity_updated(&self, entity_kind: EntityKind, id: &str) {
        let msg = BridgeMessage::EntityUpdated {
            entity_kind: entity_kind.as_camel_case().to_string(),
            id: id.to_string(),
        };
        self.client.send(msg).await;
    }

    // Implement other trait methods as no-ops, since only entity_updated is
    // wired across the bridge today. (Verify trait method list before commit.)
    async fn emit_event(&self, _event: &str, _payload: serde_json::Value) {}
}
```

(`EntityKind::as_camel_case()` may need to be added — it should produce the same string as `serde_json::to_string(&kind)` strips quotes. Verify against `crates/desktop-shared/src/types.rs:48`.)

- [ ] **Step 3: Add a unit test**

Append to `crates/ipc-bridge/src/emitter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emit_to_missing_socket_is_silent() {
        let client = BridgeClient::new("/tmp/klynt-emitter-no-socket.sock".into());
        let emitter = SocketBridgeEmitter::new(client);
        // Must not panic
        emitter
            .emit_entity_updated(EntityKind::Task, "t1")
            .await;
    }
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p ipc-bridge 2>&1 | tail -10
git add crates/ipc-bridge/src/emitter.rs
git commit -m "feat(ipc-bridge): SocketBridgeEmitter implementing AppEventEmitter"
```

---

## Phase F — Wire into desktop + MCP processes

### Task F1: Spawn `BridgeServer` in the desktop boot

**Files:**
- Modify: `crates/desktop/src/app_core.rs`
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Add the dep**

In `crates/desktop/Cargo.toml`, under `[dependencies]`:

```toml
ipc-bridge = { path = "../ipc-bridge" }
```

- [ ] **Step 2: Spawn the server during desktop init**

In `crates/desktop/src/app_core.rs`, near where `domain_event_bus.subscribe()` happens (~line 303):

```rust
// Cross-process entity bridge — receives entity-update messages from MCP
// child processes and republishes them to the local DomainEventBus, which
// app_core's existing forwarder converts into Tauri `entity:updated` events.
let bridge_socket_path = config_dir.join("entity-bridge.sock");
let bridge_server = ipc_bridge::BridgeServer::start(
    bridge_socket_path,
    channels.domain_event_bus.clone(),
)
.await
.map_err(|e| {
    tracing::warn!("ipc-bridge: failed to bind socket: {e}");
    e
})
.ok();
// Stash on AppCore so it lives as long as the process.
self.bridge_server = bridge_server;
```

(`config_dir` here is whatever path is already used for `~/.klyntbot/`. `AppCore` needs a new field `bridge_server: Option<ipc_bridge::BridgeServer>`.)

- [ ] **Step 3: Add the field to `AppCore`**

```bash
grep -n "pub struct AppCore" /Users/jayden/Projects/Klynt/bot/crates/desktop/src/app_core.rs
```

Add `bridge_server: Option<ipc_bridge::BridgeServer>` field and initialize to `None` in the `new` constructor.

- [ ] **Step 4: Make `app_core.rs:321` forwarder route `EntityUpdated` events**

The forwarder currently emits `cognitive:domain_event` for every variant. Add explicit handling for `DomainEvent::EntityUpdated`:

```rust
match &event {
    DomainEvent::EntityUpdated { entity_kind, id } => {
        let payload = desktop_shared::events::EntityUpdatedPayload {
            entity_kind: common::types::EntityKind::parse(entity_kind)
                .unwrap_or(common::types::EntityKind::Task),
            id: id.clone(),
        };
        let _ = app_handle_clone.emit(
            desktop_shared::events::ENTITY_UPDATED,
            &payload,
        );
    }
    _ => {}
}
```

(Place this BEFORE the existing `cognitive:domain_event` emit so both fire.)

- [ ] **Step 5: Build + smoke**

```bash
cargo build -p desktop 2>&1 | tail -10
```

Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/app_core.rs
git commit -m "feat(desktop): start ipc-bridge server + forward EntityUpdated to Tauri"
```

### Task F2: Inject `SocketBridgeEmitter` in `run_mcp_stdio`

**Files:**
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/klyntbot-server/src/lib.rs` (if needed)

The MCP child currently passes `event_emitter: None` (`crates/desktop/src/main.rs:165` ish). Replace with a `SocketBridgeEmitter`.

- [ ] **Step 1: Construct the emitter in `run_mcp_stdio`**

In `crates/desktop/src/main.rs`, inside `run_mcp_stdio`, before `AppCore::init`:

```rust
let bridge_socket_path = std::path::PathBuf::from(
    std::env::var("KLYNTBOT_HOME")
        .unwrap_or_else(|_| {
            format!("{}/.klyntbot", std::env::var("HOME").unwrap_or_default())
        }),
)
.join("entity-bridge.sock");
let bridge_client = ipc_bridge::BridgeClient::new(bridge_socket_path);
let event_emitter: Arc<dyn AppEventEmitter> =
    Arc::new(ipc_bridge::SocketBridgeEmitter::new(bridge_client));
```

- [ ] **Step 2: Pass it into `AppCore::init`**

The current call is `AppCore::init(common::AppMode::Server, Some(config))`. The signature must accept an emitter — verify and update the call:

```rust
let (app, events) = app_core::AppCore::init_with_emitter(
    common::AppMode::Server,
    Some(config),
    Some(event_emitter),
)
.await
.expect("init failed");
```

If `init_with_emitter` doesn't exist, modify `crates/app-core/src/init/mod.rs:init_with_sender` to accept the emitter (verify whether it already does — the explorer report suggested it does, just defaulted to `None`).

- [ ] **Step 3: Add `Cargo.toml` dep**

In `crates/desktop/Cargo.toml`, ensure `ipc-bridge` is listed for the binary's path. (Already added in Task F1.)

- [ ] **Step 4: Build**

```bash
cargo build -p desktop 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/main.rs crates/app-core/src/init/mod.rs
git commit -m "feat(desktop): wire SocketBridgeEmitter into MCP child startup"
```

---

## Phase G — End-to-end verification

### Task G1: Cross-process integration test

**Files:**
- Create: `crates/ipc-bridge/tests/cross_process.rs`

This test spawns the bridge server inside the test process, connects a client from the same process, sends an entity update, and asserts the bus event fires. (A truly cross-process test requires forking a child binary; we cover that manually.)

- [ ] **Step 1: Write**

```rust
use bus::{DomainEvent, DomainEventBus};
use ipc_bridge::{BridgeClient, BridgeMessage, BridgeServer};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn entity_updated_propagates_to_bus() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("e2e.sock");
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let server = BridgeServer::start(path.clone(), bus.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(path);
    client
        .send(BridgeMessage::EntityUpdated {
            entity_kind: "task".into(),
            id: "abc".into(),
        })
        .await;

    let evt = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("timeout")
        .expect("bus closed");
    match evt {
        DomainEvent::EntityUpdated { entity_kind, id } => {
            assert_eq!(entity_kind, "task");
            assert_eq!(id, "abc");
        }
        _ => panic!("unexpected event: {evt:?}"),
    }
    server.shutdown();
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p ipc-bridge --test cross_process 2>&1 | tail -10
git add crates/ipc-bridge/tests/cross_process.rs
git commit -m "test(ipc-bridge): cross-process entity-updated round-trip"
```

### Task G2: Manual end-to-end test

**Files:** none.

- [ ] **Step 1: Run the desktop**

```bash
cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev
```

- [ ] **Step 2: From a separate terminal, run the MCP server in stdio mode and pipe a tool call**

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tasks","arguments":{"action":"create","title":"bridge test"}}}' | cargo run -p desktop -- mcp serve --stdio
```

(Adjust to match the actual MCP request format.)

- [ ] **Step 3: Watch the desktop's tray window**

Expected: a new task appears in the tray within ~50 ms of the MCP call completing — no manual refresh.

- [ ] **Step 4: Verify the socket cleanup**

After Cmd+Q on the desktop:

```bash
ls -la ~/.klyntbot/entity-bridge.sock
```

Expected: file does not exist (cleaned up by `BridgeServer::shutdown`).

- [ ] **Step 5: Commit nothing (manual test)**

---

## Self-Review

**1. Spec coverage:**
- Crate scaffold + protocol + framing → A1, B1, B2 ✓
- Client (no-op fallback) → C1 ✓
- Server (UnixListener + bus publish) → D1 ✓
- Emitter implementing AppEventEmitter → E1 ✓
- Wiring into desktop + MCP startup → F1, F2 ✓
- Cross-process verification → G1, G2 ✓
- Cleanup contract (socket removal) → D1 + G2 ✓
- Standalone fallback (no desktop) → C1 ✓

**2. Placeholder scan:** A few "verify against the trait" / "exact variant name" lines — these are explicit instructions to confirm against actual code, not placeholders to fill in later. Acceptable.

**3. Type consistency:** `BridgeMessage::EntityUpdated { entity_kind, id }` matches `DomainEvent::EntityUpdated { entity_kind, id }` matches `EntityUpdatedPayload { entityKind, id }` (camelCase via serde) — same shape across the three layers.

---

## Definition of Done (Plan 3)

- `cargo nextest run -p ipc-bridge` green (5+ unit tests, 2+ integration tests).
- `cargo build --workspace` clean.
- Manual end-to-end (G2) confirms cross-process invalidation works.
- MCP standalone (no desktop) confirmed via `cargo run -p desktop -- mcp tools --list` with no `entity-bridge.sock` present — should succeed silently.
- Socket file cleanly removed on desktop shutdown.
