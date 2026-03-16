---
name: notes-workflows
description: Common note-taking workflows and examples
---

# Note Workflows

## Quick Capture

```
User: "jot down: the API uses OAuth2 with PKCE flow"

1. list_notebooks → find relevant notebook (e.g., "Dev Notes")
2. create_note(
     title: "API Auth: OAuth2 with PKCE",
     body: "The API uses OAuth2 with PKCE flow for authentication.",
     notebook_id: "dev-notes-uuid",
     tags: ["api", "auth", "oauth"])
```

## Meeting Notes

```
User: "create meeting notes for the design review"

1. create_note(
     title: "Design Review — 2026-03-14",
     body: "## Attendees\n\n## Discussion\n\n## Action Items\n\n",
     tags: ["meeting", "design"])
```

## Research & Linking

```
User: "I found some info about the migration, link it to my earlier note"

1. search_notes(query: "migration") → find related notes
2. create_note with new info
3. link_notes(id: "new-note-id", target_ids: ["earlier-note-id"])
```

## Organizing

```
User: "tag all my API notes with 'backend'"

1. search_notes(query: "API")
2. For each: tag_note(id: "...", tags: ["backend"])
```
