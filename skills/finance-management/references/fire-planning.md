---
name: fire-planning
description: FIRE (Financial Independence, Retire Early) planning workflow and calculator guide
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-14"
  source: official
  tags: "finance,fire,retirement,independence"
  always: false
  triggers: ""
  agent: finance
---

## FIRE Planning Workflow

Follow these 5 steps when a user asks about FIRE, retirement, or financial independence.

### Step 1: Assess Current Financial Position

Gather baseline data before calculating anything:

1. `net_worth` — current net worth
2. `report_spending(period: "yearly")` — annual expenses
3. `report_spending(period: "monthly")` — monthly breakdown for savings rate

From these, derive:
- **Annual expenses** — needed for all FIRE variants
- **Savings rate** — (income - expenses) / income
- **Current savings** — from net worth or investment accounts

If the user hasn't provided these numbers, ask. Never guess financial figures.

### Step 2: Choose FIRE Variant

| Variant | Who it's for | Action |
|---------|-------------|--------|
| **Traditional FIRE** | Standard 25x expenses target | `fire_traditional` |
| **Coast FIRE** | "I have enough saved, just let it grow" | `fire_coast` |
| **Lean FIRE** | Minimal lifestyle, retire faster | `fire_lean` |
| **Fat FIRE** | Comfortable retirement, higher target | `fire_fat` |

Guide the user if they're unsure:
- "I want to retire as fast as possible" → Lean FIRE
- "I want to maintain my current lifestyle" → Traditional FIRE
- "I want a comfortable retirement" → Fat FIRE
- "When can I stop aggressively saving?" → Coast FIRE

### Step 3: Calculate FIRE Number and Timeline

Run the chosen variant action. Present results clearly:
- **FIRE number** — the total portfolio value needed
- **Years to FIRE** — how long until they reach the target
- **Monthly savings needed** — to stay on track

Always show assumptions (withdrawal rate, expected return, inflation).

### Step 4: Run Withdrawal Simulation

After calculating the target, validate it with Monte Carlo simulation:

`fire_withdrawal_sim(portfolio_value: <fire_number>, annual_withdrawal: <annual_expenses>, years: 30, num_simulations: 1000)`

Present results:
- **Success rate** — % of simulations where money lasted
- **Median outcome** — typical ending portfolio value
- **Worst-case scenario** — lowest ending value
- Target 95%+ success rate. If below, suggest adjustments.

Optionally run `fire_backtest` to show how the strategy would have performed historically.

### Step 5: Sensitivity Analysis

Run `fire_sensitivity` to show how results change across variable ranges:
- What if expenses are 10-20% higher?
- What if savings rate changes?
- What if withdrawal rate is 3% vs 4% vs 5%?

This helps the user understand the margin of safety in their plan.

## FIRE Variant Quick Reference

| Variant | Multiplier | Typical Withdrawal Rate | Notes |
|---------|-----------|------------------------|-------|
| Lean | 25x lean expenses | 4% | Minimal lifestyle, fastest path |
| Traditional | 25x current expenses | 4% | Standard FIRE |
| Fat | 25x * fat_multiplier | 3-4% | Comfortable, slower path |
| Coast | Varies by age | N/A (keep working for expenses) | Stop saving, let compounding work |

## Common Mistakes

- **Forgetting inflation** — FIRE calculations should use real (inflation-adjusted) returns
- **Ignoring healthcare costs** — especially for early retirees without employer coverage
- **Using pre-tax income for expenses** — always use actual spending, not gross income
- **Not accounting for sequence-of-returns risk** — that's why we run simulations
