---
name: klyntbot-productivity
description: >
  Use when the user mentions focus, pomodoro, productivity, activity tracking,
  time logging, productivity score, work goals, "start a focus session", "what
  did I do today", "how productive was I", deep work, or wants to track time spent.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [productivity]
    mcp_tools: []
    invokes: [klyntbot-tasks, klyntbot-work-context]
---

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `focus_start` | duration_mins, project_id | Start a focus/deep work session |
| `focus_end` | notes | End the current focus session |
| `focus_status` | -- | Check if a focus session is active |
| `pomodoro_start` | work_mins, break_mins | Start a pomodoro cycle |
| `activity_today` | -- | "What did I do today?" |
| `activity_week` | -- | Weekly activity overview |
| `activity_score` | -- | Current productivity score |
| `set_goal` | metric, target_value, goal_type | Set productivity targets |
| `check_goals` | -- | Check goal progress |
| `log_time` | description, duration_mins | Manually log work time |

Use the `klyntbot - productivity` MCP tool for focus and activity tracking.

For all actions and goal metrics, read `references/actions.md`.

## Common Mistakes

1. **Starting a focus session without checking status** — Always call `focus_status` first. Starting a new session while one is active may cause issues.
2. **Forgetting to end focus sessions** — If the user says they're done working, call `focus_end` to properly close the session.
3. **Wrong duration units** — `duration_mins` is in minutes, not seconds or hours. 1 hour = 60, not 3600.
4. **Not linking to a project** — When starting a focus session, suggest linking it to a project_id for better tracking. Call `project(action: "list")` from the tasks tool if needed.
5. **Confusing focus_start with pomodoro_start** — `focus_start` is a single continuous session. `pomodoro_start` cycles between work and break intervals.

## Related Skills

- **klyntbot-tasks** — Focus on specific tasks, track time against task estimates
- **klyntbot-work-context** — View work context during focus sessions
- **klyntbot-okr** — Link focus time to objectives for goal tracking
- **klyntbot-automation** — Set up daily focus session reminders
