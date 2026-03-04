---
name: cron
description: Schedule reminders and recurring tasks
always: true
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
```

## Time Expressions

| User says | Parameters |
|-----------|------------|
| every 20 minutes | every_seconds: 1200 |
| every hour | every_seconds: 3600 |
| every day at 8am | cron_expr: "0 8 * * *" |
| weekdays at 5pm | cron_expr: "0 17 * * 1-5" |
