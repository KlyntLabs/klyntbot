---
name: time-expressions
description: Converting natural language time to cron parameters
---

# Time Expression Conversion

## Simple Intervals (use every_seconds)

| User says | every_seconds |
|-----------|--------------|
| every 5 minutes | 300 |
| every 15 minutes | 900 |
| every 20 minutes | 1200 |
| every 30 minutes | 1800 |
| every hour | 3600 |
| every 2 hours | 7200 |
| every 4 hours | 14400 |
| every 12 hours | 43200 |
| every day | 86400 |

## Specific Times (use cron_expr)

| User says | cron_expr |
|-----------|-----------|
| every day at 8am | `0 8 * * *` |
| every day at 9pm | `0 21 * * *` |
| weekdays at 5pm | `0 17 * * 1-5` |
| weekends at 10am | `0 10 * * 0,6` |
| monday at 9am | `0 9 * * 1` |
| first day of month | `0 9 1 * *` |
| every sunday at 6pm | `0 18 * * 0` |
| twice daily 9am/5pm | Two separate jobs |

## Cron Format

```
┌───── minute (0-59)
│ ┌───── hour (0-23)
│ │ ┌───── day of month (1-31)
│ │ │ ┌───── month (1-12)
│ │ │ │ ┌───── day of week (0=Sun, 6=Sat)
│ │ │ │ │
* * * * *
```

## Two Modes

- **Reminder**: message sent directly to user
- **Task**: message is a task description, agent executes and sends result
