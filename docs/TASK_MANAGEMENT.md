# Task & Project Management Guide

Klyntbot includes a built-in task and project management system. All task and project management is done through **natural language chat** — via any connected channel (Telegram, Discord, Slack, etc.) or the built-in REPL (`klyntbot chat`). All data is stored in PostgreSQL.

---

## Table of Contents

- [Getting Started](#getting-started)
- [Managing Tasks](#managing-tasks)
- [Focus Mode & Time Tracking](#focus-mode--time-tracking)
- [Subtasks & Hierarchies](#subtasks--hierarchies)
- [Projects](#projects)
- [Recurring Tasks](#recurring-tasks)
- [Task Dependencies](#task-dependencies)
- [Attachments](#attachments)
- [Smart Reminders & Notifications](#smart-reminders--notifications)
- [Apple Calendar Sync](#apple-calendar-sync)
- [Reports & Analytics](#reports--analytics)
- [Chat Examples](#chat-examples)
- [Reference](#reference)

---

## Getting Started

Start a chat session and talk to klyntbot in natural language:

```bash
klyntbot chat
```

### Your first task

```
You:  "Add a task to buy groceries, priority 3, due tomorrow, tag it shopping"
Bot:  Created task [abc123] "Buy groceries" — P3, due Feb 19, tags: shopping
```

### View your tasks

```
You:  "Show my tasks"
You:  "List tasks that are in progress"
You:  "Show shopping tasks"
You:  "What's high priority?"
```

### Complete a task

```
You:  "Complete the groceries task"
Bot:  Completed [abc123] "Buy groceries"
```

---

## Managing Tasks

All task operations are available through natural language:

- **Create**: "Add a task to...", "Create a task for..."
- **View**: "Show task abc123", "Give me a task summary"
- **Update**: "Change the priority of abc123 to 5", "Update the due date to next week"
- **Search**: "Search for groceries", "Find tasks about design"
- **Delete**: "Delete task abc123"

The agent uses the `TodoTool` internally, which supports 16+ actions including semantic search (pgvector-powered).

---

## Focus Mode & Time Tracking

Focus mode helps you concentrate on specific tasks while automatically tracking your time.

```
You:  "Focus on task abc123"          # Start a focus session
You:  "Show my focus board"           # See focused tasks with time remaining
You:  "Unfocus abc123"                # End focus (time auto-recorded)
You:  "Log 45 minutes on abc123"      # Manual time logging
```

The focus board has configurable slots to encourage working on fewer things at once.

---

## Subtasks & Hierarchies

Break large tasks into smaller pieces up to 16 levels deep.

```
You:  "Add a subtask to abc123: Design the header"
You:  "Show the task tree"
You:  "Move abc123 under def456"      # Make it a subtask
You:  "Move abc123 to top level"      # Remove parent
```

Tree view output:
```
├─ ○ Build website [abc123]  P4  Feb 20
│  ├─ ✓ Design mockups [def456]  P3
│  └─ ○ Implement frontend [ghi789]  P4
└─ ● Write documentation [jkl012]  P2
```

---

## Projects

Group related tasks into projects for better organization.

```
You:  "Create a project called Website Redesign, color blue, tags client and design"
You:  "List my projects"
You:  "Show project tasks for proj123"
You:  "Archive the website project"
```

Project lifecycle: **Active** → **Paused** → **Completed** → **Archived**

---

## Recurring Tasks

Set up tasks that repeat automatically on a schedule.

```
You:  "Create a recurring task for daily standup at 9am"
You:  "Add a weekly review every Friday at 4pm"
You:  "Set up a monthly report on the first of each month"
You:  "List my recurring tasks"
You:  "Delete recurring template tmpl123"
```

Common schedule patterns (RRULE format used internally):
| Schedule | Rule |
|----------|------|
| Every day at 9am | `FREQ=DAILY;BYHOUR=9` |
| Every weekday | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR` |
| Every Monday | `FREQ=WEEKLY;BYDAY=MO` |
| First of each month | `FREQ=MONTHLY;BYMONTHDAY=1` |

---

## Task Dependencies

Mark tasks that must be completed before others can start.

```
You:  "abc123 is blocked by def456"
You:  "Remove the dependency between abc123 and def456"
You:  "What blocks abc123?"
```

If you try to complete a task that has incomplete blockers, klyntbot will tell you which tasks need to be finished first.

---

## Attachments

Add files, links, or notes to any task.

```
You:  "Attach this file to abc123: /path/to/mockup.png"
You:  "Add a link to abc123: https://docs.example.com/spec"
You:  "Add a note to abc123: Remember to check edge cases"
You:  "Remove attachment att_456 from abc123"
```

---

## Smart Reminders & Notifications

Klyntbot proactively reminds you about deadlines and events. Reminders are sent to your configured notification targets (OS notifications and/or chat channels like Telegram, Discord, etc.).

| Reminder | When |
|----------|------|
| Deadline approaching | 2 hours, 1 hour, 30 min, 15 min before due |
| Focus session expiring | When focus time is about to run out |
| Overdue tasks | Daily reminder for tasks past their due date |
| Calendar events | 30 minutes before upcoming events |

### Configure notifications

In your config (`~/.klyntbot/config.json`):

```json
{
  "todo": {
    "notifications": {
      "enabled": true,
      "targets": ["os_native", "telegram"]
    }
  }
}
```

---

## Apple Calendar Sync

Tasks with due dates can sync to Apple Calendar, appearing on your iPhone, Mac, and iPad.

### Setup

See [CALENDAR_SETUP.md](CALENDAR_SETUP.md) for initial configuration.

### Use

```
You:  "Sync my calendar"
You:  "Show calendar status"
You:  "List events from Feb 14 to Feb 21"
```

Once configured, tasks with due dates automatically appear in your Apple Calendar. Changes sync in the background at regular intervals.

---

## Reports & Analytics

```
You:  "Show my task report"
You:  "Monthly productivity report"
You:  "Report for the website project"
```

Reports include:
- Tasks created and completed in the period
- Total time tracked (focus sessions + manual entries)
- Completion rate
- Priority distribution (P1-P5 breakdown)
- Overdue task highlights

---

## Chat Examples

```
You:  "Add a task to review the pull request, high priority, due tomorrow"
Bot:  Created task [abc123] "Review the pull request" — P4, due Feb 16

You:  "What am I working on?"
Bot:  You have 3 focused tasks: ...

You:  "Show me my tasks for the website project"
Bot:  Here are 5 tasks in Website Redesign: ...

You:  "Mark the design task as done"
Bot:  Completed [def456] "Design mockups"

You:  "Create a recurring task for daily standup at 9am"
Bot:  Created recurring template — Every day at 9:00 AM

You:  "What's overdue?"
Bot:  2 tasks are overdue: ...
```

The AI has full access to all task operations — adding, updating, searching, focusing, managing dependencies, and more. It can also provide intelligent summaries and suggestions based on your task data.

---

## Reference

### Priority Scale

| Level | Meaning | When to use |
|:-----:|---------|-------------|
| 1 | Low | Nice-to-have, no deadline pressure |
| 2 | Normal | Standard tasks |
| 3 | Medium | Important, should be done this week |
| 4 | High | Urgent, needs attention today |
| 5 | Critical | Drop everything, do this now |

### Task Statuses

| Status | Meaning |
|--------|---------|
| `todo` | Not started |
| `doing` | In progress |
| `done` | Completed |
| `archived` | Hidden from default views |
