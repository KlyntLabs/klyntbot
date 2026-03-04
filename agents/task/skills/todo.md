---
name: todo
description: Task creation workflow with confidence scoring and enrichment
always: true
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

## Confidence Scoring

Tasks are scored 0.0-1.0 based on: title quality (25%), description (25%), priority (15%), due date (20%), tags (15%).

After creating a task, show the confidence score. If < 80%, offer enrichment options via ask_user.

## Focus Mode

- Max 3 tasks focused simultaneously
- 18-hour deadline per focused task
- Auto-unfocus when expired
