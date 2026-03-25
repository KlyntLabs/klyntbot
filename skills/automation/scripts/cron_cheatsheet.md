# Cron Expression Quick Reference

## Format

Five fields: `minute hour day month weekday`

## Common Patterns

| Schedule | Expression | Notes |
|----------|-----------|-------|
| Every day at 8am | `0 8 * * *` | |
| Weekdays at 5pm | `0 17 * * 1-5` | Mon=1, Sun=0 |
| Every Sunday 6pm | `0 18 * * 0` | |
| First of month 9am | `0 9 1 * *` | |
| Every 30 min | Use `every_seconds: 1800` | NOT cron |
| Every 5 min | Use `every_seconds: 300` | NOT cron |

## Weekday Numbers

Sun=0, Mon=1, Tue=2, Wed=3, Thu=4, Fri=5, Sat=6

## Validation

Before creating a job, mentally verify:
1. Five fields exactly (not 6 — no seconds field)
2. Minutes 0-59, hours 0-23, day 1-31, month 1-12, weekday 0-6
3. Ranges use `-` (e.g. `1-5`), lists use `,` (e.g. `1,3,5`)
4. `*/N` for every N units (e.g. `*/15 * * * *` = every 15 min)

## One-Shot Reminders

For one-time future reminders, use `every_seconds` with the delay.
Example: "remind me in 20 minutes" → `every_seconds: 1200`
The job fires once and auto-deletes.
