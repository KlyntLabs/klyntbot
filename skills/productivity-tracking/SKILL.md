---
name: productivity-tracking
description: Productivity insights, focus session management, goals, Pomodoro timer, and time analysis
metadata: '{"klyntbot":{"triggers":["productivity","focus session","time tracking","work patterns","how productive","start focus","end focus","focus report","daily report","weekly report","productivity report","screen time","app usage","distracting","productive time","pomodoro","productivity score","goal","log time","export activity","compare productivity"],"always":false}}'
---

# Productivity Tracking

## Agent Instructions

**When the user asks about productivity or focus sessions**, use the ProductivityTool with the appropriate action.

### Focus Sessions

**Starting a focus session:**
```json
{"action": "focus_start", "duration_mins": 45}
```

Optional params: `action_id` (link to a task), `project_id`.

**Ending a focus session:**
```json
{"action": "focus_end", "notes": "Completed API refactor"}
```

**Checking focus status:**
```json
{"action": "focus_status"}
```

### Pomodoro Timer

**Starting a Pomodoro session:**
```json
{"action": "pomodoro_start", "work_mins": 25, "break_mins": 5}
```

Optional params: `action_id`, `project_id`. Defaults to 25min work / 5min break. Uses the same focus session infrastructure — end with `focus_end`.

### Reports & Insights

**Today's summary:**
```json
{"action": "activity_today"}
```

Show the summary in a readable format. Includes AI-generated summary and productivity score when available:
```
Today's Productivity:
  Active: 4.2h (Productive: 3.1h, Neutral: 0.8h, Distracting: 0.3h)
  Focus sessions: 3 (avg quality: 82%)
  Context switches: 24
  Top apps: VS Code (2.1h), Terminal (0.8h), Slack (0.5h)
  Productivity score: 75/100
```

**Productivity score:**
```json
{"action": "activity_score"}
```

Returns a 0-100 score based on productive ratio (40%), focus quality (30%), low distraction (20%), and continuity (10%).

**Weekly summary:**
```json
{"action": "activity_week"}
```

Show day-by-day trends and highlight patterns.

**Custom date range summary:**
```json
{"action": "activity_summary", "start_date": "2026-03-01", "end_date": "2026-03-04"}
```

**Historical comparison (today vs yesterday, this week vs last week):**
```json
{"action": "activity_compare", "period": "day"}
{"action": "activity_compare", "period": "week"}
```

### Goals

**Set a productivity goal:**
```json
{"action": "set_goal", "metric": "productive_hours", "target_value": 4.0, "goal_type": "daily"}
```

Available metrics: `productive_hours`, `focus_sessions`, `productivity_score`, `max_distracting_mins`. Goal types: `daily`, `weekly`.

**Check goal progress:**
```json
{"action": "check_goals"}
```

**List active goals:**
```json
{"action": "list_goals"}
```

**Remove a goal:**
```json
{"action": "remove_goal", "goal_id": 1}
```

### Manual Time Entry

**Log time for meetings, offline work, etc.:**
```json
{"action": "log_time", "description": "Team standup meeting", "duration_mins": 30, "category_id": "communication"}
```

Optional params: `project_id`. Logged entries are included in daily aggregation.

### Data Export

**Export activity data as CSV or JSON:**
```json
{"action": "activity_export", "start_date": "2026-03-01", "end_date": "2026-03-04", "format": "csv"}
```

Formats: `csv` (default), `json`. Exports up to 50,000 events.

### Category Management

**List categories:**
```json
{"action": "list_categories"}
```

**Set a category:**
```json
{"action": "set_category", "id": "gaming", "name": "Gaming", "category_type": "distracting", "app_names": ["Steam"], "bundle_ids": [], "url_patterns": []}
```

## Behavior Guidelines

- When a user is in a focus session, remind them of remaining time if they ask
- Proactively note if distracting time is unusually high compared to their patterns
- Suggest starting a focus session when the user mentions wanting to concentrate
- When showing reports, highlight deviations from their normal patterns
- Use the productivity context (available in system prompt) to personalize responses
- Respect quiet hours — don't suggest focus sessions during configured break times
- When goals are set, mention progress toward them in daily summaries
- Suggest Pomodoro sessions for users who mention wanting structured work/break cycles

## Response Style

- Be concise with numbers — use hours (e.g., "3.2h") not seconds
- Round percentages to whole numbers
- Highlight trends with simple comparisons ("up 15% from last week")
- When the user has been productive, acknowledge it briefly
- When patterns show room for improvement, frame suggestions positively
- Show productivity score prominently when reporting daily summaries
