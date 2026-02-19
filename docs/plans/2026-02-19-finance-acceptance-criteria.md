# FinanceTool — Acceptance Criteria & Specification

**Date:** 2026-02-19
**Status:** Approved (architect-reviewed)
**Depends on:** `2026-02-19-ai-first-personal-finance-design.md`

---

## Clarified Decisions (from user review)

1. **File structure**: Split into `finance_tool/` directory — `mod.rs` (dispatcher) + `accounts.rs`, `transactions.rs`, `budgets.rs`, `investments.rs`, `goals.rs`, `reports.rs`, `settings.rs`
2. **Domain types**: Two-layer pattern like TodoTool — `finance_types.rs` in tools crate with domain types + `From<StorageRow>` / `From<&DomainType>` conversions
3. **Transfers**: Two linked rows with nullable `transfer_id TEXT` column on `finance_transactions`. Transfers create paired transactions (expense on source + income on destination) with matching `transfer_id`
4. **Settings persistence**: `settings_update` writes changes to `~/.klyntbot/config.json` immediately via config loader. In-memory config values (e.g., `default_currency` held as `String` in FinanceTool) remain stale until restart. Response: "Settings saved. Restart to apply."
5. **Resolved**: PriceService in tools crate (reqwest already there), budget usage via SQL in repo, `outbound_tx` via constructor injection, `tx_date` = user date / `created_at` = system timestamp, `current_value` stored and updated on `price_fetch`, graceful DB skip for tests
6. **Param naming**: LLM-facing params use short names (`type`, not `account_type`). Mapped internally: `type` → `account_type` / `tx_type` / `liability_type` in storage. Transfer destination param: `transfer_to_account_id`.
7. **Liabilities routing**: `liability_*` and `net_worth` actions dispatch to `goals.rs` alongside `goal_*` actions (conceptually related as "balance sheet" view).
8. **Cost basis method**: Average cost basis on sell (not FIFO/LIFO). `cost_basis -= (cost_basis / old_quantity) * quantity`.
9. **Balance atomicity**: Sequential operations in same `execute()` call, no `pool.begin()` in v1. Acceptable for single-user personal finance.

---

## Migration Note

The `transfer_id TEXT` column and its partial index are included in the initial `CREATE TABLE finance_transactions` statement (not a separate ALTER, since the table is brand new):

```sql
-- In the CREATE TABLE statement:
transfer_id TEXT,
-- After the table:
CREATE INDEX IF NOT EXISTS idx_finance_tx_transfer ON finance_transactions(transfer_id) WHERE transfer_id IS NOT NULL;
```

---

## 1. Account Management (4 actions)

File: `crates/tools/src/finance_tool/accounts.rs`

### 1.1 `account_add`

**Given** a user provides name, account_type, currency, and balance
**When** they call `account_add`
**Then** a new account is created with a UUID id, the provided fields, `is_archived = false`, and timestamps set to now
**And** the response includes the account id, name, formatted balance, and currency

**Params:**
- `name` (required, string)
- `type` (required, string — one of: cash, bank, ewallet, crypto_wallet, brokerage, other)
- `currency` (required, string — ISO 4217)
- `balance` (required, i64 — in smallest currency unit)
- `institution` (optional, string)
- `notes` (optional, string)

**Edge cases:**
- **Given** `type` is not in the allowed set, **Then** return `InvalidParams("Invalid account type: {type}. Must be one of: cash, bank, ewallet, crypto_wallet, brokerage, other")`
- **Given** `currency` is empty or longer than 3 characters, **Then** return `InvalidParams("Currency must be a valid ISO 4217 code")`
- **Given** `name` is empty, **Then** return `InvalidParams("Account name is required")`
- **Given** `balance` is negative, **Then** allow it (overdraft/credit line accounts)

### 1.2 `account_list`

**Given** the user has one or more accounts
**When** they call `account_list`
**Then** all non-archived accounts are returned, grouped by currency, with formatted balances and totals per currency

**Params:**
- `include_archived` (optional, bool — default false)
- `currency` (optional, string — filter to specific currency)

**Edge cases:**
- **Given** no accounts exist, **Then** return "No accounts found. Add one with account_add."
- **Given** `include_archived = true`, **Then** include archived accounts, marked with `[archived]`
- **Given** `currency = "VND"`, **Then** only return accounts with currency VND

### 1.3 `account_update`

**Given** a valid account id
**When** the user calls `account_update` with one or more fields to change
**Then** only the provided fields are updated, `updated_at` is set to now, and the updated account is returned

**Params:**
- `id` (required, string)
- `name` (optional, string)
- `balance` (optional, i64 — direct override, not delta)
- `is_archived` (optional, bool)
- `institution` (optional, string — pass null to clear)
- `notes` (optional, string — pass null to clear)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Account {id} not found")`
- **Given** no update fields provided, **Then** return `InvalidParams("No fields to update")`
- **Given** `is_archived = true` and account has unarchived transactions this month, **Then** still allow (archiving is not deletion)

### 1.4 `account_delete`

**Given** a valid account id
**When** the user calls `account_delete`
**Then** the account and all its transactions are deleted (CASCADE), and confirmation is returned

**Params:**
- `id` (required, string)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Account {id} not found")`
- **Given** account has transactions, **Then** delete proceeds (ON DELETE CASCADE), response notes how many transactions were removed
- **Given** account is referenced by a budget (same currency), **Then** still delete (budgets reference categories, not accounts)

---

## 2. Transactions (6 actions)

File: `crates/tools/src/finance_tool/transactions.rs`

### 2.1 `tx_add`

**Given** a user provides account_id, type, and amount
**When** they call `tx_add`
**Then** a transaction is created, the account balance is updated (income adds, expense subtracts), and the response includes: transaction summary, updated account balance, and budget impact (if category matches an active budget)

**Params:**
- `account_id` (required, string)
- `type` (required, string — income, expense, transfer)
- `amount` (required, i64 — always positive)
- `category` (optional, string — LLM-inferred if absent)
- `subcategory` (optional, string)
- `counterparty` (optional, string)
- `notes` (optional, string)
- `date` (optional, DATE — defaults to today)
- `transfer_to_account_id` (optional, string — required when type=transfer)

**Balance update logic:**
- `income`: `account.balance += amount`
- `expense`: `account.balance -= amount`
- `transfer`: source `account.balance -= amount`, destination `account.balance += amount` (with currency conversion if different currencies — NOT in v1, return error if currencies differ)

**Transfer mechanics (two linked rows):**
1. Generate a shared `transfer_id` (UUID)
2. Insert expense row on source account with `transfer_id`
3. Insert income row on destination account with `transfer_id`
4. Update both account balances atomically

**Budget impact response:**
- After recording, check if `category` matches any active budget for the current period
- If match: include spent/limit/percentage in response
- If spent > budget.amount * (alert_threshold / 100): include warning

**Edge cases:**
- **Given** `account_id` does not exist, **Then** return `ExecutionFailed("Account {account_id} not found")`
- **Given** `type = "transfer"` but `transfer_to_account_id` is missing, **Then** return `InvalidParams("transfer_to_account_id is required for transfer transactions")`
- **Given** `type = "transfer"` and source and destination have different currencies, **Then** return `ExecutionFailed("Cross-currency transfers not yet supported. Record as separate expense and income.")`
- **Given** `type = "transfer"` and source == destination, **Then** return `InvalidParams("Cannot transfer to the same account")`
- **Given** `type` not in allowed set, **Then** return `InvalidParams("Invalid transaction type")`
- **Given** `amount <= 0`, **Then** return `InvalidParams("Amount must be positive")`
- **Given** `date` is in the future, **Then** allow it (scheduled/planned transactions)
- **Given** expense would make account balance negative, **Then** allow it (overdraft is valid)

### 2.2 `tx_list`

**Given** the user wants to see transactions
**When** they call `tx_list` with optional filters
**Then** transactions are returned in reverse chronological order, formatted as a table

**Params:**
- `account_id` (optional, string)
- `category` (optional, string)
- `date_from` (optional, DATE)
- `date_to` (optional, DATE)
- `type` (optional, string — income, expense, transfer)
- `limit` (optional, i64 — default 20, max 100)

**Edge cases:**
- **Given** no transactions match filters, **Then** return "No transactions found matching your filters."
- **Given** no filters provided, **Then** return most recent 20 transactions across all accounts
- **Given** `limit > 100`, **Then** cap at 100

### 2.3 `tx_update`

**Given** a valid transaction id
**When** the user calls `tx_update` with fields to change
**Then** the transaction is updated and, if amount changed, the account balance is adjusted by the delta

**Params:**
- `id` (required, string)
- `amount` (optional, i64)
- `category` (optional, string — pass null to clear)
- `subcategory` (optional, string — pass null to clear)
- `counterparty` (optional, string — pass null to clear)
- `notes` (optional, string — pass null to clear)
- `date` (optional, DATE)

**Balance adjustment on amount change:**
- Calculate delta: `new_amount - old_amount`
- If type was `income`: `account.balance += delta`
- If type was `expense`: `account.balance -= delta`
- If type was `transfer`: adjust both linked accounts (find via `transfer_id`)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Transaction {id} not found")`
- **Given** transaction is part of a transfer pair (has `transfer_id`), and amount is changed, **Then** update both transactions and both account balances
- **Given** no update fields provided, **Then** return `InvalidParams("No fields to update")`

### 2.4 `tx_delete`

**Given** a valid transaction id
**When** the user calls `tx_delete`
**Then** the transaction is deleted, the account balance is reversed, and confirmation is returned

**Params:**
- `id` (required, string)

**Balance reversal:**
- `income` transaction deleted: `account.balance -= amount`
- `expense` transaction deleted: `account.balance += amount`
- `transfer` transaction deleted: reverse both sides, delete both linked rows

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Transaction {id} not found")`
- **Given** transaction has `transfer_id`, **Then** delete both linked transactions and reverse both account balances
- **Given** balance reversal would make account balance negative, **Then** allow it

### 2.5 `tx_search`

**Given** the user wants to find specific transactions
**When** they call `tx_search` with search criteria
**Then** matching transactions are returned, filtered by keyword, amount range, and/or date range

**Params:**
- `query` (optional, string — searches notes, counterparty, category via ILIKE)
- `amount_min` (optional, i64)
- `amount_max` (optional, i64)
- `date_from` (optional, DATE)
- `date_to` (optional, DATE)
- `limit` (optional, i64 — default 20, max 100)

**Edge cases:**
- **Given** no search criteria provided, **Then** return `InvalidParams("At least one search criterion is required")`
- **Given** `amount_min > amount_max`, **Then** return `InvalidParams("amount_min cannot exceed amount_max")`
- **Given** no results found, **Then** return "No transactions found matching your search."

### 2.6 `tx_recurring_add`

**Given** the user wants to mark a transaction pattern as recurring
**When** they call `tx_recurring_add`
**Then** a transaction template is created with `is_recurring = true` and the specified `recurring_rule`

**Params:**
- `account_id` (required, string)
- `type` (required, string)
- `amount` (required, i64)
- `category` (required, string)
- `recurring_rule` (required, string — cron-style, e.g. "0 0 1 * *" for monthly on 1st)
- `counterparty` (optional, string)
- `notes` (optional, string)

**Edge cases:**
- **Given** `account_id` does not exist, **Then** return `ExecutionFailed("Account not found")`
- **Given** `recurring_rule` is not a valid cron expression, **Then** return `InvalidParams("Invalid recurring rule. Use cron format, e.g., '0 0 1 * *' for monthly on the 1st")`
- **Given** rule already exists for same account/category/amount combo, **Then** still create (user may have multiple recurring expenses to same category)

---

## 3. Budgets (5 actions)

File: `crates/tools/src/finance_tool/budgets.rs`

### 3.1 `budget_create`

**Given** a user wants to set a spending limit
**When** they call `budget_create`
**Then** a budget is created and confirmation includes the budget name, limit, period, and category

**Params:**
- `name` (required, string)
- `amount` (required, i64 — in smallest currency unit)
- `currency` (required, string)
- `period` (required, string — monthly, weekly, yearly, custom)
- `category` (optional, string — null = total budget across all categories)
- `method` (optional, string — "standard" or "six_jar", default "standard")
- `jar_type` (optional, string — required when method = "six_jar")
- `start_date` (optional, DATE — defaults to today)
- `end_date` (optional, DATE — null = ongoing)

**Edge cases:**
- **Given** `period` is not in allowed set, **Then** return `InvalidParams("Invalid period")`
- **Given** `method = "six_jar"` but `jar_type` is missing, **Then** return `InvalidParams("jar_type is required for six_jar budgeting")`
- **Given** `jar_type` is not in the allowed set, **Then** return `InvalidParams("Invalid jar type")`
- **Given** `amount <= 0`, **Then** return `InvalidParams("Budget amount must be positive")`
- **Given** `end_date < start_date`, **Then** return `InvalidParams("End date cannot be before start date")`

### 3.2 `budget_list`

**Given** active budgets exist
**When** the user calls `budget_list`
**Then** all active budgets are returned with their current usage percentage for the active period

**Params:**
- `period` (optional, string — filter by period type)

**Usage calculation (SQL in repo):**
- For each budget, `SUM(amount) FROM finance_transactions WHERE tx_type = 'expense' AND category = budget.category AND tx_date BETWEEN period_start AND period_end`
- If budget.category is NULL, sum all expenses in the period
- Period start/end derived from budget.period + budget.start_date

**Response format:**
```
Budget Status
| Budget         | Category    | Spent     | Limit     | %    | Remaining |
|----------------|-------------|-----------|-----------|------|-----------|
| Monthly Food   | food        | 3,500,000 | 5,000,000 | 70%  | 1,500,000 |
| Entertainment  | fun         | 900,000   | 1,000,000 | 90%  | 100,000   |
```

**Edge cases:**
- **Given** no active budgets, **Then** return "No active budgets. Create one with budget_create."
- **Given** budget has no matching transactions, **Then** show 0% used

### 3.3 `budget_status`

**Given** a valid budget id
**When** the user calls `budget_status`
**Then** a detailed breakdown is returned: total spent, remaining, percentage, and per-subcategory breakdown within the budget's category

**Params:**
- `id` (required, string)

**Response includes:**
- Budget name, category, period, limit
- Total spent this period
- Remaining amount
- Usage percentage
- Subcategory breakdown (GROUP BY subcategory)
- Days remaining in period
- Daily burn rate projection

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Budget {id} not found")`
- **Given** budget is inactive, **Then** still show status but note it is inactive
- **Given** budget category is NULL (total budget), **Then** sum all expense categories

### 3.4 `budget_update`

**Given** a valid budget id
**When** the user calls `budget_update`
**Then** specified fields are updated and the updated budget is returned

**Params:**
- `id` (required, string)
- `name` (optional, string)
- `amount` (optional, i64)
- `category` (optional, string — pass null to clear, making it a total budget)
- `is_active` (optional, bool)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Budget {id} not found")`
- **Given** no fields to update, **Then** return `InvalidParams("No fields to update")`

### 3.5 `budget_delete`

**Given** a valid budget id
**When** the user calls `budget_delete`
**Then** the budget is deleted and confirmation is returned

**Params:**
- `id` (required, string)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Budget {id} not found")`

---

## 4. Investments (7 actions)

File: `crates/tools/src/finance_tool/investments.rs`

### 4.1 `portfolio_create`

**Given** a user wants to group investments
**When** they call `portfolio_create`
**Then** a portfolio is created with a UUID id

**Params:**
- `name` (required, string)
- `description` (optional, string)
- `currency` (optional, string — defaults to config.finance.default_currency)

**Edge cases:**
- **Given** `name` is empty, **Then** return `InvalidParams("Portfolio name is required")`

### 4.2 `portfolio_list`

**Given** portfolios exist
**When** the user calls `portfolio_list`
**Then** all portfolios are returned with their total value, cost basis, and return %

**Response format:**
```
Portfolios
| Name            | Holdings | Cost Basis  | Current Value | Return   |
|-----------------|----------|-------------|---------------|----------|
| Vietnam Stocks  | 5        | 50,000,000  | 62,000,000    | +24.0%   |
| Crypto          | 3        | 20,000,000  | 18,500,000    | -7.5%    |
```

**Edge cases:**
- **Given** no portfolios exist, **Then** return "No portfolios found. Create one with portfolio_create."
- **Given** a portfolio has no investments, **Then** show 0 for all values

### 4.3 `investment_add`

**Given** a valid portfolio_id
**When** the user calls `investment_add`
**Then** an investment holding is created and added to the portfolio

**Params:**
- `portfolio_id` (required, string)
- `asset_type` (required, string — stock, etf, crypto, real_estate, bond, other)
- `symbol` (optional, string — nullable for real_estate)
- `name` (required, string)
- `quantity` (required, f64)
- `cost_basis` (required, i64 — total invested in smallest unit)
- `currency` (required, string)
- `purchase_date` (optional, DATE)
- `notes` (optional, string)

**Edge cases:**
- **Given** `portfolio_id` does not exist, **Then** return `ExecutionFailed("Portfolio not found")`
- **Given** `asset_type` not in allowed set, **Then** return `InvalidParams("Invalid asset type")`
- **Given** `asset_type` is "stock" or "etf" or "crypto" but `symbol` is missing, **Then** return `InvalidParams("Symbol is required for {asset_type} investments")`
- **Given** `quantity <= 0`, **Then** return `InvalidParams("Quantity must be positive")`

### 4.4 `investment_update`

**Given** a valid investment id
**When** the user calls `investment_update`
**Then** specified fields are updated, `updated_at` is refreshed

**Params:**
- `id` (required, string)
- `current_price` (optional, i64 — per-unit, in smallest currency unit)
- `current_value` (optional, i64 — total value override)
- `quantity` (optional, f64)
- `notes` (optional, string — pass null to clear)

**Auto-compute:** If `current_price` is provided but `current_value` is not, compute `current_value = current_price * quantity`. If `current_value` is provided, use it directly (for real estate or manual overrides).

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Investment not found")`
- **Given** no update fields, **Then** return `InvalidParams("No fields to update")`

### 4.5 `investment_tx`

**Given** a valid investment id
**When** the user records an investment transaction (buy, sell, dividend, etc.)
**Then** the transaction is recorded and the investment's quantity, cost_basis, and current_value are updated accordingly

**Params:**
- `investment_id` (required, string)
- `type` (required, string — buy, sell, dividend, rental_income, interest, split)
- `quantity` (optional, f64 — required for buy/sell/split)
- `price_per_unit` (optional, i64)
- `total_amount` (required, i64)
- `currency` (optional, string — defaults to investment's currency)
- `fees` (optional, i64 — default 0)
- `date` (optional, DATE — defaults to today)
- `notes` (optional, string)

**Side effects by type:**
- `buy`: `investment.quantity += quantity`, `investment.cost_basis += total_amount + fees`
- `sell`: `investment.quantity -= quantity`, `investment.cost_basis -= (cost_basis / old_quantity) * quantity` (proportional cost reduction)
- `dividend` / `rental_income` / `interest`: no quantity/cost change, purely informational income record
- `split`: `investment.quantity *= quantity` (quantity here is the split ratio, e.g. 2.0 for 2:1), `investment.current_price /= quantity`

**Edge cases:**
- **Given** `investment_id` does not exist, **Then** return `ExecutionFailed("Investment not found")`
- **Given** `type = "sell"` and `quantity > investment.quantity`, **Then** return `ExecutionFailed("Cannot sell more than current holding ({quantity} available)")`
- **Given** `type = "buy"` or `type = "sell"` but `quantity` is missing, **Then** return `InvalidParams("Quantity is required for buy/sell transactions")`
- **Given** `type` not in allowed set, **Then** return `InvalidParams("Invalid investment transaction type")`

### 4.6 `investment_summary`

**Given** investments exist
**When** the user calls `investment_summary`
**Then** a detailed portfolio summary is returned with P&L, return %, and asset allocation

**Params:**
- `portfolio_id` (optional, string — if omitted, show all portfolios combined)

**Response includes:**
- Per-investment: name, symbol, quantity, cost basis, current value, P&L, return %
- Totals: total cost basis, total current value, total P&L, weighted return %
- Asset allocation: percentage breakdown by asset_type
- Last price update timestamps

**Edge cases:**
- **Given** `portfolio_id` does not exist, **Then** return `ExecutionFailed("Portfolio not found")`
- **Given** portfolio has no investments, **Then** return summary with zero values
- **Given** some investments have no `current_value`, **Then** show "N/A" for those, exclude from totals with a note

### 4.7 `price_fetch`

**Given** a symbol and asset type
**When** the user calls `price_fetch`
**Then** PriceService fetches the latest price, updates matching investment(s), and returns the fetched price

**Params:**
- `symbol` (required, string)
- `asset_type` (required, string — stock, etf, crypto)

**Side effects:**
- Find all investments with matching `symbol` and `asset_type`
- Update `current_price` with fetched price (converted to smallest unit)
- Recompute `current_value = current_price * quantity`
- Update `updated_at`

**Edge cases:**
- **Given** `asset_type = "real_estate"`, **Then** return `ExecutionFailed("Price fetch is not available for real estate. Use investment_update to set the value manually.")`
- **Given** API call fails (network error, rate limit), **Then** return `ExecutionFailed("Failed to fetch price for {symbol}: {error}. Last known price retained.")`
- **Given** symbol not found by the API, **Then** return `ExecutionFailed("Symbol {symbol} not found on {provider}")`
- **Given** cache has a fresh price (within TTL), **Then** return cached price without API call
- **Given** no investments match the symbol, **Then** still return the fetched price (informational)

---

## 5. Net Worth & Liabilities (4 actions)

File: `crates/tools/src/finance_tool/goals.rs` (liabilities section)

### 5.1 `liability_add`

**Given** a user has a debt or obligation
**When** they call `liability_add`
**Then** the liability is recorded

**Params:**
- `name` (required, string)
- `type` (required, string — mortgage, credit_card, personal_loan, student_loan, other)
- `principal` (required, i64 — original amount)
- `remaining` (required, i64 — current balance)
- `currency` (required, string)
- `interest_rate` (optional, f64 — annual %)
- `monthly_payment` (optional, i64)
- `due_date` (optional, DATE)
- `notes` (optional, string)

**Edge cases:**
- **Given** `type` not in allowed set, **Then** return `InvalidParams("Invalid liability type")`
- **Given** `remaining > principal`, **Then** allow it (interest may have increased the balance)
- **Given** `principal <= 0` or `remaining < 0`, **Then** return `InvalidParams("Amounts must be positive")`

### 5.2 `liability_list`

**Given** liabilities exist
**When** the user calls `liability_list`
**Then** all liabilities are returned with totals

**Response format:**
```
Liabilities
| Name            | Type          | Remaining   | Rate  | Monthly   | Due Date   |
|-----------------|---------------|-------------|-------|-----------|------------|
| Home Mortgage   | mortgage      | 800,000,000 | 7.5%  | 8,500,000 | 2040-01-15 |
| Credit Card     | credit_card   | 5,000,000   | 18.0% | 500,000   | 2026-03-01 |
Total remaining: 805,000,000 VND
```

**Edge cases:**
- **Given** no liabilities exist, **Then** return "No liabilities recorded."

### 5.3 `liability_update`

**Given** a valid liability id
**When** the user calls `liability_update`
**Then** specified fields are updated

**Params:**
- `id` (required, string)
- `remaining` (optional, i64)
- `monthly_payment` (optional, i64 — pass null to clear)
- `interest_rate` (optional, f64 — pass null to clear)
- `notes` (optional, string — pass null to clear)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Liability not found")`
- **Given** no update fields, **Then** return `InvalidParams("No fields to update")`

### 5.4 `net_worth`

**Given** the user wants a financial snapshot
**When** they call `net_worth`
**Then** a full breakdown is returned: total assets (accounts + investments) minus total liabilities

**Params:**
- `currency` (optional, string — display currency; if omitted, use config.finance.default_currency)

**Calculation:**
1. Sum all account balances (grouped by currency)
2. Sum all investment `current_value` (grouped by currency)
3. Sum all liability `remaining` (grouped by currency)
4. If `currency` is specified and balances are multi-currency, convert using PriceService exchange rates
5. Net worth = total_assets - total_liabilities

**Response format:**
```
Net Worth Summary
Assets:
  Bank accounts:  15,000,000 VND
  Investments:    62,000,000 VND
  Total assets:   77,000,000 VND

Liabilities:
  Mortgage:       800,000,000 VND
  Credit card:    5,000,000 VND
  Total liabilities: 805,000,000 VND

Net Worth: -728,000,000 VND
```

**Edge cases:**
- **Given** no data exists at all, **Then** return "No financial data recorded yet. Start by adding an account."
- **Given** multi-currency data and no `currency` param, **Then** show separate totals per currency, no conversion
- **Given** `currency` specified but exchange rate fetch fails, **Then** show per-currency totals and note "Exchange rate unavailable for conversion"
- **Given** some investments have no `current_value`, **Then** exclude from total and note "X investments have no current value set"

---

## 6. Goals & FIRE (5 actions)

File: `crates/tools/src/finance_tool/goals.rs`

### 6.1 `goal_create`

**Given** a user has a financial target
**When** they call `goal_create`
**Then** the goal is created with status "active"

**Params:**
- `name` (required, string)
- `goal_type` (required, string — savings, purchase, debt_payoff, fire, custom)
- `target_amount` (required, i64)
- `currency` (required, string)
- `deadline` (optional, DATE)
- `monthly_contribution` (optional, i64)
- `expected_return_rate` (optional, f64 — annual %)
- `inflation_rate` (optional, f64 — annual %; defaults to config.finance.inflation.rate)
- `notes` (optional, string)

**Edge cases:**
- **Given** `goal_type` not in allowed set, **Then** return `InvalidParams`
- **Given** `target_amount <= 0`, **Then** return `InvalidParams("Target amount must be positive")`
- **Given** `deadline` is in the past, **Then** return `InvalidParams("Deadline must be in the future")`

### 6.2 `goal_list`

**Given** goals exist
**When** the user calls `goal_list`
**Then** all goals are returned with progress bars

**Response format:**
```
Financial Goals
| Goal               | Type      | Progress        | Target      | Deadline   |
|--------------------|-----------|-----------------|-------------|------------|
| Emergency Fund     | savings   | [======    ] 60% | 50,000,000  | 2026-12-31 |
| New Car            | purchase  | [==        ] 20% | 200,000,000 | 2027-06-01 |
| FIRE               | fire      | [===       ] 27% | 4,500,000,000 | —        |
```

**Edge cases:**
- **Given** no goals, **Then** return "No financial goals set. Create one with goal_create."
- **Given** goal has `status = "achieved"`, **Then** show with checkmark

### 6.3 `goal_update`

**Given** a valid goal id
**When** the user calls `goal_update`
**Then** specified fields are updated

**Params:**
- `id` (required, string)
- `name` (optional, string)
- `current_amount` (optional, i64)
- `target_amount` (optional, i64)
- `monthly_contribution` (optional, i64 — pass null to clear)
- `expected_return_rate` (optional, f64 — pass null to clear)
- `inflation_rate` (optional, f64 — pass null to clear)
- `deadline` (optional, DATE — pass null to clear)
- `status` (optional, string — active, achieved, abandoned)

**Edge cases:**
- **Given** `id` does not exist, **Then** return `ExecutionFailed("Goal not found")`
- **Given** `status` not in allowed set, **Then** return `InvalidParams`
- **Given** `current_amount >= target_amount`, **Then** auto-suggest changing status to "achieved"

### 6.4 `goal_fire`

**Given** the user wants to calculate their FIRE (Financial Independence, Retire Early) number
**When** they call `goal_fire`
**Then** the FIRE number, current progress, and estimated timeline are returned

**Params:**
- `annual_expenses` (optional, i64 — if omitted, calculate from last 12 months of expense transactions)
- `savings_rate` (optional, f64 — if omitted, calculate from income vs expenses)
- `expected_return` (optional, f64 — defaults to config.finance.expected_returns weighted average)
- `inflation_rate` (optional, f64 — defaults to config.finance.inflation.rate)

**FIRE calculation:**
1. FIRE number = `annual_expenses * 25` (4% rule)
2. Current net worth from `net_worth` calculation
3. Monthly savings = monthly_income - monthly_expenses (from transactions)
4. Real return rate = (expected_return - inflation_rate) / 100 / 12
5. Months to FIRE: solve `fire_number = net_worth * (1+r)^n + monthly_savings * ((1+r)^n - 1) / r`
6. FIRE date = today + n months

**Response format:**
```
FIRE Analysis
FIRE Number: 4,500,000,000 VND (180M/year x 25)
Current Net Worth: 1,200,000,000 VND (27%)
Monthly Savings: 15,000,000 VND
Expected Return: 10.0% (real: 6.7%)
Estimated FIRE Date: March 2039 (~13 years)

Want me to run a what-if scenario?
```

**Edge cases:**
- **Given** no transactions exist (can't compute expenses/income), **Then** require `annual_expenses` param, return `InvalidParams("No transaction history. Please provide annual_expenses.")`
- **Given** monthly savings is 0 or negative, **Then** return FIRE number with message "At current spending, FIRE is not achievable without increasing income or reducing expenses."
- **Given** user is already at FIRE number, **Then** return "Congratulations! Your net worth exceeds your FIRE number."

### 6.5 `goal_whatif`

**Given** the user wants to explore scenarios
**When** they call `goal_whatif`
**Then** a simulation is run with the adjusted parameters and compared to the baseline

**Params:**
- `monthly_contribution` (optional, i64 — override monthly savings)
- `expected_return` (optional, f64 — override return rate)
- `inflation_rate` (optional, f64 — override inflation)
- `annual_expenses` (optional, i64 — override expenses)
- `additional_income` (optional, i64 — monthly, added to current savings)
- `lump_sum` (optional, i64 — one-time addition to net worth)
- `retirement_age` (optional, i32 — target age instead of FIRE number)

**Response format:**
```
What-If Scenario
Baseline FIRE date: March 2039 (13 years)
With changes:
  + 5,000,000/month additional savings
  + 100,000,000 lump sum investment
New FIRE date: September 2035 (9 years)
Improvement: 3.5 years earlier
```

**Edge cases:**
- **Given** no params provided, **Then** return `InvalidParams("Provide at least one scenario parameter")`
- **Given** negative `monthly_contribution`, **Then** allow (models reduced savings)
- **Given** baseline can't be computed (no data), **Then** return error asking for explicit baseline params

---

## 7. Reports & Analytics (4 actions)

File: `crates/tools/src/finance_tool/reports.rs`

### 7.1 `report_spending`

**Given** transaction history exists
**When** the user calls `report_spending`
**Then** a category breakdown of expenses is returned for the specified period

**Params:**
- `period` (required, string — "week", "month", "quarter", "year", "custom")
- `date_from` (optional, DATE — required if period = "custom")
- `date_to` (optional, DATE — required if period = "custom")
- `category` (optional, string — drill into specific category for subcategory breakdown)
- `format` (optional, string — "table" or "chart", default "table")

**Period resolution:**
- "week": last 7 days
- "month": current calendar month (1st to today)
- "quarter": current quarter
- "year": current calendar year
- "custom": date_from to date_to

**Response format (table):**
```
Spending Report — February 2026
| Category        | Amount      | %    | Txns |
|-----------------|-------------|------|------|
| Food & Dining   | 4,250,000   | 34%  | 23   |
| Transportation  | 2,100,000   | 17%  | 15   |
| Entertainment   | 1,800,000   | 14%  | 8    |
| ...             | ...         | ...  | ...  |
Total: 12,500,000 VND
```

**Response format (chart):**
```
Spending — February 2026
Food & Dining   ████████████████░░░░ 34%
Transportation  ████████░░░░░░░░░░░░ 17%
Entertainment   ██████░░░░░░░░░░░░░░ 14%
Shopping        █████░░░░░░░░░░░░░░░ 12%
Utilities       ████░░░░░░░░░░░░░░░░ 10%
Other           █████░░░░░░░░░░░░░░░ 13%
```

**Edge cases:**
- **Given** `period = "custom"` but `date_from` or `date_to` missing, **Then** return `InvalidParams("date_from and date_to are required for custom period")`
- **Given** no expenses in period, **Then** return "No spending recorded for this period."
- **Given** `category` is specified, **Then** show subcategory breakdown within that category

### 7.2 `report_income`

**Given** income transactions exist
**When** the user calls `report_income`
**Then** a category breakdown of income is returned

**Params:**
- `period` (required, string — same as report_spending)
- `date_from` (optional, DATE)
- `date_to` (optional, DATE)
- `category` (optional, string)

**Same period resolution and format as `report_spending`, but filtered to `tx_type = 'income'`.**

**Edge cases:**
- Same as `report_spending`

### 7.3 `report_trends`

**Given** historical data exists
**When** the user calls `report_trends`
**Then** a period-over-period comparison is returned showing the trend direction

**Params:**
- `metric` (required, string — "spending", "income", "savings", "net_worth", "investment_value")
- `periods` (optional, i32 — number of periods to compare, default 6)

**Period resolution:** Uses monthly periods by default.

**Response format:**
```
Spending Trend (last 6 months)
| Month    | Amount      | Change   |
|----------|-------------|----------|
| Feb 2026 | 12,500,000  | +5.2%    |
| Jan 2026 | 11,880,000  | -3.1%    |
| Dec 2025 | 12,260,000  | +8.7%    |
| Nov 2025 | 11,280,000  | -1.2%    |
| Oct 2025 | 11,420,000  | +2.3%    |
| Sep 2025 | 11,160,000  | —        |
Average: 11,750,000 VND/month
Trend: +2.1% month-over-month
```

**Edge cases:**
- **Given** `metric` not in allowed set, **Then** return `InvalidParams`
- **Given** less than 2 periods of data, **Then** return what's available with note "Not enough data for trend analysis"
- **Given** `metric = "net_worth"` or `"investment_value"`, **Then** use point-in-time values (not sums)

### 7.4 `report_net_worth_history`

**Given** the user wants to track net worth over time
**When** they call `report_net_worth_history`
**Then** a time series of net worth snapshots is returned

**Params:**
- `date_from` (optional, DATE — defaults to 6 months ago)
- `date_to` (optional, DATE — defaults to today)
- `interval` (optional, string — "daily", "weekly", "monthly"; default "monthly")

**Calculation:** For each interval point, compute:
- Account balances as of that date (reconstruct from transactions)
- Investment values as of that date (use last known `current_value` at or before that date)
- Liabilities as of that date (use last known `remaining`)

**Response format:**
```
Net Worth History
| Date       | Assets      | Liabilities | Net Worth   | Change     |
|------------|-------------|-------------|-------------|------------|
| 2026-02-01 | 77,000,000  | 805,000,000 | -728,000,000| +2,000,000 |
| 2026-01-01 | 73,000,000  | 803,000,000 | -730,000,000| +5,000,000 |
| 2025-12-01 | 66,000,000  | 801,000,000 | -735,000,000| —          |
```

**Edge cases:**
- **Given** `interval` not in allowed set, **Then** return `InvalidParams`
- **Given** no historical data, **Then** return current snapshot only
- **Given** `date_from > date_to`, **Then** return `InvalidParams`

---

## 8. Settings (2 actions)

File: `crates/tools/src/finance_tool/settings.rs`

### 8.1 `settings_get`

**Given** the finance feature is enabled
**When** the user calls `settings_get`
**Then** the current finance configuration is returned in human-readable format

**Params:** None

**Response format:**
```
Finance Settings
Default currency: USD
Proactivity level: full
Inflation rate: 3.3% (manual)
Expected returns: stocks 10.0%, crypto 15.0%, real estate 8.0%, bonds 5.0%
Budgeting: standard method, alert at 80%
Price refresh: every 4 hours, cache 15 min
Scheduling: daily review 9PM, budget check 9AM, weekly report Monday
Auto-categorize: enabled (confidence 0.8)
```

### 8.2 `settings_update`

**Given** the user wants to change finance configuration
**When** they call `settings_update` with one or more settings
**Then** the settings are persisted to `~/.klyntbot/config.json` immediately, and the response notes "Settings saved. Restart to apply." (in-memory config values remain stale until restart, matching existing codebase convention)

**Params:**
- `default_currency` (optional, string)
- `proactivity_level` (optional, string — "full", "moderate", "reactive")
- `inflation_rate` (optional, f64)
- `expected_returns` (optional, object — `{ stocks?, crypto?, real_estate?, bonds? }`)
- `alert_threshold` (optional, u8 — 1-100)
- `auto_categorize` (optional, bool)
- `confidence_threshold` (optional, f64 — 0.0-1.0)

**Persistence:** Uses config loader's `save()` which calls `diff_json()` — only changed fields are written.

**Edge cases:**
- **Given** `proactivity_level` not in allowed set, **Then** return `InvalidParams("Invalid proactivity level")`
- **Given** `confidence_threshold` outside 0.0-1.0 range, **Then** return `InvalidParams("Confidence threshold must be between 0.0 and 1.0")`
- **Given** `alert_threshold` outside 1-100, **Then** return `InvalidParams("Alert threshold must be between 1 and 100")`
- **Given** no settings to update, **Then** return `InvalidParams("No settings to update")`
- **Given** config file write fails, **Then** return `ExecutionFailed("Failed to save settings: {error}")`

---

## Cross-Cutting Concerns

### Error Handling

All actions follow three error paths:
1. `ToolError::InvalidParams(msg)` — bad user input, wrong enum value, missing required field
2. `ToolError::ExecutionFailed(msg)` — domain logic failure (not found, business rule violation)
3. Propagated storage errors — `StorageError` auto-converts via `From` to `KlyntbotError`

Pattern for "not found":
```rust
let account = self.accounts.get(id).await?
    .ok_or_else(|| ToolError::ExecutionFailed(format!("Account {} not found", id)))?;
```

### Response Formatting

All responses are plain text with markdown-compatible formatting:
- Tables use `| col | col |` format
- Numbers are formatted with thousands separators appropriate to the amount (e.g., 1,000,000)
- Currencies shown as suffix (e.g., "1,000,000 VND")
- Percentages to 1 decimal place
- Dates as YYYY-MM-DD

### Balance Atomicity

For operations that modify both a transaction and an account balance:
- Both operations happen in the same `execute()` call
- If the balance update fails after transaction insert, the error propagates and the caller sees a failure
- Future improvement: wrap in a database transaction via `pool.begin()`

### Currency Handling

- All monetary amounts stored as BIGINT in smallest unit (cents, dong, satoshi)
- Currency code stored alongside every amount
- Cross-currency operations require explicit conversion via PriceService
- No implicit currency conversion — user must specify target currency

### Transfer Consistency

- Both linked transactions share a `transfer_id` UUID
- Delete one side → must delete both sides
- Update amount on one side → must update both sides
- Query by `transfer_id` to find the paired transaction
