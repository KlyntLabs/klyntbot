---
name: klyntbot-okr
description: >
  Manage objectives and key results (OKRs) using Klyntbot.
  Use when the user mentions objectives, key results, OKRs, goals,
  metrics, tracking progress, or quarterly planning.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [okr, project]
    mcp_tools: []
---

Use the `klyntbot - okr` MCP tool. Actions use dotted namespace (e.g., `objective.create`).

## Quick Reference

| Action | Key params |
|--------|-----------|
| `objective.create` | project_id, title, description, priority, due_date |
| `objective.list` | project_id, status |
| `kr.create` | objective_id, title, tracking_mode ("metric"/"action"), target_value, unit |
| `kr.update_metric` | id, current_value |

## Workflow

1. `project(action: "list")` — get project_id
2. `objective.create` — under a project
3. `kr.create` — 2-4 measurable key results per objective
4. `kr.update_metric` — track progress over time

For scoring guide and review workflows, read `references/reviews.md`.
