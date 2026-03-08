# Web Chat Backend Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the browser dev mode chat by adding `chat_send` + SSE streaming to the dev server backend.

**Architecture:** The frontend (`desktop-ui`) already supports browser mode — `useIpc.ts` falls back to HTTP, `useAgentStream.ts` connects to SSE at `/api/events/{sessionKey}`. We just need to implement the backend half: a `chat_send` dispatch handler and an SSE endpoint, both in `crates/desktop/src/dev_server.rs`.

**Tech Stack:** Rust, axum 0.8 (SSE via `axum::response::sse`), tokio broadcast channels, `AppEventEmitter` trait from `app-core`.

---

### Task 1: Add shared SSE state and `SseEmitter`

**Files:**
- Modify: `crates/desktop/src/dev_server.rs:1-23` (imports + state type)

**Step 1: Add SSE types and `SseEmitter` struct**

Add at the top of `dev_server.rs`, after existing imports:

```rust
use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use dashmap::DashMap;
use desktop_shared::commands::SessionContextInput;
use futures_util::stream::Stream;
use tokio::sync::broadcast;

/// Shared state: session_key → broadcast sender for SSE events.
type SseChannels = Arc<DashMap<String, broadcast::Sender<(String, Value)>>>;

/// Combined state for axum handlers.
#[derive(Clone)]
struct DevState {
    core: Arc<AppCore>,
    sse_channels: SseChannels,
}

/// Bridges `AppEventEmitter` to a tokio broadcast channel for SSE streaming.
struct SseEmitter {
    tx: broadcast::Sender<(String, Value)>,
}

impl app_core::events::AppEventEmitter for SseEmitter {
    fn emit_event(&self, event_name: &str, payload: Value) {
        let _ = self.tx.send((event_name.to_string(), payload));
    }
}
```

**Step 2: Update `start()` to use `DevState` and add SSE route**

Replace the `start` function signature and router setup:

```rust
pub async fn start(core: Arc<AppCore>) {
    let state = DevState {
        core,
        sse_channels: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/api/events/{sessionKey}", get(sse_handler))
        .route("/api/{cmd}", post(dispatch))
        .with_state(state);

    // ... rest of CORS + bind unchanged ...
}
```

Update `dispatch` signature from `State(core): State<AppState>` to `State(state): State<DevState>` and use `state.core` where `core` was used.

**Step 3: Commit**

```bash
git add crates/desktop/src/dev_server.rs
git commit -m "feat(desktop): add SSE state and SseEmitter to dev server"
```

---

### Task 2: Add `chat_send` to dispatch

**Files:**
- Modify: `crates/desktop/src/dev_server.rs` — the `dispatch` match block, around line 870

**Step 1: Add the `chat_send` case**

In the `dispatch` function, after the `"chat_cancel"` case (around line 876) and before `"chat_respond_interaction"`, add:

```rust
"chat_send" => {
    let content = match get_str(&body, "content") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let session_key = match get_str(&body, "sessionKey") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let context: Option<SessionContextInput> = get(&body, "context");

    let result = state.core.chat_send(content, session_key.clone(), context).await;
    match result {
        Ok((user_msg, stream_info)) => {
            // Create broadcast channel and spawn the relay
            let (tx, _) = broadcast::channel(256);
            state.sse_channels.insert(session_key, tx.clone());
            let emitter: Arc<dyn app_core::events::AppEventEmitter> =
                Arc::new(SseEmitter { tx });
            state.core.spawn_chat_relay(stream_info, emitter);
            ok(user_msg)
        }
        Err(e) => err(e),
    }
}
```

**Step 2: Update module doc comment**

Remove the line "Chat streaming (chat_send) is not supported here — use the Tauri app for that." from the module doc at line 7-8 since it's no longer true.

**Step 3: Commit**

```bash
git add crates/desktop/src/dev_server.rs
git commit -m "feat(desktop): wire chat_send with SSE broadcast in dev server"
```

---

### Task 3: Add SSE streaming endpoint

**Files:**
- Modify: `crates/desktop/src/dev_server.rs` — add `sse_handler` function

**Step 1: Implement the SSE handler**

Add after the `dispatch` function:

```rust
/// SSE endpoint — streams agent events for a chat session.
///
/// The frontend (`useAgentStream.ts`) connects here in browser dev mode
/// via `new EventSource("/api/events/{sessionKey}")`.
async fn sse_handler(
    State(state): State<DevState>,
    Path(session_key): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe to the broadcast channel (or create a dummy one if chat_send
    // hasn't been called yet — the stream will just wait).
    let rx = state
        .sse_channels
        .get(&session_key)
        .map(|entry| entry.value().subscribe())
        .unwrap_or_else(|| {
            let (tx, rx) = broadcast::channel(256);
            state.sse_channels.insert(session_key.clone(), tx);
            rx
        });

    let sse_channels = Arc::clone(&state.sse_channels);
    let sk = session_key.clone();

    let stream = async_stream::stream! {
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok((event_name, payload)) => {
                    let data = serde_json::to_string(&payload).unwrap_or_default();
                    let event = Event::default().event(&event_name).data(data);
                    let is_terminal = event_name == "agent:done" || event_name == "agent:error";
                    yield Ok(event);
                    if is_terminal {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE stream for {sk} lagged by {n} events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
        // Cleanup
        sse_channels.remove(&sk);
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

**Step 2: Add `async-stream` dependency**

Check if `async-stream` is already in the workspace. If not, add it to `crates/desktop/Cargo.toml`:

```toml
async-stream = "0.3"
```

Or alternatively, use `futures_util::stream::unfold` to avoid the new dependency — but `async_stream::stream!` is cleaner. If using `unfold`, the implementation wraps the recv loop in state.

**Step 3: Commit**

```bash
git add crates/desktop/src/dev_server.rs crates/desktop/Cargo.toml
git commit -m "feat(desktop): add SSE streaming endpoint for browser chat"
```

---

### Task 4: Add `entity:updated` to SSE event bridge

**Files:**
- Modify: `desktop-ui/src/hooks/useAgentStream.ts:28-46` — the `SSE_AGENT_EVENTS` array

**Step 1: Add the missing event**

The `relay_chat_stream` in `app-core` emits `entity:updated` events when tools mutate data, but `SSE_AGENT_EVENTS` doesn't include it. The `useEvent` listeners for `entity:updated` in components (Finance, Tasks, etc.) won't fire in browser mode without this.

Add `"entity:updated"` to the `SSE_AGENT_EVENTS` array:

```typescript
const SSE_AGENT_EVENTS = [
  "agent:content_chunk",
  "agent:tool_start",
  "agent:tool_end",
  "agent:done",
  "agent:error",
  "agent:classification_complete",
  "agent:execution_started",
  "agent:iteration_start",
  "agent:usage_report",
  "agent:memory_access",
  "agent:skill_loaded",
  "agent:learning_event",
  "agent:agent_selected",
  "agent:subagent_spawned",
  "agent:delegation_started",
  "agent:delegation_completed",
  "agent:interaction_request",
  "entity:updated",
] as const;
```

**Step 2: Commit**

```bash
git add desktop-ui/src/hooks/useAgentStream.ts
git commit -m "fix(desktop-ui): add entity:updated to SSE event bridge"
```

---

### Task 5: Build and smoke test

**Step 1: Verify it compiles**

```bash
cargo build -p desktop 2>&1
```

Expected: successful build with no errors.

**Step 2: Run clippy**

```bash
cargo clippy -p desktop --all-targets 2>&1
```

Expected: no warnings.

**Step 3: Run existing tests**

```bash
cargo nextest run --workspace 2>&1
```

Expected: all tests pass (no existing tests break).

**Step 4: Manual smoke test**

1. Start backend: `cargo run -p dev-api` (or `cargo tauri dev`)
2. Start frontend: `cd desktop-ui && bun run dev`
3. Open `http://localhost:1420` in Chrome
4. Open the Chat view
5. Send a message — should see streaming response with tool calls, transparency data, etc.

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat(desktop): complete browser dev mode chat streaming (R14)"
```
