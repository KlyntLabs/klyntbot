---
name: finance
description: Personal finance management — accounts, transactions, budgets, investments, goals, and reports.
metadata: '{"klyntbot":{"triggers":["finance","money","budget","spending","investment","savings","net worth","account","transaction","portfolio","goal","FIRE","net_worth","price","crypto"],"always":true}}'
---

# Personal Finance

## Agent Instructions

**When the user asks about finances** (money, budget, spending, investment, savings, net worth, accounts, transactions, portfolios, goals, FIRE), use the `finance` tool.

### First-Time Setup (Onboarding)

Before any financial action, check if accounts exist:

```json
{"action": "account_list"}
```

**If no accounts exist**, guide the user through setup:

1. Create their first account:
   ```json
   {"action": "account_add", "name": "Main Bank", "type": "bank", "currency": "VND", "balance": 0}
   ```
2. Confirm default currency: `{"action": "settings_get"}`
3. Optionally create a budget: `{"action": "budget_create", ...}`
4. Optionally create a portfolio: `{"action": "portfolio_create", ...}`

### Action Routing

| User says | Action | Key params |
|-----------|--------|-----------|
| "How much did I spend?" | `report_spending` | period (default: monthly) |
| "Add $50 groceries" | `tx_add` | amount (in cents), category, type=expense |
| "Check my budget" | `budget_status` | (no ID = show all) |
| "What's my net worth?" | `net_worth` | — |
| "How's my portfolio?" | `investment_summary` | — |
| "Bitcoin price" | `price_fetch` | symbol=BTC, asset_type=crypto |
| "Set up recurring rent" | `tx_recurring_add` | amount, recurring_rule, type=expense |
| "FIRE number" | `goal_fire` | annual_expenses (or omit to derive) |
| "Financial health check" | `finance_health_check` | — |
| "Add a savings goal" | `goal_create` | name, goal_type=savings, target_amount |
| "Add a liability" | `liability_add` | name, type, principal |
| "Spending trends" | `report_trends` | metric=spending |

### Critical Rules

1. **Amounts are in smallest currency unit** (cents for USD, dong for VND). $50 = 5000 cents.
2. **Never guess IDs.** Use `account_list`, `portfolio_list`, `budget_list`, `goal_list` to discover IDs.
3. **Auto-account selection works.** If `account_id` is omitted in `tx_add`/`tx_recurring_add`, the first active account is used automatically.
4. **Accept both `type` and `tx_type`** for transaction type (income/expense/transfer).
5. **Period defaults to "monthly"** for reports when not specified.
6. **Default currency** comes from `settings_get` — don't hardcode "USD".
7. **Don't create duplicate budgets** for the same category + period.

### Response Formatting

- **Currency amounts**: Always show with symbol and grouping (e.g., $1,234.56 or 1,234,500 VND)
- **Budget status**: Include percentage and visual indicator
- **Tables**: Use tables for multi-row data (transaction lists, budget breakdowns)
- **Single values**: Show inline (net worth, price fetch results)

### Prerequisite Errors

If a finance action returns a JSON response with `"error"` and `"suggested_action"` fields, follow the suggestion. Example:

```json
{"error": "no_accounts", "message": "No accounts found.", "suggested_action": "account_add"}
```

> Guide the user to create the missing resource before retrying.

### Settings

View current settings: `{"action": "settings_get"}`
Update settings: `{"action": "settings_update", "default_currency": "USD", "proactivity_level": "full"}`

Proactivity levels:
- **full**: Daily reviews, budget alerts, price refreshes, in-chat nudges
- **moderate**: Scheduled alerts only, no in-chat nudges
- **reactive**: No automated actions, user must explicitly ask
