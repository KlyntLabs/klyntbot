---
name: okr-reviews
description: OKR scoring guide and review cadence
---

# OKR Reviews

## Scoring Guide (0.0–1.0)

| Score | Meaning |
|-------|---------|
| 0.0–0.3 | Failed to make meaningful progress |
| 0.4–0.6 | Made progress but fell short |
| 0.7–0.9 | Hit most targets (sweet spot) |
| 1.0 | Fully achieved (might have been too easy) |

## Review Cadence

- **Weekly** (during weekly review): Quick pulse — "Any movement on KR1?"
- **Monthly**: Deeper assessment — "Is effort compounding toward this objective?"
- **Quarterly**: Formal scoring with lessons learned

## Monthly Review Steps

1. `objective.list` — pull all active objectives
2. For each: show KR progress with trend indicators
3. Ask about blockers
4. Identify neglected areas

## Quarterly Review Steps

1. Score each KR (ask user, don't assign)
2. Average KR scores → objective score
3. Lessons learned per objective
4. Archive completed, carry forward in-progress
5. Set new objectives for next quarter

## Key Result Types

- **Metric**: Has target_value and unit (e.g., "Reduce churn to 5%")
  - Track with `kr.update_metric(id, current_value)`
- **Action**: Binary done/not-done (e.g., "Launch loyalty program")
  - Track with `kr.update(id, status: "done")`
