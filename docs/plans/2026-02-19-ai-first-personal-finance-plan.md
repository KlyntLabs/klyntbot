# AI-First Personal Finance Agent — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a personal finance system to klyntbot — a single `FinanceTool` with ~37 actions, 8 PostgreSQL tables, 6 repos, auto-fetch price APIs, and a `FinanceHandler` trait for autonomous agent behaviors.

**Architecture:** Single mega `FinanceTool` (like TodoTool) in the `tools` crate, with a `FinanceHandler` trait for dependency-inverted autonomous behaviors. Six new `*Repo` structs in `storage`, a `FinanceConfig` in `config`, and a `PriceService` for external API calls. Handler implementation in `agent` crate.

**Tech Stack:** Rust, sqlx (PostgreSQL), async-trait, serde_json, reqwest (price APIs), DashMap (price cache), chrono

**Design doc:** `docs/plans/2026-02-19-ai-first-personal-finance-design.md`

---

## Task 1: Database Migrations

Create all 8 tables in a single migration file.

**Files:**
- Create: `crates/storage/migrations/20260219100000_finance_tables.sql`

**Step 1: Write the migration**

```sql
-- Finance accounts (bank accounts, wallets, cash pools)
CREATE TABLE IF NOT EXISTS finance_accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,  -- cash, bank, ewallet, crypto_wallet, brokerage, other
    currency TEXT NOT NULL,       -- ISO 4217
    balance BIGINT NOT NULL DEFAULT 0,
    institution TEXT,
    notes TEXT,
    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Finance transactions (every money movement)
CREATE TABLE IF NOT EXISTS finance_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES finance_accounts(id) ON DELETE CASCADE,
    tx_type TEXT NOT NULL,        -- income, expense, transfer
    amount BIGINT NOT NULL,       -- always positive; tx_type determines direction
    currency TEXT NOT NULL,
    category TEXT,
    subcategory TEXT,
    counterparty TEXT,
    notes TEXT,
    tx_date DATE NOT NULL DEFAULT CURRENT_DATE,
    is_recurring BOOLEAN NOT NULL DEFAULT FALSE,
    recurring_rule TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_finance_tx_account_date ON finance_transactions(account_id, tx_date);
CREATE INDEX IF NOT EXISTS idx_finance_tx_category_date ON finance_transactions(category, tx_date);

-- Finance budgets
CREATE TABLE IF NOT EXISTS finance_budgets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT,               -- NULL = total budget
    amount BIGINT NOT NULL,
    currency TEXT NOT NULL,
    period TEXT NOT NULL,         -- monthly, weekly, yearly, custom
    method TEXT NOT NULL DEFAULT 'standard', -- standard, six_jar
    jar_type TEXT,                -- essentials, savings, investment, education, entertainment, charity
    start_date DATE NOT NULL DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

-- Finance portfolios (groups of investments)
CREATE TABLE IF NOT EXISTS finance_portfolios (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    currency TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Finance investments (individual holdings)
CREATE TABLE IF NOT EXISTS finance_investments (
    id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_type TEXT NOT NULL,     -- stock, etf, crypto, real_estate, bond, other
    symbol TEXT,
    name TEXT NOT NULL,
    currency TEXT NOT NULL,
    quantity DOUBLE PRECISION NOT NULL DEFAULT 0,
    cost_basis BIGINT NOT NULL DEFAULT 0,
    current_price BIGINT,
    current_value BIGINT,
    purchase_date DATE,
    notes TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_finance_inv_portfolio_symbol ON finance_investments(portfolio_id, symbol);

-- Finance investment transactions (buy/sell/dividend)
CREATE TABLE IF NOT EXISTS finance_investment_transactions (
    id TEXT PRIMARY KEY,
    investment_id TEXT NOT NULL REFERENCES finance_investments(id) ON DELETE CASCADE,
    tx_type TEXT NOT NULL,        -- buy, sell, dividend, rental_income, interest, split
    quantity DOUBLE PRECISION,
    price_per_unit BIGINT,
    total_amount BIGINT NOT NULL,
    currency TEXT NOT NULL,
    fees BIGINT NOT NULL DEFAULT 0,
    tx_date DATE NOT NULL DEFAULT CURRENT_DATE,
    notes TEXT
);

-- Finance goals (FIRE, savings targets, etc.)
CREATE TABLE IF NOT EXISTS finance_goals (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    goal_type TEXT NOT NULL,      -- savings, purchase, debt_payoff, fire, custom
    target_amount BIGINT NOT NULL,
    current_amount BIGINT NOT NULL DEFAULT 0,
    currency TEXT NOT NULL,
    deadline DATE,
    monthly_contribution BIGINT,
    expected_return_rate DOUBLE PRECISION,
    inflation_rate DOUBLE PRECISION,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'active', -- active, achieved, abandoned
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Finance liabilities (debts)
CREATE TABLE IF NOT EXISTS finance_liabilities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    liability_type TEXT NOT NULL, -- mortgage, credit_card, personal_loan, student_loan, other
    principal BIGINT NOT NULL,
    remaining BIGINT NOT NULL,
    currency TEXT NOT NULL,
    interest_rate DOUBLE PRECISION,
    monthly_payment BIGINT,
    due_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Step 2: Verify migration compiles**

Run: `cargo build -p storage`
Expected: PASS (sqlx::migrate! macro picks up new file)

**Step 3: Commit**

```bash
git add crates/storage/migrations/20260219100000_finance_tables.sql
git commit -m "feat(storage): add finance tables migration"
```

---

## Task 2: Row Structs

Create all row structs for the 8 finance tables, plus Patch/Filter types.

**Files:**
- Create: `crates/storage/src/rows/finance.rs`
- Modify: `crates/storage/src/rows/mod.rs` — add `pub mod finance;` and re-exports

**Step 1: Write row structs**

Create `crates/storage/src/rows/finance.rs` with these structs (all derive `Debug, Clone, FromRow`):

```rust
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;

// ── Row structs (map 1:1 to DB tables) ───────────────────────

#[derive(Debug, Clone, FromRow)]
pub struct FinanceAccountRow {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub balance: i64,
    pub institution: Option<String>,
    pub notes: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceTransactionRow {
    pub id: String,
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub notes: Option<String>,
    pub tx_date: NaiveDate,
    pub is_recurring: bool,
    pub recurring_rule: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceBudgetRow {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub amount: i64,
    pub currency: String,
    pub period: String,
    pub method: String,
    pub jar_type: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinancePortfolioRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceInvestmentRow {
    pub id: String,
    pub portfolio_id: String,
    pub asset_type: String,
    pub symbol: Option<String>,
    pub name: String,
    pub currency: String,
    pub quantity: f64,
    pub cost_basis: i64,
    pub current_price: Option<i64>,
    pub current_value: Option<i64>,
    pub purchase_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceInvestmentTxRow {
    pub id: String,
    pub investment_id: String,
    pub tx_type: String,
    pub quantity: Option<f64>,
    pub price_per_unit: Option<i64>,
    pub total_amount: i64,
    pub currency: String,
    pub fees: i64,
    pub tx_date: NaiveDate,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceGoalRow {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub current_amount: i64,
    pub currency: String,
    pub deadline: Option<NaiveDate>,
    pub monthly_contribution: Option<i64>,
    pub expected_return_rate: Option<f64>,
    pub inflation_rate: Option<f64>,
    pub notes: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FinanceLiabilityRow {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub remaining: i64,
    pub currency: String,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Patch structs (for partial updates) ──────────────────────

#[derive(Debug, Default, Clone)]
pub struct FinanceAccountPatch {
    pub id: String,
    pub name: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceTransactionPatch {
    pub id: String,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<NaiveDate>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceBudgetPatch {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentPatch {
    pub id: String,
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<f64>,
    pub notes: Option<Option<String>>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceGoalPatch {
    pub id: String,
    pub name: Option<String>,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub expected_return_rate: Option<Option<f64>>,
    pub inflation_rate: Option<Option<f64>>,
    pub deadline: Option<Option<NaiveDate>>,
    pub status: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceLiabilityPatch {
    pub id: String,
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
}

// ── Filter structs (for list queries) ────────────────────────

#[derive(Debug, Default, Clone)]
pub struct FinanceTransactionFilter {
    pub account_id: Option<String>,
    pub tx_type: Option<String>,
    pub category: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub amount_min: Option<i64>,
    pub amount_max: Option<i64>,
    pub query: Option<String>,         // keyword search in notes/counterparty
    pub limit: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentFilter {
    pub portfolio_id: Option<String>,
    pub asset_type: Option<String>,
    pub symbol: Option<String>,
}
```

**Step 2: Add module to rows/mod.rs**

In `crates/storage/src/rows/mod.rs`, add `pub mod finance;` and re-export all types.

**Step 3: Re-export from storage lib.rs**

In `crates/storage/src/lib.rs`, add re-exports for all new row/patch/filter types.

**Step 4: Verify it compiles**

Run: `cargo build -p storage`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/storage/src/rows/finance.rs crates/storage/src/rows/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add finance row, patch, and filter structs"
```

---

## Task 3: Finance Repositories

Create 6 repo structs following the TodoRepo pattern (PgPool wrapper, CRUD methods, QueryBuilder for filters).

**Files:**
- Create: `crates/storage/src/repos/finance_account_repo.rs`
- Create: `crates/storage/src/repos/finance_transaction_repo.rs`
- Create: `crates/storage/src/repos/finance_budget_repo.rs`
- Create: `crates/storage/src/repos/finance_investment_repo.rs`
- Create: `crates/storage/src/repos/finance_goal_repo.rs`
- Create: `crates/storage/src/repos/finance_liability_repo.rs`
- Modify: `crates/storage/src/repos/mod.rs` — add modules + fields to `Repos`
- Modify: `crates/storage/src/lib.rs` — re-export new repos

Each repo follows the exact same pattern as `TodoRepo`:

```rust
#[derive(Debug, Clone)]
pub struct FinanceAccountRepo {
    pool: PgPool,
}

impl FinanceAccountRepo {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    pub async fn add(&self, row: &FinanceAccountRow) -> Result<FinanceAccountRow, StorageError> {
        sqlx::query_as("INSERT INTO finance_accounts (...) VALUES (...) RETURNING *")
            .bind(...)
            .fetch_one(&self.pool).await.map_err(StorageError::from)
    }

    pub async fn get(&self, id: &str) -> Result<Option<FinanceAccountRow>, StorageError> {
        sqlx::query_as("SELECT * FROM finance_accounts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool).await.map_err(StorageError::from)
    }

    // update, delete, list patterns same as TodoRepo
}
```

### Per-repo specifics:

**FinanceAccountRepo:** CRUD + `list(include_archived: bool)` + `total_balance(currency: &str)`

**FinanceTransactionRepo:** CRUD + `list(filter: &FinanceTransactionFilter)` using `QueryBuilder` for dynamic WHERE + `sum_by_category(date_from, date_to)` + `search(query, amount_min, amount_max)`

**FinanceBudgetRepo:** CRUD + `list_active()` + `budget_usage(budget_id)` — joins finance_transactions to compute spent amount

**FinanceInvestmentRepo:** Covers 3 tables (portfolios, investments, investment_transactions). Methods: portfolio CRUD + investment CRUD + investment_tx CRUD + `portfolio_summary(portfolio_id)` + `total_investment_value()`

**FinanceGoalRepo:** CRUD + `list_active()` + `update_progress(id, current_amount)`

**FinanceLiabilityRepo:** CRUD + `list_all()` + `total_remaining()`

### Repos aggregate update:

Add to `Repos` struct in `crates/storage/src/repos/mod.rs`:

```rust
pub finance_accounts: FinanceAccountRepo,
pub finance_transactions: FinanceTransactionRepo,
pub finance_budgets: FinanceBudgetRepo,
pub finance_investments: FinanceInvestmentRepo,
pub finance_goals: FinanceGoalRepo,
pub finance_liabilities: FinanceLiabilityRepo,
```

And in `from_pool()`:

```rust
finance_accounts: FinanceAccountRepo::new(pg.clone()),
finance_transactions: FinanceTransactionRepo::new(pg.clone()),
finance_budgets: FinanceBudgetRepo::new(pg.clone()),
finance_investments: FinanceInvestmentRepo::new(pg.clone()),
finance_goals: FinanceGoalRepo::new(pg.clone()),
finance_liabilities: FinanceLiabilityRepo::new(pg.clone()),
```

**Step 1: Write failing tests** — one test per repo verifying basic CRUD (add → get → update → delete round-trip). Tests follow pattern in existing repo tests.

**Step 2: Implement repos** — follow TodoRepo SQL patterns exactly.

**Step 3: Run tests**

Run: `cargo nextest run -p storage`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/storage/src/repos/ crates/storage/src/lib.rs
git commit -m "feat(storage): add 6 finance repositories"
```

---

## Task 4: FinanceConfig

Add finance configuration to the config crate.

**Files:**
- Modify: `crates/config/src/schema/core.rs` — add `FinanceConfig` + sub-configs + field on `Config`

**Step 1: Write config structs**

Add after the existing `TodoConfig` section in `crates/config/src/schema/core.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_finance_currency")]
    pub default_currency: String,
    #[serde(default = "default_proactivity_level")]
    pub proactivity_level: String, // "full", "moderate", "reactive"
    #[serde(default)]
    pub inflation: FinanceInflationConfig,
    #[serde(default)]
    pub expected_returns: FinanceExpectedReturnsConfig,
    #[serde(default)]
    pub budgeting: FinanceBudgetingConfig,
    #[serde(default)]
    pub price_refresh: FinancePriceRefreshConfig,
    #[serde(default)]
    pub scheduling: FinanceSchedulingConfig,
    #[serde(default)]
    pub categories: FinanceCategoryConfig,
}

impl Default for FinanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_currency: default_finance_currency(),
            proactivity_level: default_proactivity_level(),
            inflation: Default::default(),
            expected_returns: Default::default(),
            budgeting: Default::default(),
            price_refresh: Default::default(),
            scheduling: Default::default(),
            categories: Default::default(),
        }
    }
}

fn default_finance_currency() -> String { "USD".into() }
fn default_proactivity_level() -> String { "full".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInflationConfig {
    #[serde(default = "default_inflation_rate")]
    pub rate: f64,
    #[serde(default = "default_inflation_source")]
    pub source: String,
}
impl Default for FinanceInflationConfig {
    fn default() -> Self {
        Self { rate: default_inflation_rate(), source: default_inflation_source() }
    }
}
fn default_inflation_rate() -> f64 { 3.3 }
fn default_inflation_source() -> String { "manual".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceExpectedReturnsConfig {
    #[serde(default = "default_stock_return")]
    pub stocks: f64,
    #[serde(default = "default_crypto_return")]
    pub crypto: f64,
    #[serde(default = "default_real_estate_return")]
    pub real_estate: f64,
    #[serde(default = "default_bond_return")]
    pub bonds: f64,
}
impl Default for FinanceExpectedReturnsConfig {
    fn default() -> Self {
        Self {
            stocks: default_stock_return(),
            crypto: default_crypto_return(),
            real_estate: default_real_estate_return(),
            bonds: default_bond_return(),
        }
    }
}
fn default_stock_return() -> f64 { 10.0 }
fn default_crypto_return() -> f64 { 15.0 }
fn default_real_estate_return() -> f64 { 8.0 }
fn default_bond_return() -> f64 { 5.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetingConfig {
    #[serde(default = "default_budget_method")]
    pub default_method: String,
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold: u8,
    #[serde(default)]
    pub six_jar_ratios: SixJarRatios,
}
impl Default for FinanceBudgetingConfig {
    fn default() -> Self {
        Self {
            default_method: default_budget_method(),
            alert_threshold: default_alert_threshold(),
            six_jar_ratios: Default::default(),
        }
    }
}
fn default_budget_method() -> String { "standard".into() }
fn default_alert_threshold() -> u8 { 80 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SixJarRatios {
    #[serde(default = "default_jar_essentials")]
    pub essentials: u8,
    #[serde(default = "default_jar_small")]
    pub savings: u8,
    #[serde(default = "default_jar_small")]
    pub investment: u8,
    #[serde(default = "default_jar_small")]
    pub education: u8,
    #[serde(default = "default_jar_small")]
    pub entertainment: u8,
    #[serde(default = "default_jar_charity")]
    pub charity: u8,
}
impl Default for SixJarRatios {
    fn default() -> Self {
        Self {
            essentials: 55, savings: 10, investment: 10,
            education: 10, entertainment: 10, charity: 5,
        }
    }
}
fn default_jar_essentials() -> u8 { 55 }
fn default_jar_small() -> u8 { 10 }
fn default_jar_charity() -> u8 { 5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePriceRefreshConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_interval_hours")]
    pub interval_hours: u32,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_minutes: u32,
}
impl Default for FinancePriceRefreshConfig {
    fn default() -> Self {
        Self { enabled: true, interval_hours: 4, cache_ttl_minutes: 15 }
    }
}
fn default_interval_hours() -> u32 { 4 }
fn default_cache_ttl() -> u32 { 15 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSchedulingConfig {
    #[serde(default = "default_daily_review_time")]
    pub daily_review_time: String,
    #[serde(default = "default_weekly_report_day")]
    pub weekly_report_day: String,
    #[serde(default = "default_budget_check_time")]
    pub budget_check_time: String,
    pub timezone: Option<String>,
}
impl Default for FinanceSchedulingConfig {
    fn default() -> Self {
        Self {
            daily_review_time: default_daily_review_time(),
            weekly_report_day: default_weekly_report_day(),
            budget_check_time: default_budget_check_time(),
            timezone: None,
        }
    }
}
fn default_daily_review_time() -> String { "21:00".into() }
fn default_weekly_report_day() -> String { "monday".into() }
fn default_budget_check_time() -> String { "09:00".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryConfig {
    #[serde(default = "default_true")]
    pub auto_categorize: bool,
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
}
impl Default for FinanceCategoryConfig {
    fn default() -> Self {
        Self { auto_categorize: true, confidence_threshold: 0.8 }
    }
}
fn default_confidence_threshold() -> f64 { 0.8 }
```

**Step 2: Add to root Config struct**

In the `Config` struct, add:
```rust
#[serde(default)]
pub finance: FinanceConfig,
```

**Step 3: Verify**

Run: `cargo build -p config && cargo nextest run -p config`
Expected: PASS (default deserialization test should work since all fields have defaults)

**Step 4: Commit**

```bash
git add crates/config/src/schema/core.rs
git commit -m "feat(config): add FinanceConfig schema"
```

---

## Task 5: PriceService

HTTP client for fetching stock/crypto prices and exchange rates.

**Files:**
- Create: `crates/tools/src/price_service.rs`
- Modify: `crates/tools/src/lib.rs` — add `pub mod price_service;`
- Modify: `crates/tools/Cargo.toml` — add `dashmap` dependency if not present

**Step 1: Write the PriceService**

```rust
use dashmap::DashMap;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct PriceService {
    client: Client,
    cache: Arc<DashMap<String, CachedPrice>>,
    cache_ttl: Duration,
}

#[derive(Debug, Clone)]
struct CachedPrice {
    price: f64,
    currency: String,
    fetched_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PriceResult {
    pub symbol: String,
    pub price: f64,
    pub currency: String,
    pub source: String,
}

impl PriceService {
    pub fn new(cache_ttl_minutes: u32) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(cache_ttl_minutes as u64 * 60),
        }
    }

    /// Fetch stock/ETF price via Yahoo Finance (unofficial API)
    pub async fn fetch_stock(&self, symbol: &str) -> Result<PriceResult, String> { ... }

    /// Fetch crypto price via CoinGecko free API
    pub async fn fetch_crypto(&self, symbol: &str, vs_currency: &str) -> Result<PriceResult, String> { ... }

    /// Fetch exchange rate
    pub async fn fetch_exchange_rate(&self, from: &str, to: &str) -> Result<f64, String> { ... }

    /// Auto-detect asset type and fetch
    pub async fn fetch_price(&self, symbol: &str, asset_type: &str) -> Result<PriceResult, String> {
        // Check cache first
        let cache_key = format!("{}:{}", asset_type, symbol);
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.fetched_at.elapsed() < self.cache_ttl {
                return Ok(PriceResult { ... });
            }
        }
        // Dispatch to provider
        let result = match asset_type {
            "stock" | "etf" => self.fetch_stock(symbol).await?,
            "crypto" => self.fetch_crypto(symbol, "usd").await?,
            _ => return Err(format!("No price provider for asset type: {}", asset_type)),
        };
        // Cache result
        self.cache.insert(cache_key, CachedPrice { ... });
        Ok(result)
    }
}
```

Implementation details for each provider:

- **Yahoo Finance**: GET `https://query1.finance.yahoo.com/v8/finance/chart/{symbol}` → parse `chart.result[0].meta.regularMarketPrice`
- **CoinGecko**: GET `https://api.coingecko.com/api/v3/simple/price?ids={id}&vs_currencies={currency}` → parse `{id}.{currency}`
- **Exchange rates**: GET `https://open.er-api.com/v6/latest/{from}` → parse `rates.{to}`

**Step 2: Write tests** — unit tests with mock HTTP responses (or integration tests that hit real APIs with `#[ignore]`)

**Step 3: Verify**

Run: `cargo build -p tools`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/tools/src/price_service.rs crates/tools/src/lib.rs crates/tools/Cargo.toml
git commit -m "feat(tools): add PriceService for stock/crypto/fx prices"
```

---

## Task 6: FinanceHandler Trait

Define the trait in the tools crate for dependency inversion (same pattern as CalendarHandler, EnrichmentHandler).

**Files:**
- Create: `crates/tools/src/finance_handler.rs`
- Modify: `crates/tools/src/lib.rs` — add module + re-export

**Step 1: Write the trait**

```rust
use async_trait::async_trait;
use common::Result;
use serde_json::Value;

/// Proactivity level for autonomous finance behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProactivityLevel {
    Full,
    Moderate,
    Reactive,
}

impl ProactivityLevel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "moderate" => Self::Moderate,
            "reactive" => Self::Reactive,
            _ => Self::Full,
        }
    }
}

/// Budget alert when spending exceeds threshold.
#[derive(Debug, Clone)]
pub struct BudgetAlert {
    pub budget_name: String,
    pub category: Option<String>,
    pub spent: i64,
    pub limit: i64,
    pub percentage: f64,
    pub currency: String,
}

/// Summary of a price refresh cycle.
#[derive(Debug, Clone)]
pub struct PriceUpdateSummary {
    pub updated: usize,
    pub failed: usize,
    pub details: Vec<String>,
}

/// Autonomous finance agent behaviors.
/// Defined in tools (Layer 3), implemented in agent (Layer 5).
#[async_trait]
pub trait FinanceHandler: Send + Sync {
    /// Daily financial review — categorize uncategorized txns, detect anomalies.
    async fn daily_review(&self) -> Result<String>;

    /// Check all budgets, return alerts for any near/over limit.
    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>>;

    /// Fetch latest prices for all tracked investments.
    async fn refresh_prices(&self) -> Result<PriceUpdateSummary>;

    /// Analyze spending patterns for a time period.
    async fn analyze_spending(&self, period: &str) -> Result<String>;

    /// Get configured proactivity level.
    fn proactivity_level(&self) -> ProactivityLevel;
}
```

**Step 2: Add to tools lib.rs**

Add `pub mod finance_handler;` and re-export `FinanceHandler`, `ProactivityLevel`, `BudgetAlert`, `PriceUpdateSummary`.

**Step 3: Verify**

Run: `cargo build -p tools`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/tools/src/finance_handler.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add FinanceHandler trait for autonomous finance behaviors"
```

---

## Task 7: FinanceTool — Core Structure + Account Actions

Build the tool incrementally. Start with the struct, Tool trait impl, and the first action group (4 account actions).

**Files:**
- Create: `crates/tools/src/finance_tool.rs`
- Modify: `crates/tools/src/lib.rs` — add module + re-export

**Step 1: Write the tool struct and first 4 actions**

Follow TodoTool pattern exactly:

```rust
use async_trait::async_trait;
use common::Result;
use serde_json::{json, Value};
use storage::{
    FinanceAccountRepo, FinanceTransactionRepo, FinanceBudgetRepo,
    FinanceInvestmentRepo, FinanceGoalRepo, FinanceLiabilityRepo,
};
use crate::{ParamExtractor, RoutingContext, Tool};
use crate::finance_handler::FinanceHandler;
use crate::price_service::PriceService;
use std::sync::Arc;

pub struct FinanceTool {
    accounts: FinanceAccountRepo,
    transactions: FinanceTransactionRepo,
    budgets: FinanceBudgetRepo,
    investments: FinanceInvestmentRepo,
    goals: FinanceGoalRepo,
    liabilities: FinanceLiabilityRepo,
    price_service: PriceService,
    finance_handler: Option<Arc<dyn FinanceHandler>>,
    default_currency: String,
    timezone: String,
}
```

Builder pattern with `new()` taking all repos + config, and `with_finance_handler()` for the optional handler injection.

Implement `Tool` trait:
- `name()` → `"finance"`
- `description()` → one-line listing all action groups
- `parameters()` → JSON Schema with `action` enum + all params
- `execute()` → match on action, dispatch to handler methods

Start with account actions (`account_add`, `account_list`, `account_update`, `account_delete`), then add remaining action groups in Tasks 8-12.

**Step 2: Write tests** — test account CRUD via tool execute with mock args

**Step 3: Verify**

Run: `cargo build -p tools`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/tools/src/finance_tool.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add FinanceTool with account management actions"
```

---

## Task 8: FinanceTool — Transaction Actions

Add the 6 transaction actions to FinanceTool.

**Files:**
- Modify: `crates/tools/src/finance_tool.rs`

**Actions:** `tx_add`, `tx_list`, `tx_update`, `tx_delete`, `tx_search`, `tx_recurring_add`

Key behavior for `tx_add`: after recording, update the account balance and return the new balance + budget impact (if category matches a budget).

**Commit:** `feat(tools): add transaction actions to FinanceTool`

---

## Task 9: FinanceTool — Budget Actions

Add the 5 budget actions.

**Files:**
- Modify: `crates/tools/src/finance_tool.rs`

**Actions:** `budget_create`, `budget_list`, `budget_status`, `budget_update`, `budget_delete`

Key behavior for `budget_status`: join against finance_transactions to compute spent amount, return usage percentage.

**Commit:** `feat(tools): add budget actions to FinanceTool`

---

## Task 10: FinanceTool — Investment Actions

Add the 7 investment actions.

**Files:**
- Modify: `crates/tools/src/finance_tool.rs`

**Actions:** `portfolio_create`, `portfolio_list`, `investment_add`, `investment_update`, `investment_tx`, `investment_summary`, `price_fetch`

Key behavior for `price_fetch`: call `self.price_service.fetch_price()`, then update the investment row's `current_price` and `current_value`.

**Commit:** `feat(tools): add investment actions to FinanceTool`

---

## Task 11: FinanceTool — Net Worth, Liability, Goal, FIRE Actions

Add the remaining 13 actions (liabilities 3 + net_worth 1 + goals 4 + reports 4 + settings 2 = 14).

**Files:**
- Modify: `crates/tools/src/finance_tool.rs`

**Actions:**
- `liability_add`, `liability_list`, `liability_update`
- `net_worth` — aggregates accounts + investments - liabilities, with optional currency conversion
- `goal_create`, `goal_list`, `goal_update`, `goal_fire`, `goal_whatif`
- `report_spending`, `report_income`, `report_trends`, `report_net_worth_history`
- `settings_get`, `settings_update`

**FIRE formula** (in `goal_fire`):
```rust
let fire_number = annual_expenses * 25; // 4% rule
let monthly_savings = monthly_income - monthly_expenses;
let r = (expected_return - inflation) / 100.0 / 12.0; // monthly real return
// Solve: fire_number = monthly_savings * ((1+r)^n - 1) / r for n
let n = ((fire_number as f64 / monthly_savings as f64 * r + 1.0).ln() / (1.0 + r).ln()).ceil();
let months = n as u32;
```

**Commit:** `feat(tools): add net worth, goals, FIRE, reports, and settings actions`

---

## Task 12: FinanceHandler Implementation in Agent Crate

Implement the `FinanceHandler` trait in the agent crate for autonomous behaviors.

**Files:**
- Create: `crates/agent/src/finance_adapter.rs`
- Modify: `crates/agent/src/lib.rs` — add module + re-export

**Implementation:**

```rust
pub struct FinanceHandlerImpl {
    repos: storage::Repos,
    price_service: PriceService,
    provider: DynProvider,
    config: FinanceConfig,
    outbound_tx: mpsc::Sender<OutboundMessage>,
}

#[async_trait]
impl FinanceHandler for FinanceHandlerImpl {
    async fn daily_review(&self) -> Result<String> {
        // 1. Find uncategorized transactions (category IS NULL) from today
        // 2. Build LLM prompt with transaction details + user's category history
        // 3. Call provider.chat() to categorize
        // 4. Apply categories with confidence > threshold
        // 5. Build daily summary text
        // 6. Send via outbound_tx
    }

    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>> {
        // For each active budget, query spent amount, compare to threshold
    }

    async fn refresh_prices(&self) -> Result<PriceUpdateSummary> {
        // List all investments with symbols, call price_service, update rows
    }

    async fn analyze_spending(&self, period: &str) -> Result<String> {
        // Query spending by category for period, build LLM prompt for analysis
    }

    fn proactivity_level(&self) -> ProactivityLevel {
        ProactivityLevel::from_str(&self.config.proactivity_level)
    }
}
```

**Commit:** `feat(agent): implement FinanceHandler for autonomous finance behaviors`

---

## Task 13: Wire Everything in AgentLoop

Register the FinanceTool and FinanceHandler in the agent loop.

**Files:**
- Modify: `crates/agent/src/agent_loop.rs` — add FinanceTool registration after CalendarTool

**Pattern (following CalendarTool registration):**

```rust
// After line ~252 (CalendarTool registration)
if config.finance.enabled {
    let price_service = PriceService::new(config.finance.price_refresh.cache_ttl_minutes);

    let finance_handler_impl = Arc::new(FinanceHandlerImpl::new(
        repos.clone(),
        price_service.clone(),
        provider.clone(),
        config.finance.clone(),
        outbound_tx.clone(),
    ));

    let finance_tool = FinanceTool::new(
        repos.finance_accounts.clone(),
        repos.finance_transactions.clone(),
        repos.finance_budgets.clone(),
        repos.finance_investments.clone(),
        repos.finance_goals.clone(),
        repos.finance_liabilities.clone(),
        price_service,
        config.finance.default_currency.clone(),
        config.timezone.clone(),
    )
    .with_finance_handler(Arc::clone(&finance_handler_impl) as Arc<dyn FinanceHandler>);

    tool_registry.register(finance_tool);
}
```

**Commit:** `feat(agent): register FinanceTool in agent loop`

---

## Task 14: Re-exports and Final Integration

Update the facade crate and verify everything compiles end-to-end.

**Files:**
- Modify: `src/lib.rs` — add re-exports for finance types if needed
- Modify: `crates/storage/src/lib.rs` — ensure all finance types are re-exported

**Step 1: Update re-exports**

**Step 2: Full build + test**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features && cargo nextest run --workspace`
Expected: PASS with 0 clippy warnings

**Step 3: Commit**

```bash
git commit -m "feat: wire finance module re-exports and verify full build"
```

---

## Task Dependency Graph

```
Task 1 (migrations)
    ↓
Task 2 (row structs)
    ↓
Task 3 (repos) ←──────────────────────────┐
    ↓                                      │
Task 4 (config) ── independent             │
    ↓                                      │
Task 5 (PriceService) ── independent       │
    ↓                                      │
Task 6 (FinanceHandler trait) ── independent│
    ↓                                      │
Task 7 (FinanceTool core + accounts) ──────┘
    ↓
Tasks 8-11 (remaining tool actions) ── sequential
    ↓
Task 12 (FinanceHandler impl in agent)
    ↓
Task 13 (wire in AgentLoop)
    ↓
Task 14 (re-exports + final verification)
```

Tasks 4, 5, 6 are independent and can be done in parallel.
Tasks 8-11 are sequential (each adds actions to the same file).
