# Session Tracker & Context-Aware Brainstorm Assistant

**Date:** 2026-03-08
**Status:** Approved
**Feature:** Real-time Claude Code session tracking with AI-assisted brainstorming

## Problem

When working with Claude Code, switching to other AI tools (ChatGPT, Grok) for brainstorming, analysis, or prompt optimization causes context loss. Those tools lack the full conversation context from the active Claude Code session, leading to incomplete or less accurate suggestions.

## Solution

A new feature integrated into the klyntbot desktop app (Tauri) that:

1. Tracks Claude Code sessions in real time by watching JSONL files
2. Mirrors the conversation in a smart-filtered chat stream
3. Provides AI-assisted brainstorming with full session context
4. Generates refined responses/prompts with Copy, Edit, and Send actions
5. Optionally sends results directly back into the active Claude Code session

## Decisions

| Question | Decision |
|----------|----------|
| Where does it live? | Integrated into existing klyntbot desktop app (Tauri) |
| LLM for brainstorming | Both: direct model selection + klyntbot agent mode |
| Session display | Full data capture, smart-filtered display with collapsible tool/agent/MCP blocks |
| Send-back mechanism | "Send to Claude Code" button via queue-operation JSONL injection, with Copy/Edit/Send action bar |
| Session management | Auto-follow active session + browsable sidebar grouped by project |
| Context injection | Sliding window + incremental summary + user-pinnable messages |

## Architecture

```
Klyntbot Desktop App (Tauri)
  ├── Session Sidebar (project-grouped, auto-follow active)
  ├── Session Mirror View (smart-filtered chat stream)
  ├── Brainstorm Chat Panel (direct model or agent mode)
  └── Action Bar (Copy / Edit / Send)
         │                              │
         │ FSEvent watch                │ queue-operation enqueue
         ▼                              ▼
  ~/.claude/projects/<project>/<session>.jsonl
```

### New Crate: feature-session-tracker (L4)

```
feature-session-tracker/
  ├── src/
  │   ├── lib.rs              — FeaturePackage impl
  │   ├── watcher.rs          — SessionWatcher (notify crate + FSEvents)
  │   ├── parser.rs           — JSONL → SessionMessage enum
  │   ├── index.rs            — SessionIndex (discovery + SQLite persistence)
  │   ├── context_builder.rs  — Incremental summarization + sliding window + pins
  │   ├── injector.rs         — Write queue-operations to JSONL
  │   └── migrations/         — SQLite tables
  └── Cargo.toml              — depends on: common, storage, providers
```

No new crate needed for brainstorming — uses existing `agent` runtime + `providers` for direct model mode.

### Integration Points

- **app-core:** New handlers — `SessionTrackerHandler`, `BrainstormHandler`
- **desktop:** New Tauri commands — `session_*`, `brainstorm_*` (thin adapters)
- **desktop-ui:** New routes — `/sessions`, `/sessions/:id`, `/sessions/:id/brainstorm/:id`

## Claude Code Session Data Model

Sessions are stored as JSONL files at `~/.claude/projects/<project-path>/<session-uuid>.jsonl`.

### Message Types

| Type | Description |
|------|-------------|
| `user` | User messages (text content blocks) |
| `assistant` | Assistant responses (text + tool_use + tool_result blocks) |
| `system` | System messages (hooks, reminders) |
| `progress` | Hook progress, tool progress events |
| `queue-operation` | Enqueue/dequeue — mechanism for injecting messages |
| `file-history-snapshot` | File state snapshots |
| `last-prompt` | Last prompt metadata |

### Session Discovery

- Scan `~/.claude/projects/` for all project directories and JSONL files
- Read `~/.claude/history.jsonl` to map session IDs to display prompts
- Track metadata: project, branch, status, message count, last activity

### Active Session Detection

- File with most recent mtime is "active"
- Debounce: require 2+ writes within 5s to switch (prevents flickering)
- User can pin a session to prevent auto-switching

## Session Message Schema (Rust)

```rust
enum SessionMessage {
    User {
        uuid: String,
        content: MessageContent,
        timestamp: DateTime<Utc>,
        is_meta: bool,
    },
    Assistant {
        uuid: String,
        content: Vec<ContentBlock>,
        timestamp: DateTime<Utc>,
    },
    System {
        uuid: String,
        subtype: String,
        content: String,
        level: String,
        timestamp: DateTime<Utc>,
    },
    Progress {
        uuid: String,
        data: ProgressData,
        timestamp: DateTime<Utc>,
    },
    QueueOperation {
        operation: String,  // "enqueue" | "dequeue"
        content: Option<String>,
        timestamp: DateTime<Utc>,
    },
    FileHistorySnapshot { ... },
}

enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}
```

## Context Builder

### Three-Layer Strategy

| Layer | Content | Token Budget |
|-------|---------|--------------|
| Summary | Incrementally generated session summary | ~25% (2000 tokens) |
| Pinned | User-selected important messages | ~25% (2000 tokens) |
| Recent Window | Last 20-30 verbatim messages | ~50% (4000 tokens) |

Budget auto-adjusts based on selected model's context window.

### Incremental Summarization

1. Messages stream in from `SessionWatcher`
2. Added to `recent_window` (FIFO, oldest drops off)
3. Every 50 messages, oldest chunk is summarized via LLM (cheapest configured model)
4. Chunk summary merged into `rolling_summary`
5. Full context = `rolling_summary` + `pinned_messages` + `recent_window`

Summary prompt extracts: current task/goal, key decisions, files modified, problems encountered, current state/blocker.

## Brainstorm Chat

### Two Modes

- **Direct Model** — User picks model (GPT-4o, Grok, Gemini, etc.). Context injected as system message. Raw LLM chat, no tools.
- **Agent Mode** — Routes through klyntbot's AgentRuntime with full pipeline (intent analysis, context engine, tool access, cognitive memory). Session context as additional context source.

### Conversation Threading

Multiple brainstorm threads per Claude Code session. Stored in SQLite via existing StoragePool.

### Three-Button Action Bar

On AI responses that produce actionable results:

- **Copy** — Copies result to clipboard
- **Edit** — Result becomes editable textarea. Edited version used by Copy/Send
- **Send** — Confirmation dialog showing target session + message preview. Writes `queue-operation` enqueue to JSONL

### Safety Guardrails

- Send only to active sessions (disabled for idle/completed)
- Confirmation always required
- Injected messages tagged in mirror view as "Sent from Klyntbot"
- Rate limit: max 1 send per 5 seconds

## Database Schema

```sql
CREATE TABLE tracked_sessions (
    session_id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    project_name TEXT NOT NULL,
    jsonl_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
    first_message_preview TEXT,
    message_count INTEGER DEFAULT 0,
    git_branch TEXT,
    last_activity TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE pinned_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id),
    message_uuid TEXT NOT NULL,
    message_content TEXT NOT NULL,
    message_role TEXT NOT NULL,
    pin_order INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE brainstorm_conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id),
    title TEXT,
    mode TEXT NOT NULL,
    model_key TEXT,
    agent_profile TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE TABLE brainstorm_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES brainstorm_conversations(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    is_result_block BOOLEAN DEFAULT FALSE,
    edited_content TEXT,
    sent_to_cc BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE session_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id),
    chunk_start INTEGER NOT NULL,
    chunk_end INTEGER NOT NULL,
    summary TEXT NOT NULL,
    files_touched TEXT,
    key_decisions TEXT,
    rolling_summary TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

## Tauri Commands

```rust
// Session tracking
get_tracked_sessions()         → Vec<TrackedSession>
get_session_messages(id, opts) → Vec<SessionMessage>
pin_message(session_id, msg)   → ()
unpin_message(pin_id)          → ()
send_to_claude_code(session_id, content) → Result<()>

// Brainstorming
create_brainstorm(session_id, mode)      → BrainstormConversation
send_brainstorm_message(conv_id, msg)    → () (streaming via event)
list_brainstorms(session_id)             → Vec<BrainstormConversation>
get_brainstorm_context(session_id)       → ContextPreview
```

## Tauri Events

```
Backend → Frontend:
  session:new          — new session discovered
  session:message      — new message in tracked session
  session:status       — session status changed
  brainstorm:token     — streaming token from LLM
  brainstorm:complete  — response finished

Frontend → Backend:
  via Tauri commands (listed above)
```

## Desktop UI Routes

```
/sessions                              — Session sidebar + mirror view
/sessions/:sessionId                   — Specific session mirror
/sessions/:sessionId/brainstorm/:id    — Brainstorm chat for a session
```

## UI Components

- `SessionSidebar` — Project-grouped session list with status indicators (active/idle/completed)
- `SessionMirrorView` — Chat stream with collapsible tool/agent/MCP blocks (transparency module pattern)
- `BrainstormPanel` — Chat interface with model/agent selector and action bar
- `PinnedContext` — Shows pinned messages above brainstorm input, drag-to-reorder
- `SendConfirmDialog` — Confirmation dialog for Send to Claude Code action
