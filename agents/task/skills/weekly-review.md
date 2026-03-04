---
name: weekly-review
description: Interactive GTD-style weekly review — walk through projects, triage overdue, plan next week
always: false
triggers: [weekly review, review my week, weekly check-in, how's my week, week in review]
---

## When to Use

- User says "weekly review", "review my week", "weekly check-in", "how's my week going"
- Automated weekly trigger (via cron on Sunday evening or Monday morning)

## Philosophy

A review is NOT a report. It's an **interactive workflow** where the agent walks the user through decisions at each step. The goal: leave the review with a clear head and a concrete plan for next week.

## Three Phases (GTD-based)

### Phase 1: Get Clear (process loose ends)

1. Check for overdue tasks: `todo list filter=overdue`
2. For each overdue task, ask the user:
   ```
   ⚠️ Overdue tasks need triage:

   1. "Fix login bug" (3 days overdue, P2)
      → Complete | Reschedule to [when] | Drop

   2. "Send invoice to client" (1 day overdue, P1)
      → Complete | Reschedule to [when] | Drop
   ```
3. Process their responses (update due dates, complete, or archive)

### Phase 2: Get Current (review active work)

1. Pull active projects: `project list`
2. For each active project, show a health summary:
   ```
   📊 Project health check:

   🟢 Website Redesign — 8/12 tasks done (67%), on track
      No action needed.

   🟡 API Migration — 3/10 tasks done (30%), 2 overdue
      What's blocking this? [reply or skip]

   🔴 Q1 Report — 0/5 tasks done, deadline in 4 days
      This needs urgent attention. Reprioritize?
   ```
3. For each yellow/red project, ask the user for a decision
4. Check OKR progress if objectives exist: `okr list`
   ```
   📈 OKR check-in:

   Objective: "Increase user retention to 80%"
   - KR1: Reduce churn rate to 5% — currently 7% (60% progress)
   - KR2: Launch loyalty program — not started ⚠️

   Quick update on KR2?
   ```

### Phase 3: Get Creative (plan next week)

1. Show upcoming deadlines for next 7 days: `todo list filter=upcoming`
2. Ask about priorities:
   ```
   📋 Looking ahead to next week:

   Due this week:
   - "Finalize design specs" (Wed)
   - "Team retrospective prep" (Fri)

   No due date but aging:
   - "Research new CI tools" (created 12 days ago)
   - "Update documentation" (created 8 days ago)

   What are your top 3 priorities for next week?
   ```
3. If the user provides priorities, focus those tasks
4. Offer to create new tasks for anything mentioned during the review

## Interaction Style

- **Ask one phase at a time** — don't dump all three phases in one message
- **Wait for user response** before moving to the next phase
- **Keep it lightweight** — if there are no overdue tasks, skip that triage quickly. If all projects are green, just say so and move on
- **End with a clear summary**:
  ```
  ✅ Weekly review complete:
  - Triaged 3 overdue tasks (2 rescheduled, 1 dropped)
  - API Migration flagged for extra focus
  - Top 3 for next week: [task], [task], [task]

  Have a great week!
  ```

## Statistics (supplementary, not primary)

After the interactive review, optionally append raw stats from `todo report period=week`:
- Tasks completed / created this week
- Time tracked (if available)
- Completion rate trend vs. last week

These stats support the review but are NOT the review itself.
