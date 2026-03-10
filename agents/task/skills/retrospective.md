---
name: retrospective
description: Monthly OKR retrospective and quarterly planning cycle
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "retrospective,review,reflect"
  always: false
  triggers: "retrospective,monthly review,quarter review,okr review,score my okrs,quarterly planning"
  agent: task
---

## When to Use

- User says "monthly review", "retro", "retrospective", "quarter review", "OKR review"
- End of month or quarter check-in
- User asks "how did I do this month" or "score my OKRs"

## Monthly Retrospective (15–30 min)

### Step 1: OKR Progress Review

Pull all active objectives: `okr list`

For each objective, present Key Results with scoring:

```
📊 Monthly OKR Review — [Month Year]

Objective: "Increase user retention to 80%"
Overall health: 🟡 At Risk

  KR1: Reduce churn rate to 5%
  Current: 7% → Target: 5% → Progress: 40%
  Trend: ↘️ Declining (was 50% last month)

  KR2: Launch loyalty program
  Current: Design complete → Target: Launched → Progress: 30%
  Trend: ↗️ Improving (was 10% last month)

What's the biggest blocker for this objective?
```

### Step 2: Area Balance Check

Review areas to check for neglected life domains:

```
📋 Area Balance:

Work: 23 tasks completed, 8 active — very active
Personal: 5 tasks completed, 12 active — falling behind
Health: 1 task completed, 4 active — ⚠️ neglected this month

Your Health area had very little activity. Want to set a goal for next month?
```

### Step 3: Monthly Priorities

Ask the user to set 3–5 priorities for the next month:

```
Based on your OKR progress and area balance, I'd suggest:

1. Push KR2 (loyalty program) to launch — biggest gap
2. Clear the Personal backlog — 12 tasks waiting
3. Restart Health area — set one weekly habit

Agree, or different priorities?
```

## Quarterly Review (60–90 min)

### Step 1: Score Objectives

For each objective, guide the user through scoring (0.0–1.0):

```
🏁 Q1 Quarterly Review

Scoring guide:
  0.0–0.3: Failed to make meaningful progress
  0.4–0.6: Made progress but fell short
  0.7–0.9: Hit most targets (sweet spot)
  1.0: Fully achieved (might have been too easy)

Objective: "Increase user retention to 80%"
  KR1: Reduce churn to 5% — achieved 6.2% → Score?
  KR2: Launch loyalty program — in beta → Score?
  KR3: Improve onboarding NPS to 8+ — achieved 7.8 → Score?

Overall objective score: [average of KR scores]
```

### Step 2: Lessons Learned

For each scored objective, ask:

```
Lessons from "Increase user retention":
- What worked well?
- What would you do differently?
- Any surprises?
```

### Step 3: Archive and Plan

1. Archive completed/scored objectives
2. Carry forward in-progress objectives (with adjusted targets if needed)
3. Brainstorm new objectives for next quarter:

```
For Q2, consider:
- Continuing objectives that scored 0.4–0.6 (adjust targets)
- New objectives based on neglected areas
- Objectives aligned with upcoming milestones

What are your top 2–3 objectives for Q2?
```

### Step 4: Set Key Results

For each new objective, guide KR creation:

```
Objective: "[user's objective]"

Good Key Results are:
- Measurable (number, percentage, yes/no)
- Time-bound (the quarter is the default deadline)
- Ambitious but achievable (aim for 70% hit rate)

What are 2–4 Key Results for this objective?
```

Create each KR via `okr add_key_result`.

## Interaction Style

- **One section at a time** — don't overwhelm with the full review at once
- **Ask for scores, don't assign them** — the user decides what a KR scored
- **Be encouraging but honest** — celebrate progress, acknowledge gaps without judgment
- **End with forward momentum** — always close with concrete next actions
