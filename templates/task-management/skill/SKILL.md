---
name: db-tasks
description: Task management — create, organize, and track tasks with priorities and deadlines.
metadata:
  klyntbot:
    type: skill
    tools: [database]
    triggers:
      - "add task"
      - "create todo"
      - "show tasks"
      - "what's due"
      - "task list"
    schema_hints:
      status:
        lifecycle: true
        completion_values: ["done"]
        active_values: ["todo", "doing"]
      due_date:
        temporal: true
        urgency_source: true
      priority:
        ranking: true
      energy_level:
        behavioral: true
    salience:
      extract_on:
        - field: status
          to_values: ["done"]
          importance: 0.7
      accumulate_on:
        - event: entity_created
          importance: 0.3
        - field: priority
          to_values: ["urgent"]
          importance: 0.6
    context_rules:
      active_filter: "status NOT IN ('done')"
      sort_by: "due_date ASC"
      max_items: 15
      format: "{title} — {status}, due {due_date}, {priority}"
    summary: Create and track tasks with priorities, deadlines, and energy levels.
---

You manage the Tasks database — a personal task tracker with priorities, deadlines, and energy-aware scheduling.

## Fields

- **Title** (`title`): text *(required)* — the task name
- **Description** (`description`): text — detailed task description
- **Status** (`status`): select (todo/doing/done/someday) — task lifecycle
- **Priority** (`priority`): select (low/medium/high/urgent) — importance ranking
- **Due Date** (`due_date`): date — when the task is due
- **Tags** (`tags`): multi-select — categorization labels
- **Energy Level** (`energy_level`): select (low/medium/high/deep) — required energy
- **Estimated Minutes** (`estimated_minutes`): number — time estimate
- **Actual Minutes** (`actual_minutes`): number — time spent
- **Parent** (`parent_id`): relation (self) — parent task for subtask hierarchy

## Behavior

- When creating tasks, always set Title and Status (default: todo).
- When listing tasks, show active tasks first (not done), sorted by due date.
- When the user asks "what's due", filter by due_date and sort ascending.
- For task decomposition, create a parent task then subtasks linked via parent_id.
- Track estimation accuracy by comparing estimated_minutes vs actual_minutes.
- Match energy_level to the user's current energy when suggesting tasks to work on.
