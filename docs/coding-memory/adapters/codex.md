# Codex Adapter

## Overview

The `CodexAdapter` normalizes OpenAI Codex CLI hook events into `AgentEvent`.

## Transport

Codex is configured via its TOML settings file to invoke:

```bash
klyntbot-hook codex <hook-event>
```

## Captured Events (5)

| Hook Event | EventKind | Notes |
|---|---|---|
| `SessionStart` | `SessionStart` | `model`, `source_reason` |
| `SessionEnd` | `SessionEnd` | `reason` |
| `UserPromptSubmit` | `UserPrompt` | `text`, `attachments` |
| `AssistantResponse` | `AssistantMsg` | `text`, `truncated`, `token_usage` |
| `ToolUse` | `ToolCall` / `FileEdit` / `TestRun` | Dispatch by `tool_name` |

## Tool Dispatch

- `bash` / `shell` → classified as `TestRun` if command matches test framework patterns
- `read` → `FileEdit` (`FileOp::Read`)
- `write` → `FileEdit` (`FileOp::Create`)
- `edit` → `FileEdit` (`FileOp::Modify`)
- Other → `ToolCall`
