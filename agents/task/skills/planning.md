---
name: planning
description: Daily planning — curated morning plan with top 3 most impactful tasks
always: true
---

## Agent Instructions

**When the user asks for their daily plan** ("daily plan", "what should I focus on", "morning plan", "plan"):

1. Generate the plan: `{"action": "plan", "count": 3}`
2. Display the full plan immediately in your response text
3. Include calendar context (if available) showing today's events
4. Ask what they want to do: accept, swap, skip, or defer

## Plan Format

```
Good morning! Here's your plan:

1. [Task Title] (P1, overdue by 3 days, est. 30min) — Score: 50.3
   Why focus: This is critical and overdue

2. [Task Title] (P2, due today, est. 45min) — Score: 20.0
   Why focus: Due today with high impact

3. [Task Title] (P3, due tomorrow, est. 15min) — Score: 15.1
   Why focus: Quick win to build momentum

Reply: yes | swap 1 and 2 | skip 2 | defer all
```

## Responses

| Reply | Action |
|-------|--------|
| `yes`, `ok`, `go` | Focus all suggested tasks |
| `swap 1 and 2` | Reorder tasks |
| `skip 2` | Remove task #2, promote next |
| `defer`, `not today` | Dismiss the plan |

## Scoring

Score = (urgency x priority_weight) + (age_days x 0.1)

- Urgency: overdue=10, today=5, tomorrow=3, future=1
- Priority: P1=5, P2=4, P3=3, P4=2, P5=1, none=3
- Age: +0.1 per day to prevent perpetual deferral
