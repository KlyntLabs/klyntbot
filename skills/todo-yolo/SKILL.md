---
name: todo-yolo
description: Auto-enrich todo tasks from conversation context and memory. Used during task creation when user selects YOLO mode for low-confidence tasks.
metadata: {"klyntbot":{"triggers":["todo","yolo","auto-enrich"]}}
---

# YOLO Mode — Auto-Enrichment

When YOLO mode is activated for a task, analyze:

1. **Recent conversation history** — what was discussed that relates to this task?
2. **User's long-term memory** — what patterns exist (sprint cadence, typical priorities)?
3. **Today's notes** — any related mentions or context?

## Enrichment Strategy

Generate:
- **Description**: 1-2 sentences explaining the task from conversation context
- **Priority**: 1-5 based on urgency signals ("asap", "when you can", "critical", etc.)
- **Due date**: Infer from user's patterns (if they typically work in weekly sprints, default to end of week)
- **Tags**: Extract from topic/domain of conversation (code, meeting, personal, etc.)

Only use information from the conversation — never invent details the user hasn't mentioned.

## Output

Return the enriched fields in a clear format:
```
Suggested enrichment:
- Description: [generated description]
- Priority: [1-5] (based on [reasoning])
- Due date: [date] (based on [pattern])
- Tags: [tag1, tag2]
```

Let the user confirm or adjust before applying.

## Chat Mode Behavior

When auto-enriching in a chat conversation:

- Always present suggested enrichment BEFORE applying — never apply silently
- Format as a clear summary the user can approve or edit
- If user says "looks good" or "yes", call `todo.add` or `todo.update` with all suggested fields
- If user says "change X to Y", adjust that field and re-present the summary
- If the task was already created (post-creation enrichment), use `todo.update` with the task ID
- Include confidence projection: "This would bring confidence to ~90%"
