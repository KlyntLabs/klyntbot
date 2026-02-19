# AI-First Personal Finance Agent — Design

**Date:** 2026-02-19
**Status:** Approved
**Scope:** All 7 feature groups, chat-only, no UI

## Overview

A personal finance and investment management system built as a klyntbot feature — not a separate app. It runs entirely through natural language chat, with an autonomous AI agent that tracks transactions, manages budgets, monitors investments, and forecasts financial independence.

### Design Decisions

- **Single FinanceTool**: One mega tool (~37 actions) covering all domains. Same pattern as TodoTool.
- **FinanceHandler trait**: Dependency inversion for autonomous behaviors (daily reviews, anomaly detection, price updates). Defined in `tools`, implemented in `agent`.
- **Fully international**: Multi-currency at the row level. No hardcoded locale. Each transaction/account/investment carries its own `currency` (ISO 4217).
- **Auto-fetch prices**: Yahoo Finance (stocks/ETFs), CoinGecko (crypto), exchange rate APIs (currency conversion). Real estate remains manual.
- **Proactivity levels**: `full` (default) → `moderate` → `reactive`. User-configurable.
- **Text tables + ASCII charts**: All output is formatted markdown. Works in every channel (Telegram, Discord, CLI).
- **Categories are strings**: The LLM infers categories from context. No rigid taxonomy — the agent learns from the user's language over time.

---

## 1. Data Model

### finance_accounts

Bank accounts, wallets, cash pools.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT NOT NULL | e.g., "VietcomBank Savings" |
| account_type | TEXT NOT NULL | cash, bank, ewallet, crypto_wallet, brokerage, other |
| currency | TEXT NOT NULL | ISO 4217 (e.g., "VND", "USD") |
| balance | BIGINT NOT NULL | Stored in smallest unit (dong, cents) |
| institution | TEXT | e.g., "VietcomBank", "Momo" |
| notes | TEXT | |
| is_archived | BOOLEAN DEFAULT FALSE | |
| created_at | TIMESTAMPTZ | |
| updated_at | TIMESTAMPTZ | |

### finance_transactions

Every money movement.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| account_id | TEXT FK | → finance_accounts |
| type | TEXT NOT NULL | income, expense, transfer |
| amount | BIGINT NOT NULL | Always positive; type determines direction |
| currency | TEXT NOT NULL | ISO 4217 |
| category | TEXT | LLM-inferred or user-specified |
| subcategory | TEXT | |
| counterparty | TEXT | e.g., "VinMart", "Grab" |
| notes | TEXT | |
| date | DATE NOT NULL | |
| is_recurring | BOOLEAN DEFAULT FALSE | |
| recurring_rule | TEXT | Cron-style pattern |
| created_at | TIMESTAMPTZ | |

**Indexes:** `(account_id, date)`, `(category, date)`

### finance_budgets

Spending limits per category/period.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT NOT NULL | |
| category | TEXT | NULL = total budget |
| amount | BIGINT NOT NULL | |
| currency | TEXT NOT NULL | |
| period | TEXT NOT NULL | monthly, weekly, yearly, custom |
| method | TEXT DEFAULT 'standard' | standard, six_jar |
| jar_type | TEXT | essentials, savings, investment, education, entertainment, charity |
| start_date | DATE NOT NULL | |
| end_date | DATE | NULL = ongoing |
| is_active | BOOLEAN DEFAULT TRUE | |

### finance_portfolios

Groups of investments.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT NOT NULL | e.g., "Vietnam Stocks", "Crypto" |
| description | TEXT | |
| currency | TEXT NOT NULL | Base currency for display |
| created_at | TIMESTAMPTZ | |

### finance_investments

Individual holdings.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| portfolio_id | TEXT FK | → finance_portfolios |
| asset_type | TEXT NOT NULL | stock, etf, crypto, real_estate, bond, other |
| symbol | TEXT | e.g., "VIC", "BTC" (nullable for real estate) |
| name | TEXT NOT NULL | |
| currency | TEXT NOT NULL | |
| quantity | DOUBLE PRECISION NOT NULL | |
| cost_basis | BIGINT NOT NULL | Total invested |
| current_price | BIGINT | Per-unit price |
| current_value | BIGINT | Total current value |
| purchase_date | DATE | |
| notes | TEXT | |
| updated_at | TIMESTAMPTZ | |

**Index:** `(portfolio_id, symbol)`

### finance_investment_transactions

Buy/sell/dividend events.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| investment_id | TEXT FK | → finance_investments |
| type | TEXT NOT NULL | buy, sell, dividend, rental_income, interest, split |
| quantity | DOUBLE PRECISION | |
| price_per_unit | BIGINT | |
| total_amount | BIGINT NOT NULL | |
| currency | TEXT NOT NULL | |
| fees | BIGINT DEFAULT 0 | |
| date | DATE NOT NULL | |
| notes | TEXT | |

### finance_goals

Financial targets including FIRE.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT NOT NULL | |
| goal_type | TEXT NOT NULL | savings, purchase, debt_payoff, fire, custom |
| target_amount | BIGINT NOT NULL | |
| current_amount | BIGINT DEFAULT 0 | |
| currency | TEXT NOT NULL | |
| deadline | DATE | |
| monthly_contribution | BIGINT | |
| expected_return_rate | DOUBLE PRECISION | Annual % |
| inflation_rate | DOUBLE PRECISION | Annual % |
| notes | TEXT | |
| status | TEXT DEFAULT 'active' | active, achieved, abandoned |
| created_at | TIMESTAMPTZ | |

### finance_liabilities

Debts and obligations.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT NOT NULL | |
| type | TEXT NOT NULL | mortgage, credit_card, personal_loan, student_loan, other |
| principal | BIGINT NOT NULL | Original amount |
| remaining | BIGINT NOT NULL | Current balance |
| currency | TEXT NOT NULL | |
| interest_rate | DOUBLE PRECISION | Annual % |
| monthly_payment | BIGINT | |
| due_date | DATE | |
| notes | TEXT | |
| created_at | TIMESTAMPTZ | |

---

## 2. FinanceTool Actions (~37 total)

Single tool registered as `"finance"`. Action dispatched via `"action"` parameter.

### Account Management (4)

| Action | Params | Returns |
|--------|--------|---------|
| `account_add` | name, type, currency, balance, institution? | Created account with id |
| `account_list` | include_archived?, currency? | All accounts with balances |
| `account_update` | id, name?, balance?, is_archived? | Updated account |
| `account_delete` | id | Confirmation |

### Transactions (6)

| Action | Params | Returns |
|--------|--------|---------|
| `tx_add` | account_id, type, amount, category?, date?, notes?, counterparty? | Created tx + updated balance + budget impact |
| `tx_list` | account_id?, category?, date_from?, date_to?, type?, limit? | Filtered transaction list |
| `tx_update` | id, amount?, category?, notes? | Updated tx |
| `tx_delete` | id | Confirmation + balance adjustment |
| `tx_search` | query?, amount_min?, amount_max?, date_from?, date_to? | Matching transactions |
| `tx_recurring_add` | account_id, type, amount, category, recurring_rule, notes? | Created recurring rule |

### Budgets (5)

| Action | Params | Returns |
|--------|--------|---------|
| `budget_create` | name, amount, currency, period, category?, method?, jar_type? | Created budget |
| `budget_list` | period? | All active budgets with usage % |
| `budget_status` | id | Detailed breakdown: spent/remaining/% per subcategory |
| `budget_update` | id, amount?, category?, is_active? | Updated budget |
| `budget_delete` | id | Confirmation |

### Investments (7)

| Action | Params | Returns |
|--------|--------|---------|
| `portfolio_create` | name, description?, currency? | Created portfolio |
| `portfolio_list` | — | All portfolios with total values |
| `investment_add` | portfolio_id, asset_type, symbol?, name, quantity, cost_basis, currency | Created holding |
| `investment_update` | id, current_price?, current_value?, notes? | Updated holding |
| `investment_tx` | investment_id, type, quantity?, price_per_unit?, total_amount?, fees?, date? | Recorded investment transaction |
| `investment_summary` | portfolio_id? | P&L, return %, asset allocation table |
| `price_fetch` | symbol, asset_type | Fetched price + updated holding |

### Net Worth & Liabilities (4)

| Action | Params | Returns |
|--------|--------|---------|
| `liability_add` | name, type, principal, remaining, interest_rate?, monthly_payment?, currency | Created liability |
| `liability_list` | — | All debts with totals |
| `liability_update` | id, remaining?, monthly_payment? | Updated liability |
| `net_worth` | currency? | Assets - liabilities with full breakdown |

### Goals & FIRE (5)

| Action | Params | Returns |
|--------|--------|---------|
| `goal_create` | name, goal_type, target_amount, currency, deadline?, monthly_contribution?, expected_return_rate? | Created goal |
| `goal_list` | — | All goals with progress bars |
| `goal_update` | id, current_amount?, target_amount?, status? | Updated goal |
| `goal_fire` | annual_expenses?, savings_rate?, expected_return?, inflation_rate? | FIRE number + projection + timeline |
| `goal_whatif` | scenario params | Simulation results (adjusted timeline, amounts) |

### Reports & Analytics (4)

| Action | Params | Returns |
|--------|--------|---------|
| `report_spending` | period, date_from?, date_to?, category?, format? | Category breakdown table/chart |
| `report_income` | period, date_from?, date_to?, category? | Income breakdown |
| `report_trends` | metric, periods? | Period-over-period comparison |
| `report_net_worth_history` | date_from?, date_to?, interval? | Net worth over time |

### Settings (2)

| Action | Params | Returns |
|--------|--------|---------|
| `settings_get` | — | Current finance config |
| `settings_update` | default_currency?, proactivity_level?, inflation_rate?, expected_returns? | Updated config |

---

## 3. FinanceHandler — Autonomous Behaviors

Trait defined in `tools` crate, implemented in `agent` crate (dependency inversion).

```rust
#[async_trait]
pub trait FinanceHandler: Send + Sync {
    async fn daily_review(&self) -> Result<FinanceInsight>;
    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>>;
    async fn refresh_prices(&self) -> Result<PriceUpdateSummary>;
    async fn analyze_spending(&self, period: Period) -> Result<SpendingAnalysis>;
    fn proactivity_level(&self) -> ProactivityLevel;
}
```

### Proactivity Levels

| Level | Behavior |
|-------|----------|
| `full` (default) | Daily summaries, budget alerts, anomaly detection, auto-categorization, price updates, spending analysis, FIRE nudges |
| `moderate` | Budget alerts + weekly summaries only |
| `reactive` | Only responds when asked |

### Scheduled Jobs (via CronTool)

| Job | Schedule | Description |
|-----|----------|-------------|
| `finance_daily_review` | 9 PM daily | Categorize uncategorized txns, daily summary |
| `finance_budget_check` | 9 AM daily | Budget threshold alerts (>80%) |
| `finance_price_refresh` | Every 4 hours | Fetch stock/crypto prices |
| `finance_weekly_report` | Monday 10 AM | Weekly trends, investment performance, net worth |
| `finance_monthly_close` | 1st of month | Month-end summary, FIRE progress |

### Anomaly Detection (LLM-Powered)

Daily review passes transactions to LLM with historical context to flag:
- Unusual amounts vs. historical averages
- Category spending spikes mid-period
- Missing expected recurring transactions
- Potential duplicate transactions

### Auto-Categorization

When `tx_add` has no category:
1. LLM infers from notes, counterparty, amount, account context
2. Checks user's historical patterns (same counterparty → same category)
3. Confidence > threshold (default 0.8) → apply automatically
4. Below threshold → queue for daily review or ask user

---

## 4. External API Integration

### Price Providers

| Provider | Assets | Auth | Rate Limit |
|----------|--------|------|------------|
| Yahoo Finance | Stocks, ETFs, indices | None | ~2000 req/hour |
| CoinGecko | Crypto | None (free tier) | 30 req/min |
| Exchange rate API | Currency conversion | None (free tier) | ~1500 req/month |

### PriceService

Lives in `tools` crate. No dependency inversion needed (HTTP calls only).

- **Cache**: In-memory `DashMap<String, (f64, Instant)>`, 15-minute TTL
- **Degradation**: API failure → keep last known price, report the failure
- **Real estate**: Manual updates only (no reliable API for individual properties)
- **Currency conversion**: On-demand for cross-currency summaries (e.g., net_worth in USD)

---

## 5. Configuration

New `"finance"` section in `~/.klyntbot/config.json`:

```json
{
  "finance": {
    "enabled": true,
    "defaultCurrency": "USD",
    "proactivityLevel": "full",
    "inflation": {
      "rate": 3.3,
      "source": "manual"
    },
    "expectedReturns": {
      "stocks": 10.0,
      "crypto": 15.0,
      "realEstate": 8.0,
      "bonds": 5.0
    },
    "budgeting": {
      "defaultMethod": "standard",
      "alertThreshold": 80,
      "sixJarRatios": {
        "essentials": 55,
        "savings": 10,
        "investment": 10,
        "education": 10,
        "entertainment": 10,
        "charity": 5
      }
    },
    "priceRefresh": {
      "enabled": true,
      "intervalHours": 4,
      "cacheTtlMinutes": 15
    },
    "scheduling": {
      "dailyReviewTime": "21:00",
      "weeklyReportDay": "monday",
      "budgetCheckTime": "09:00",
      "timezone": null
    },
    "categories": {
      "autoCategorize": true,
      "confidenceThreshold": 0.8
    }
  }
}
```

All values modifiable via chat (`settings_update` action).

---

## 6. Chat Interaction Principles

### Behavioral Rules

1. **Infer everything possible** — currency from account, category from context, date defaults to today, account defaults to primary
2. **Confirm only when ambiguous** — "350k groceries" needs nothing more; "paid rent" needs an amount
3. **Always show impact** — every transaction shows updated balance/budget/net worth context
4. **Offer next steps** — after FIRE calc suggest what-if, after budget alert suggest adjustments
5. **Terse by default, detailed on request** — quick recording gets one line, "show details" gets the full table
6. **Learn from history** — same counterparty → same category, recurring amounts → suggest recurring rule

### Example Interactions

**Quick recording:**
```
User: spent 350k on groceries at vinmart
Agent: Recorded: -350,000 VND → Food & Groceries (VinMart), today.
       Cash account balance: 12,450,000 VND.
```

**Natural query:**
```
User: how much did I spend on food this month?
Agent: Food & Dining this month: 4,250,000 VND across 23 transactions.
       [category breakdown table]
       Budget: 5,000,000 VND → 85% used, 750K remaining (12 days left).
```

**FIRE calculator:**
```
User: when can I retire?
Agent: FIRE number: 4,500,000,000 VND (180M/year × 25)
       Current net worth: 1,200,000,000 VND (27%)
       Estimated FIRE date: March 2039 (~13 years)
       Want me to run a what-if?
```

**Proactive daily summary (autonomous):**
```
Agent: Daily Finance Summary — Feb 19
       Today: -1,250,000 VND (3 transactions)
       [transaction list]
       Warning: Transportation at 92% of monthly budget.
       Portfolio: +0.8% today. Net worth: 1,201,500,000 VND.
```

---

## 7. Storage Layer

### New Repositories

| Repo | Tables | Added to `Repos` aggregate |
|------|--------|---------------------------|
| `FinanceAccountRepo` | `finance_accounts` | `repos.finance_accounts` |
| `FinanceTransactionRepo` | `finance_transactions` | `repos.finance_transactions` |
| `FinanceBudgetRepo` | `finance_budgets` | `repos.finance_budgets` |
| `FinanceInvestmentRepo` | `finance_investments`, `finance_portfolios`, `finance_investment_transactions` | `repos.finance_investments` |
| `FinanceGoalRepo` | `finance_goals` | `repos.finance_goals` |
| `FinanceLiabilityRepo` | `finance_liabilities` | `repos.finance_liabilities` |

### Key Computed Queries (SQL)

- **Budget usage**: `SUM(amount) FROM finance_transactions WHERE category = $1 AND date BETWEEN $2 AND $3`
- **Net worth**: `SUM(accounts.balance) + SUM(investments.current_value) - SUM(liabilities.remaining)`
- **Spending trends**: `GROUP BY date_trunc('month', date), category`
- **FIRE projection**: Application-level future value formula using aggregated monthly savings + expected returns

### Migrations

8 migration files, auto-run via `StoragePool::connect()`. Indexes on high-query columns (account_id+date, category+date, portfolio_id+symbol).

### Price Cache

In-memory `DashMap<String, (f64, Instant)>` in PriceService. Not persisted. 4-hour cron repopulates after restart.
