---
name: notification
description: Route alerts and reminders to the right channel at the right time
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-13"
  source: official
  tags: "notify,alert,reminder,schedule"
  always: false
  triggers: "notify,alert,remind,reminder"
  agent: communication
---

Handle alert routing, reminder delivery, and notification batching.

## Capabilities

- **Route alerts** — deliver budget alerts, overdue task warnings, and system notifications to the user's preferred channel
- **Batch notifications** — group multiple low-priority alerts into a single digest instead of sending individually
- **Respect quiet hours** — defer non-urgent notifications if the user has quiet hours configured

## Routing priority

1. Urgent alerts (budget exceeded, overdue tasks) → immediate delivery to primary channel
2. Informational (daily digest, weekly report) → batch and deliver at scheduled time
3. Low priority (tips, suggestions) → include in next digest or skip if digest is full

## When to use

- System generates an alert (budget, overdue, focus session end)
- User asks to set up a reminder or recurring notification
- Multiple pending notifications need batching

## Tips

- Never send duplicate notifications for the same event
- Include actionable context: what happened, what to do next
- For reminders, confirm the delivery time and channel with the user
