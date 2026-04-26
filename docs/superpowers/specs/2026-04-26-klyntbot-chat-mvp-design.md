# klyntbot Chat MVP in new desktop-ui — Design

**Status:** approved (brainstorming)
**Date:** 2026-04-26
**Owner:** desktop-ui / chat integration

## Goal

Wire the **"New chat"** button in `desktop-ui/src/features/app/components/SidebarChatLayout.tsx` to klyntbot's existing Rust chat backend (sessions, threads, streaming agent responses) so users can hold a real conversation in the new UI. The new `desktop-ui/` is intended to fully replace `desktop-ui.bak/`; the old UI is reference only.

## Non-goals (v1)

Squad/debate mode, persona messages, voice phase indicator, autotuner promotion toasts, provider-degraded banner, context resume from navigation state, thread context menu (rename/delete), PARA grouping, virtualized message list, transparency panel, persistence of selected thread across cold starts, retry/telemetry, toast notifications.

## Backend (already exists, no changes)

Tauri commands in `crates/desktop/src/commands/chat.rs`, registered in `crates/desktop/src/main.rs`:

- `chat_threads()` → `ChatThread[]`
- `chat_messages({ sessionKey, limit? })` → `ChatMessage[]`
- `chat_send({ content, sessionKey, context? })` → starts streaming; returns user message
- `chat_rename_thread({ sessionKey, title })` (not used in v1)
- `chat_delete_thread({ sessionKey })` (not used in v1)

`chat_send` is dispatched specially in `crates/desktop/src/dev_server/streaming.rs`. Streaming is delivered via `app.emit(event_name, payload)` to Tauri events (`agent:segment`, `agent:tool_call`, `agent:done`, etc.) and via SSE in browser dev mode through `/api/events/{sessionKey}`. The thread-list lifecycle emits `chat:thread_created`, `chat:thread_updated`, and `chat:message_added`.

## Stack constraints (new desktop-ui)

- No Tailwind. Plain CSS in `src/styles/` with BEM-ish class names; design tokens in `src/styles/ds-tokens.css` and themes in `src/styles/themes.*.css`.
- No router. Single `MainApp` shell, view-by-state.
- No `useQuery`/`useMutation`/`useEvent`/`useIpc` hooks. Direct `invoke()` from `src/api/client.ts`.
- Path aliases: `@/*`, `@app/*`, `@settings/*`, `@threads/*`, `@services/*`, `@utils/*` (no `@shared` or `@features`).
- Existing reusable: `Markdown` component at `@/features/messages/components/Markdown`.
- React 19, `react-markdown`, `lucide-react` already installed.

## Architecture

### Wedge into MainApp — conditional swap (option A)

Add to `MainApp.tsx`:

- `appView: "home" | "chat"` state
- `selectedSessionKey: string | null` state

Both flow into `buildPrimaryNodes` (`src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx`) via a new prop:

```ts
chatViewProps: {
  active: boolean;
  sessionKey: string | null;
  onThreadsChanged: () => void;
}
```

When `active && sessionKey`, `buildPrimaryNodes` returns `<ChatPanel sessionKey={...} onThreadsChanged={...}/>` as `messagesNode` and `null` as `composerNode`. `onThreadsChanged` is wired to `useChatThreads().refetch` so a completed message triggers sidebar refresh. Otherwise the existing `<Messages>` and `<Composer>` are rendered unchanged.

The foreign `Messages`/`Composer` components stay in the codebase, dormant when in chat mode. They are not refactored in v1.

### File layout

```
desktop-ui/src/features/chat/             ← NEW
├── store/
│   └── chatStreamStore.ts                ← ported from .bak (~907 lines)
├── types.ts                              ← trimmed from .bak shared/types/chat.ts
├── hooks/
│   ├── useAgentStream.ts                 ← ported, store-driven
│   ├── useChatSession.ts                 ← ported, ipc() → invoke()
│   └── useChatThreads.ts                 ← NEW
├── components/
│   ├── ChatPanel.tsx                     ← NEW
│   ├── MessageBubble.tsx                 ← NEW
│   └── ChatInput.tsx                     ← NEW (minimal; not the bak ChatInput)
└── chat.css                              ← NEW
```

`chat.css` is added to `src/styles/index.css`'s import chain.

## Components

### `store/chatStreamStore.ts` (ported)

Singleton. Owns Tauri event listeners (`agent:segment`, `agent:tool_call`, `agent:done`, …). Maintains per-session state (`segments`, `isStreaming`, `error`, `activeTools`, `activeInteraction`, `transparency`, etc.). Exposes `subscribe`/`getSnapshot` for `useSyncExternalStore`. Browser dev mode falls back to `EventSource` on `/api/events/{sessionKey}`.

Squad/debate/persona/judge/learning fields are kept in the store unchanged unless their imports break — minimizing porting risk dominates over slimming. They simply remain unread by v1 React surface.

### `types.ts`

Trimmed copy of `.bak/src/shared/types/chat.ts`. MVP needs `ChatThread`, `ChatMessage`, `MessageSegment`, `ActiveInteraction`. Fields imported by `chatStreamStore` are kept as type-only re-exports rather than inlined.

### `hooks/useAgentStream.ts` (ported)

`useSyncExternalStore`-driven wrapper over `chatStreamStore`. Surface unchanged from bak.

### `hooks/useChatSession.ts` (ported, two edits)

1. Replace `import { ipc } from "./useIpc"` with `import { invoke } from "@/api/client"`. Rename calls.
2. Replace bak's `useQuery("chat_messages", { sessionKey }, [], { invalidateOn: ["chat:message_added"] })` with an inline pattern: `useState<ChatMessage[]>` + `useEffect` that calls `invoke("chat_messages", { sessionKey })` on mount/sessionKey change, plus a `listen("chat:message_added", …)` subscription that refetches when payload's `sessionKey` matches.

Returns the same surface (messages, segments, isStreaming, send, setInput, etc.).

### `hooks/useChatThreads.ts` (new)

`useEffect` loads via `invoke<ChatThread[]>("chat_threads")`. Subscribes to Tauri events `chat:thread_created` and `chat:thread_updated` (via `@tauri-apps/api/event`'s `listen`) to refetch. Returns `{ threads, refetch }`.

### `components/ChatPanel.tsx`

Props: `{ sessionKey: string; onThreadsChanged: () => void }`.
Renders header (thread title or "New chat"), scrollable message list, error banner if `chat.error`, `<ChatInput>` at bottom. Uses `useChatSession(sessionKey, onThreadsChanged)`. Auto-scrolls on new message or streaming segment. Empty state when no messages and not streaming: "Start a conversation. Ask Klynt anything about your tasks, projects, or schedule."

### `components/MessageBubble.tsx`

Renders one `ChatMessage`. User messages right-aligned, assistant left. Uses existing `Markdown` from `@/features/messages/components/Markdown` for content. Streaming segments are concatenated and rendered the same way under a "streaming" assistant bubble.

### `components/ChatInput.tsx` (minimal — not the bak version)

Auto-resizing `<textarea>`, send button, Enter-to-send, Shift+Enter for newline, disabled while `isStreaming`. ~80 lines. No attachments, no slash commands, no autocomplete.

### `SidebarChatLayout.tsx` (modify)

Add props: `onNewChat: () => void`, `threads: ChatThread[]`, `selectedSessionKey: string | null`, `onSelectThread: (key: string) => void`. "New chat" button calls `onNewChat`. The "Chats" section replaces the "No chats" placeholder with rendered thread list (selected highlighted). No context menu in v1.

### `MainApp.tsx` (modify, surgically)

- Hold `appView` and `selectedSessionKey` state.
- Call `useChatThreads()` once at this level so the result can be shared with sidebar and chat panel.
- Wire `onNewChat`, `onSelectThread` handlers; pass to sidebar.
- Pass `chatViewProps` and thread props through to `buildPrimaryNodes`.

### `buildPrimaryNodes.tsx` (modify)

Accept `chatViewProps`. Substitute `<ChatPanel>` for `messagesNode` and `null` for `composerNode` when active. Sidebar always receives the new thread props.

## Data flow

### Cold start → "New chat"

1. User clicks "New chat".
2. `MainApp` sets `appView="chat"`, `selectedSessionKey="chat:" + crypto.randomUUID()`.
3. `buildPrimaryNodes` returns `<ChatPanel sessionKey={...}/>`.
4. `ChatPanel` mounts → `useChatSession` → `invoke("chat_messages", { sessionKey })` returns `[]`. Empty state.

### First message

1. User types, presses Enter.
2. `useChatSession.send()` appends optimistic pending user message; calls `chatStreamStore.startStream(sessionKey)`; `invoke("chat_send", { content, sessionKey })`.
3. Rust persists user message, returns; spawns relay task that emits `agent:*` Tauri events.
4. `chatStreamStore` listeners append events; React re-renders via `useSyncExternalStore`.
5. On `agent:done`: store clears `isStreaming`, fires `onDone` → `useChatSession` refetches `chat_messages`; segments cleared once new assistant message count exceeds previous count (avoids flash).
6. Rust emits `chat:thread_created` (first message) → `useChatThreads` refetches → sidebar shows the new thread.

### Subsequent messages

Same flow minus `chat:thread_created`. `chat:thread_updated` fires → sidebar reorders.

### Switch threads

1. User clicks a thread in sidebar.
2. `MainApp` sets `selectedSessionKey` to that thread's key, `appView="chat"`.
3. `ChatPanel` re-keys via `key={sessionKey}` → fresh `useChatSession` → fresh `chat_messages` invoke.
4. `chatStreamStore` snapshots are per-session, so a stream on the previous session keeps running silently in the store; switching back resumes its view.

### Back to home

For v1, no UI affordance to leave chat mode. The sidebar's other buttons (Project / Plugins / etc.) are not wired in v1 and remain inert. Closing/reopening the app returns to home (state is in-memory).

## Error handling

- `invoke("chat_send")` rejects → `useChatSession` catches, calls `chatStreamStore.failStream(sessionKey, message)` → `error` set → red banner above input. User can retry.
- `agent:error` event from Rust → store sets `error`, clears `isStreaming`. Same banner.
- `invoke("chat_messages")` rejects → `ChatPanel` shows inline error; messages stay empty. Re-selecting thread retries.
- `invoke("chat_threads")` rejects → sidebar shows "Failed to load threads" inline; rest of app unaffected. Refetched on next thread event.
- Browser dev mode (no Tauri): `chatStreamStore` falls back to `EventSource` on `/api/events/{sessionKey}` via Vite proxy. Verified working in bak; expected to work unchanged.
- No retries, no toasts, no telemetry in v1. Errors are inline-only.

## Testing

- **Unit:** `useChatThreads` — mock `invoke`, assert fetch on mount and refetch on synthesized `chat:thread_created` event. One test file.
- **Smoke:** `ChatPanel` renders empty state with no messages; renders messages when `useChatSession` returns them (mocked). Two tests.
- **No tests for ported code** (`chatStreamStore`, `useAgentStream`, `useChatSession`). The bak has none either; we trust by parity.
- **Manual verification checklist** in PR:
  1. Cold start → "New chat" → empty panel renders.
  2. Type message → send → user message appears immediately.
  3. Streaming response renders incrementally.
  4. After completion, message persists in panel.
  5. Reload app → thread appears in sidebar (cold start lands on home; sidebar shows threads).
  6. Click thread → messages reload, can continue conversation.
  7. Open a new chat while a previous chat is mid-stream → switch back → in-flight stream still rendering.
  8. Send empty message → no-op.
  9. Trigger error (e.g., kill backend mid-send) → red banner; UI recovers on retry.

## Out-of-band: stale `CLAUDE.md`

As part of this work, update `CLAUDE.md`'s "Desktop UI (desktop-ui/)" section to reflect the new UI's stack: no Tailwind, no Biome, no `useQuery`/`ipc()` abstractions. Replace with: plain CSS + ds-tokens, eslint via `bun run lint`, direct `invoke()` from `src/api/client.ts`. Reference `src/styles/index.css` and `src/api/client.ts` as sources of truth. ~30-line edit.

## Risks & mitigations

- **`MainApp.tsx` is dense (~1800 lines).** State additions are additive and small; no restructuring. We thread two new state values through one existing prop bag.
- **Rust event names must match `chatStreamStore` listeners.** Verified: Rust uses `app.emit("agent:segment", …)` etc., matching the listener strings in the store. Will spot-check during implementation.
- **Browser dev mode SSE fallback** uses `/api/events/{sessionKey}` — assumes Vite proxy is wired. The bak's `vite.config.ts` proxies `/api`; need to confirm new UI does too. If not, add the proxy.
- **`chatStreamStore` is large and ported as-is.** If a field references something we trimmed from `types.ts`, we re-add the type rather than edit the store. Port-fidelity over slimming.

## Success criteria

- Clicking "New chat" opens the chat panel.
- Typing a message and pressing send calls `chat_send`, shows the user message, streams the assistant response into the panel, and persists it (visible after reload via `chat_messages`).
- The sidebar "Chats" section shows real threads from `chat_threads`; clicking one loads its messages.
- No regressions in the rest of the new UI (Home, Settings, etc. still render and function).
