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
{"action": "start_focus", "targetMins": 45, "sessionType": "focus"}
```

Session types: `focus` (default), `deepWork`, `meeting`, `break`

**Ending a focus session:**
```json
{"action": "end_focus", "notes": "Completed API refactor"}
```

**Recording a distraction during focus:**
```json
{"action": "record_distraction", "appName": "Twitter", "durationSecs": 120}
```

### Reports & Insights

**Today's summary:**
```json
{"action": "today_summary"}
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
{"action": "weekly_summary"}
```

Show day-by-day trends and highlight patterns.

**Activity timeline:**
```json
{"action": "timeline", "date": "2026-03-04"}
```

### Category Management

**List categories:**
```json
{"action": "list_categories"}
```

**Update a category:**
```json
{"action": "update_category", "id": "gaming", "name": "Gaming", "categoryType": "distracting", "rules": {"appNames": ["Steam"], "bundleIds": [], "urlPatterns": []}}
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
