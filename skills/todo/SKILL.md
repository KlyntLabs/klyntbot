---
name: todo
description: Task creation workflow — ask-first by default, with confidence scoring and enrichment modes.
metadata: '{"klyntbot":{"triggers":["todo","task","focus"],"always":true}}'
---

# Todo Task Creation

## CRITICAL RULE: Every Task Needs an Area

Every task belongs to an area. You MUST determine the area before creating any task. The system prompt includes "Available areas" with names and IDs — use those.

## CRITICAL RULE: Ask Before Creating

When the user asks to create a task, you MUST follow this workflow:

### Step 0: Determine the area

Before anything else, resolve which area this task belongs to:

- **If the user specifies an area** (e.g., "add task to Work: fix bug") → use that area's ID directly
- **If only 1 active area exists** → auto-assign it. Mention which area in your response (e.g., "Added to Work area")
- **If multiple active areas exist and user didn't specify** → ask the user via `ask_user` with a `single_select` of available areas:

```json
{
  "id": "area",
  "title": "Area",
  "text": "Which area does this belong to?",
  "type": "single_select",
  "options": [
    {"value": "area_abc", "label": "Personal"},
    {"value": "area_def", "label": "Work"}
  ]
}
```

Use the area names and IDs from the "Available areas" section in your context.

### Step 1: Assess — Is the request detailed enough?

A request is "detailed enough" if it has:
- A clear title (> 3 words describing a specific action)
- OR the user explicitly provides priority, due date, or description

**Detailed enough examples (create immediately):**
- "add task to Work: buy milk from the corner store, due tomorrow"
- "create task: fix authentication bug in login flow, priority high"
- "todo: review PR #42 for the payments refactor"

**NOT detailed enough examples (must ask first):**
- "add task: buy"
- "create task: fix"
- "todo: meeting"
- "task: stuff"

### Step 2: If NOT detailed enough — Use ask_user FIRST

Call the `ask_user` tool to gather details BEFORE calling `todo add`. Include area selection if multiple areas exist and area wasn't already determined in Step 0:

```json
{
  "title": "New Task Details",
  "questions": [
    {
      "id": "area",
      "title": "Area",
      "text": "Which area does this belong to?",
      "type": "single_select",
      "options": [
        {"value": "area_abc", "label": "Personal"},
        {"value": "area_def", "label": "Work"}
      ]
    },
    {
      "id": "title",
      "title": "Title",
      "text": "What specifically do you want to do? (e.g., 'buy groceries for dinner tonight')",
      "type": "free_text",
      "placeholder": "Describe the task..."
    },
    {
      "id": "priority",
      "title": "Priority",
      "text": "How urgent is this?",
      "type": "single_select",
      "options": [
        {"value": "1", "label": "Urgent", "description": "Do today"},
        {"value": "2", "label": "High", "description": "Do this week"},
        {"value": "3", "label": "Medium", "description": "Normal priority"},
        {"value": "4", "label": "Low", "description": "When you get to it"}
      ]
    }
  ]
}
```

After ask_user returns, call `todo add` with the gathered details AND `confirmed: true`.

### Step 3: If detailed enough — Create with confirmed=true

Call `todo add` with all user-provided fields, `area_id`, and `confirmed: true`:

```json
{
  "action": "add",
  "title": "Buy milk from the corner store",
  "area_id": "area_abc",
  "due_date": "tomorrow",
  "confirmed": true
}
```

### NEVER DO THIS

- NEVER call `todo add` without `area_id`
- NEVER guess or fabricate an `area_id` — use the IDs from "Available areas" context
- NEVER expand a vague title into a specific one without asking (e.g., "buy" → "Buy groceries")
- NEVER invent a description the user didn't provide
- NEVER guess priority, due date, or tags
- NEVER call todo add with optional fields the user didn't explicitly state
- If in doubt, call todo add with ONLY the title and area_id — the enrichment engine will suggest improvements

## Optional: Link to Project or OKR

After confirming the area, you can optionally link the task to a project or OKR key result within that area:

- Use `area show` with the area ID to see its projects
- Use `project list` filtered by area to discover projects
- Set `key_result_id` on the task to link it to an OKR key result

This is optional but improves the confidence score.

## Confidence Scoring

Tasks are scored 0.0-1.0 based on:
- Title quality (25%): > 3 words
- Description (25%): > 10 chars
- Priority (15%): set to 1-5
- Due date (20%): concrete deadline
- Tags (15%): at least one tag

After creating a task, show the confidence score. If < 80%, offer enrichment options via ask_user.

## Focus Mode

- Max 3 tasks focused simultaneously
- 18-hour deadline per focused task
- Auto-unfocus when expired

## Deep Dive

For advanced task management topics, load these references with `read_file` when needed:

- **Enrichment engine**: See `CLAUDE.md` section "Enrichment Configuration" for keyword-based priority/duration inference
- **Semantic search**: See `CLAUDE.md` section "Semantic Search" for embedding-based task discovery
- **Creation modes**: See `CLAUDE.md` section "Task Creation Mode" for ask-first vs yolo vs party modes
