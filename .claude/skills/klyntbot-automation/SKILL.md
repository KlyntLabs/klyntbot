---
name: klyntbot-automation
description: >
  Use when the user mentions reminders, schedules, cron, recurring, "every day",
  "every hour", automate, automation, "remind me", "in 20 minutes", "set a timer",
  "notify me", or wants to schedule any repeating action.
license: MIT
metadata:
  author: klyntbot
  version: "2.1.0"
  klyntbot:
    type: skill
    tools: [cron]
    mcp_tools: []
    max_iterations: 10
    invokes: [klyntbot-tasks]
---

## CRITICAL: Use klyntbot's cron tool, NOT Claude's CronCreate

**NEVER use Claude Code's built-in `CronCreate` tool for klyntbot reminders.**
Claude's CronCreate is for Claude Code automation (running prompts on a schedule).
Klyntbot has its own `cron` MCP tool that sends reminders through the user's chat channels (Telegram, Discord, desktop notifications, etc.).

**Always use:** `cron(action: "add", message: "...", cron_expr: "...")`
**Never use:** `CronCreate`, `CronList`, `CronDelete` (those are Claude Code's tools)

## Quick Reference

| User says | Action | Key params |
|-----------|--------|-----------|
| "remind me in 20min" | `add` | message: "...", every_seconds: 1200 |
| "every day at 8am" | `add` | message: "...", cron_expr: "0 8 * * *" |
| "weekdays at 5pm" | `add` | message: "...", cron_expr: "0 17 * * 1-5" |
| "list my reminders" | `list` | -- |
| "cancel that reminder" | `remove` | job_id: "..." |

Use the `klyntbot - cron` MCP tool for scheduling.

For time expression conversions, read `references/time-expressions.md`.

## Common Mistakes

1. **Using Claude's CronCreate instead of klyntbot's cron tool** — Claude Code has its own cron system. For klyntbot reminders, ALWAYS use the `cron` MCP tool, never `CronCreate`.
2. **Wrong cron format** — Cron uses 5 fields: `minute hour day month weekday`. Not 6 (no seconds field). Example: `0 9 * * *` = 9:00 AM daily.
3. **Confusing one-time reminders with recurring** — Use `every_seconds` for a one-shot delay (fires once). Use `cron_expr` for repeating schedules.
4. **Wrong weekday numbers** — Sunday = 0, Monday = 1, ..., Saturday = 6. "Weekdays" = `1-5`.
5. **Timezone confusion** — Cron expressions run in the user's configured timezone. Don't convert to UTC.
6. **Forgetting to confirm the schedule** — Always echo back the interpreted schedule to the user before creating.

## Red Flags — STOP

If you're about to do any of these, STOP:
- Use `CronCreate` or `CronList` or `CronDelete` — those are Claude Code's tools, not klyntbot's
- Create a cron job without confirming the schedule with the user
- Use a 6-field cron expression (klyntbot uses 5-field standard cron)
- Guess a job_id when removing — always call `cron(action: "list")` first

## Related Skills

- **klyntbot-tasks** — Create recurring task reminders, schedule daily planning
- **klyntbot-finance** — Schedule recurring budget checks or spending summaries
- **klyntbot-productivity** — Automate daily focus session reminders
