# Opencode Adapter

## Overview

The `OpencodeAdapter` polls opencode's local SQLite database for messages and normalizes them into `AgentEvent`.

## Transport

**Poll-only** — no shell hook. The daemon spawns an `OpencodePoller` task that diffs the `messages` table every 500 ms.

```toml
[coding_memory.opencode]
enabled = false
poll_interval_ms = 500
db_path = "~/.opencode/messages.db"
```

## Schema

```sql
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,  -- 'system' | 'user' | 'assistant' | 'tool'
    content TEXT NOT NULL,
    tool_calls TEXT,     -- JSON array of tool calls
    tool_call_id TEXT,   -- ID for tool response messages
    metadata TEXT,       -- JSON metadata
    created_at TEXT NOT NULL
);
```

## Normalization

| `role` | EventKind | Notes |
|---|---|---|
| `system` | Skipped | Not recorded |
| `user` | `UserPrompt` | `content` → `text` |
| `assistant` | `AssistantMsg` or `ToolCall` | Heuristic JSON detection for tool calls |
| `tool` | `ToolCall` | `content` → `result_preview` |
