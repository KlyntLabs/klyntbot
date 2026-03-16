---
name: portfolio-analysis
description: Portfolio analytics workflow — drift, rebalancing, returns, and correlation analysis
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-14"
  source: official
  tags: "finance,portfolio,investment,allocation"
  always: false
  triggers: ""
  agent: finance
---

## Portfolio Analysis Workflow

Follow these 5 steps for comprehensive portfolio analysis.

### Step 1: Check Allocation Drift

`portfolio_drift`

Shows current allocation vs targets and drift amounts. Requires allocation targets to be set first — if none exist, guide the user to set them (Step 5).

Look for:
- Any asset class drifted more than 5% from target → flag for rebalancing
- Overall drift score — higher means more urgent rebalancing needed

### Step 2: Identify Rebalancing Needs

If drift is significant:

`portfolio_rebalance`

Returns specific trades needed to bring portfolio back to target allocation. Present as a clear action list:
- "Sell $X of Asset A"
- "Buy $Y of Asset B"

Never execute trades — only present recommendations. The user must decide and act.

### Step 3: Calculate Portfolio Returns

`portfolio_returns(period: "yearly")`

Shows time-weighted return (TWR) and money-weighted return (MWR):
- **TWR** — measures portfolio performance independent of cash flows (good for comparing to benchmarks)
- **MWR** — measures actual investor experience including timing of deposits/withdrawals

Compare against relevant benchmarks (e.g., S&P 500, total market index).

Available periods: "monthly", "quarterly", "yearly", "all".

### Step 4: Analyze Asset Correlations

`portfolio_correlation(months: 12)`

Shows correlation matrix between portfolio assets. Look for:
- **High positive correlation (> 0.8)** — assets move together, less diversification benefit
- **Low/negative correlation (< 0.3)** — good diversification
- **Concentration risk** — if most assets are highly correlated

Use this to suggest diversification improvements.

### Step 5: Set/Adjust Allocation Targets

If no targets exist, or user wants to change them:

`allocation_target_set(asset_class: "stocks", target_percent: 60)`
`allocation_target_set(asset_class: "bonds", target_percent: 30)`
`allocation_target_set(asset_class: "cash", target_percent: 10)`

`allocation_target_list` — verify all targets sum to 100%.

Common allocation templates:
- **Aggressive (young)**: 90% stocks, 10% bonds
- **Moderate**: 60% stocks, 30% bonds, 10% alternatives
- **Conservative (near retirement)**: 40% stocks, 50% bonds, 10% cash
- **Target-date**: Adjust based on years to retirement

## Response Guidelines

- Always show percentages alongside dollar amounts
- Compare returns to benchmarks when possible
- Highlight diversification gaps
- Never recommend specific securities — present analysis, let user decide
- If portfolio data is sparse, note limitations in the analysis
