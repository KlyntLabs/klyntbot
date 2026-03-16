---
name: klyntbot-tasks
description: >
  Use when the user mentions todos, tasks, projects, areas, planning, goals,
  decompose, subtasks, time tracking, "add to my list", "what should I work on",
  "plan my day", "break this down", or asks to organize work into actionable items.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [tasks, area, project]
    mcp_tools: ["google-calendar"]
    max_iterations: 12
    invokes: [klyntbot-productivity, klyntbot-okr]
---

## Quick Reference

| Action | Tool | Key params |
|--------|------|-----------|
| Create task | `tasks` | action: "create", title, area_id, priority |
| List tasks | `tasks` | action: "list", status, area_id |
| Complete task | `tasks` | action: "complete", id |
| Plan my day | `tasks` | action: "plan_day", count: 3 |
| Break down task | `tasks` | action: "decompose", id |
| Focus on task | `tasks` | action: "focus", id |
| Search | `tasks` | action: "search", query |
| List areas | `area` | action: "list" |
| List projects | `project` | action: "list" |

Use the `klyntbot` MCP tools (`tasks`, `area`, `project`) for task management.

## Creating a Task — MANDATORY WORKFLOW

**Never create a task without confirming details with the user first.**

### Step 1: List areas and pick one
`area(action: "list")` — get available areas with IDs.
- If only 1 area: auto-select it
- If multiple areas: use **AskUserQuestion** to ask which area (list them by name)

### Step 2: Propose details — USE AskUserQuestion before creating
Based on what the user said, **infer** the best values, then use the **AskUserQuestion tool** to confirm:

```
AskUserQuestion: "I'll create this task — confirm or adjust:

• Title: [verb-led title]
• Area: [area name]
• Priority: [1-5] ([reason])
• Due: [date if known]
• Estimate: [minutes]
• Energy: [low/medium/high/deep]
• Tags: [tags]

Reply 'yes' to create, or tell me what to change."
```

**You MUST use the AskUserQuestion tool** — do NOT just print text and assume confirmation. Wait for the user's explicit response before calling create.

### Step 3: Create after confirmation
`tasks(action: "create", title: "...", area_id: "...", priority: N, ...)` with confirmed values.

### Inference Guide
| Context clue | Inferred value |
|-------------|---------------|
| "urgent" / "asap" / "critical" | priority: 1 |
| "important" / "high priority" | priority: 2 |
| No urgency mentioned | priority: 3 (medium) |
| "when I get to it" / "someday" | status: "someday" |
| "by Friday" / "tomorrow" / date | due_date: parsed date |
| Complex / multi-step | energy: "deep", suggest decompose |
| Quick / simple | energy: "low", estimate: 15min |

For full action reference, read `references/actions.md`.
For worked examples, read `references/workflows.md`.

## Common Mistakes

1. **Creating a task without fetching areas first** — You MUST call `area(action: "list")` before creating. Never hardcode or guess an `area_id`.
2. **Skipping AskUserQuestion** — Never create a task based on assumptions. Always confirm with the user via AskUserQuestion before calling create.
3. **Guessing IDs** — Task IDs, area IDs, and project IDs must come from list/search results. Never fabricate them.
4. **Using wrong status values** — Valid statuses: "todo", "in_progress", "done", "someday". Not "pending", "completed", etc.
5. **Forgetting to suggest decompose** — When a task is complex or multi-step, suggest breaking it down with `action: "decompose"`.

## Red Flags — STOP

If you're about to do any of these, STOP:
- Create a task without calling `area(action: "list")` first
- Use a hardcoded or memorized `area_id` from a previous conversation
- Skip the AskUserQuestion confirmation step
- Pass an ID you didn't retrieve in this session
- Create multiple tasks in a loop without user confirmation for each

## Related Skills

- **klyntbot-productivity** — Start focus sessions on tasks, track time spent
- **klyntbot-okr** — Link tasks to objectives and key results for goal tracking
- **klyntbot-automation** — Set up recurring task reminders
- **klyntbot-work-context** — View what work context a task belongs to
