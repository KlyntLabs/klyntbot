---
name: analytics-actions
description: Quick reference for all 19 analytical finance actions — spending, FIRE, portfolio, allocation, and snapshots
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-14"
  source: official
  tags: "finance,analytics,fire,portfolio,spending"
  always: false
  triggers: ""
  agent: finance
---

## Spending Analytics

| Action | Description | Key Params |
|--------|-------------|------------|
| `analyze_spending_anomalies` | Detect unusual spending vs historical norms | lookback_months (default: 3), sensitivity ("low"/"medium"/"high", default: "medium") |
| `analyze_spending_trends` | Spending trends over time with direction/velocity | months (default: 6), group_by ("category"/"month"/"week") |
| `analyze_recurring_charges` | Identify recurring/subscription charges | lookback_months (default: 3) |
| `analyze_category_correlation` | Correlation between spending categories | months (default: 6), categories (optional list) |

## FIRE Planning

| Action | Description | Key Params |
|--------|-------------|------------|
| `fire_traditional` | Classic FIRE number and timeline | annual_expenses, savings_rate, withdrawal_rate (default: 0.04), current_savings, annual_income |
| `fire_coast` | Coast FIRE — stop saving, let compounding work | current_savings, age, target_retirement_age, annual_expenses, expected_return (default: 0.07) |
| `fire_lean` | Lean FIRE — minimal expenses baseline | annual_expenses, savings_rate, current_savings, annual_income |
| `fire_fat` | Fat FIRE — comfortable/luxurious retirement | annual_expenses, savings_rate, current_savings, annual_income, fat_multiplier (default: 1.5) |
| `fire_withdrawal_sim` | Monte Carlo withdrawal simulation | portfolio_value, annual_withdrawal, years, num_simulations (default: 1000) |
| `fire_backtest` | Historical backtest of withdrawal strategy | portfolio_value, annual_withdrawal, start_year, end_year |
| `fire_sensitivity` | Sensitivity analysis across variable ranges | base_annual_expenses, base_savings_rate, base_withdrawal_rate, current_savings, annual_income |

## Portfolio Analytics

| Action | Description | Key Params |
|--------|-------------|------------|
| `portfolio_drift` | Current allocation vs targets with drift amounts | — (uses existing portfolio and allocation targets) |
| `portfolio_rebalance` | Specific rebalance trades to reach target allocation | total_value (optional — uses portfolio if omitted) |
| `portfolio_returns` | Time-weighted and money-weighted returns | period ("monthly"/"quarterly"/"yearly"/"all") |
| `portfolio_correlation` | Correlation matrix between portfolio assets | months (default: 12) |

## Allocation Targets

| Action | Description | Key Params |
|--------|-------------|------------|
| `allocation_target_set` | Set target allocation for an asset class | asset_class, target_percent (0-100) |
| `allocation_target_list` | List all allocation targets | — |

## Net Worth Snapshots

| Action | Description | Key Params |
|--------|-------------|------------|
| `snapshot_record` | Record current net worth snapshot | note (optional description) |
| `snapshot_history` | View net worth over time | months (default: 12) |
