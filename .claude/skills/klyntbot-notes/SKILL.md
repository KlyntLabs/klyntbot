---
name: klyntbot-notes
description: >
  Create, search, and organize notes and notebooks using Klyntbot.
  Use when the user mentions notes, notebooks, writing something down,
  jotting down, or linking information together.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [notes]
    mcp_tools: []
---

Use the `klyntbot - notes` MCP tool for note management.

## Quick Reference

| Action | Key params |
|--------|-----------|
| `create_note` | title, body, notebook_id, tags |
| `list_notes` | notebook_id (optional) |
| `search_notes` | query |
| `update_note` | id, title/body/tags |
| `tag_note` | id, tags[] |
| `link_notes` | id, target_ids[] |
| `create_notebook` | title, icon |
| `list_notebooks` | — |

## Workflow

1. `list_notebooks` — find or create the right notebook
2. `create_note` — with body, tags, and notebook_id
3. Suggest linking to related notes if context suggests connections

For detailed examples, read `references/workflows.md`.
