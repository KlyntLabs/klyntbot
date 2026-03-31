# Real-Time Event Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the chat page (and all feature pages) update in real-time when mutations happen from any source — MCP, voice, cron, external channels, or direct UI.

**Architecture:** Backend emits lightweight notification events (`chat:thread_created`, `chat:thread_updated`, `chat:message_added`, and existing `entity:updated`) via `AppEventEmitter` whenever data is written. Frontend `useQuery` gains an `invalidateOn` option that listens for these events and auto-refetches from local SQLite (<1ms). No new transport — Tauri IPC (desktop) and SSE (browser dev mode) already carry all events.

**Tech Stack:** Rust (desktop-shared, app-core), TypeScript/React (desktop-ui), existing Tauri event system + SSE

**Spec:** `docs/superpowers/specs/2026-03-31-realtime-event-layer-design.md`

---

## File Structure

### Backend (Rust) — Files to Modify

| File | Responsibility | Change |
|---|---|---|
| `crates/desktop-shared/src/events.rs` | Event constants and payload structs | Add 3 constants + 2 payload structs |
| `crates/app-core/src/handlers/chat/streaming.rs` | Chat send, relay, interaction | Emit `chat:thread_created`/`updated` after upsert, `chat:message_added` on Done |
| `crates/app-core/src/handlers/chat/threads.rs` | Thread rename/delete/pin | Emit `chat:thread_updated` from AppCore methods |

### Frontend (TypeScript) — Files to Modify

| File | Responsibility | Change |
|---|---|---|
| `desktop-ui/src/shared/hooks/useQuery.ts` | SWR cache + invalidation | Add `invalidateOn`/`invalidateFilter` options |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Chat page component | Wire `invalidateOn` to thread query |
| `desktop-ui/src/features/chat/hooks/useChatSession.ts` | Chat session hook | Wire `invalidateOn` to message query |
| `desktop-ui/src/app/BrainEventBridge.tsx` | SSE → CustomEvent bridge | Add 3 new event names |

---

## Task 1: Add Event Constants and Payload Structs

**Files:**
- Modify: `crates/desktop-shared/src/events.rs`

- [ ] **Step 1: Add chat event constants**

In `crates/desktop-shared/src/events.rs`, add the three new event constants after `ENTITY_UPDATED`:

```rust
pub const CHAT_THREAD_CREATED: &str = "chat:thread_created";
pub const CHAT_THREAD_UPDATED: &str = "chat:thread_updated";
pub const CHAT_MESSAGE_ADDED: &str = "chat:message_added";
```

- [ ] **Step 2: Add payload structs**

In the same file, after the `EntityUpdatedPayload` struct, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadPayload {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagePayload {
    pub session_key: String,
    /// Source that produced the message (e.g., "chat", "voice", "mcp", "cron").
    pub source: String,
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p desktop-shared`
Expected: Compiles with 0 errors.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/events.rs
git commit -m "feat(events): add chat real-time event constants and payloads"
```

---

## Task 2: Emit Events from `chat_send` (Free Function + AppCore)

The free function `chat_send()` doesn't have access to the event emitter. The `AppCore::chat_send()` convenience method does via `self.event_emitter`. Strategy: modify the free function to return whether the session was newly created, then emit in the `AppCore` method.

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Modify `chat_send` free function to return `is_new_session`**

The `upsert_session()` returns a `SessionRow` with `created_at` and `updated_at`. When both are equal, the session was just created. Change the return type to include this info.

Find the struct `ChatStreamInfo` definition and add a field:

```rust
pub struct ChatStreamInfo {
    pub session_key: String,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    pub has_context: bool,
    /// True when the session was just created (not a follow-up message in an existing thread).
    pub is_new_session: bool,
}
```

In the `chat_send` free function, after the `upsert_session()` call at line ~787, capture the returned row:

```rust
    let session_row = repos
        .sessions
        .upsert_session(&session_key, &metadata, squad_id_ref)
        .await
        .map_err(map_storage_err)?;
    let is_new_session = session_row.created_at == session_row.updated_at;
```

Update the `ChatStreamInfo` construction at the bottom of the function to include the new field:

```rust
    let stream_info = ChatStreamInfo {
        session_key,
        event_rx: streaming_handle.event_rx,
        interaction_rx: streaming_handle.interaction_rx,
        has_context,
        is_new_session,
    };
```

- [ ] **Step 2: Emit thread event from `AppCore::chat_send`**

In `AppCore::chat_send()` (the convenience method around line 1582), after the result is obtained but before returning, emit the thread event:

```rust
    // Emit real-time chat event for UI
    let event_name = if result.1.is_new_session {
        events::CHAT_THREAD_CREATED
    } else {
        events::CHAT_THREAD_UPDATED
    };
    if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
        session_key: result.1.session_key.clone(),
    }) {
        self.event_emitter.emit_event(event_name, payload);
    }
```

Make sure `events` is imported at the top of the file. It already is: `use desktop_shared::events::{self, *};`

- [ ] **Step 3: Do the same for `AppCore::chat_send_voice`**

In `AppCore::chat_send_voice()` (around line 1635), add the same emit block after the `chat_send` call returns:

```rust
    let event_name = if result.1.is_new_session {
        events::CHAT_THREAD_CREATED
    } else {
        events::CHAT_THREAD_UPDATED
    };
    if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
        session_key: result.1.session_key.clone(),
    }) {
        self.event_emitter.emit_event(event_name, payload);
    }
```

- [ ] **Step 4: Do the same for `AppCore::chat_send_squad`**

In `chat_send_squad()` (around line 1664), after `upsert_session()` returns, detect new vs. existing and emit. Since this is a method on `AppCore` (not a free function), it has direct access to `self.event_emitter`:

After the existing `upsert_session()` call at line ~1679:

```rust
    let session_row = self.repos
        .sessions
        .upsert_session(&session_key, &metadata, Some(&squad_id))
        .await
        .map_err(map_storage_err)?;

    // Emit real-time thread event
    let thread_event = if session_row.created_at == session_row.updated_at {
        events::CHAT_THREAD_CREATED
    } else {
        events::CHAT_THREAD_UPDATED
    };
    if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
        session_key: session_key.clone(),
    }) {
        self.event_emitter.emit_event(thread_event, payload);
    }
```

Note: the existing code discards the return from `upsert_session` (it only calls `.await?`). You need to capture the returned `SessionRow` as `session_row`.

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p app-core`
Expected: Compiles with 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(chat): emit thread_created/thread_updated events on chat_send"
```

---

## Task 3: Emit `chat:message_added` from `relay_chat_stream`

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Emit `chat:message_added` after the `agent:done` event**

In `relay_chat_stream()`, right after the `emit!(AGENT_DONE, ...)` call (around line 1157-1163), and before the `break`, add:

```rust
                        // Notify frontend that a new message was added to this session
                        if let Ok(payload) = serde_json::to_value(ChatMessagePayload {
                            session_key: sk.clone(),
                            source: "chat".to_string(),
                        }) {
                            emitter.emit_event(CHAT_MESSAGE_ADDED, payload);
                        }
```

Make sure `ChatMessagePayload` and `CHAT_MESSAGE_ADDED` are available. They should be since the file already imports `use desktop_shared::events::{self, *};`.

- [ ] **Step 2: Emit `chat:message_added` from `chat_respond_interaction`**

In `chat_respond_interaction()`, after the `add_message` call succeeds (around line 906), add:

This is a free function without emitter access. We need to add the emitter as a parameter.

Update the signature:

```rust
pub async fn chat_respond_interaction(
    repos: &Repos,
    pending_interactions: &PendingInteractions,
    emitter: &dyn crate::events::AppEventEmitter,
    session_key: String,
    request_id: String,
    response: common::FormResponse,
) -> Result<(), ApiError> {
```

After the `add_message` call (after line 906), add:

```rust
    // Notify frontend of the new interaction message
    if let Ok(payload) = serde_json::to_value(events::ChatMessagePayload {
        session_key: session_key.clone(),
        source: "chat".to_string(),
    }) {
        emitter.emit_event(events::CHAT_MESSAGE_ADDED, payload);
    }
```

Update the `AppCore::chat_respond_interaction` convenience method to pass the emitter:

```rust
    pub async fn chat_respond_interaction(
        &self,
        session_key: String,
        request_id: String,
        response: common::FormResponse,
    ) -> Result<(), ApiError> {
        chat_respond_interaction(
            &self.repos,
            &self.pending_interactions,
            self.event_emitter.as_ref(),
            session_key,
            request_id,
            response,
        )
        .await
    }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p app-core`
Expected: Compiles with 0 errors.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(chat): emit chat:message_added on agent done and interaction response"
```

---

## Task 4: Emit Events from Thread Management Handlers

Thread operations (rename, delete, pin) are free functions in `threads.rs`. The `AppCore` convenience methods call them and have access to `self.event_emitter`. Emit from the AppCore methods after the free function call returns.

**Files:**
- Modify: `crates/app-core/src/handlers/chat/threads.rs`

- [ ] **Step 1: Add event import**

The file already imports `use desktop_shared::events;` but ensure the new constants and payload types are accessible. Since the file uses `events::` prefix, no changes needed — the wildcard re-export from `events.rs` covers them.

- [ ] **Step 2: Emit `chat:thread_updated` from `AppCore::chat_rename_thread`**

```rust
    pub async fn chat_rename_thread(
        &self,
        session_key: String,
        title: String,
    ) -> Result<(), ApiError> {
        chat_rename_thread(&self.repos, session_key.clone(), title).await?;
        if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
            session_key,
        }) {
            self.event_emitter.emit_event(events::CHAT_THREAD_UPDATED, payload);
        }
        Ok(())
    }
```

Note: the current code passes `session_key` by value. We need to clone it before passing so we still have it for the event. Update the call to `chat_rename_thread(&self.repos, session_key.clone(), title)`.

- [ ] **Step 3: Emit `chat:thread_updated` from `AppCore::chat_delete_thread`**

```rust
    pub async fn chat_delete_thread(&self, session_key: String) -> Result<(), ApiError> {
        chat_delete_thread(
            &self.repos,
            &self.active_streams,
            &self.pending_interactions,
            session_key.clone(),
        )
        .await?;
        if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
            session_key,
        }) {
            self.event_emitter.emit_event(events::CHAT_THREAD_UPDATED, payload);
        }
        Ok(())
    }
```

- [ ] **Step 4: Emit `chat:thread_updated` from `AppCore::chat_pin_thread`**

```rust
    pub async fn chat_pin_thread(&self, session_key: String) -> Result<(), ApiError> {
        chat_pin_thread(&self.repos, session_key.clone()).await?;
        if let Ok(payload) = serde_json::to_value(events::ChatThreadPayload {
            session_key,
        }) {
            self.event_emitter.emit_event(events::CHAT_THREAD_UPDATED, payload);
        }
        Ok(())
    }
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p app-core`
Expected: Compiles with 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/chat/threads.rs
git commit -m "feat(chat): emit chat:thread_updated on rename, delete, and pin"
```

---

## Task 5: Add `invalidateOn` to `useQuery`

This is the core frontend change — extend `useQuery` with event-driven cache invalidation.

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useQuery.ts`

- [ ] **Step 1: Add the `UseQueryOptions` interface and update the function signature**

Replace the current function signature:

```typescript
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
  staleTime = DEFAULT_STALE_TIME,
): QueryResult<T> {
```

With:

```typescript
interface UseQueryOptions {
  /** Stale time in ms before auto-refetch. Default 30s. */
  staleTime?: number;
  /** Event names that trigger cache invalidation and refetch. */
  invalidateOn?: string[];
  /** Only invalidate if the event payload passes this filter. */
  invalidateFilter?: (payload: unknown) => boolean;
}

export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
  options?: UseQueryOptions | number,
): QueryResult<T> {
```

The `options?: UseQueryOptions | number` union preserves backward compatibility — existing callers passing `staleTime` as a number still work.

- [ ] **Step 2: Parse options inside the hook**

At the top of the hook body, normalize the options:

```typescript
  const opts: UseQueryOptions =
    typeof options === "number" ? { staleTime: options } : options ?? {};
  const staleTime = opts.staleTime ?? DEFAULT_STALE_TIME;
```

Replace all references to the old `staleTime` parameter with this local variable. Update `staleTimeRef`:

```typescript
  const staleTimeRef = useRef(staleTime);
  staleTimeRef.current = staleTime;
```

- [ ] **Step 3: Add the `invalidateOn` effect**

After the existing invalidation listener effect (the one that listens for `invalidationListeners`), add a new effect:

```typescript
  // Event-driven invalidation — refetch when specified events fire
  const invalidateOnRef = useRef(opts.invalidateOn);
  invalidateOnRef.current = opts.invalidateOn;
  const invalidateFilterRef = useRef(opts.invalidateFilter);
  invalidateFilterRef.current = opts.invalidateFilter;

  useEventInvalidation(
    opts.invalidateOn,
    (payload: unknown) => {
      if (invalidateFilterRef.current && !invalidateFilterRef.current(payload)) return;
      doFetch(true);
    },
  );
```

- [ ] **Step 4: Implement `useEventInvalidation` helper**

Add this function before the `useQuery` export. It uses the existing `useEvent` hook but handles the RAF debounce and array of events:

```typescript
import { useEvent } from "./useEvent";

/**
 * Listen for multiple events and call handler on any match.
 * Debounces via requestAnimationFrame to coalesce batch mutations.
 */
function useEventInvalidation(
  events: string[] | undefined,
  handler: (payload: unknown) => void,
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;
  const rafRef = useRef(0);

  const debouncedHandler = useCallback((payload: unknown) => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(() => {
      handlerRef.current(payload);
    });
  }, []);

  // Register one useEvent per event name.
  // We need stable event subscriptions, so we use a single combined listener
  // via window events for browser mode and Tauri listen for desktop mode.
  // Since useEvent only handles one event at a time, we register for each.
  // However, hooks can't be called in loops. Instead, use a single
  // catch-all approach: listen for all events at once via a wrapper.

  useEffect(() => {
    if (!events || events.length === 0) return;

    const listeners: (() => void)[] = [];

    if (isTauri) {
      for (const event of events) {
        let cancelled = false;
        let unlisten: UnlistenFn | undefined;
        listen(event, (e: { payload: unknown }) => {
          if (!cancelled) debouncedHandler(e.payload);
        }).then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        });
        listeners.push(() => {
          cancelled = true;
          unlisten?.();
        });
      }
    } else {
      for (const event of events) {
        const onCustom = (e: Event) => {
          debouncedHandler((e as CustomEvent).detail);
        };
        window.addEventListener(event, onCustom);
        listeners.push(() => window.removeEventListener(event, onCustom));
      }
    }

    return () => {
      cancelAnimationFrame(rafRef.current);
      for (const cleanup of listeners) cleanup();
    };
  }, [events?.join(","), debouncedHandler]);
}
```

Also add at the top of the file:

```typescript
import { isTauri } from "@shared/lib/utils";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
```

- [ ] **Step 5: Verify the frontend compiles**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/shared/hooks/useQuery.ts
git commit -m "feat(ui): add invalidateOn option to useQuery for event-driven cache invalidation"
```

---

## Task 6: Wire Chat Page Thread Query

**Files:**
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`

- [ ] **Step 1: Add `invalidateOn` to the thread list query**

Find the `useQuery` call for `chat_threads` (around line 44-48):

```typescript
  const { data: threads, refetch: refetchThreads } = useQuery<ChatThread[]>(
    "chat_threads",
    undefined,
    [],
  );
```

Replace with:

```typescript
  const { data: threads, refetch: refetchThreads } = useQuery<ChatThread[]>(
    "chat_threads",
    undefined,
    [],
    {
      invalidateOn: [
        "chat:thread_created",
        "chat:thread_updated",
        "chat:message_added",
      ],
    },
  );
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/pages/ChatPage.tsx
git commit -m "feat(chat): wire thread list to real-time events"
```

---

## Task 7: Wire Chat Message Query

**Files:**
- Modify: `desktop-ui/src/features/chat/hooks/useChatSession.ts`

- [ ] **Step 1: Add `invalidateOn` to the message query**

Find the `useQuery` call for `chat_messages` (around line 57-60):

```typescript
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    "chat_messages",
    sessionKey ? { sessionKey } : null,
    [],
  );
```

Replace with:

```typescript
  const { data: messages, refetch } = useQuery<ChatMessage[]>(
    "chat_messages",
    sessionKey ? { sessionKey } : null,
    [],
    {
      invalidateOn: ["chat:message_added"],
      invalidateFilter: (payload) =>
        (payload as { sessionKey?: string })?.sessionKey === sessionKey,
    },
  );
```

This ensures only messages for the active session are refetched — not all sessions.

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/chat/hooks/useChatSession.ts
git commit -m "feat(chat): wire message list to real-time chat:message_added events"
```

---

## Task 8: Add Chat Events to BrainEventBridge

The `BrainEventBridge` forwards SSE events from the dev server as `CustomEvent`s on `window`, which `useEvent` (and now `useEventInvalidation`) listens for in browser dev mode. Without this, the new chat events won't reach the frontend when running in the browser.

**Files:**
- Modify: `desktop-ui/src/app/BrainEventBridge.tsx`

- [ ] **Step 1: Add the three new event names**

Replace the `GLOBAL_SSE_EVENTS` array:

```typescript
const GLOBAL_SSE_EVENTS = [
  "brain:ambient",
  "provider:degraded",
  "entity:updated",
  "focus:state_changed",
] as const;
```

With:

```typescript
const GLOBAL_SSE_EVENTS = [
  "brain:ambient",
  "provider:degraded",
  "entity:updated",
  "focus:state_changed",
  "chat:thread_created",
  "chat:thread_updated",
  "chat:message_added",
] as const;
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/app/BrainEventBridge.tsx
git commit -m "feat(ui): bridge chat real-time events via BrainEventBridge SSE"
```

---

## Task 9: Full Build Verification

**Files:** None (verification only)

- [ ] **Step 1: Run full Rust workspace build**

Run: `cargo build --workspace`
Expected: Compiles with 0 errors.

- [ ] **Step 2: Run Rust clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (zero clippy warnings policy).

- [ ] **Step 3: Run Rust tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 4: Run frontend lint + type check**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 5: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

---

## Task 10: Manual Browser Verification

**Files:** None (manual test only)

- [ ] **Step 1: Start the dev environment**

Start the Rust backend with dev server:
```bash
cargo tauri dev
```

In another terminal:
```bash
cd desktop-ui && bun run dev
```

Open `http://localhost:1420` in the browser.

- [ ] **Step 2: Test thread creation appears in real-time**

1. Open the /chat page in the browser
2. In a separate terminal, use the MCP tool or send a chat message via the API to create a new session
3. Observe: the new thread should appear in the sidebar without refreshing the page

- [ ] **Step 3: Test message arrival appears in real-time**

1. Open the /chat page and select a thread
2. Use MCP or another channel to add a message to that same session
3. Observe: the new message should appear in the message list without refreshing

- [ ] **Step 4: Test thread rename reflects immediately**

1. Right-click a thread and rename it (this already uses the UI)
2. Observe: the thread title updates in the sidebar immediately (this was already working via `refetchThreads` in the rename handler, but now it's also event-driven for external renames)

- [ ] **Step 5: Test thread deletion reflects immediately**

1. Delete a thread via right-click context menu
2. Observe: the thread disappears from the sidebar immediately
