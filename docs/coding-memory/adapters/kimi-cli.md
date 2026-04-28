# Kimi CLI Adapter

## Overview

The `KimiAdapter` normalizes Moonshot kimi-cli hook events into `AgentEvent`.

## Transport

### Tier 1 — Shell Hooks

kimi-cli is configured via shell hooks to invoke:

```bash
klyntbot-hook kimi-cli <hook-event>
```

### Tier 2 — Wire Client (future)

A streaming WebSocket client connects to kimi-cli's local Wire server for richer `AssistantMsg` capture.

## Captured Events (13)

| Hook Event | EventKind | Notes |
|---|---|---|
| `SessionStart` | `SessionStart` | `model`, `source_reason` |
| `SessionEnd` | `SessionEnd` | `reason` |
| `UserPrompt` | `UserPrompt` | `text`, `attachments` |
| `AssistantMsg` | `AssistantMsg` | `text`, `truncated`, `token_usage` |
| `ToolCall` | `ToolCall` | `tool`, `args_preview`, `ok`, `duration_ms` |
| `FileEdit` | `FileEdit` | `path`, `op`, `bytes`, `diff_preview` |
| `TestRun` | `TestRun` | `command`, `framework`, `passed`, `failed` |
| `CompactEvent` | `CompactEvent` | `trigger`, `token_count` |
| `Error` | `Error` | `tool`, `message` |
| `SkillActivated` | — | Rich event (generic parse) |
| `RecallInjected` | — | Rich event (generic parse) |
| `ApprovalDecision` | — | Rich event (generic parse) |
| `ProviderCall` | — | Rich event (generic parse) |
