---
name: klyntbot-productivity
description: >
  Track focus sessions, activity, and productivity goals using Klyntbot.
  Use when the user mentions focus, pomodoro, productivity, activity tracking,
  time logging, productivity score, or work goals.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [productivity]
    mcp_tools: []
---

Use the `klyntbot - productivity` MCP tool for focus and activity tracking.

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `focus_start` | duration_mins, project_id | User wants to start a focus session |
| `focus_end` | notes | User finished working |
| `focus_status` | — | Check if a focus session is active |
| `pomodoro_start` | work_mins, break_mins | User wants pomodoro technique |
| `activity_today` | — | "What did I do today?" |
| `activity_week` | — | Weekly activity overview |
| `activity_score` | — | Current productivity score |
| `set_goal` | metric, target_value, goal_type | Set productivity targets |
| `check_goals` | — | Check goal progress |
| `log_time` | description, duration_mins | Manually log work time |

For all actions and goal metrics, read `references/actions.md`.
