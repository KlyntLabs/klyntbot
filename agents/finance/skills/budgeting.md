---
name: budgeting
description: Personal finance management — accounts, transactions, budgets, investments, goals, and reports
always: true
---

## Agent Instructions

Use the `finance` tool for all financial operations.

### First-Time Setup

Check if accounts exist first: `{"action": "account_list"}`

If no accounts, guide setup:
1. Create first account: `{"action": "account_add", "name": "Main Bank", "type": "bank", "currency": "VND", "balance": 0}`
2. Confirm default currency: `{"action": "settings_get"}`
3. Optionally create budget and portfolio

### Action Routing

| User says | Action | Key params |
|-----------|--------|-----------|
| "How much did I spend?" | `report_spending` | period (default: monthly) |
| "Add $50 groceries" | `tx_add` | amount (in cents), category, type=expense |
| "Check my budget" | `budget_status` | (no ID = show all) |
| "What's my net worth?" | `net_worth` | — |
| "Bitcoin price" | `price_fetch` | symbol=BTC, asset_type=crypto |
| "FIRE number" | `goal_fire` | annual_expenses (or derive) |

### Critical Rules

1. **Amounts are in smallest currency unit** (cents for USD, dong for VND). $50 = 5000 cents.
2. **Never guess IDs.** Use list actions to discover IDs.
3. **Auto-account selection works.** If `account_id` omitted, first active account is used.
4. **Period defaults to "monthly"** for reports when not specified.
5. **Default currency** comes from `settings_get` — don't hardcode.
6. **Don't create duplicate budgets** for same category + period.

### Response Formatting

- Currency amounts: always show with symbol and grouping
- Budget status: include percentage and visual indicator
- Tables: use for multi-row data
