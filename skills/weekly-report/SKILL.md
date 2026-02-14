---
name: weekly-report
description: Generate a weekly progress report from todo data with task completion, time tracking, and recommendations.
---

# Weekly Todo Report

Generate a comprehensive weekly progress report based on your todo data.

## When to use

- Automated weekly summary (triggered by cron)
- User requests "weekly report" or "show my progress this week"
- End-of-week review sessions

## How it works

This skill:
1. Calls `todo report period=week` to get raw statistics
2. Formats the data into a narrative markdown report
3. Provides actionable insights and recommendations

## Report structure

### Project-by-project breakdown
- Tasks completed vs created per project
- Time invested in each project (hours)
- Progress indicators

### Completed tasks list
- All tasks completed this week
- Organized by project

### Time tracking summary
- Total time tracked across all projects
- Focus sessions count and duration
- Time distribution visualization

### Upcoming deadlines
- Tasks due in the next 7 days
- Overdue tasks requiring attention
- Priority indicators

### Recommendations
- Based on velocity (tasks completed vs created)
- Project health indicators
- Focus areas for next week

## Usage

When invoked (manually or via cron):

1. Call the todo report action to get raw data:
   ```
   todo report period=week
   ```

2. Parse the statistics from the report output

3. Generate a narrative report with sections:

   **Weekly Progress Report**

   *Summary:* [High-level overview of the week's productivity]

   **📊 Project Highlights:**
   - [Project A]: [X] tasks completed, [Y]h invested
   - [Project B]: [X] tasks completed, [Y]h invested

   **✅ Completed This Week:**
   - [ ] Task 1 (Project A)
   - [ ] Task 2 (Project B)

   **⏰ Time Tracking:**
   - Total: [X]h tracked
   - Focus sessions: [Y] sessions ([Z]h)
   - Average: [X]h per day

   **📅 Upcoming:**
   - [Task] due [date]
   - [X] tasks overdue ⚠️

   **💡 Recommendations:**
   - [Based on velocity and project health]
   - [Suggested focus areas]

4. If triggered by cron, send via configured channels (Telegram/Discord/Email)

## Configuration

Automated delivery is configured via cron job `__klyntbot_weekly_report`:
- Schedule: Sunday 18:00 (customizable in agent config)
- Delivery channels: Configured in TodoNotificationConfig.targets
- Format: Markdown suitable for LLM formatting

## Example output

```markdown
📊 **Weekly Progress Report** (Feb 7-14, 2026)

This week you completed 12 tasks across 3 projects, investing 18.5 hours of focused work.

**Project Highlights:**
- 🚀 **klyntbot**: 8 tasks completed, 12.3h invested - Strong momentum!
- 📱 **mobile-app**: 3 tasks completed, 4.2h invested
- 📚 **docs**: 1 task completed, 2.0h invested

**Time Tracking:**
- Total tracked: 18.5h
- Focus sessions: 15 sessions (14.2h)
- Average: 2.6h per day

**Upcoming Deadlines:**
- 3 tasks due this week
- ⚠️ 2 tasks overdue

**Recommendations:**
- Great velocity on klyntbot! Consider maintaining this pace.
- Mobile-app has 5 open tasks - might need more time allocation
- 2 overdue tasks need immediate attention
```

## Notes

- Raw data comes from TodoTool report action (no LLM calls for stats)
- This skill focuses on formatting and narrative generation
- Time calculations: duration_secs converted to hours (÷ 3600)
- Focus sessions are identified by TimeEntrySource::Focus
