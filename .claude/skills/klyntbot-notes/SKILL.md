---
name: klyntbot-notes
description: >
  Use when the user mentions notes, notebooks, writing something down, jotting
  down, journaling, linking information together, "save this", "note that",
  "write down", or wants to capture and organize free-form text.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [notes]
    mcp_tools: []
    invokes: [klyntbot-memory, klyntbot-tasks]
---

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `create_note` | title, body, notebook_id, tags | User wants to save or write something |
| `list_notes` | notebook_id (optional) | User wants to see their notes |
| `search_notes` | query | User wants to find a specific note |
| `update_note` | id, title/body/tags | User wants to edit an existing note |
| `tag_note` | id, tags[] | User wants to categorize a note |
| `link_notes` | id, target_ids[] | User wants to connect related notes |
| `create_notebook` | title, icon | User wants a new notebook/category |
| `list_notebooks` | -- | User wants to see available notebooks |

Use the `klyntbot - notes` MCP tool for note management.

## Workflow

1. `list_notebooks` — find or create the right notebook
2. `create_note` — with body, tags, and notebook_id
3. Suggest linking to related notes if context suggests connections

For detailed examples, read `references/workflows.md`.

## Common Mistakes

1. **Creating notes without a notebook** — Always call `list_notebooks` first to find the right notebook_id. Don't create orphan notes.
2. **Guessing note or notebook IDs** — Always retrieve IDs from list/search results in the current session.
3. **Not suggesting tags** — When creating a note, infer useful tags from the content and suggest them to the user.
4. **Missing linking opportunities** — If the note relates to existing notes, suggest linking them with `link_notes`.
5. **Overwriting instead of updating** — When the user wants to add to a note, use `update_note` to append, don't recreate it.

## Related Skills

- **klyntbot-memory** — Search past conversations for context to include in notes
- **klyntbot-tasks** — Create tasks from action items found in notes
- **klyntbot-work-context** — Link notes to active work contexts
