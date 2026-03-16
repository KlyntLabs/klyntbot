---
name: todo
description: Task creation with smart defaults, confidence scoring, and multi-task capture
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-14"
  source: official
  tags: "task,todo,create,productivity"
  always: true
  triggers: ""
  agent: task
---

# Todo Task Creation

## MANDATORY WORKFLOW — Follow Every Time

**Before calling `tasks(action: "create")`, you MUST have an `area_id`. Never guess it.**

1. **List areas first**: `area(action: "list")` to get available areas with IDs
2. **Pick area**: If user specified one, use it. If only 1 exists, auto-assign. If multiple, use `ask_user` with `single_select` to let user choose.
3. **Assess detail**: If title is vague (< 3 words), use `ask_user` to clarify before creating.
4. **Create**: `tasks(action: "create", title: "...", area_id: "...", confirmed: true)`

## ask_user for Area Selection

```json
{"question": "Which area for this task?", "type": "single_select", "options": ["Work", "Personal", "Health"]}
```

## Smart Defaults

- If behavioral patterns are in the system prompt, use them to pre-fill area/priority/tags
- **Always mention** what was inferred: "Defaulted to Work (your usual area) — change?"

## Multi-Task Capture

When user gives multiple tasks in one message:
1. Parse each task
2. Show list via `ask_user` for confirmation
3. Create each separately

## Confidence Scoring

Tasks scored 0.0–1.0: title (25%), description (25%), priority (15%), due date (20%), tags (15%).
If < 80%, offer enrichment via `ask_user`.

## Focus Mode

- Max 3 focused tasks
- 18-hour deadline per focused task
- Auto-unfocus when expired
