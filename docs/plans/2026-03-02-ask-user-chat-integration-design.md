# ask_user Chat Integration — Design

## Problem

The `ask_user` tool lets the agent present structured questions (single select, multi select, yes/no, free text) to users. It works on Telegram (inline keyboards) and Discord (select menus), but the desktop chat UI ignores it entirely. The `StreamingHandle.interaction_rx` channel is never drained, so the tool hangs when called from desktop.

## Design Decisions

- **Inline in chat flow** — prompts appear as special message bubbles, not modals or panels
- **Card list (vertical)** — each option is a card with label + description, stacked vertically
- **Tabbed card** — multi-question requests (up to 4) use tabs across the top
- **Collapse to summary** — after answering, the card collapses to a single line showing question + selected answer
- **Final answers** — no editing after submit; agent can re-ask if needed
- **Keyboard navigation** — arrow keys / j,k to move between options, Enter/Space to select, Tab to switch question tabs

## Architecture: Tauri Event Bridge

Extends the existing event relay pattern (same as `agent:content_chunk`, `agent:tool_start`, etc.).

### Data Flow

```
Agent loop calls ask_user tool
    ↓
Tool sends InteractionBundle on interaction_tx
    (InteractionBundle = InteractionRequest + oneshot::Sender<FormResponse>)
    ↓
chat_send background task receives on interaction_rx
    ↓
Generates request_id (UUID), stores oneshot sender in
    AppCore.pending_interactions: DashMap<String, oneshot::Sender<FormResponse>>
    ↓
Emits Tauri event "agent:interaction_request" with payload:
    { sessionKey, requestId, request: InteractionRequest }
    ↓
Tool is blocked waiting on the oneshot receiver
    ↓
User answers in frontend → calls IPC chat_respond_interaction
    { sessionKey, requestId, response: FormResponse }
    ↓
Backend removes sender from DashMap, sends FormResponse through oneshot
    ↓
Tool unblocks, formats semantic response, returns to LLM
```

### Cancellation

`chat_cancel` iterates `pending_interactions` for that session key and sends `FormResponse::Cancelled` through each oneshot, in addition to cancelling the stream token.

If the frontend never responds (window closed), the oneshot sender drops and the tool gets a channel-closed error it already handles.

## Event Types & IPC Contracts

### New Tauri Event: `agent:interaction_request`

```
{
  sessionKey: string,
  requestId: string,
  title: string,
  questions: [{
    id: string,
    title: string,         // tab label (≤20 chars)
    text: string,           // full question text
    answerType: "single_select" | "multi_select" | "yes_no" | "free_text",
    options?: [{ label: string, description?: string }],
    default?: boolean,      // yes_no only
    placeholder?: string    // free_text only
  }]
}
```

### New IPC Command: `chat_respond_interaction`

```
Args: { sessionKey, requestId, response }

response =
  | { type: "completed", answers: [{ questionId, value }] }
  | { type: "cancelled" }

AnswerValue =
  | { type: "selected", index: number }            // single_select
  | { type: "multi_selected", indices: number[] }   // multi_select
  | { type: "yes_no", value: boolean }               // yes_no
  | { type: "text", value: string }                  // free_text
```

## Frontend Component: InteractionCard

### Layout

```
┌─────────────────────────────────────────────┐
│  ● Klynt is asking...                       │
│                                             │
│  ┌─ Tab1 ─┬─ Tab2 ─┐   (if >1 question)    │
│  │                  │                       │
│  │  Which auth method should we use?        │
│  │                                          │
│  │  ┌─────────────────────────────────┐     │
│  │  │ ● JWT Tokens                    │     │
│  │  │   Stateless, good for APIs      │  ↑↓ │
│  │  └─────────────────────────────────┘  jk │
│  │  ┌─────────────────────────────────┐     │
│  │  │   Session Cookies               │     │
│  │  │   Server-side, simpler setup    │     │
│  │  └─────────────────────────────────┘     │
│  │                                          │
│  │              [Submit]                    │
│  └──────────────────────────────────────────┘
└─────────────────────────────────────────────┘
```

### Keyboard Navigation

| Key | Action |
|-----|--------|
| `↑` / `k` | Move focus to previous option |
| `↓` / `j` | Move focus to next option |
| `Enter` / `Space` | Select (single) or toggle (multi) |
| `Tab` | Next question tab |
| `Shift+Tab` | Previous question tab |
| `Enter` on Submit | Send response |

### Question Type Rendering

| Type | Render |
|------|--------|
| `single_select` | Vertical card list, radio-style highlight |
| `multi_select` | Vertical card list with checkboxes |
| `yes_no` | Two pill buttons side by side |
| `free_text` | Text input with placeholder |

### Collapsed State (after submit)

```
┌─────────────────────────────────────────────┐
│  ● You answered: JWT Tokens                 │
└─────────────────────────────────────────────┘
```

## Message Storage & History

Interactions are persisted as synthetic messages so they render correctly when chat history is reopened.

### Storage Format

```sql
INSERT INTO session_messages (role, content, metadata)
VALUES (
  'interaction',
  'You answered: JWT Tokens',   -- human-readable fallback
  '{"type":"interaction_response","title":"...","questions":[...],"answers":[...]}'
);
```

### Who Writes

The `chat_respond_interaction` backend command writes the interaction message *and* sends the `FormResponse` through the oneshot — in that order, so persistence happens before the agent continues.

### Chat History Rendering

```
MessageList:
  role == "user"         → UserBubble
  role == "assistant"    → AssistantBubble (markdown)
  role == "interaction"  → CollapsedInteraction (summary line)

  activeInteraction set? → InteractionCard at the end (live, interactive)
```

The `chat_messages` query filter expands from `user|assistant` to `user|assistant|interaction`.

### Streaming Indicator

While the InteractionCard is active (awaiting response), the streaming/thinking indicator pauses — the agent is blocked, not thinking. After submit, it resumes.

## Files to Modify

### Backend (Rust)

| File | Change |
|------|--------|
| `crates/desktop/src/app_core.rs` | Add `pending_interactions: DashMap<String, oneshot::Sender<FormResponse>>` |
| `crates/desktop/src/commands/chat.rs` | Drain `interaction_rx` in background task, emit event; add `chat_respond_interaction` command; extend `chat_cancel` |
| `crates/desktop/src/main.rs` | Register `chat_respond_interaction` |
| `crates/desktop-shared/src/events.rs` | Add `AGENT_INTERACTION_REQUEST` constant |
| `crates/desktop-shared/src/commands.rs` | Add `InteractionRequestPayload`, `InteractionResponseInput` types |

### Frontend (TypeScript)

| File | Change |
|------|--------|
| `desktop-ui/src/lib/types.ts` | Add `InteractionRequest`, `Question`, `AnswerType`, `FormResponse` types |
| `desktop-ui/src/hooks/useAgentStream.ts` | Listen for `agent:interaction_request`, expose `activeInteraction` |
| `desktop-ui/src/hooks/useChatSession.ts` | Surface `activeInteraction` from stream hook |
| `desktop-ui/src/components/chat/InteractionCard.tsx` | New component — tabbed card with keyboard nav |
| `desktop-ui/src/components/chat/CollapsedInteraction.tsx` | New component — summary line for history |
| `desktop-ui/src/components/chat/MessageList.tsx` | Render `InteractionCard` and `CollapsedInteraction` |
