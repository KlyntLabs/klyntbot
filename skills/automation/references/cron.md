---
name: cron
description: Schedule reminders and recurring tasks
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "schedule,cron,recurring,automation"
  always: true
  triggers: ""
  agent: automation
---

Use the `cron` tool to schedule reminders or recurring tasks.

## Two Modes

1. **Reminder** — message is sent directly to user
2. **Task** — message is a task description, agent executes and sends result

## Examples

```
cron(action="add", message="Time to take a break!", every_seconds=1200)
cron(action="add", message="Check GitHub stars and report", every_seconds=600)
cron(action="list")
cron(action="remove", job_id="abc123")
cron(action="enable", job_id="abc123")
cron(action="disable", job_id="abc123")
cron(action="run", job_id="abc123")
```

## Actions

- `add` — create a new scheduled job (requires `message` + `every_seconds` or `cron_expr`)
- `list` — list all scheduled jobs
- `remove` — delete a job by ID
- `enable` — re-enable a disabled job
- `disable` — pause a job without deleting it
- `run` — manually trigger a job immediately

## Time Expressions

| User says | Parameters |
|-----------|------------|
| every 20 minutes | every_seconds: 1200 |
| every hour | every_seconds: 3600 |
| every day at 8am | cron_expr: "0 8 * * *" |
| weekdays at 5pm | cron_expr: "0 17 * * 1-5" |
