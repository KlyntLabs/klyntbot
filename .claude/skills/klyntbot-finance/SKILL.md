---
name: klyntbot-finance
description: >
  Use when the user mentions budget, spending, money, accounts, transactions,
  investments, savings, net worth, portfolio, FIRE, expenses, income, "how much
  did I spend", "add an expense", "check my budget", or any financial tracking.
license: MIT
metadata:
  author: klyntbot
  version: "2.0.0"
  klyntbot:
    type: skill
    tools: [finance]
    mcp_tools: []
    max_iterations: 10
    invokes: [klyntbot-automation, klyntbot-tasks]
---

## Quick Reference

| User says | Action | Key params |
|-----------|--------|-----------|
| "add expense" | `tx_add` | amount (in cents!), category, type: "expense" |
| "check budget" | `budget_status` | budget_id (optional) |
| "spending report" | `report_spending` | period (default: "monthly") |
| "net worth" | `net_worth` | -- |
| "list accounts" | `account_list` | -- |
| "bitcoin price" | `price_fetch` | symbol: "BTC", asset_type: "crypto" |
| "FIRE number" | `goal_fire` | annual_expenses |

Use the `klyntbot - finance` MCP tool for financial management.

## Critical Rules

- **Amounts in smallest unit**: $50 = 5000 cents, 100k VND = 100000 dong
- **Never guess IDs** — use list actions first
- **Default currency** from `settings_get` — don't hardcode

For all actions and setup workflow, read `references/actions.md`.

## Common Mistakes

1. **Using dollars instead of cents** — All amounts are in the smallest currency unit. $25.50 = 2550, not 25.50. This is the most critical rule.
2. **Guessing account or budget IDs** — Always call `account_list` or `budget_list` first. Never reuse IDs from a previous conversation.
3. **Hardcoding currency** — Call `settings_get` to determine the user's default currency. Don't assume USD.
4. **Wrong transaction type** — Use "expense" for spending, "income" for earnings. Not "debit"/"credit".
5. **Missing category on transactions** — Always include a category when adding transactions.

## Red Flags — STOP

If you're about to do any of these, STOP:
- Pass a dollar amount (25.50) instead of cents (2550)
- Use a hardcoded account_id or budget_id
- Assume the user's currency is USD without checking settings
- Add a transaction without confirming the amount and category with the user
- Modify financial data without explicit user confirmation

## Related Skills

- **klyntbot-automation** — Set up recurring budget check reminders or spending alerts
- **klyntbot-tasks** — Create tasks for financial action items (e.g., "pay invoice")
- **klyntbot-okr** — Track financial objectives and savings goals
