---
name: task-actions
description: Complete reference for all 27 task tool actions with parameters
---

# Task Actions — Complete Reference

## Create & Mutate

### create
```json
{
  "action": "create",
  "title": "Clear verb-led title",
  "area_id": "uuid (required — list areas first)",
  "description": "What and why (markdown)",
  "priority": 2,
  "due_date": "2026-03-20",
  "tags": ["dev", "release"],
  "estimated_minutes": 60,
  "energy_level": "high",
  "task_type": "manual",
  "project_id": "uuid",
  "parent_id": "uuid (for subtasks)",
  "key_result_id": "uuid (link to OKR)",
  "objective_id": "uuid",
  "acceptance_criteria": "for agentic tasks",
  "agent_config": "{\"requireApproval\": true}"
}
```

| Field | Values | When to set |
|-------|--------|-------------|
| priority | 1 (critical) → 5 (nice-to-have) | Always infer from urgency |
| due_date | ISO: "YYYY-MM-DD" or "YYYY-MM-DD HH:MM" | If deadline mentioned |
| estimated_minutes | Integer (15, 30, 60, 120, 240) | If scope is clear |
| energy_level | "low" (admin), "medium" (standard), "high" (creative), "deep" (complex) | Based on complexity |
| task_type | "manual" (default), "agentic" (auto-execute), "hybrid" | If user wants automation |
| status | "todo" (default), "doing", "done", "someday" | "someday" for backlog |

### update
Same fields as create, plus `id` (required). Pass only fields to change.
Clear a field by passing empty string or "null".

### complete
`{action: "complete", id: "..."}` — checks blockers, logs time, cascades to subtasks, updates KR progress.

### delete
`{action: "delete", id: "..."}`

## Query & Search

### show
`{action: "show", id: "..."}` — full details + last 5 activity log entries.

### list
```json
{
  "action": "list",
  "status": "todo",
  "priority_min": 1,
  "tag": "dev",
  "area_id": "uuid",
  "project_id": "uuid",
  "energy_level": "high",
  "task_type": "agentic",
  "limit": 20,
  "unassigned": true
}
```

### search
`{action: "search", query: "...", limit: 10}` — hybrid keyword + semantic search.

### summary
`{action: "summary"}` — task counts by status.

### tree
`{action: "tree"}` — hierarchy with dependencies.

## Focus & Time

### focus
`{action: "focus", id: "..."}` — starts focus timer (max 3 concurrent, 8h deadline).

### unfocus
`{action: "unfocus", id: "..."}` — stops timer, logs duration.

### log_time
`{action: "log_time", id: "...", duration_minutes: 30, note: "code review", energy_level: "medium"}`

## Dependencies

### add_dep
`{action: "add_dep", task_id: "...", blocked_by: "...", dep_type: "blocks"}` — types: "blocks", "depends_on", "related".

### remove_dep
`{action: "remove_dep", task_id: "...", blocked_by: "..."}`

## Planning & Decomposition

### plan_day
`{action: "plan_day", count: 3, energy_level: "medium"}` — AI-powered daily plan with scoring.

### decompose
`{action: "decompose", id: "..."}` — AI breaks task into subtasks. Auto-applies if confidence is high.

## Recurrence

### recur
```json
{
  "action": "recur",
  "title": "Weekly standup prep",
  "rule": "FREQ=WEEKLY;BYDAY=MO;BYHOUR=9",
  "area_id": "uuid"
}
```

### list_recurring / delete_recurring
`{action: "list_recurring"}` / `{action: "delete_recurring", template_id: "..."}`

## Execution (Agentic Tasks)

### execute
```json
{
  "action": "execute",
  "id": "...",
  "agent_profile": "task",
  "max_cost": 0.50,
  "max_iterations": 10,
  "timeout_secs": 300,
  "require_approval": false
}
```

### cancel_execution
`{action: "cancel_execution", execution_id: "..."}`

## Suggestions & Forecasting

### suggest
`{action: "suggest", project_id: "..."}` — AI generates reprioritize/reschedule/decompose suggestions.

### apply_suggestion / dismiss_suggestion / list_suggestions
`{action: "apply_suggestion", suggestion_id: "..."}` — executes suggestion.

### forecast_task
`{action: "forecast_task", id: "..."}` — duration estimate with confidence bounds.

### forecast_project
`{action: "forecast_project", project_id: "..."}` — project timeline forecast.

### accuracy_report
`{action: "accuracy_report", project_id: "..."}` — historical estimation accuracy.

## Batch Operations

```json
{
  "action": "batch",
  "operations": [
    {"op": "complete", "id": "abc"},
    {"op": "tag", "id": "def", "tags": ["shipped"]},
    {"op": "reorder", "id": "ghi", "position": 1}
  ]
}
```
