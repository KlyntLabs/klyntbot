---
name: project-management
description: Project, area, and OKR management with health scoring and stagnation detection
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-04-02"
  source: official
  tags: "project,manage,plan,milestone"
  always: false
  triggers: "project status,project health,how are my projects,stagnation,okr check"
  agent: task
---

## Framework

Tasks are organized using PARA (Projects, Areas, Resources, Archives) combined with OKR (Objectives and Key Results).

## Areas

Areas are top-level categories (e.g., Work, Personal, Health). Every task belongs to an area.

- `area(action: "list")` — show all areas (optional `status` filter)
- `area(action: "create")` — create a new area
- `area(action: "show")` — view area details with project/task counts
- `area(action: "update")` — update name, description, color, icon, status
- `area(action: "delete")` — permanently remove an area
- `area(action: "reorder")` — change display position

## Projects

Projects group related tasks within an area. They have deadlines, instructions, and completion targets.

- `project(action: "list")` — list projects (filterable by area, status, tag)
- `project(action: "create")` — create a project within an area (supports `start_date`, `target_end_date`)
- `project(action: "show")` — view project details with tasks and all metadata
- `project(action: "update")` — update any field including `instructions`, `ai_personality`, `user_role`, `start_date`, `target_end_date`, `settings`, `workflow_id`. Pass `null` to clear nullable fields.
- `project(action: "delete")` — permanently remove a project (linked tasks keep their data)
- `project(action: "archive")` — soft-archive a project
- `project(action: "tasks")` — list tasks belonging to a project

## OKRs

Objectives and Key Results provide goal-setting at the project level.

- `okr(action: "objective.list")` — list objectives (filterable by project, status)
- `okr(action: "objective.create")` — create an objective with title, description, priority, due_date
- `okr(action: "objective.show")` — view objective with KRs, timestamps, and completion info
- `okr(action: "objective.update")` — update title, description, status, priority, due_date
- `okr(action: "objective.delete")` — delete an objective
- `okr(action: "kr.create")` — add a measurable key result (metric or action tracking mode)
- `okr(action: "kr.update")` — update KR title, description, status, due_date
- `okr(action: "kr.update_metric")` — update current value for metric-mode KRs
- `okr(action: "kr.set_progress")` — directly set progress (0-100) for action-mode KRs
- `okr(action: "kr.delete")` — delete a key result

## Project Health Scoring

When showing projects (via `project list` or `project show`), calculate and display a health indicator:

- **🟢 On Track**: >50% tasks completed AND no overdue tasks AND deadline not within 3 days
- **🟡 At Risk**: Has overdue tasks OR <30% completion with deadline within 2 weeks
- **🔴 Off Track**: >3 overdue tasks OR <10% completion with deadline within 1 week
- **⚪ Stale**: No task completions in the last 14 days

### Stagnation Detection

When a project has had no task completions in 14+ days, proactively flag it:

```
⚪ "API Migration" hasn't had any completions in 18 days.
   Last activity: Mar 2 (task created)
   → Is this still active? [Continue | Pause | Archive]
```

When a user asks "how are my projects" or "project status", show the health dashboard:

```
Project Health Dashboard:

🟢 Website Redesign — 8/12 tasks (67%), due Apr 15
🟡 API Migration — 3/10 tasks (30%), 2 overdue, due Mar 30
🔴 Q1 Report — 0/5 tasks (0%), due in 4 days
⚪ Blog Rewrite — no activity in 21 days

1 project needs urgent attention. Want to drill into Q1 Report?
```

## OKR Check-In Cadence

Prompt OKR check-ins at the right cadence:

- **Weekly** (during weekly review): Quick progress pulse — "Any movement on KR1?"
- **Monthly**: Deeper assessment — "Is effort compounding toward this objective?"
- **Quarterly**: Formal scoring (see retrospective skill)

When updating Key Result progress, use the PPP format:
- **Progress**: What changed since last update
- **Plans**: What's planned next
- **Problems**: What's blocking

## Workflow

1. User creates areas for life domains
2. Within areas, create projects for time-bound initiatives
3. Set objectives with measurable key results
4. Tasks link to projects and key results for tracking
5. Project health is continuously monitored and surfaced during reviews
