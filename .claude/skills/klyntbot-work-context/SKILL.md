---
name: klyntbot-work-context
description: >
  View and manage work context sessions using Klyntbot.
  Use when the user mentions work context, what am I working on,
  current context, or project context linking.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [work_context]
    mcp_tools: []
---

Use the `klyntbot - work_context` MCP tool for work context management.

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `list` | — | Show all work context sessions |
| `show` | id | View a specific context session |
| `rename` | id, title | Rename a context session |
| `link_project` | id, project_id | Link context to a project |
| `search` | query | Search across work contexts |

## When to Use

- User asks "what am I working on?" or "show my context"
- User wants to link activity to a project
- User wants to search past work contexts
