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
