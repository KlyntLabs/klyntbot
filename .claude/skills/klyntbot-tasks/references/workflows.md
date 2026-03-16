---
name: task-workflows
description: Worked examples for common task management workflows
---

# Task Workflows

## 1. Create a Rich Task

```
User: "I need to finish the API migration by next Friday, it's high priority"

Steps:
1. area(action: "list") → get areas
2. tasks(action: "create",
     title: "Finish API migration",
     area_id: "work-uuid",
     priority: 1,
     due_date: "2026-03-21",
     estimated_minutes: 480,
     energy_level: "deep",
     description: "Complete the API migration to the new endpoints",
     tags: ["api", "migration"])
3. Suggest: "This sounds complex — want me to break it into subtasks?"
4. If yes: tasks(action: "decompose", id: "new-task-id")
```

## 2. Daily Planning

```
User: "Plan my day"

Steps:
1. tasks(action: "plan_day", count: 3)
2. Present ranked tasks with scores
3. Ask: "Focus on these, swap any, or skip?"
4. If accepted: tasks(action: "focus", id: "top-task-id")
```

## 3. Weekly Review

```
User: "Review my week"

Phase 1 — Triage overdue:
1. tasks(action: "list", status: "todo") → find overdue
2. For each: ask "Complete | Reschedule | Drop"

Phase 2 — Project health:
1. project(action: "list") → show health indicators
2. Flag stale/blocked projects

Phase 3 — Plan next week:
1. tasks(action: "list") → upcoming deadlines
2. Ask top 3 priorities
3. Focus those tasks
```

## 4. Task Decomposition

```
User: "Break down the product launch"

Steps:
1. tasks(action: "show", id: "launch-task-id") → get context
2. tasks(action: "decompose", id: "launch-task-id")
3. AI generates subtasks with estimates and dependencies
4. Review with user → auto-creates approved subtasks
```

## 5. Time Tracking Workflow

```
User: "I'm going to work on the design review"

Steps:
1. tasks(action: "search", query: "design review") → find task
2. tasks(action: "focus", id: "task-id") → starts timer
... user works ...
3. User: "done with the design review"
4. tasks(action: "unfocus", id: "task-id") → stops timer, logs duration
5. tasks(action: "complete", id: "task-id") → if fully done
```

## 6. Recurring Tasks

```
User: "Remind me to review PRs every weekday at 10am"

Steps:
1. area(action: "list") → get area
2. tasks(action: "recur",
     title: "Review open PRs",
     rule: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=10",
     area_id: "work-uuid",
     energy_level: "medium",
     estimated_minutes: 30)
```

## 7. Batch Operations

```
User: "Complete all the tasks I finished today and tag them as shipped"

Steps:
1. tasks(action: "list", status: "doing") → find in-progress tasks
2. Confirm which ones are done
3. tasks(action: "batch", operations: [
     {op: "complete", id: "task1"},
     {op: "complete", id: "task2"},
     {op: "tag", id: "task1", tags: ["shipped"]},
     {op: "tag", id: "task2", tags: ["shipped"]}
   ])
```
