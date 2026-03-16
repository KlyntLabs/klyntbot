---
name: productivity-actions
description: Complete reference for all productivity tool actions
---

# Productivity Actions

## Focus Sessions

### focus_start
```json
{"action": "focus_start", "duration_mins": 60, "project_id": "uuid"}
```
Starts a tracked focus session. Optional project link.

### focus_end
```json
{"action": "focus_end", "notes": "Completed API refactor"}
```
Ends active focus session and logs duration.

### focus_status
```json
{"action": "focus_status"}
```
Check if a focus session is currently running.

### pomodoro_start
```json
{"action": "pomodoro_start", "work_mins": 25, "break_mins": 5}
```
Pomodoro-style focus with automatic break timer.

## Activity Tracking

| Action | Params | Returns |
|--------|--------|---------|
| `activity_today` | — | Today's activity summary |
| `activity_week` | — | Weekly activity overview |
| `activity_summary` | start_date, end_date | Custom date range summary |
| `activity_score` | — | Current productivity score |
| `activity_compare` | period: "day"/"week" | Compare vs previous period |
| `activity_export` | start_date, end_date, format: "csv"/"json" | Export data |

## Goals

### set_goal
```json
{"action": "set_goal", "metric": "productive_hours", "target_value": 6, "goal_type": "daily"}
```

Available metrics:
- `productive_hours` — total productive time per day/week
- `focus_sessions` — number of focus sessions
- `productivity_score` — overall score target
- `max_distracting_mins` — cap on distracting app usage

### check_goals / list_goals / remove_goal
```json
{"action": "check_goals"}
{"action": "list_goals"}
{"action": "remove_goal", "goal_id": "uuid"}
```

## Manual Time Logging

```json
{"action": "log_time", "description": "Code review", "duration_mins": 45, "category_id": "uuid", "project_id": "uuid"}
```

## Categories

```json
{"action": "list_categories"}
{"action": "set_category", "id": "uuid", "name": "VS Code", "category_type": "productive", "app_names": ["Code"], "bundle_ids": ["com.microsoft.VSCode"]}
```

Category types: `productive`, `neutral`, `distracting`
