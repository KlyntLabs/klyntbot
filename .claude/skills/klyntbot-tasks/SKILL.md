---
name: klyntbot-tasks
description: >
  Create, organize, and track tasks using Klyntbot.
  Use when the user mentions todos, tasks, projects, areas, planning,
  goals, focus, decompose, subtasks, or time tracking.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [tasks, area, project]
    mcp_tools: ["google-calendar"]
    max_iterations: 12
---

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

## Other Actions — Quick Reference

| Action | Tool | Key params |
|--------|------|-----------|
| List tasks | `tasks` | action: "list", status, area_id |
| Complete task | `tasks` | action: "complete", id |
| Plan my day | `tasks` | action: "plan_day", count: 3 |
| Break down task | `tasks` | action: "decompose", id |
| Focus on task | `tasks` | action: "focus", id |
| Search | `tasks` | action: "search", query |
| List areas | `area` | action: "list" |
| List projects | `project` | action: "list" |
