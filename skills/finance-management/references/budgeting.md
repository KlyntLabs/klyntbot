---
name: budgeting
description: Personal finance management — accounts, transactions, budgets, investments, goals, and reports
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "finance,budget,money,spending"
  always: true
  triggers: ""
  agent: finance
---

## Agent Instructions

Use the `finance` tool for all financial operations.

### First-Time Setup

Check if accounts exist first: `{"action": "account_list"}`

If no accounts, guide setup:
1. Create first account: `{"action": "account_add", "name": "Main Bank", "type": "bank", "currency": "VND", "balance": 0}`
2. Confirm default currency: `{"action": "settings_get"}`
3. Optionally create budget and portfolio
4. Consider running `analyze_recurring_charges` to identify existing subscriptions and recurring costs

### Action Routing

| User says | Action | Key params |
|-----------|--------|-----------|
| "How much did I spend?" | `report_spending` | period (default: monthly) |
| "Add $50 groceries" | `tx_add` | amount (in cents), category, type=expense |
| "Check my budget" | `budget_status` | (no ID = show all) |
| "Any unusual spending?" | `analyze_spending_anomalies` | lookback_months, sensitivity |
| "What's my net worth?" | `net_worth` | — |
| "Bitcoin price" | `price_fetch` | symbol=BTC, asset_type=crypto |
| "FIRE number" | `fire_traditional` | annual_expenses, savings_rate |
| "Spending trends" | `analyze_spending_trends` | months, group_by |
| "Spending correlations" | `analyze_category_correlation` | months, categories |

### Critical Rules

1. **Amounts are in smallest currency unit** (cents for USD, dong for VND). $50 = 5000 cents.
2. **Never guess IDs.** Use list actions to discover IDs.
3. **Auto-account selection works.** If `account_id` omitted, first active account is used.
4. **Period defaults to "monthly"** for reports when not specified.
5. **Default currency** comes from `settings_get` — don't hardcode.
6. **Don't create duplicate budgets** for same category + period.

### Cross-References

- After checking budget status, use `analyze_spending_anomalies` to detect unusual spending that may be affecting budgets
- For trends and correlations, use `analyze_spending_trends` and `analyze_category_correlation` to understand spending drivers
- Use `analyze_recurring_charges` to identify subscriptions and recurring costs during budget setup or review
- For complete analytical action reference, see `references/analytics-actions.md`

### Response Formatting

- Currency amounts: always show with symbol and grouping
- Budget status: include percentage and visual indicator
- Tables: use for multi-row data
