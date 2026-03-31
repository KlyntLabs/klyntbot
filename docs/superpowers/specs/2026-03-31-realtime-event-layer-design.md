# Real-Time Event Layer

**Date:** 2026-03-31
**Status:** Draft
**Scope:** Platform-wide real-time UI updates via event-driven cache invalidation

## Problem

The UI goes stale when mutations happen outside the current page's direct control:

- **MCP tool calls** — Claude Code creates a task via `mcp__klyntbot__tasks`, but the Tasks page doesn't refresh
- **Voice sessions** — a voice conversation creates messages in a session, but the Chat thread list doesn't update
- **Cron jobs** — a scheduled agent run adds messages, but the chat page shows nothing until manual refresh
- **Cross-feature agent tools** — an agent running in chat creates notes/tasks/finance entries, but those pages are stale
- **External channels** — Telegram/Discord/Slack messages arrive, but the chat thread list doesn't reflect them

Currently, `entity:updated` is only emitted from two places:
1. **Frontend `useMutation`** — client-side only, doesn't cover MCP/cron/external channels
2. **`relay_chat_stream`** — only during active agent tool execution in chat

## Solution: Event-Driven Cache Invalidation

Emit lightweight notification events from the backend whenever any source mutates data. The frontend listens for these events and auto-invalidates the relevant `useQuery` caches, triggering a refetch. Local SQLite queries are <1ms, so refetch latency is imperceptible.

### Design Principles

1. **Backend is the single source of truth** — events are notifications, not data carriers. The frontend always refetches from the DB.
2. **Transport-agnostic** — uses existing `AppEventEmitter` trait (Tauri IPC in desktop, SSE in browser dev mode).
3. **Platform-wide** — the same mechanism works for chat, tasks, notes, finance, and any future feature.
4. **Additive** — no breaking changes to existing patterns. Features opt in by adding `invalidateOn` to their `useQuery` calls.

## Architecture

### Event Flow

```
Mutation source (MCP / cron / voice / agent / direct UI)
    │
    ▼
app-core handler (business logic)
    │
    ▼
Storage write (SessionRepo, TaskRepo, etc.)
    │
    ▼
AppEventEmitter.emit_event("entity:updated", { entityKind, id })
    │
    ├─► Tauri IPC (desktop)──► useEvent listeners ──► invalidateQueries()
    │
    └─► broadcast channel ──► SSE /api/brain/events ──► BrainEventBridge
                                                          │
                                                          ▼
                                                    window.CustomEvent
                                                          │
                                                          ▼
                                                    useEvent listeners ──► invalidateQueries()
```

### Backend Changes

#### 1. New Chat-Specific Event Constants (`desktop-shared/src/events.rs`)

```rust
pub const CHAT_THREAD_CREATED: &str = "chat:thread_created";
pub const CHAT_THREAD_UPDATED: &str = "chat:thread_updated";
pub const CHAT_MESSAGE_ADDED: &str = "chat:message_added";
```

Payloads:

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
    /// Source that produced the message (e.g., "chat", "voice", "mcp", "cron", "telegram").
    pub source: String,
}
```

#### 2. Emit Events from App-Core Chat Handlers

Every chat write path in `crates/app-core/src/handlers/chat/` emits the appropriate event via `self.event_emitter`:

| Write Path | File | Event |
|---|---|---|
| `chat_send()` — session upsert | `streaming.rs:787` | `chat:thread_created` (if new) or `chat:thread_updated` |
| `chat_send_squad()` — session upsert | `streaming.rs:1679` | `chat:thread_created` / `chat:thread_updated` |
| `relay_chat_stream()` — agent:done persists assistant msg | `streaming.rs:1119` | `chat:message_added` (source: "chat") |
| `chat_respond_interaction()` — interaction response | `streaming.rs:891` | `chat:message_added` (source: "chat") |
| `chat_rename_thread()` | `threads.rs:208` | `chat:thread_updated` |
| `chat_delete_thread()` | `threads.rs:230` | `chat:thread_updated` |
| `chat_pin_thread()` | `threads.rs:191` | `chat:thread_updated` |

**New vs. updated detection:** Before `upsert_session()`, check if the session key already exists via `repos.sessions.get_session()`. If `None` → `chat:thread_created`. If `Some` → `chat:thread_updated`. This is a single extra SELECT on a primary key — negligible cost.

#### 3. Emit from Agent Loop Session Persistence

When the `SessionManager` flushes messages to SQL (cache eviction or explicit save), it currently has no reference to the event emitter. Two options:

**Option A (recommended): Emit from `relay_chat_stream` only.** The relay already runs after the agent loop and persists metadata. It's the definitive "assistant message is done" signal. This covers the primary use case without threading an emitter into the session manager.

**Option B: Inject emitter into SessionManager.** More comprehensive but adds complexity to a lower-layer crate.

Recommendation: **Option A** for now. The relay is the right place because it's where the message is fully formed (content + metadata + segments). The session manager's deferred writes are an implementation detail of the LRU cache — the relay's `agent:done` → `chat:message_added` covers the user-visible moment.

#### 4. Emit from External Channel Ingestion

External channels (Telegram, Discord, Slack, etc.) flow through `MessageBus` → agent loop → `SessionManager`. Since we're using Option A above, these are covered when the agent's relay emits `chat:message_added` after processing the inbound message.

#### 5. Emit `entity:updated` from MCP Server (Already Exists — Verify Coverage)

`klyntbot-server/src/handler.rs:297` already emits `entity:updated` for Task, Project, Note after MCP tool calls. Verify this covers all entity kinds that have UI pages. Add missing kinds if any (Finance, Productivity, etc.).

### Frontend Changes

#### 1. Add `invalidateOn` to `useQuery` (`shared/hooks/useQuery.ts`)

Extend `useQuery` with an optional array of event names. When any listed event fires, the cache for this query is invalidated and a refetch is triggered.

```typescript
interface UseQueryOptions {
  /** Event names that trigger cache invalidation and refetch. */
  invalidateOn?: string[];
  /** Filter function — only invalidate if event payload matches. */
  invalidateFilter?: (payload: unknown) => boolean;
}

export function useQuery<T>(
  cmd: string,
  args: Record<string, unknown> | null | undefined,
  fallback: T,
  options?: UseQueryOptions,
): { data: T; loading: boolean; error: string | null; refetch: () => void }
```

Implementation: inside the hook, register a `useEvent` listener for each event in `invalidateOn`. When fired, check `invalidateFilter` (if provided), then call the existing `doFetch(true)` to force a refetch.

**Debounce:** If multiple events fire within 50ms (e.g., batch tool execution creates several tasks), coalesce into a single refetch using `requestAnimationFrame`.

#### 2. Wire Chat Page Queries

```typescript
// ThreadList — refetch thread list on any chat mutation
const { data: threads, refetch: refetchThreads } = useQuery<ChatThread[]>(
  "chat_threads", undefined, [], {
    invalidateOn: ["chat:thread_created", "chat:thread_updated", "chat:message_added"],
  }
);

// MessageList — refetch messages when new ones arrive for THIS session
const { data: messages, refetch: refetchMessages } = useQuery<ChatMessage[]>(
  "chat_messages",
  sessionKey ? { sessionKey } : null,
  [],
  {
    invalidateOn: ["chat:message_added"],
    invalidateFilter: (payload) =>
      (payload as { sessionKey?: string })?.sessionKey === sessionKey,
  }
);
```

#### 3. Add Chat Events to BrainEventBridge

In `app/BrainEventBridge.tsx`, add the new events to `GLOBAL_SSE_EVENTS`:

```typescript
const GLOBAL_SSE_EVENTS = [
  "brain:ambient",
  "provider:degraded",
  "entity:updated",
  "focus:state_changed",
  // New:
  "chat:thread_created",
  "chat:thread_updated",
  "chat:message_added",
] as const;
```

#### 4. Add Chat Events to Dev Server Brain SSE Endpoint

In `crates/desktop/src/dev_server/streaming.rs`, the `/api/brain/events` handler forwards events from the `CompoundEmitter`'s broadcast channel. No changes needed — it already forwards ALL events emitted via `AppEventEmitter`. The `BrainEventBridge` just needs to listen for the new event names (step 3 above).

#### 5. Platform-Wide Adoption (Other Features)

Other features can adopt the same pattern without any backend changes — `entity:updated` is already emitted by `relay_chat_stream` for mutating tool calls and by the MCP server. Features just add `invalidateOn`:

```typescript
// Tasks page — already listens via useEvent, migrate to invalidateOn:
const { data: tasks } = useQuery("task_list", filters, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as any)?.entityKind === "task",
});

// Notes page:
const { data: notes } = useQuery("note_list", params, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as any)?.entityKind === "note",
});
```

This is a gradual migration — existing `useEvent` + manual `refetch()` patterns continue to work. Features can adopt `invalidateOn` one at a time.

## Scenarios Covered

| Scenario | Event Emitted | Frontend Effect |
|---|---|---|
| Voice session creates messages | `chat:message_added` (source: "voice") | Thread list + active message list refresh |
| MCP tool creates a task | `entity:updated` (kind: Task) | Tasks page refreshes |
| MCP tool sends chat message | `chat:message_added` (source: "mcp") | Chat page refreshes |
| Cron job runs agent | `chat:thread_created` + `chat:message_added` | New thread appears in sidebar |
| User renames thread | `chat:thread_updated` | Sidebar title updates |
| User deletes thread | `chat:thread_updated` | Thread removed from sidebar |
| Agent creates note during chat | `entity:updated` (kind: Note) | Notes page refreshes (if using `invalidateOn`) |
| Navigate away and back to /chat | `chatStreamStore` preserves streaming state | Streaming resumes seamlessly (existing behavior) |
| New thread created while on /chat | `chat:thread_created` | Thread appears in sidebar without refresh |

## Files to Modify

### Backend (Rust)

| File | Change |
|---|---|
| `crates/desktop-shared/src/events.rs` | Add 3 event constants + 2 payload structs |
| `crates/app-core/src/handlers/chat/streaming.rs` | Emit `chat:thread_created`/`chat:thread_updated` in `chat_send()`, `chat_send_squad()`. Emit `chat:message_added` in `relay_chat_stream()` on Done. |
| `crates/app-core/src/handlers/chat/threads.rs` | Emit `chat:thread_updated` in `rename`, `delete`, `pin` handlers |

### Frontend (TypeScript)

| File | Change |
|---|---|
| `desktop-ui/src/shared/hooks/useQuery.ts` | Add `invalidateOn` + `invalidateFilter` options with RAF-debounced refetch |
| `desktop-ui/src/shared/types/chat.ts` | Add payload types for new events (if needed) |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Add `invalidateOn` to thread and message queries |
| `desktop-ui/src/features/chat/components/ThreadList.tsx` | Add `invalidateOn` to thread query (if fetched here) |
| `desktop-ui/src/app/BrainEventBridge.tsx` | Add 3 new event names to `GLOBAL_SSE_EVENTS` |

### No Changes Needed

- **Dev server SSE** — already forwards all `AppEventEmitter` events via broadcast channel
- **`chatStreamStore`** — already survives route changes, handles streaming state
- **`useEvent`** — already works for both Tauri and browser modes
- **MCP server** — already emits `entity:updated` for tool mutations

## Non-Goals

- **Full-push data sync** — over-engineering for a local-first single-user app
- **Polling** — wasteful, adds latency
- **WebSocket transport** — Tauri events + SSE already cover both desktop and browser dev mode
- **Optimistic updates** — not needed when refetch is <1ms
- **Message ordering guarantees** — SQLite is the source of truth; refetch always gets correct order

## Testing

- **Unit:** `useQuery` with `invalidateOn` — mock events, verify refetch triggers
- **Integration:** Send a message via MCP tool, verify chat thread list updates in browser
- **Manual:** Open /chat, use voice to send a message in another session, verify it appears without refresh
