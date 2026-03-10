---
name: todo
description: Task creation with smart defaults, confidence scoring, and multi-task capture
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,todo,create,productivity"
  always: true
  triggers: ""
  agent: task
---

# Todo Task Creation

## CRITICAL RULE: Every Task Needs an Area

Every task belongs to an area. You MUST determine the area before creating any task. The system prompt includes "Available areas" with names and IDs — use those.

## CRITICAL RULE: Ask Before Creating

When the user asks to create a task, you MUST follow this workflow:

### Step 0: Determine the area

Before anything else, resolve which area this task belongs to:

- **If the user specifies an area** (e.g., "add task to Work: fix bug") — use that area's ID directly
- **If only 1 active area exists** — auto-assign it. Mention which area in your response
- **If multiple active areas exist and user didn't specify** — ask the user via `ask_user` with a `single_select` of available areas

### Step 1: Assess — Is the request detailed enough?

A request is "detailed enough" if it has:
- A clear title (> 3 words describing a specific action)
- OR the user explicitly provides priority, due date, or description

**Detailed enough (create immediately):**
- "add task to Work: buy milk from the corner store, due tomorrow"
- "create task: fix authentication bug in login flow, priority high"

**NOT detailed enough (must ask first):**
- "add task: buy"
- "create task: fix"
- "todo: meeting"

### Step 2: If NOT detailed enough — Use ask_user FIRST

Call the `ask_user` tool to gather details BEFORE calling `todo add`. Include area selection if needed.

After ask_user returns, call `todo add` with the gathered details AND `confirmed: true`.

### Step 3: If detailed enough — Create with confirmed=true

Call `todo add` with all user-provided fields, `area_id`, and `confirmed: true`.

### NEVER DO THIS

- NEVER call `todo add` without `area_id`
- NEVER guess or fabricate an `area_id`
- NEVER expand a vague title into a specific one without asking
- NEVER invent a description the user didn't provide
- NEVER guess priority, due date, or tags
- If in doubt, call todo add with ONLY the title and area_id

## Smart Defaults (Learning-Based)

When the system prompt includes behavioral patterns or agent adaptations, use them to pre-fill task fields:

- **Area inference**: If the user's pattern shows "Work" tasks are created 80%+ on weekday mornings, default to Work during those times
- **Priority inference**: If tasks in a project are historically P2, suggest P2 for new tasks in that project
- **Tag inference**: If the user consistently tags coding tasks with "dev", suggest "dev" for similar tasks

When applying a smart default, **always mention it**: "I've defaulted this to Work (your usual area at this time) — change?"

Never silently apply defaults. The user must see what was inferred and have the chance to override.

## Multi-Task Capture

When a user provides multiple tasks in one message (e.g., "I need to fix the login bug, send the report to Sarah, and buy groceries"), extract each task individually:

1. Parse the message for distinct actionable items
2. Show the extracted list to the user for confirmation via `ask_user`
3. Create each confirmed task separately with appropriate areas
4. Summarize all created tasks at the end

**Format for confirmation:**
```
I found 3 tasks in your message:
1. Fix the login bug → Work
2. Send the report to Sarah → Work
3. Buy groceries → Personal

Create all, or edit first?
```

## Confidence Scoring

Tasks are scored 0.0-1.0 based on: title quality (25%), description (25%), priority (15%), due date (20%), tags (15%).

After creating a task, show the confidence score. If < 80%, offer enrichment options via ask_user.

## Focus Mode

- Max 3 tasks focused simultaneously
- 18-hour deadline per focused task
- Auto-unfocus when expired
