---
name: klyntbot-work-context
description: >
  Use when the user mentions work context, "what am I working on", current
  context, project context linking, "show my context", "what's active",
  session history, or wants to see or manage their active work sessions.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [work_context]
    mcp_tools: []
    invokes: [klyntbot-tasks, klyntbot-productivity]
---

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `list` | -- | Show all work context sessions |
| `show` | id | View a specific context session |
| `rename` | id, title | Rename a context session |
| `link_project` | id, project_id | Link context to a project |
| `search` | query | Search across work contexts |

Use the `klyntbot - work_context` MCP tool for work context management.

## When to Use

- User asks "what am I working on?" or "show my context"
- User wants to link activity to a project
- User wants to search past work contexts
- User asks about session history or what they were doing earlier

## Common Mistakes

1. **Guessing context IDs** — Always call `list` first to get valid session IDs before using `show`, `rename`, or `link_project`.
2. **Not linking to projects** — When a context session clearly relates to a project, suggest linking it with `link_project` for better organization.
3. **Confusing work context with memory** — Work context tracks active sessions and recent activity. For searching past conversations, use the memory skill instead.

## Related Skills

- **klyntbot-tasks** — View tasks related to the current work context
- **klyntbot-productivity** — Track focus sessions within a work context
- **klyntbot-memory** — Search past conversations for broader history beyond work contexts
- **klyntbot-notes** — Capture notes about the current work context
