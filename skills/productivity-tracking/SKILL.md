---
name: productivity-tracking
description: Productivity insights, focus session management, and time analysis
metadata: '{"klyntbot":{"triggers":["productivity","focus session","time tracking","work patterns","how productive","start focus","end focus","focus report","daily report","weekly report","productivity report","screen time","app usage","distracting","productive time"],"always":false}}'
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

### Reports & Insights

**Today's summary:**
```json
{"action": "activity_today"}
```

Show the summary in a readable format:
```
Today's Productivity:
  Active: 4.2h (Productive: 3.1h, Neutral: 0.8h, Distracting: 0.3h)
  Focus sessions: 3 (avg quality: 82%)
  Context switches: 24
  Top apps: VS Code (2.1h), Terminal (0.8h), Slack (0.5h)
```

**Weekly summary:**
```json
{"action": "activity_week"}
```

Show day-by-day trends and highlight patterns.

**Custom date range summary:**
```json
{"action": "activity_summary", "start_date": "2026-03-01", "end_date": "2026-03-04"}
```

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

## Response Style

- Be concise with numbers — use hours (e.g., "3.2h") not seconds
- Round percentages to whole numbers
- Highlight trends with simple comparisons ("up 15% from last week")
- When the user has been productive, acknowledge it briefly
- When patterns show room for improvement, frame suggestions positively
