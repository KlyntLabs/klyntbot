---
name: weekly-report
description: Generate weekly progress reports from todo data with completion stats and recommendations
always: false
---

## When to use

- User requests "weekly report" or "show my progress this week"
- Automated weekly summary (via cron)

## Workflow

1. Call `todo report period=week` for raw statistics
2. Format into narrative markdown report
3. Provide actionable insights

## Report Structure

```
Weekly Progress Report (date range)

Summary: [High-level productivity overview]

Project Highlights:
- [Project]: [X] tasks completed, [Y]h invested

Completed This Week:
- Task 1 (Project A)
- Task 2 (Project B)

Time Tracking:
- Total: [X]h tracked
- Focus sessions: [Y] sessions
- Average: [X]h per day

Upcoming:
- [Task] due [date]
- [X] tasks overdue

Recommendations:
- [Based on velocity and project health]
```
