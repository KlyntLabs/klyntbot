---
name: klyntbot-automation
description: >
  Schedule reminders and recurring tasks using Klyntbot.
  Use when the user mentions reminders, schedules, cron, recurring,
  every day, every hour, automate, or automation.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: skill
    tools: [cron]
    mcp_tools: []
    max_iterations: 10
---

Use the `klyntbot - cron` MCP tool for scheduling.

## Quick Reference

| User says | Params |
|-----------|--------|
| "remind me in 20min" | `action: "add", message: "...", every_seconds: 1200` |
| "every day at 8am" | `action: "add", message: "...", cron_expr: "0 8 * * *"` |
| "weekdays at 5pm" | `action: "add", message: "...", cron_expr: "0 17 * * 1-5"` |
| "list my reminders" | `action: "list"` |
| "cancel that reminder" | `action: "remove", job_id: "..."` |

For time expression conversions, read `references/time-expressions.md`.
