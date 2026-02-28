# Feature Finance Crate

## Section 1: Narrative Overview

### What This Crate Does

`feature-finance` is a self-contained personal finance feature package for Klyntbot. It lives at Layer 3 of the workspace dependency graph and provides a single unified tool (`FinanceTool`) with 40+ actions covering accounts, transactions, budgets, investments, portfolios, goals, liabilities, FIRE planning, net-worth calculation, spending reports, and runtime settings. All monetary amounts are stored as integers in the smallest currency unit (cents for 2-decimal currencies) to avoid floating-point rounding errors.

Source: `crates/feature-finance/src/lib.rs` (lines 1-128)

### Finance Tool Suite

The `FinanceTool` is a single `Tool` impl registered under the name `"finance"`. The LLM selects a sub-action via the `"action"` parameter, and the tool dispatches to one of eight handler modules:

| Module | File | Actions |
|--------|------|---------|
| Accounts | `src/tool/accounts.rs` | `account_add`, `account_list`, `account_update`, `account_delete` |
| Transactions | `src/tool/transactions.rs` | `tx_add`, `tx_list`, `tx_update`, `tx_delete`, `tx_search`, `tx_recurring_add` |
| Budgets | `src/tool/budgets.rs` | `budget_create`, `budget_list`, `budget_status`, `budget_update`, `budget_delete` |
| Investments | `src/tool/investments.rs` | `portfolio_create`, `portfolio_list`, `investment_add`, `investment_update`, `investment_tx`, `investment_summary`, `price_fetch`, `price_refresh` |
| Goals | `src/tool/goals.rs` | `goal_create`, `goal_list`, `goal_update`, `goal_fire`, `goal_whatif`, `liability_add`, `liability_list`, `liability_update`, `net_worth` |
| Reports | `src/tool/reports.rs` | `report_spending`, `report_income`, `report_trends`, `report_net_worth_history`, `daily_review` |
| Health | `src/tool/health.rs` | `finance_health_check` |
| Settings | `src/tool/settings.rs` | `settings_get`, `settings_update` |

Each handler module is an `impl FinanceTool` block with `pub(crate)` dispatch methods. The top-level `execute()` in `src/tool/mod.rs` (lines 217-257) matches the action string and delegates.

### FinanceFeature Implementing FeaturePackage

`FinanceFeature` (defined in `src/lib.rs`, lines 40-128) wraps a `FinanceTool` in an `Arc` and implements the `FeaturePackage` trait from `tools-core`:

- **`name()`** returns `"finance"`.
- **`tools()`** returns a single-element `Vec<DynTool>` containing the wrapped `FinanceTool`.
- **`migrations()`** returns one `FeatureMigration` (version 1) whose SQL is embedded at compile time via `include_str!("../migrations/001_finance_tables.sql")`. The migration is idempotent (`CREATE TABLE IF NOT EXISTS`) and creates 8 tables with indexes.
- **`config_key()`** returns `"finance"`.
- **`default_config()`** serializes `FinanceConfig::default()` to JSON.
- **`health_check()`** always returns `HealthStatus::Healthy`.

Static helpers (`migrations_static()`, `default_config_static()`, `for_tests()`) allow tests to inspect metadata without constructing a full agent context.

### Price Service

`PriceService` (`src/price_service.rs`, lines 30-341) fetches live market prices from three free APIs:

| Asset Type | API | Method |
|-----------|-----|--------|
| Stock, ETF | Yahoo Finance (`query1.finance.yahoo.com/v8/finance/chart/`) | `fetch_stock()` |
| Crypto | CoinGecko (`api.coingecko.com/api/v3/simple/price`) | `fetch_crypto()` |
| Exchange Rate | open.er-api.com (`open.er-api.com/v6/latest/`) | `fetch_exchange_rate()` |

Key design details:

- **DashMap TTL cache**: An `Arc<DashMap<String, CachedPrice>>` stores fetched prices keyed by symbol (or `SYMBOL:VS_CURRENCY` for crypto/forex). Cache entries expire after a configurable TTL (default 15 minutes).
- **Stale fallback**: On API failure, the service falls back to stale (expired) cache entries, marking the source as `"cache_stale"`.
- **Retry with backoff**: HTTP 429 (rate limit) responses trigger up to 2 retries with 1s and 3s delays (`get_with_retry()`).
- **Ticker mapping**: A `ticker_to_coingecko_id()` function maps common crypto tickers (BTC, ETH, SOL, etc.) to CoinGecko IDs.
- **Cheaply cloneable**: Both `reqwest::Client` and `DashMap` are `Arc`-wrapped internally, so `PriceService` is `Clone + Send + Sync`.

The unified `fetch_price(symbol, asset_type)` method routes to the correct API based on `AssetType`.

### Handler Design

The `FinanceHandler` trait (`src/handler.rs`, lines 72-80) follows the same dependency-inversion pattern used by `SpawnHandler` and `CronHandler` in the codebase. It is defined in `feature-finance` (not in `tools`) so `FinanceTool` can call it directly, while the implementation lives in the `agent` crate at Layer 5.

The trait exposes five async methods for autonomous/proactive finance behaviors:

- `daily_review()` -- generate a daily financial summary
- `check_budgets()` -- detect budget threshold breaches, returning `Vec<BudgetAlert>`
- `refresh_prices()` -- refresh all investment prices, returning `PriceUpdateSummary`
- `analyze_spending(period)` -- generate spending analysis for a period
- `run_health_check()` -- run all diagnostic checks
- `proactivity_level()` -- return the current `ProactivityLevel`

`ProactivityLevel` (`src/handler.rs`, lines 11-18) is an enum with three variants: `Full` (daily reviews, proactive warnings), `Moderate` (significant events only), and `Reactive` (explicit queries only). It parses from string via `FromStr`.

The handler is optional -- `FinanceTool` holds `Option<Arc<dyn FinanceHandler>>` and gracefully degrades when unset (e.g., `daily_review` action returns a "handler not configured" message).

### Config and Types

**Config** (`src/config.rs`, lines 1-286): `FinanceConfig` is self-contained (no dependency on the config crate) with `#[serde(rename_all = "camelCase")]`. It contains seven nested config sections:

- `FinanceInflationConfig` -- inflation rate assumption and source
- `FinanceExpectedReturnsConfig` -- annual return rates by asset class (stocks, crypto, real estate, bonds)
- `FinanceBudgetingConfig` -- default budget method, alert threshold, six-jar allocation ratios
- `FinancePriceRefreshConfig` -- price refresh interval and cache TTL
- `FinanceSchedulingConfig` -- daily review time, weekly report day, budget check time, timezone
- `FinanceCategoryConfig` -- auto-categorization enable/disable and confidence threshold

**Types** (`src/types.rs`, lines 1-698): 10 domain enums (all using the `DomainEnum` derive macro from `tools-core`) and 8 domain structs with bidirectional `From` impls for row-to-domain conversion. Each enum supports `from_str_loose()` for case-insensitive parsing with alias support (e.g., `"6jar"` maps to `BudgetMethod::SixJar`). Two domain filter structs (`FinanceTransactionFilter`, `FinanceInvestmentDomainFilter`) provide type-safe query parameters that convert to storage-layer filters via `to_storage_filter()`.

### Storage Integration (FinanceRepo)

`FinanceTool` holds six repository structs from the `storage` crate, each wrapping a `SqlitePool`:

- `FinanceAccountRepo`
- `FinanceTransactionRepo`
- `FinanceBudgetRepo`
- `FinanceInvestmentRepo`
- `FinanceGoalRepo`
- `FinanceLiabilityRepo`

All repos are constructed from `pool.inner().clone()` in the `from_storage_pool()` convenience constructor (`src/tool/mod.rs`, lines 90-104). Since `SqlitePool` is `Clone + Send + Sync`, repos can be shared across tasks without locking.

The migration SQL (`migrations/001_finance_tables.sql`) creates the following tables with foreign-key cascades:

`finance_accounts` -> `finance_transactions` (cascade on delete)
`finance_portfolios` -> `finance_investments` (cascade on delete) -> `finance_investment_transactions` (cascade on delete)
`finance_budgets`, `finance_goals`, `finance_liabilities` (standalone)

Balance adjustments on transactions are handled at the tool level: when a transaction is added, updated, or deleted, `FinanceTool` calls `accounts.adjust_balance()` to maintain consistency.

---

## Section 2: API Reference

### FinanceTool

**File**: `crates/feature-finance/src/tool/mod.rs` (lines 34-258)

| Field | Type | Description |
|-------|------|-------------|
| `accounts` | `FinanceAccountRepo` | Account CRUD and balance queries |
| `transactions` | `FinanceTransactionRepo` | Transaction CRUD, search, sum queries |
| `budgets` | `FinanceBudgetRepo` | Budget CRUD and usage queries |
| `investments` | `FinanceInvestmentRepo` | Portfolio, investment, and investment-tx CRUD |
| `goals` | `FinanceGoalRepo` | Goal CRUD |
| `liabilities` | `FinanceLiabilityRepo` | Liability CRUD |
| `price_service` | `PriceService` | Live market price fetcher with cache |
| `finance_handler` | `Option<Arc<dyn FinanceHandler>>` | Optional proactive handler (agent-layer impl) |
| `default_currency` | `String` | Fallback currency code (ISO 4217) |
| `config_persistence` | `Option<Arc<dyn ConfigPersistence>>` | Optional config read/write for settings actions |

**Constructors**:

- `new(accounts, transactions, budgets, investments, goals, liabilities, price_service, default_currency)` -- full constructor
- `from_storage_pool(pool, default_currency)` -- convenience constructor from `StoragePool`
- `with_finance_handler(self, handler)` -- builder to attach handler
- `with_config_persistence(self, cp)` -- builder to attach config persistence

**Tool trait impl** (`name: "finance"`, 39 actions):

#### Account Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `account_add` | `name`, `type` | `currency`, `balance`, `institution`, `notes` | Create account. Sends `EntityCard` via `ctx.entity_tx`. |
| `account_list` | -- | `is_archived`, `currency` | List accounts with total balance by currency. |
| `account_update` | `id` | `name`, `balance`, `institution`, `notes`, `is_archived` | Patch account fields. At least one field required. |
| `account_delete` | `id` | -- | Delete account and cascade-delete its transactions. Returns deleted tx count. |

Source: `src/tool/accounts.rs` (lines 1-237)

#### Transaction Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `tx_add` | `type`, `amount` | `account_id`, `currency`, `category`, `subcategory`, `counterparty`, `notes`, `tx_date`, `transfer_to_account_id` | Add transaction. Auto-selects first account if `account_id` omitted. For transfers, creates paired expense+income rows linked by `transfer_id`. Adjusts account balance. Checks budget impact for expenses with categories. |
| `tx_list` | -- | `account_id`, `type`, `category`, `date_from`, `date_to`, `limit` | List transactions with filters. Default limit 50. |
| `tx_update` | `id` | `amount`, `category`, `subcategory`, `counterparty`, `notes`, `tx_date` | Update transaction fields. Adjusts account balance if amount changes. |
| `tx_delete` | `id` | -- | Delete transaction and reverse balance impact. For transfers, deletes both paired rows and reverses both accounts. |
| `tx_search` | -- (at least one criterion) | `query`, `amount_min`, `amount_max`, `date_from`, `date_to` | Search transactions by text, amount range, or date range. |
| `tx_recurring_add` | `type`/`tx_type`, `amount`, `recurring_rule` | `account_id`, `category`, `counterparty`, `notes` | Create a recurring transaction template. `recurring_rule` must be a 5-field cron expression. |

Source: `src/tool/transactions.rs` (lines 1-781)

#### Budget Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `budget_create` | `name`, `amount`, `period` | `currency`, `category`, `method`, `jar_type`, `start_date`, `end_date`, `alert_threshold` | Create budget. `jar_type` required when `method=six_jar`. Default alert threshold 80%. |
| `budget_list` | -- | `period` | List all budgets with usage (spent, remaining, percentage). Filterable by period. |
| `budget_status` | -- | `id` | Detailed budget status with subcategory breakdown and recent transactions. Without `id`, shows all budgets summary. |
| `budget_update` | `id` | `name`, `amount`, `category`, `is_active` | Patch budget fields. |
| `budget_delete` | `id` | -- | Delete budget. |

Source: `src/tool/budgets.rs` (lines 1-315)

#### Investment Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `portfolio_create` | `name` | `description`, `currency` | Create portfolio. |
| `portfolio_list` | -- | -- | List portfolios with summary (total value, return, holding count). |
| `investment_add` | `portfolio_id`, `asset_type`, `quantity`, `cost_basis` | `symbol`, `name`, `currency`, `purchase_date`, `notes` | Add investment holding. `symbol` required for stock/ETF. Name defaults to symbol. |
| `investment_update` | `id` | `current_price`, `current_value`, `quantity`, `notes` | Update investment fields. |
| `investment_tx` | `id`/`investment_id`, `tx_type`, `total_amount` | `quantity`, `price_per_unit`, `fees`, `currency`, `date`, `notes` | Record investment transaction. Buy: increases quantity and cost basis. Sell: reduces quantity, computes average cost reduction. Split: multiplies quantity by ratio. Dividend/rental/interest: record only. Validates sell quantity does not exceed holdings. |
| `investment_summary` | -- | `portfolio_id` | Portfolio summary with allocation breakdown by asset type and per-holding return. |
| `price_fetch` | `symbol`, `asset_type` | -- | Fetch live price via `PriceService`. Updates all matching investments. Not available for `real_estate`. |
| `price_refresh` | -- | -- | Refresh prices for all symbol-bearing investments. Returns updated/failed counts with details. |

Source: `src/tool/investments.rs` (lines 1-625)

#### Goal / Liability / Net Worth Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `goal_create` | `name`, `goal_type`, `target_amount` | `currency`, `current_amount`, `deadline`, `monthly_contribution`, `expected_return_rate`, `inflation_rate`, `notes` | Create financial goal. Deadline must be in the future. |
| `goal_list` | -- | -- | List active goals with progress percentage. |
| `goal_update` | `id` | `current_amount`, `target_amount`, `monthly_contribution`, `expected_return_rate`, `deadline`, `status` | Update goal fields. At least one field required. |
| `goal_fire` | -- | `annual_expenses`, `withdrawal_rate`, `expected_return_rate`, `inflation_rate`, `monthly_contribution` | FIRE calculation. Computes FIRE number (`annual_expenses / withdrawal_rate`), current progress, and months remaining using compound growth formula. If `annual_expenses` not provided, sums last 12 months of expense transactions. |
| `goal_whatif` | -- | (same as `goal_fire` plus `extra_monthly_savings`, `extra_return_rate`) | Same as `goal_fire` but with an additional what-if scenario showing adjusted timeline. |
| `liability_add` | `name`, `type`, `principal` | `remaining`, `currency`, `interest_rate`, `monthly_payment`, `due_date`, `notes` | Add liability. `remaining` defaults to `principal`. |
| `liability_list` | -- | -- | List all liabilities with total remaining by currency. |
| `liability_update` | `id` | `remaining`, `monthly_payment`, `interest_rate`, `notes` | Update liability fields. |
| `net_worth` | -- | -- | Calculate net worth: accounts + investments - liabilities, broken down by currency. |

Source: `src/tool/goals.rs` (lines 1-616)

#### Report Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `report_spending` | -- | `period`, `date_from`, `date_to`, `category` | Spending breakdown by category for a period. Periods: `month`, `week`, `quarter`, `year`, `last_30_days`. Explicit dates override period. |
| `report_income` | -- | `period`, `date_from`, `date_to`, `category` | Income breakdown by category for a period. Same period options as spending. |
| `report_trends` | `metric` | `periods` | Time-series data over N months. Metrics: `spending`, `income`, `savings_rate`. Default 6 periods. Includes month-over-month change percentages. |
| `report_net_worth_history` | -- | -- | Current net-worth snapshot (accounts, investments, liabilities by currency). Note: historical snapshots not yet available. |
| `daily_review` | -- | -- | Delegates to `FinanceHandler::daily_review()`. Requires configured handler. |

Source: `src/tool/reports.rs` (lines 1-333)

#### Health Check Action

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `finance_health_check` | -- | -- | Runs 7 diagnostic checks. Returns issues with severity (error/warning/info). |

Health checks performed:

| Check | Severity | Condition |
|-------|----------|-----------|
| `no_accounts` | info | No finance accounts exist |
| `negative_balance` | warning | Non-crypto account has negative balance |
| `stale_prices` | warning | Investment prices older than 24 hours |
| `duplicate_budgets` | warning | Multiple active budgets for same category + period |
| `overdue_goals` | info | Active goals past their deadline |
| `negative_remaining` | error | Liabilities with negative remaining balance |
| `empty_portfolios` | info | Portfolios with no holdings |

Source: `src/tool/health.rs` (lines 1-208)

#### Settings Actions

| Action | Required Params | Optional Params | Description |
|--------|----------------|-----------------|-------------|
| `settings_get` | -- | -- | Return current finance config JSON. Falls back to defaults if no `ConfigPersistence` attached. |
| `settings_update` | -- (at least one setting) | `default_currency`, `proactivity_level`, `inflation_rate`, `alert_threshold`, `auto_categorize`, `confidence_threshold` | Update settings. Validates `proactivity_level` (full/moderate/reactive) and `confidence_threshold` (0.0-1.0). Persists via `ConfigPersistence` if attached. |

Source: `src/tool/settings.rs` (lines 1-145)

---

### FinanceFeature: FeaturePackage Implementation

**File**: `crates/feature-finance/src/lib.rs` (lines 40-128)

| Method | Return | Description |
|--------|--------|-------------|
| `name()` | `"finance"` | Feature identifier |
| `tools()` | `Vec<DynTool>` (1 element) | The wrapped `FinanceTool` |
| `migrations()` | `Vec<FeatureMigration>` (1 element) | Version 1 migration creating 8 tables |
| `config_key()` | `"finance"` | Config section key |
| `default_config()` | `Value` | Serialized `FinanceConfig::default()` |
| `health_check()` | `HealthStatus::Healthy` | Always healthy |

Static methods:

| Method | Description |
|--------|-------------|
| `migration_sql()` | Raw SQL string for the finance migration |
| `migrations_static()` | Migrations without `self` |
| `default_config_static()` | Default config without `self` |
| `for_tests()` | Construct instance backed by temp SQLite directory |

---

### Account Types and Fields

**Enum `AccountType`** (`src/types.rs`, lines 20-32):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Cash` | -- | -- |
| `Bank` | -- | -- |
| `Ewallet` | `e_wallet` | -- |
| `CryptoWallet` | `cryptowallet` | -- |
| `Brokerage` | -- | -- |
| `Other` | -- | yes |

**Struct `FinanceAccount`** (`src/types.rs`, lines 189-201):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `name` | `String` | Display name |
| `account_type` | `AccountType` | Type enum |
| `currency` | `String` | ISO 4217 code |
| `balance` | `i64` | Current balance (smallest unit) |
| `institution` | `Option<String>` | Bank/institution name |
| `notes` | `Option<String>` | Free-text notes |
| `is_archived` | `bool` | Archive flag |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

---

### Budget Types and Fields

**Enum `BudgetPeriod`** (`src/types.rs`, lines 49-60):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Monthly` | `month` | yes |
| `Weekly` | `week` | -- |
| `Yearly` | `year`, `annual` | -- |
| `Custom` | -- | -- |

**Enum `BudgetMethod`** (`src/types.rs`, lines 65-72):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Standard` | -- | yes |
| `SixJar` | `sixjar`, `6jar` | -- |

**Enum `JarType`** (`src/types.rs`, lines 77-91):

| Variant | Aliases |
|---------|---------|
| `Essentials` | `necessities` |
| `Savings` | `saving` |
| `Investment` | `investments` |
| `Education` | -- |
| `Entertainment` | `play` |
| `Charity` | `give` |

**Struct `FinanceBudget`** (`src/types.rs`, lines 224-240):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `name` | `String` | Budget name |
| `amount` | `i64` | Budget limit (smallest unit) |
| `currency` | `String` | ISO 4217 code |
| `period` | `BudgetPeriod` | Period enum |
| `category` | `Option<String>` | Expense category to track |
| `method` | `BudgetMethod` | Allocation method |
| `jar_type` | `Option<JarType>` | Six-Jar category (required when method=SixJar) |
| `start_date` | `NaiveDate` | Period start |
| `end_date` | `Option<NaiveDate>` | Period end |
| `is_active` | `bool` | Active flag |
| `alert_threshold` | `i32` | Percentage threshold for budget warnings (default 80) |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

---

### Investment Types and Fields

**Enum `AssetType`** (`src/types.rs`, lines 96-113):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Stock` | `stocks`, `equity` | -- |
| `Etf` | -- | -- |
| `Crypto` | `cryptocurrency` | -- |
| `RealEstate` | `realestate`, `property` | -- |
| `Bond` | `bonds`, `fixed_income` | -- |
| `Other` | -- | yes |
| `ExchangeRate` | `exchangerate`, `forex`, `fx` | -- |

**Enum `InvestmentTxType`** (`src/types.rs`, lines 118-131):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Buy` | `purchase` | yes |
| `Sell` | `sale` | -- |
| `Dividend` | -- | -- |
| `RentalIncome` | `rental`, `rent` | -- |
| `Interest` | -- | -- |
| `Split` | -- | -- |

**Struct `FinancePortfolio`** (`src/types.rs`, lines 243-251):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `name` | `String` | Portfolio name |
| `description` | `Option<String>` | Free-text description |
| `currency` | `String` | Base currency |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

**Struct `FinanceInvestment`** (`src/types.rs`, lines 254-270):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `portfolio_id` | `String` | Parent portfolio FK |
| `asset_type` | `AssetType` | Asset class |
| `symbol` | `Option<String>` | Ticker symbol (required for stock/ETF) |
| `name` | `String` | Display name |
| `quantity` | `f64` | Number of units held |
| `cost_basis` | `i64` | Total cost of acquisition (smallest unit) |
| `currency` | `String` | ISO 4217 code |
| `current_price` | `Option<i64>` | Latest price per unit (smallest unit) |
| `current_value` | `Option<i64>` | Current market value (smallest unit) |
| `purchase_date` | `Option<NaiveDate>` | Original purchase date |
| `notes` | `Option<String>` | Free-text notes |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

**Struct `FinanceInvestmentTx`** (`src/types.rs`, lines 273-286):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `investment_id` | `String` | Parent investment FK |
| `tx_type` | `InvestmentTxType` | Transaction type |
| `quantity` | `Option<f64>` | Units traded (or split ratio) |
| `price_per_unit` | `Option<i64>` | Price per unit at time of trade |
| `total_amount` | `i64` | Total transaction amount (smallest unit) |
| `currency` | `String` | ISO 4217 code |
| `fees` | `i64` | Trading fees (default 0) |
| `tx_date` | `NaiveDate` | Trade date |
| `notes` | `Option<String>` | Free-text notes |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

---

### Transaction Types and Fields

**Enum `TransactionType`** (`src/types.rs`, lines 37-44):

| Variant | Default |
|---------|---------|
| `Income` | -- |
| `Expense` | yes |
| `Transfer` | -- |

**Struct `FinanceTransaction`** (`src/types.rs`, lines 204-221):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `account_id` | `String` | Parent account FK |
| `tx_type` | `TransactionType` | Income, Expense, or Transfer |
| `amount` | `i64` | Transaction amount (always positive, smallest unit) |
| `currency` | `String` | ISO 4217 code |
| `category` | `Option<String>` | Expense/income category |
| `subcategory` | `Option<String>` | Sub-category for finer breakdown |
| `counterparty` | `Option<String>` | Payee or payer name |
| `notes` | `Option<String>` | Free-text notes |
| `tx_date` | `NaiveDate` | Transaction date |
| `transfer_id` | `Option<String>` | Links paired transfer transactions |
| `is_recurring` | `bool` | Recurring template flag |
| `recurring_rule` | `Option<String>` | 5-field cron expression |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

**Struct `FinanceTransactionFilter`** (`src/types.rs`, lines 653-664):

| Field | Type | Description |
|-------|------|-------------|
| `account_id` | `Option<String>` | Filter by account |
| `tx_type` | `Option<TransactionType>` | Filter by type |
| `category` | `Option<String>` | Filter by category |
| `date_from` | `Option<NaiveDate>` | Start date (inclusive) |
| `date_to` | `Option<NaiveDate>` | End date (inclusive) |
| `amount_min` | `Option<i64>` | Minimum amount |
| `amount_max` | `Option<i64>` | Maximum amount |
| `query` | `Option<String>` | Text search in notes/counterparty |
| `limit` | `Option<usize>` | Row limit |

---

### Financial Goal Types

**Enum `GoalType`** (`src/types.rs`, lines 136-149):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Savings` | `saving`, `emergency_fund` | yes |
| `Purchase` | `buy` | -- |
| `DebtPayoff` | `debt`, `payoff` | -- |
| `Fire` | `financial_independence` | -- |
| `Custom` | -- | -- |

**Enum `GoalStatus`** (`src/types.rs`, lines 154-164):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Active` | `in_progress` | yes |
| `Achieved` | `completed`, `done` | -- |
| `Abandoned` | `cancelled` | -- |

**Struct `FinanceGoal`** (`src/types.rs`, lines 289-305):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `name` | `String` | Goal name |
| `goal_type` | `GoalType` | Goal type enum |
| `target_amount` | `i64` | Target amount (smallest unit) |
| `current_amount` | `i64` | Current saved amount |
| `currency` | `String` | ISO 4217 code |
| `status` | `GoalStatus` | Status enum |
| `deadline` | `Option<NaiveDate>` | Target date |
| `monthly_contribution` | `Option<i64>` | Monthly savings toward goal |
| `expected_return_rate` | `Option<f64>` | Expected annual return (%) |
| `inflation_rate` | `Option<f64>` | Assumed inflation rate (%) |
| `notes` | `Option<String>` | Free-text notes |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

---

### Liability Types

**Enum `LiabilityType`** (`src/types.rs`, lines 169-182):

| Variant | Aliases | Default |
|---------|---------|---------|
| `Mortgage` | `home_loan` | -- |
| `CreditCard` | `creditcard`, `cc` | -- |
| `PersonalLoan` | `personal` | -- |
| `StudentLoan` | `student`, `education_loan` | -- |
| `Other` | -- | yes |

**Struct `FinanceLiability`** (`src/types.rs`, lines 308-322):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID |
| `name` | `String` | Liability name |
| `liability_type` | `LiabilityType` | Type enum |
| `principal` | `i64` | Original principal (smallest unit) |
| `remaining` | `i64` | Remaining balance |
| `currency` | `String` | ISO 4217 code |
| `interest_rate` | `Option<f64>` | Annual interest rate (%) |
| `monthly_payment` | `Option<i64>` | Monthly payment amount |
| `due_date` | `Option<NaiveDate>` | Final due date |
| `notes` | `Option<String>` | Free-text notes |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |

---

### Health Check Types

**File**: `crates/feature-finance/src/tool/health.rs`

**Enum `Severity`** (lines 12-17, private):

| Variant | String |
|---------|--------|
| `Error` | `"error"` |
| `Warning` | `"warning"` |
| `Info` | `"info"` |

**Struct `Issue`** (lines 29-34, private):

| Field | Type | Description |
|-------|------|-------------|
| `check` | `&'static str` | Check identifier |
| `severity` | `Severity` | Issue severity |
| `count` | `usize` | Number of affected items |
| `detail` | `String` | Human-readable description |

Output JSON includes `status` (`all_clear`, `info_only`, `warnings_found`, `errors_found`), `checks_run` (always 7), `issues` array, and `summary` string.

---

### Report Types

**File**: `crates/feature-finance/src/tool/reports.rs`

Period keywords supported by `derive_date_range()` (lines 18-71):

| Keyword | Aliases | Range |
|---------|---------|-------|
| `month` | `monthly` | First to last day of current month |
| `week` | `weekly` | Monday to Sunday of current week |
| `quarter` | `quarterly` | First to last day of current quarter |
| `year` | `yearly`, `this_year` | Jan 1 to Dec 31 of current year |
| `last_30_days` | -- | Today minus 29 days to today |
| `custom` | -- | Requires explicit `date_from` and `date_to` |

Trend metrics: `spending` (sum expenses by month), `income` (sum income by month), `savings_rate` (computed as `(income - expenses) * 100 / income` per month).

---

### PriceService

**File**: `crates/feature-finance/src/price_service.rs` (lines 30-341)

**Struct `PriceService`** (lines 35-39):

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client (10s timeout, custom user-agent) |
| `cache` | `Arc<DashMap<String, CachedPrice>>` | In-memory TTL cache |
| `cache_ttl` | `Duration` | Cache entry lifetime |

**Public methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(cache_ttl_minutes: u32) -> Self` | Construct with cache TTL |
| `fetch_stock` | `(&self, symbol: &str) -> Result<PriceResult, String>` | Fetch stock/ETF price from Yahoo Finance |
| `fetch_crypto` | `(&self, symbol: &str, vs_currency: &str) -> Result<PriceResult, String>` | Fetch crypto price from CoinGecko |
| `fetch_exchange_rate` | `(&self, from: &str, to: &str) -> Result<f64, String>` | Fetch forex rate from open.er-api.com |
| `fetch_price` | `(&self, symbol: &str, asset_type: AssetType) -> Result<PriceResult, String>` | Unified dispatcher: routes to stock/crypto/forex based on asset type |

**Struct `PriceResult`** (lines 22-28):

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | `String` | Normalized symbol |
| `price` | `f64` | Market price in display units |
| `currency` | `String` | Price currency |
| `source` | `String` | Data source: `yahoo_finance`, `coingecko`, `er_api`, `cache`, `cache_stale` |

**Struct `CachedPrice`** (lines 14-18):

| Field | Type | Description |
|-------|------|-------------|
| `price` | `f64` | Cached price |
| `currency` | `String` | Currency code |
| `fetched_at` | `Instant` | When this entry was fetched |

---

### Config Types

**File**: `crates/feature-finance/src/config.rs` (lines 1-286)

**`FinanceConfig`** (lines 11-31):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable/disable finance feature |
| `default_currency` | `String` | `"USD"` | Default ISO 4217 currency |
| `proactivity_level` | `String` | `"full"` | Agent proactivity: `full`, `moderate`, `reactive` |
| `inflation` | `FinanceInflationConfig` | (see below) | Inflation assumptions |
| `expected_returns` | `FinanceExpectedReturnsConfig` | (see below) | Return rate assumptions |
| `budgeting` | `FinanceBudgetingConfig` | (see below) | Budget method and alerting |
| `price_refresh` | `FinancePriceRefreshConfig` | (see below) | Price fetch scheduling |
| `scheduling` | `FinanceSchedulingConfig` | (see below) | Review/report schedule |
| `categories` | `FinanceCategoryConfig` | (see below) | Auto-categorization |

**`FinanceInflationConfig`** (lines 58-65):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `rate` | `f64` | `3.3` | Assumed annual inflation rate (%) |
| `source` | `String` | `"manual"` | Data source for rate |

**`FinanceExpectedReturnsConfig`** (lines 85-96):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stocks` | `f64` | `10.0` | Expected annual stock return (%) |
| `crypto` | `f64` | `15.0` | Expected annual crypto return (%) |
| `real_estate` | `f64` | `8.0` | Expected annual real estate return (%) |
| `bonds` | `f64` | `5.0` | Expected annual bond return (%) |

**`FinanceBudgetingConfig`** (lines 126-135):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `default_method` | `String` | `"standard"` | Default budget method |
| `alert_threshold` | `u8` | `80` | Percentage trigger for budget warnings |
| `six_jar_ratios` | `SixJarRatios` | (see below) | Allocation percentages for Six Jar method |

**`SixJarRatios`** (lines 156-171):

| Field | Type | Default |
|-------|------|---------|
| `essentials` | `u8` | `55` |
| `savings` | `u8` | `10` |
| `investment` | `u8` | `10` |
| `education` | `u8` | `10` |
| `entertainment` | `u8` | `10` |
| `charity` | `u8` | `5` |

**`FinancePriceRefreshConfig`** (lines 199-208):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable automatic price refresh |
| `interval_hours` | `u32` | `4` | Hours between refresh cycles |
| `cache_ttl_minutes` | `u32` | `15` | Price cache TTL in minutes |

**`FinanceSchedulingConfig`** (lines 229-240):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `daily_review_time` | `String` | `"21:00"` | Time for daily review |
| `weekly_report_day` | `String` | `"monday"` | Day for weekly report |
| `budget_check_time` | `String` | `"09:00"` | Time for budget checks |
| `timezone` | `Option<String>` | `None` | Optional timezone override |

**`FinanceCategoryConfig`** (lines 266-273):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto_categorize` | `bool` | `true` | Enable auto-categorization |
| `confidence_threshold` | `f64` | `0.8` | Minimum confidence for auto-applying a category |

---

### FinanceHandler Trait

**File**: `crates/feature-finance/src/handler.rs` (lines 72-80)

```rust
#[async_trait]
pub trait FinanceHandler: Send + Sync {
    async fn daily_review(&self) -> Result<String>;
    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>>;
    async fn refresh_prices(&self) -> Result<PriceUpdateSummary>;
    async fn analyze_spending(&self, period: &str) -> Result<String>;
    async fn run_health_check(&self) -> Result<String>;
    fn proactivity_level(&self) -> ProactivityLevel;
}
```

**`ProactivityLevel`** (lines 11-18):

| Variant | String | Behavior |
|---------|--------|----------|
| `Full` | `"full"` | Daily reviews, proactive budget warnings, price alerts |
| `Moderate` | `"moderate"` | Alerts for significant events only (budget >80%, large price moves) |
| `Reactive` | `"reactive"` | Responds to explicit queries only |

**`BudgetAlert`** (lines 53-61):

| Field | Type | Description |
|-------|------|-------------|
| `budget_name` | `String` | Budget name |
| `category` | `Option<String>` | Budget category |
| `spent` | `i64` | Amount spent |
| `limit` | `i64` | Budget limit |
| `percentage` | `f64` | Usage percentage |
| `currency` | `String` | Currency code |

**`PriceUpdateSummary`** (lines 64-69):

| Field | Type | Description |
|-------|------|-------------|
| `updated` | `usize` | Investments successfully updated |
| `failed` | `usize` | Investments that failed to update |
| `details` | `Vec<String>` | Per-investment detail messages |
