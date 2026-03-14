---
name: finance-actions
description: Complete reference for all finance tool actions
---

# Finance Actions

## First-Time Setup

1. `account_list` — check if accounts exist
2. If none: `account_add` with name, type ("bank"/"cash"/"investment"), currency, balance
3. `settings_get` — confirm default currency

## Accounts
| Action | Params |
|--------|--------|
| `account_list` | — |
| `account_add` | name, type ("bank"/"cash"/"credit"/"investment"/"crypto"), currency, balance |
| `account_update` | id, name/balance/status |

## Transactions
| Action | Params |
|--------|--------|
| `tx_add` | amount (cents!), category, type ("income"/"expense"/"transfer"), account_id, description, date |
| `tx_list` | account_id, category, type, period, limit |
| `tx_update` | id, amount/category/description |
| `tx_delete` | id |

## Budgets
| Action | Params |
|--------|--------|
| `budget_add` | category, amount (cents), period ("monthly"/"weekly"/"yearly") |
| `budget_status` | budget_id (optional — shows all if omitted) |
| `budget_update` | id, amount/category |

## Reports
| Action | Params |
|--------|--------|
| `report_spending` | period ("daily"/"weekly"/"monthly"/"yearly"), account_id |
| `report_trends` | metric ("spending"/"income"/"savings"), months |
| `finance_health_check` | — |
| `net_worth` | — |

## Investments & Goals
| Action | Params |
|--------|--------|
| `portfolio_summary` | — |
| `price_fetch` | symbol, asset_type ("stock"/"crypto"/"commodity") |
| `goal_fire` | annual_expenses, withdrawal_rate |
| `goal_add` | name, target_amount, deadline |
| `goal_status` | goal_id |

## Spending Analytics

| Action | Params | Description | Example MCP call |
|--------|--------|-------------|-----------------|
| `analyze_spending_anomalies` | lookback_months: u32 (default: 3), sensitivity: String ("low"/"medium"/"high", default: "medium") | Detect unusual spending compared to historical norms. Low sensitivity catches more anomalies; high catches fewer. | `mcp__klyntbot__finance(action: "analyze_spending_anomalies", lookback_months: 3, sensitivity: "medium")` |
| `analyze_spending_trends` | months: u32 (default: 6), group_by: String ("category"/"month"/"week") | Analyze spending direction and velocity over time. | `mcp__klyntbot__finance(action: "analyze_spending_trends", months: 6, group_by: "category")` |
| `analyze_recurring_charges` | lookback_months: u32 (default: 3) | Identify recurring/subscription charges with frequency and annual cost. | `mcp__klyntbot__finance(action: "analyze_recurring_charges", lookback_months: 3)` |
| `analyze_category_correlation` | months: u32 (default: 6), categories: Option<Vec<String>> (optional subset) | Find correlations between spending categories to identify lifestyle patterns. | `mcp__klyntbot__finance(action: "analyze_category_correlation", months: 6)` |

## FIRE Planning

| Action | Params | Description | Example MCP call |
|--------|--------|-------------|-----------------|
| `fire_traditional` | annual_expenses: f64, savings_rate: f64, withdrawal_rate: f64 (default: 0.04), current_savings: f64, annual_income: f64 | Classic FIRE: 25x expenses target with timeline projection. | `mcp__klyntbot__finance(action: "fire_traditional", annual_expenses: 40000, savings_rate: 0.5, current_savings: 200000, annual_income: 80000)` |
| `fire_coast` | current_savings: f64, age: u32, target_retirement_age: u32, annual_expenses: f64, expected_return: f64 (default: 0.07) | Coast FIRE: stop saving now, let compounding reach target by retirement. | `mcp__klyntbot__finance(action: "fire_coast", current_savings: 300000, age: 35, target_retirement_age: 60, annual_expenses: 40000)` |
| `fire_lean` | annual_expenses: f64, savings_rate: f64, current_savings: f64, annual_income: f64 | Lean FIRE: minimal expenses baseline for fastest path. | `mcp__klyntbot__finance(action: "fire_lean", annual_expenses: 25000, savings_rate: 0.6, current_savings: 150000, annual_income: 80000)` |
| `fire_fat` | annual_expenses: f64, savings_rate: f64, current_savings: f64, annual_income: f64, fat_multiplier: f64 (default: 1.5) | Fat FIRE: comfortable/luxurious retirement with higher target. | `mcp__klyntbot__finance(action: "fire_fat", annual_expenses: 60000, savings_rate: 0.4, current_savings: 300000, annual_income: 120000)` |
| `fire_withdrawal_sim` | portfolio_value: f64, annual_withdrawal: f64, years: u32, num_simulations: u32 (default: 1000) | Monte Carlo withdrawal simulation — validates plan survival rate. | `mcp__klyntbot__finance(action: "fire_withdrawal_sim", portfolio_value: 1000000, annual_withdrawal: 40000, years: 30)` |
| `fire_backtest` | portfolio_value: f64, annual_withdrawal: f64, start_year: u32, end_year: u32 | Historical backtest of withdrawal strategy against real market data. | `mcp__klyntbot__finance(action: "fire_backtest", portfolio_value: 1000000, annual_withdrawal: 40000, start_year: 1990, end_year: 2020)` |
| `fire_sensitivity` | base_annual_expenses: f64, base_savings_rate: f64, base_withdrawal_rate: f64, current_savings: f64, annual_income: f64 | Sensitivity analysis — shows how results vary across assumption ranges. | `mcp__klyntbot__finance(action: "fire_sensitivity", base_annual_expenses: 40000, base_savings_rate: 0.5, base_withdrawal_rate: 0.04, current_savings: 200000, annual_income: 80000)` |

## Portfolio Analytics

| Action | Params | Description | Example MCP call |
|--------|--------|-------------|-----------------|
| `portfolio_drift` | — | Compare current allocation vs targets; shows drift amounts per asset class. Requires allocation targets. | `mcp__klyntbot__finance(action: "portfolio_drift")` |
| `portfolio_rebalance` | total_value: Option<f64> (uses portfolio if omitted) | Calculate specific rebalance trades to reach target allocation. | `mcp__klyntbot__finance(action: "portfolio_rebalance")` |
| `portfolio_returns` | period: String ("monthly"/"quarterly"/"yearly"/"all") | Time-weighted (TWR) and money-weighted (MWR) return calculations. | `mcp__klyntbot__finance(action: "portfolio_returns", period: "yearly")` |
| `portfolio_correlation` | months: u32 (default: 12) | Correlation matrix between portfolio assets for diversification analysis. | `mcp__klyntbot__finance(action: "portfolio_correlation", months: 12)` |

## Allocation Targets

| Action | Params | Description | Example MCP call |
|--------|--------|-------------|-----------------|
| `allocation_target_set` | asset_class: String, target_percent: f64 (0-100) | Set or update target allocation for an asset class. All targets should sum to 100. | `mcp__klyntbot__finance(action: "allocation_target_set", asset_class: "stocks", target_percent: 60)` |
| `allocation_target_list` | — | List all current allocation targets. | `mcp__klyntbot__finance(action: "allocation_target_list")` |

## Net Worth Snapshots

| Action | Params | Description | Example MCP call |
|--------|--------|-------------|-----------------|
| `snapshot_record` | note: Option<String> (optional description) | Record a point-in-time net worth snapshot from current account balances. | `mcp__klyntbot__finance(action: "snapshot_record", note: "End of Q1 2026")` |
| `snapshot_history` | months: u32 (default: 12) | View net worth trend over time. | `mcp__klyntbot__finance(action: "snapshot_history", months: 12)` |
