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
| "FIRE number" | `fire_traditional` | annual_expenses, savings_rate, current_savings, annual_income |
| "coast FIRE" | `fire_coast` | current_savings, age, target_retirement_age, annual_expenses |
| "unusual spending" | `analyze_spending_anomalies` | lookback_months (3), sensitivity ("medium") |
| "spending trends" | `analyze_spending_trends` | months (6), group_by ("category") |
| "recurring charges" | `analyze_recurring_charges` | lookback_months (3) |
| "portfolio drift" | `portfolio_drift` | -- |
| "rebalance" | `portfolio_rebalance` | -- |
| "record net worth" | `snapshot_record` | note (optional) |
| "net worth history" | `snapshot_history` | months (12) |

Use the `klyntbot - finance` MCP tool for financial management.

## Critical Rules

- **Amounts in smallest unit**: $50 = 5000 cents, 100k VND = 100000 dong
- **Never guess IDs** — use list actions first
- **Default currency** from `settings_get` — don't hardcode

For all actions and setup workflow, read `references/actions.md`.

## Common Mistakes

1. **Using dollars instead of cents** — All amounts are in the smallest currency unit. $25.50 = 2550, not 25.50. This is the most critical rule.
2. **Negative amounts on tx_add** — Amounts must ALWAYS be positive. The `type` field ("expense"/"income") determines the direction. Never pass negative amounts.
3. **Using `tx_type` instead of `type`** — The transaction type parameter is `type`, not `tx_type`. Use `type: "expense"` or `type: "income"`.
   - **But for investments, use `asset_type`** — NOT `type`. Use `asset_type: "stock"` or `asset_type: "crypto"`. The `type` param is ONLY for transactions.
4. **Guessing account or budget IDs** — Always call `account_list` or `budget_list` first. Never reuse IDs from a previous conversation.
5. **Hardcoding currency** — Call `settings_get` to determine the user's default currency. Don't assume USD.
6. **Wrong transaction type values** — Use "expense" for spending, "income" for earnings. Not "debit"/"credit".
7. **Missing category on transactions** — Always include a category when adding transactions.
8. **Duplicate snapshot_record on same day** — `snapshot_record` can only be called ONCE per day per currency (UNIQUE constraint). If you get a UNIQUE constraint error, the snapshot was already recorded today — just inform the user.
9. **Wrong sensitivity direction** — For `analyze_spending_anomalies`, "low" catches MORE anomalies (lower threshold), "high" catches FEWER (stricter). Think of it as "how anomalous must it be to report."
10. **Forgetting to set allocation targets before drift check** — `portfolio_drift` requires targets. Use `allocation_target_list` first; if empty, guide the user to set targets with `allocation_target_set`.
11. **Not chaining FIRE actions** — A complete FIRE analysis chains: calculate variant -> withdrawal simulation -> sensitivity analysis. Don't just run one action in isolation.
12. **Wrong account type on account_add** — Valid types: `bank`, `cash`, `ewallet` (or `e_wallet`), `crypto_wallet` (or `cryptowallet`), `brokerage`, `other`. NOT "checking", "savings", "credit", "investment", "deposit", or "loan".

**Parameter name cheat sheet — `type` vs `asset_type`:**
| Action | Param name | Values |
|--------|-----------|--------|
| `tx_add` | `type` | "expense", "income", "transfer" |
| `account_add` | `type` | "bank", "cash", "ewallet", "crypto_wallet", "brokerage", "other" |
| `investment_add` | `asset_type` | "stock", "crypto", "bond", "real_estate", "commodity" |
| `liability_add` | `type` | "personal_loan", "student_loan", "credit_card", "mortgage" |
13. **Ignoring auto-conversion** — When adding a transaction with a `currency` different from the default, `base_amount` is computed automatically. Do NOT manually pass `base_amount`.
14. **Forgetting market_currency on investments** — For investments quoted in a foreign currency (e.g., BTC in USD), pass `market_currency: "USD"` on `investment_add`. This enables three-tier display: quantity + market price + home equivalent.

## Workflow Tip: FIRE Planning Chain

For a complete FIRE analysis, chain these actions in sequence:
1. `fire_traditional` (or coast/lean/fat) — get the FIRE number
2. `fire_withdrawal_sim` — Monte Carlo validation of the plan
3. `fire_sensitivity` — show how results change with different assumptions

## Red Flags — STOP

If you're about to do any of these, STOP:
- Pass a dollar amount (25.50) instead of cents (2550)
- Pass a negative amount to `tx_add` (amounts are always positive)
- Use `tx_type` instead of `type` on `tx_add`
- Use a hardcoded account_id or budget_id
- Assume the user's currency is USD without checking settings
- Add a transaction without confirming the amount and category with the user
- Modify financial data without explicit user confirmation
- Call `snapshot_record` twice in the same day (will fail with UNIQUE constraint)

## Related Skills

- **klyntbot-automation** — Set up recurring budget check reminders or spending alerts
- **klyntbot-tasks** — Create tasks for financial action items (e.g., "pay invoice")
- **klyntbot-okr** — Track financial objectives and savings goals
