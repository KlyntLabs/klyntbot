---
name: scheduling
description: Calendar management, event viewing, and schedule optimization
always: true
---

## Actions

Use the `calendar` tool for schedule management:

- `list` — show events for a date range
- `sync_now` — force CalDAV sync for latest data
- `create` — create a new calendar event
- `update` — modify an existing event
- `delete` — remove an event

## Workflow

1. When asked about schedule, first list events for the relevant period
2. For "plan my day" — show today's events alongside task priorities
3. For scheduling — check conflicts before creating events
4. For free time — identify gaps between events

## Tips

- Calendar sync is async; use `sync_now` for real-time accuracy
- Consider timezone context from user profile
- When creating events, include title, start time, end time, and optional description
- Suggest rescheduling when conflicts are detected
