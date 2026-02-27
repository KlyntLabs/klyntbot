# Chat UI Redesign — Design Document

**Date**: 2026-02-27
**Status**: Approved
**Scope**: Dashboard frontend (`crates/dashboard/frontend/src/`)

## Problem

The current chat page (`Chat.tsx`, 1574 lines) has three issues:

1. **No URL routing for sessions** — sessions tracked via `sessionStorage`, not the URL. Users can't bookmark, share, or use browser back/forward with conversations.
2. **Sidebar clutter** — Session Info, Session Context, and Recent Sessions take up sidebar space with data most users rarely need. Session management should live on its own page.
3. **No tool visibility** — users can't see what system capabilities (Tasks, Plans, Calendar, etc.) are being used during a conversation. Tool calls exist but aren't categorized or visually highlighted.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Root URL behavior | `/` stays as new-chat, sessions at `/chat/{id}` | Backwards-compatible, clean UX |
| Session management location | New `/sessions` page in nav rail | Discoverable without cluttering chat sidebar |
| Sidebar empty state | Show all tools dimmed | Doubles as capability discovery surface |
| Tool tag click behavior | Tooltip only (no navigation) | Keeps sidebar purely informational |
| Architecture approach | Full chat module rework (Approach B) | Clean long-term architecture with ChatProvider context |

## 1. Routing & URL Structure

### Route Changes

```
/                    → ChatPage (new session state, empty chat)
/chat/:sessionId     → ChatPage (loads session from URL param)
/sessions            → SessionsPage (new - session management)
```

All other routes unchanged.

### URL Sync Flow

1. User visits `/` — sees empty chat with suggestion cards
2. User sends first message → `useAgent.sendMessage()` returns session key → `navigate(`/chat/${sessionKey}`, { replace: true })`
3. User visits `/chat/abc123` directly → ChatProvider reads `sessionId` from params, calls `loadSession(sessionId)` on mount
4. User clicks "+ New" → `navigate('/')` clears state
5. Browser back/forward works naturally

### sessionStorage Removed

The URL is the source of truth for the active session. No more storing `sessionKey` in `sessionStorage`.

## 2. Chat Module Architecture

### New Directory Structure

```
app/
├── chat/
│   ├── ChatPage.tsx              # Route component - reads params, renders ChatLayout
│   ├── ChatProvider.tsx          # Context provider: useAgent + URL sync + tool tracking
│   ├── ChatLayout.tsx            # Two-column layout (messages | sidebar)
│   ├── components/
│   │   ├── MessageArea.tsx       # Message list + auto-scroll + empty state
│   │   ├── MessageBubble.tsx     # Single message with markdown rendering
│   │   ├── MessageInput.tsx      # Input textarea + New/Send/Cancel buttons
│   │   ├── ThinkingIndicator.tsx # Agent processing phases display
│   │   ├── InteractionPanel.tsx  # User interaction request form
│   │   └── InteractionField.tsx  # Single interaction input field
│   └── sidebar/
│       ├── ChatSidebar.tsx       # Sidebar container
│       ├── ConnectionStatus.tsx  # WebSocket status indicator
│       ├── ToolActivityPanel.tsx # Tool/system activity tags (NEW)
│       ├── ToolCallList.tsx      # Live tool execution details
│       ├── QuickTasks.tsx        # Top 5 pending tasks
│       └── UpcomingEvents.tsx    # Next 5 calendar events
├── pages/
│   ├── Sessions.tsx              # Session management page (NEW)
│   └── ...unchanged...
```

### ChatProvider Context

```typescript
interface ChatContextValue {
  // From useAgent (passthrough)
  messages: ChatMessage[];
  thinking: ThinkingState | null;
  isStreaming: boolean;
  status: ConnectionStatus;
  sessionKey: string | null;
  pendingInteraction: PendingInteraction | null;
  sendMessage: (text: string) => void;
  cancel: () => void;
  respondToInteraction: (requestId: string, response: Record<string, unknown>) => void;

  // URL-driven session management
  startNewSession: () => void;

  // Tool activity tracking
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}
```

Key changes from current useAgent interface:
- `sendMessage` no longer takes `sessionKey` param (provider handles internally)
- `loadSession` and `deleteSession` removed from context (live on Sessions page)
- `newSession` becomes `startNewSession` (navigates to `/`)
- New: `activeTools` and `toolHistory` for sidebar

## 3. Tool Activity Panel

### Tool Categories

| Category | Icon | Triggered by tools |
|----------|------|--------------------|
| Tasks | CheckSquare | `todo` |
| Plans | FileText | `plan` |
| Calendar | Calendar | `calendar` |
| Finance | DollarSign | `finance` |
| Skills | Zap | `skill` |
| Cron | Clock | `cron` |
| Projects | FolderKanban | `project` |
| Web | Globe | `web_search`, `web_fetch` |
| Files | File | `file_read`, `file_write`, `file_list`, `file_append` |
| Message | MessageSquare | `message`, `ask_user` |
| Spawn | GitBranch | `spawn` |

### Visual States

1. **Inactive** (default): Dimmed, `opacity: 0.3`, subtle border
2. **Active** (tool executing): Full opacity, accent border, pulse animation
3. **Used** (tool called earlier): Full opacity, no animation, solid border

### Layout & Interaction

- Compact grid/flow layout (2-3 tags per row in 260px sidebar)
- Each tag: `icon + label` in a pill/chip shape
- Hover: tooltip with last operation (e.g., `todo search "buy" — 3 results`)
- Click: no action (purely informational)

### Internal Data Model

```typescript
interface ToolActivityEntry {
  category: string;
  toolName: string;
  args?: Record<string, unknown>;
  timestamp: number;
  status: 'active' | 'completed' | 'failed';
}
```

Event handling in ChatProvider:
- `toolStart` → add entry as 'active', add category to `activeTools`
- `toolEnd` → update to 'completed'/'failed', remove from `activeTools`
- New session → clear `toolHistory` and `activeTools`

## 4. Sessions Page

### Route: `/sessions`

Full-width page with:

**Header**: "Sessions" title + search bar + sort dropdown

**Session rows**:
- Session key (truncated hash, e.g., `#aa5f10`)
- First message preview (truncated ~80 chars, deferred — show key+date for v1)
- Message count
- Created date (relative)
- Last active date
- Tools used (small chips)
- Delete button (with confirmation)

**Actions**:
- Click session → navigate to `/chat/{sessionId}`
- Delete → confirmation, then remove
- Search → filter by session key
- Sort: Most Recent (default), Oldest, Most Messages

**Nav rail**: Add "Sessions" item with History icon between Chat and Tasks.

## 5. Change Summary

### Files Created

| File | Purpose |
|------|---------|
| `app/chat/ChatPage.tsx` | Route component |
| `app/chat/ChatProvider.tsx` | Context provider |
| `app/chat/ChatLayout.tsx` | Two-column layout |
| `app/chat/components/MessageArea.tsx` | Message list |
| `app/chat/components/MessageBubble.tsx` | Single message |
| `app/chat/components/MessageInput.tsx` | Input controls |
| `app/chat/components/ThinkingIndicator.tsx` | Processing phases |
| `app/chat/components/InteractionPanel.tsx` | Interaction form |
| `app/chat/components/InteractionField.tsx` | Interaction input |
| `app/chat/sidebar/ChatSidebar.tsx` | Sidebar container |
| `app/chat/sidebar/ConnectionStatus.tsx` | Status indicator |
| `app/chat/sidebar/ToolActivityPanel.tsx` | Tool activity tags |
| `app/chat/sidebar/ToolCallList.tsx` | Tool call details |
| `app/chat/sidebar/QuickTasks.tsx` | Quick tasks |
| `app/chat/sidebar/UpcomingEvents.tsx` | Upcoming events |
| `app/pages/Sessions.tsx` | Session management |

### Files Modified

| File | Change |
|------|--------|
| `app/routes.tsx` | Add `/chat/:sessionId` and `/sessions` routes |
| `app/components/Layout.tsx` | Add "Sessions" nav item |
| `lib/hooks/useAgent.ts` | Remove sessionStorage persistence |
| `lib/types.ts` | Add ToolActivityEntry, ToolCategory types |

### Files Deleted

| File | Reason |
|------|--------|
| `app/pages/Chat.tsx` | Replaced by `app/chat/` module |

### Sidebar Sections Removed

- Session Info (message count, streaming, phase, strategy, engine, iteration)
- Session Context (your msgs, assistant msgs, total)
- Recent Sessions (moved to `/sessions` page)

### Backend Changes

None required. All data from existing endpoints and WebSocket events.
