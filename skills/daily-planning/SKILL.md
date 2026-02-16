---
name: daily-planning
description: Get a curated morning plan with your top 3 most impactful tasks.
metadata: '{"klyntbot":{"triggers":["daily plan","plan","morning plan","focus","what should I focus on"],"always":true}}'
---

# Daily Planning

## Agent Instructions

**When the user asks for their daily plan** (phrases like "daily plan", "what should I focus on", "morning plan", "plan"), YOU MUST:

1. **Generate the plan** using the TodoTool:
   ```json
   {"action": "plan", "count": 3}
   ```

2. **IMMEDIATELY display the full plan** to the user in your response text (NOT via message tool). Show:
   ```
   ☀ Good morning! Here's your daily plan:

   1. [Task Title] (P1, overdue by 3 days, est. 30min) — Score: 50.3
      Why focus: This is critical and overdue

   2. [Task Title] (P2, due today, est. 45min) — Score: 20.0
      Why focus: Due today with high impact

   3. [Task Title] (P3, due tomorrow, est. 15min) — Score: 15.1
      Why focus: Quick win to build momentum
   ```

3. **Include calendar context** (if available) showing today's events

4. **Then ask** what they want to do: accept, swap, skip, or defer

**CRITICAL:** Show the task titles and details FIRST in your text response. Don't hide them in tool calls or message actions. Users need to SEE what they're focusing on before deciding.

---

## Feature Overview

Start each day with clarity. The daily planning feature analyzes your tasks and suggests the top 3 most impactful items to focus on, considering urgency, priority, and age.

## How It Works

**Automatic (Default):**
- Runs every morning at your configured digest time (default: 08:00)
- Sends a notification via your active chat channels
- Includes today's calendar events (if calendar sync is configured)
- Suggests top 3 tasks with reasoning for each choice

**Manual Trigger:**
```bash
klyntbot todo plan              # Show today's plan
```

## Responding to Plans

When you receive a morning plan, you can:

| Reply | What It Does |
|-------|--------------|
| `yes`, `y`, `ok`, `go` | Accept the plan and focus all suggested tasks |
| `swap 1 and 2` | Reorder tasks (shows updated plan) |
| `skip 2` | Remove task #2, promote next eligible task |
| `defer`, `defer all`, `not today` | Dismiss the plan entirely |

**Examples:**
```
Reply: yes                  → Focuses tasks 1, 2, and 3
Reply: swap 1 and 3         → Moves task 3 to position 1
Reply: skip 2               → Removes task 2, promotes task 4
Reply: defer all            → No action, dismiss plan
```

## Plan Format

```
☀ Good morning! Here's your plan for Monday, Feb 16.

📅 Today's calendar:
  • 10:00 – Team standup
  • 14:00 – Design review

📋 Suggested focus (3 tasks):

  1. ⚡ Fix auth token expiry bug
     P1 · Overdue by 2 days · 30 min est.

  2. 🔨 Implement user settings page
     P2 · Due tomorrow · 60 min est.

  3. 🧹 Update API docs for v2 endpoints
     P3 · No deadline · 15 min est.

Reply: yes · swap 1 and 2 · skip 2 · defer all
```

## Scoring Algorithm

Tasks are ranked by a score formula that balances urgency and priority:

**Score = (urgency × priority_weight) + (age_days × 0.1)**

- **Urgency tiers:**
  - Overdue: 10 points
  - Due today: 5 points
  - Due tomorrow: 3 points
  - Future: 1 point

- **Priority weights (inverse):**
  - P1 (critical): 5 points
  - P2 (high): 4 points
  - P3 (medium): 3 points
  - P4 (low): 2 points
  - P5 (minor): 1 point
  - No priority: 3 points (default)

- **Age factor:** Older tasks get +0.1 per day to prevent perpetual deferral

**Example:** An overdue P1 task that's 5 days old scores: `(10 × 5) + (5 × 0.1) = 50.5`

## Configuration

Enable/disable daily planning in `~/.klyntbot/config.json`:

```json
{
  "todo": {
    "dailyPlanning": {
      "enabled": true,
      "planningTime": "08:00"
    }
  }
}
```

**Config options:**
- `todo.dailyPlanning.enabled` (boolean, default: `true`) - Enable/disable the feature
- `todo.dailyPlanning.planningTime` (string, default: `"08:00"`) - When to send morning plans

## Edge Cases

**No eligible tasks?**
```
☀ Good morning! All clear for Monday, Feb 16.

No tasks need your attention today. Enjoy your day!
```

**Focus slots full?**
Accepting a plan unfocuses your current tasks and replaces them with the new plan.

**Task completed after plan sent?**
Klyntbot re-validates tasks before focusing. Completed tasks are skipped, and the next eligible task is promoted.

**Plan already confirmed?**
```
Your plan for today is already confirmed.

Currently focused:
  1. Fix auth token expiry bug
  2. Implement user settings page

Wait until tomorrow, or manually unfocus tasks and run `todo plan` again.
```

## Tips

- **Keep task due dates current:** The urgency score heavily weighs due dates
- **Set priorities:** Tasks without priority default to P3 weight
- **Use calendar sync:** Morning plans include today's events for better context
- **Defer strategically:** If the plan doesn't fit your day, defer and manually focus tasks instead
