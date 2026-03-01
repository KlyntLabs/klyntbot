# feature-finance

## Purpose

The `feature-finance` crate is a self-contained personal finance feature package for Klyntbot. It sits at **Layer 3** of the workspace architecture and implements the `FeaturePackage` trait from `tools-core`. It provides a single `FinanceTool` with 40+ actions spanning accounts, transactions, budgets, investments, goals, liabilities, and reporting. The crate also includes a `PriceService` for live market data and a `FinanceHandler` trait for proactive financial analysis.

## Key Types

### Domain enums (10 total, using the `DomainEnum` derive macro)

**`AccountType`** -- Cash, Bank, Ewallet, CryptoWallet, Brokerage, Other. Aliases allow flexible parsing (e.g., "e_wallet" -> Ewallet, "cryptowallet" -> CryptoWallet).

**`TransactionType`** -- Income, Expense (default), Transfer.

**`BudgetPeriod`** -- Monthly (default), Weekly, Yearly, Custom. Aliases include shorthand like "month", "week", "year", "annual".

**`BudgetMethod`** -- Standard (default) or SixJar. The Six-Jar method allocates income across six categories with configurable ratios.

**`JarType`** -- Essentials (55%), Savings (10%), Investment (10%), Education (10%), Entertainment (10%), Charity (5%). Used when `BudgetMethod` is SixJar.

**`AssetType`** -- Stock, Etf, Crypto, RealEstate, Bond, Other, ExchangeRate. The ExchangeRate variant is used internally by `PriceService` to route to the forex API.

**`InvestmentTxType`** -- Buy (default), Sell, Dividend, RentalIncome, Interest, Split.

**`GoalType`** -- Savings (default), Purchase, DebtPayoff, Fire, Custom.

**`GoalStatus`** -- Active (default), Achieved, Abandoned.

**`LiabilityType`** -- Mortgage, CreditCard, PersonalLoan, StudentLoan, Other.

All domain enums derive `DomainEnum`, which generates `from_str_loose` (case-insensitive parsing with alias support) and `as_str` methods.

### Domain structs (8 total)

**`FinanceAccount`** -- Represents a financial account with id, name, type, currency, balance (in minor units), optional institution, archived flag, and timestamps.

**`FinanceTransaction`** -- A financial transaction tied to an account. Includes amount (minor units), category/subcategory, counterparty, transaction date, optional transfer linkage, and recurrence support.

**`FinanceBudget`** -- A spending limit for a category over a period. Supports both Standard and SixJar methods, with configurable alert thresholds (percentage of budget consumed before triggering a warning).

**`FinancePortfolio`** -- A container for investment holdings, with name, description, and currency.

**`FinanceInvestment`** -- An individual investment holding within a portfolio. Tracks asset type, symbol, quantity, cost basis, current price/value, and purchase date.

**`FinanceInvestmentTx`** -- A transaction on an investment (buy, sell, dividend, etc.) with quantity, price per unit, fees, and date.

**`FinanceGoal`** -- A financial goal (savings, purchase, debt payoff, FIRE) with target amount, current amount, deadline, monthly contribution, expected return rate, and inflation rate.

**`FinanceLiability`** -- A debt or obligation (mortgage, credit card, personal loan, student loan) with principal, remaining balance, interest rate, and monthly payment.

### Filter types

**`FinanceTransactionFilter`** -- Filters transactions by account, type, category, date range, amount range, free-text query, and result limit. Converts to the storage-layer filter via `to_storage_filter()`.

**`FinanceInvestmentDomainFilter`** -- Filters investments by portfolio, asset type, and symbol presence.

### Services

**`PriceService`** -- HTTP price fetcher with an in-memory `DashMap`-backed TTL cache. Routes price requests by asset type:
- Stocks/ETFs: Yahoo Finance API (`/v8/finance/chart/{symbol}`)
- Crypto: CoinGecko API (`/api/v3/simple/price`), with a built-in ticker-to-CoinGecko-ID mapping for 25+ major coins
- Exchange rates: Open Exchange Rates API (`open.er-api.com`)

Features retry logic with backoff for HTTP 429 responses, and falls back to stale cache entries when live fetches fail. The service is cheaply cloneable (Arc-wrapped internals).

### Handler trait

**`FinanceHandler`** -- Dependency inversion trait defined here, implemented in the agent crate. Methods:
- `daily_review()` -- Generate a daily financial summary
- `check_budgets()` -> `Vec<BudgetAlert>` -- Scan budgets for overspend
- `refresh_prices()` -> `PriceUpdateSummary` -- Update all investment prices
- `analyze_spending(period)` -- AI-powered spending analysis
- `run_health_check()` -- Comprehensive financial health assessment
- `proactivity_level()` -> `ProactivityLevel` -- Current proactivity setting

**`ProactivityLevel`** -- Three tiers: Full (daily reviews, proactive warnings, price alerts), Moderate (alerts for significant events only), Reactive (responds to explicit queries only).

**`BudgetAlert`** -- Raised when category spending exceeds the budget limit. Contains budget name, category, spent amount, limit, percentage consumed, and currency.

**`PriceUpdateSummary`** -- Returned after a price refresh cycle with counts of updated/failed holdings and detail messages.

### Configuration

**`FinanceConfig`** -- Top-level config with nested sections:
- `enabled` (default true), `default_currency` (default "USD"), `proactivity_level` (default "full")
- `inflation`: rate (default 3.3%), source (default "manual")
- `expected_returns`: per-asset-class annual returns (stocks 10%, crypto 15%, real estate 8%, bonds 5%)
- `budgeting`: default method, alert threshold (default 80%), six-jar ratios
- `price_refresh`: enabled, interval hours (default 4), cache TTL minutes (default 15)
- `scheduling`: daily review time (default "21:00"), weekly report day (default "monday"), budget check time (default "09:00"), optional timezone
- `categories`: auto-categorize enabled, confidence threshold (default 0.8)

### Feature package

**`FinanceFeature`** -- Implements `FeaturePackage` with:
- `name()` -> "finance"
- `tools()` -> the single `FinanceTool`
- `migrations()` -> one SQL migration creating accounts, transactions, budgets, portfolios, investments, investment_txs, goals, and liabilities tables
- `config_key()` -> "finance"
- `default_config()` -> serialized `FinanceConfig`
- `health_check()` -> always returns Healthy

## How It Works

### FinanceTool actions (40+ actions grouped by category)

**Accounts** -- `account_add`, `account_list`, `account_update`, `account_delete`. Manage financial accounts across types (bank, cash, e-wallet, crypto wallet, brokerage). Amounts are stored in minor units (cents/satoshi) as integers to avoid floating-point precision issues.

**Transactions** -- `tx_add`, `tx_list`, `tx_update`, `tx_delete`, `tx_search`, `tx_recurring_add`. Record income, expenses, and transfers. Transfers create linked transaction pairs. Recurring transactions store an RRULE for automatic scheduling. Search supports filtering by account, category, date range, amount range, and free-text query.

**Budgets** -- `budget_create`, `budget_list`, `budget_status`, `budget_update`, `budget_delete`. Create spending limits per category with weekly/monthly/yearly/custom periods. Budget status computes actual spending against the limit for the current period. Supports the Six-Jar budgeting method (Essentials 55%, Savings 10%, Investment 10%, Education 10%, Entertainment 10%, Charity 5%) with configurable ratios.

**Investments** -- `portfolio_create`, `portfolio_list`, `investment_add`, `investment_update`, `investment_tx`, `investment_summary`, `price_fetch`, `price_refresh`. Manage investment portfolios and holdings. Investment summary computes total value, gain/loss, and allocation breakdown. `price_fetch` retrieves a single asset price via `PriceService`. `price_refresh` updates all holdings with current market data.

**Goals and liabilities** -- `goal_create`, `goal_list`, `goal_update`, `goal_fire`, `goal_whatif`, `liability_add`, `liability_list`, `liability_update`, `net_worth`. Financial goals support FIRE (Financial Independence, Retire Early) planning with configurable withdrawal rate, annual expenses, expected returns, and inflation. What-if scenarios model the impact of extra savings or different return rates. Net worth aggregates account balances, investment values, and liability remaining balances.

**Reports** -- `report_spending`, `report_income`, `report_trends`, `report_net_worth_history`, `daily_review`. Spending and income reports aggregate transactions by category over a date range. Trends show spending patterns over configurable intervals (weekly, monthly). Net worth history tracks total assets minus liabilities over time. Daily review provides a comprehensive financial snapshot.

**Settings** -- `settings_get`, `settings_update`. Read and modify the finance configuration at runtime, including default currency, proactivity level, budgeting method, and alert thresholds. Persisted via the `ConfigPersistence` trait.

**Health check** -- `finance_health_check`. Runs a comprehensive financial health assessment via the `FinanceHandler`, analyzing spending patterns, budget adherence, investment diversification, and goal progress.

### Row-domain conversions

All 8 domain structs have bidirectional `From` implementations to/from their storage row counterparts. Domain enums use `from_str_loose` for deserialization (case-insensitive with alias support) and `as_str` for serialization. Amounts are stored as `i64` in minor currency units throughout.

### Builder pattern

`FinanceTool` uses a builder pattern for optional dependencies:
- `with_finance_handler(Arc<dyn FinanceHandler>)` for proactive analysis
- `with_config_persistence(Arc<dyn ConfigPersistence>)` for settings persistence
- `from_storage_pool(&StoragePool, currency)` convenience constructor

## Connections

**Depends on:**
- `common` (error types, Result alias)
- `storage` (FinanceStorage aggregate, row types, SqlitePool)
- `tools-core` (Tool trait, FeaturePackage trait, ParamExtractor, RoutingContext, ConfigPersistence, DomainEnum derive macro)
- `chrono`, `serde`, `serde_json`, `uuid`, `async-trait`, `reqwest`, `dashmap`, `urlencoding`, `tracing`

**Depended on by:**
- `agent` (implements FinanceHandler; constructs FinanceFeature)
- `klyntbot` (re-exports via facade)
- Integration tests
