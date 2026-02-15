# Task & Project Management Guide

Klyntbot includes a built-in task and project management system. You can manage your work through **CLI commands** or by **chatting with the AI** — both access the same underlying system, so your tasks stay in sync no matter how you interact.

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
- [Using Chat Instead of CLI](#using-chat-instead-of-cli)
- [Quick Reference](#quick-reference)

---

## Getting Started

### Your first task

```bash
klyntbot todo add "Buy groceries" --priority 3 --due tomorrow --tags shopping
```

This creates a task with:
- **Priority 3** (scale of 1-5, where 5 is critical)
- **Due date** set to tomorrow (natural language dates work!)
- **Tagged** with "shopping" for easy filtering

### View your tasks

```bash
klyntbot todo list                    # All active tasks
klyntbot todo list --status doing     # Only in-progress tasks
klyntbot todo list --tag shopping     # Filter by tag
klyntbot todo list --priority-min 4   # High priority only
```

### Complete a task

```bash
klyntbot todo complete abc123         # Use the task ID shown in list
```

---

## Managing Tasks

### Create tasks

```bash
klyntbot todo add "Task title"
klyntbot todo add "Task title" --description "More details here"
klyntbot todo add "Task title" --priority 4 --due "next Friday" --tags "work,urgent"
```

Due dates support natural language: `tomorrow`, `next Monday`, `in 3 days`, or standard `2026-03-01` format.

### View task details

```bash
klyntbot todo show abc123             # Full details for one task
klyntbot todo summary                 # Overview with statistics
```

### Update tasks

```bash
klyntbot todo update abc123 --title "New title"
klyntbot todo update abc123 --priority 5 --due "next week"
klyntbot todo update abc123 --status doing        # Mark as in-progress
klyntbot todo update abc123 --description none     # Clear description
```

### Search tasks

```bash
klyntbot todo search "groceries"                   # Search titles & descriptions
klyntbot todo search "design" --include-attachments # Also search attachments
```

### Delete tasks

```bash
klyntbot todo delete abc123
```

---

## Focus Mode & Time Tracking

Focus mode helps you concentrate on specific tasks while automatically tracking your time.

### Start a focus session

```bash
klyntbot todo focus abc123            # Add task to your focus board
```

This starts a timer and adds the task to your focus board. The board has a limited number of slots (configurable) to encourage working on fewer things at once.

### View your focus board

```bash
klyntbot todo focus                   # No ID = show focus board
```

Shows all focused tasks with progress bars indicating time remaining:
- Green bar = plenty of time
- Yellow bar = getting close
- Red bar = almost expired or overdue

### End focus

```bash
klyntbot todo unfocus abc123          # Remove from focus board
```

Time spent is automatically recorded when you unfocus.

### Log time manually

For work done outside of focus sessions:

```bash
klyntbot todo log-time abc123 45 --note "Worked on the design mockups"
```

This logs 45 minutes against the task.

---

## Subtasks & Hierarchies

Break large tasks into smaller pieces up to 16 levels deep.

### Create subtasks

```bash
klyntbot todo add-subtask abc123 "Design the header"
klyntbot todo add-subtask abc123 "Build the footer" --priority 3
```

### View the hierarchy

```bash
klyntbot todo tree
```

Output:
```
├─ ○ Build website [abc123]  P4  Feb 20
│  ├─ ✓ Design mockups [def456]  P3
│  └─ ○ Implement frontend [ghi789]  P4
└─ ● Write documentation [jkl012]  P2
```

Icons: ○ = todo, ● = in progress, ✓ = done, ▪ = archived

### Move tasks around

```bash
klyntbot todo move abc123 --parent def456          # Make it a subtask of def456
klyntbot todo move abc123 --parent none             # Make it top-level
klyntbot todo move abc123 --project proj1           # Move to a project
```

---

## Projects

Group related tasks into projects for better organization.

### Create a project

```bash
klyntbot project create "Website Redesign" --color blue --tags "client,design"
```

Available colors: red, orange, yellow, green, blue, purple, gray.

### List & view projects

```bash
klyntbot project list                               # All active projects
klyntbot project list --status completed             # Completed projects
klyntbot project show proj123                        # Project details
```

### View project tasks

```bash
klyntbot project tasks proj123                       # Flat list
klyntbot project tasks proj123 --tree                # Hierarchical view
```

### Update & archive

```bash
klyntbot project update proj123 --status paused
klyntbot project archive proj123                     # Shortcut for archiving
```

Project lifecycle: **Active** → **Paused** → **Completed** → **Archived**

---

## Recurring Tasks

Set up tasks that repeat automatically on a schedule.

### Create a recurring task

```bash
klyntbot todo recur add "Daily standup" --rule "FREQ=DAILY;BYHOUR=9"
klyntbot todo recur add "Weekly review" --rule "FREQ=WEEKLY;BYDAY=FR;BYHOUR=16"
klyntbot todo recur add "Monthly report" --rule "FREQ=MONTHLY;BYMONTHDAY=1"
```

Common schedule patterns:
| Schedule | Rule |
|----------|------|
| Every day at 9am | `FREQ=DAILY;BYHOUR=9` |
| Every weekday | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR` |
| Every Monday | `FREQ=WEEKLY;BYDAY=MO` |
| Every 2 weeks | `FREQ=WEEKLY;INTERVAL=2` |
| First of each month | `FREQ=MONTHLY;BYMONTHDAY=1` |
| Every year | `FREQ=YEARLY` |

### Manage recurring templates

```bash
klyntbot todo recur list                             # See all recurring templates
klyntbot todo recur delete tmpl123                   # Stop a recurring task
```

Klyntbot automatically creates new task instances at the scheduled times. Existing instances are kept even if you delete the template.

---

## Task Dependencies

Mark tasks that must be completed before others can start.

### Add a dependency

```bash
klyntbot todo depend abc123 --on def456             # abc123 is blocked by def456
```

### Remove a dependency

```bash
klyntbot todo depend abc123 --remove def456
```

### View dependencies

```bash
klyntbot todo depend abc123                          # Show what blocks this task
```

Output:
```
Dependencies for abc123 — Deploy to prod
  Blocked by:
    def456 — Run tests
  Blocks:
    ghi789 — Write release notes
```

If you try to complete a task that has incomplete blockers, klyntbot will tell you which tasks need to be finished first.

---

## Attachments

Add files, links, or notes to any task.

### Attach items

```bash
klyntbot todo attach abc123 --file /path/to/mockup.png --title "Design v2"
klyntbot todo attach abc123 --url "https://docs.example.com/spec" --title "Spec doc"
klyntbot todo attach abc123 --note "Remember to check the edge cases"
```

### Remove attachments

```bash
klyntbot todo detach abc123 att_456                  # Remove specific attachment
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

Targets can include `os_native` (desktop notifications) and any enabled chat channel name.

---

## Apple Calendar Sync

Tasks with due dates can sync to Apple Calendar, appearing on your iPhone, Mac, and iPad.

### Setup

See [CALENDAR_SETUP.md](CALENDAR_SETUP.md) for initial configuration.

### Use

```bash
klyntbot calendar sync                # Manual sync
klyntbot calendar status              # Check sync status
klyntbot calendar list --from 2026-02-14 --to 2026-02-21   # View events
klyntbot calendar conflicts           # Check for sync conflicts
```

Once configured, tasks with due dates automatically appear in your Apple Calendar. Changes sync in the background at regular intervals.

---

## Reports & Analytics

Generate reports to review your productivity.

```bash
klyntbot todo report                                 # Weekly report (default)
klyntbot todo report --period month                  # Monthly report
klyntbot todo report --project proj123               # Project-specific report
```

Reports include:
- Tasks created and completed in the period
- Total time tracked (focus sessions + manual entries)
- Completion rate with visual progress bar
- Priority distribution (P1-P5 breakdown)
- Overdue task highlights

---

## Using Chat Instead of CLI

Everything above can also be done by chatting with the AI in natural language — through any connected channel (Telegram, Discord, Slack, etc.) or the built-in REPL.

### Examples

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

## Quick Reference

### Task Commands

| Command | Description |
|---------|-------------|
| `todo add <title>` | Create a task |
| `todo list` | List tasks (with filters) |
| `todo show <id>` | Task details |
| `todo update <id>` | Modify a task |
| `todo complete <id>` | Mark as done |
| `todo delete <id>` | Remove a task |
| `todo focus [id]` | Focus on a task / view focus board |
| `todo unfocus <id>` | Stop focusing |
| `todo summary` | Task statistics |
| `todo tree` | Hierarchical view |
| `todo search <query>` | Find tasks |
| `todo add-subtask <parent> <title>` | Create a subtask |
| `todo move <id>` | Change parent/project |
| `todo attach <id>` | Add attachment |
| `todo detach <id> <att_id>` | Remove attachment |
| `todo log-time <id> <min>` | Log time manually |
| `todo report` | Productivity report |
| `todo depend <id>` | Manage dependencies |
| `todo recur add <title>` | Create recurring task |
| `todo recur list` | List recurring templates |
| `todo recur delete <id>` | Delete recurring template |

### Project Commands

| Command | Description |
|---------|-------------|
| `project create <name>` | Create a project |
| `project list` | List projects |
| `project show <id>` | Project details |
| `project update <id>` | Modify a project |
| `project archive <id>` | Archive a project |
| `project tasks <id>` | View project tasks |

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
