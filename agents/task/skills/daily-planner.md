---
name: daily-planner
description: Morning plan with top 3 tasks, calendar awareness, and evening wrap-up
always: true
---

## Morning Plan

**When the user asks for their daily plan** ("daily plan", "what should I focus on", "morning plan", "plan my day"):

1. Check today's calendar events via `calendar list` (if calendar tool available, otherwise skip)
2. Generate the plan: `{"action": "plan", "count": 3}`
3. Display the integrated plan with calendar context
4. Ask what they want to do: accept, swap, skip, or defer

### Plan Format

```
Good morning! Here's your plan for [day]:

📅 Calendar:
- 10:00–11:00 Team standup
- 14:00–15:00 Design review
- Free blocks: 08:00–10:00 (2h), 11:00–14:00 (3h), 15:00–17:00 (2h)

🎯 Top 3 Tasks:
1. [Task Title] (P1, overdue by 3 days, est. 30min) — Score: 50.3
   Best slot: 08:00–08:30 (before standup)

2. [Task Title] (P2, due today, est. 45min) — Score: 20.0
   Best slot: 11:00–11:45 (long focus block)

3. [Task Title] (P3, due tomorrow, est. 15min) — Score: 15.1
   Best slot: 15:00–15:15 (quick win after review)

Total estimated work: 1h30 across 7h of free time.

Reply: yes | swap 1 and 2 | skip 2 | defer all
```

When calendar is unavailable, omit the calendar section and slot suggestions — just show the ranked tasks.

### Responses

| Reply | Action |
|-------|--------|
| `yes`, `ok`, `go` | Focus all suggested tasks |
| `swap 1 and 2` | Reorder tasks |
| `skip 2` | Remove task #2, promote next |
| `defer`, `not today` | Dismiss the plan |

## Evening Wrap-Up

**When the user asks for wrap-up** ("wrap up", "end of day", "how'd today go", "done for the day"):

1. Check today's focused tasks and their status
2. Check tasks completed today: `todo report period=day`
3. Present a summary with next-day prep

### Wrap-Up Format

```
End of day summary:

✅ Completed:
- [Task] (took ~30min)
- [Task] (took ~1h)

⏳ Still in progress:
- [Task] — carry to tomorrow?

📋 Tomorrow preview:
- [Overdue task] needs attention
- [Calendar event] at 09:00

Anything to capture before signing off?
```

The wrap-up is a lightweight check-in, not a full review. If the user mentions tasks they did that aren't in the system, offer to log them.

## Scoring

Score = (urgency × priority_weight) + (age_days × 0.1)

- Urgency: overdue=10, today=5, tomorrow=3, future=1
- Priority: P1=5, P2=4, P3=3, P4=2, P5=1, none=3
- Age: +0.1 per day to prevent perpetual deferral

## Pattern-Based Suggestions

When behavioral patterns are available in the system prompt (from the learning system), incorporate them:

- "You're most productive on [day] — consider scheduling deep work tasks then"
- "You usually handle [area] tasks in the morning — I've weighted those higher for your AM plan"
- Only mention patterns when they're relevant to the current plan, not every time
