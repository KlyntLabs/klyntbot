---
name: klyntbot-okr
description: >
  Use when the user mentions objectives, key results, OKRs, goals, metrics,
  tracking progress, quarterly planning, "set a goal", "measure progress",
  "how am I doing on", or wants to define and track measurable outcomes.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [okr, project]
    mcp_tools: []
    invokes: [klyntbot-tasks, klyntbot-productivity]
---

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `objective.create` | project_id, title, description, priority, due_date | Define a new objective |
| `objective.list` | project_id, status | View existing objectives |
| `kr.create` | objective_id, title, tracking_mode, target_value, unit | Add a key result |
| `kr.update_metric` | id, current_value | Update progress on a key result |
| `project(action: "list")` | -- | Get project IDs (required first step) |

Use the `klyntbot - okr` MCP tool. Actions use dotted namespace (e.g., `objective.create`).

## Workflow

1. `project(action: "list")` — get project_id
2. `objective.create` — under a project
3. `kr.create` — 2-4 measurable key results per objective
4. `kr.update_metric` — track progress over time

For scoring guide and review workflows, read `references/reviews.md`.

## Common Mistakes

1. **Creating objectives without a project** — Always call `project(action: "list")` first to get a valid project_id.
2. **Vague key results** — Key results must be measurable. "Improve code quality" is bad. "Reduce bug count to under 5 per sprint" with tracking_mode: "metric", target_value: 5, unit: "bugs" is good.
3. **Too many key results** — Stick to 2-4 KRs per objective. More than that dilutes focus.
4. **Wrong tracking_mode** — Use "metric" for numerical targets (revenue, count). Use "action" for binary done/not-done milestones.
5. **Guessing IDs** — Always retrieve project_id, objective_id from list results. Never hardcode.

## Related Skills

- **klyntbot-tasks** — Break objectives down into actionable tasks
- **klyntbot-productivity** — Track focus time against OKR-linked projects
- **klyntbot-finance** — Set financial objectives with measurable key results
