# Finance Analytical Engine — Design Spec

**Date:** 2026-03-14
**Sub-project:** 1 of 4 (Analytical Engine → Multi-step Workflows → Investment Intelligence → Proactive Coaching)
**Status:** Approved (all 8 sections reviewed)

## Context

The current `feature-finance` crate provides 41 tool actions across 7 SQLite tables — a solid personal finance ledger (accounts, transactions, budgets, investments, goals/FIRE, reports). This upgrade transforms it from a *recording system* into an *analytical engine* capable of FIRE projections, spending intelligence, portfolio analysis, and Monte Carlo simulations.

Inspired by Anthropic's `financial-services-plugins` architecture (skill-driven workflows, analytical depth, AI-agent-optimized interfaces), adapted for personal finance rather than institutional finance.

**Breaking changes are acceptable** — pre-release, no user data to migrate.

## Approach: New `analytics` Crate at Layer 3.5

A standalone computation crate with no awareness of tools, MCP, skills, or the agent runtime. Pure `fn(input) -> output` functions. `feature-finance` depends on it and exposes capabilities through new tool actions.

```
L0: common           — Money, Currency, Decimal re-export, Result<T> (owns rust_decimal dep)
L2: storage           — FinanceStorage, Row types
L3: analytics (NEW)   — pure computation, depends on common for Money/Decimal, rand with seed
L3: feature-finance   — depends on analytics, exposes via Tool actions
```

**Workspace dependency changes:** `rust_decimal` and `rand_chacha` must be added to `[workspace.dependencies]` in the root `Cargo.toml`. `common` gains `rust_decimal` as a dependency (for `Money` and `Currency` types). `analytics` depends on `common` (not the reverse).

---

## Section 1: Money Type & Precision Foundation

### Problem

- Amounts stored as `i64` (minor units) — fine for storage, but Rust code does `(price * 100.0).round() as i64` which introduces float errors
- Investment quantities are `f64` (`REAL` in SQLite) — float throughout the stack
- No currency-aware precision (JPY=0 decimals, KWD=3 decimals treated same as USD=2)

### Design

**`Money` type** (lives in `common` crate — it's a domain primitive like `Currency`, not an analytics concept):

```rust
use rust_decimal::Decimal;

/// Monetary value with currency-aware precision.
/// Fields are private — all construction goes through typed constructors,
/// and arithmetic is via Add/Sub trait impls that enforce same-currency.
pub struct Money {
    amount: Decimal,       // private — access via money.amount()
    currency: Currency,    // private — access via money.currency()
}

impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Self;
    pub fn from_minor_units(minor: i64, currency: Currency) -> Self;
    pub fn amount(&self) -> Decimal;
    pub fn currency(&self) -> &Currency;
    pub fn to_minor_units(&self) -> i64;
}

/// Add/Sub enforce same currency at runtime, return Result<Money>
impl Add for Money { /* returns Err if currencies differ */ }
impl Sub for Money { /* returns Err if currencies differ */ }

pub enum Currency {
    USD, EUR, THB, JPY, GBP, KWD, /* ... */
    Custom { code: String, decimal_places: u8 },
}

impl Currency {
    pub fn decimal_places(&self) -> u8; // USD=2, JPY=0, KWD=3
}
```

**Storage strategy:**

| Field type | SQLite | Rust | Conversion |
|---|---|---|---|
| Monetary amounts | `INTEGER` (minor units, same as now) | `Decimal` via `Money::from_minor_units(i64, Currency)` | Lossless |
| Investment quantities | `TEXT` (was `REAL`) | `Decimal` | Parse string, no float |
| Prices from APIs | fetched as `f64` → `Decimal::from_f64_retain()` | `Decimal` | One controlled conversion point |

**Rules:**
- `Money` arithmetic only works between same-currency values
- No `impl From<f64> for Money` — float conversion is explicit
- `Money::display()` respects `currency.decimal_places()`

**Schema change:** `finance_investments.quantity` from `REAL` to `TEXT`.

---

## Section 2: Analytics Crate Architecture

### Crate Structure

```
crates/analytics/
├── Cargo.toml          — deps: rust_decimal, rand, rand_chacha, common
├── src/
│   ├── lib.rs          — public API re-exports
│   ├── types.rs        — shared types (TimeSeries, PercentileBands, CorrelationMatrix, Anomaly)
│   ├── monte_carlo/
│   │   ├── mod.rs      — MonteCarloEngine, SimulationConfig, SimulationResult
│   │   ├── distributions.rs — LogNormal, HistoricalBootstrap
│   │   └── sampling.rs     — seeded RNG, draw sequences
│   ├── fire/
│   │   ├── mod.rs      — FIRECalculator, FIREResult
│   │   ├── variants.rs — Traditional, Coast, Lean, Fat FIRE
│   │   ├── withdrawal.rs — SWR strategies (Fixed, Guyton-Klinger, VPW)
│   │   └── sequence_risk.rs — historical backtesting, stress testing
│   ├── spending/
│   │   ├── mod.rs      — SpendingAnalyzer
│   │   ├── anomaly.rs  — z-score / modified z-score detection
│   │   ├── trends.rs   — moving averages, period-over-period growth
│   │   ├── recurring.rs — recurring charge detection
│   │   └── correlation.rs — category correlation matrix
│   └── portfolio/
│       ├── mod.rs      — PortfolioAnalyzer
│       ├── drift.rs    — allocation drift detection + rebalancing suggestions
│       ├── returns.rs  — TWR, MWR calculations
│       └── correlation.rs — asset correlation matrix
```

### Analytics Input Types

The analytics crate defines its own input types — lightweight structs that `feature-finance` converts storage `Row` types into. This keeps analytics decoupled from storage layout:

```rust
/// A financial transaction for spending analysis
pub struct SpendingRecord {
    pub date: NaiveDate,
    pub amount: Decimal,       // always positive (direction inferred from tx_type)
    pub tx_type: SpendingType, // Income or Expense
    pub category: Option<String>,
    pub counterparty: Option<String>,
}

pub enum SpendingType { Income, Expense }

/// A portfolio holding for drift/returns analysis
pub struct Holding {
    pub name: String,
    pub symbol: Option<String>,
    pub asset_class: String,
    pub current_value: Decimal,
    pub cost_basis: Decimal,
    pub quantity: Decimal,
}

/// An investment transaction for returns calculation (TWR/MWR)
pub struct InvestmentCashFlow {
    pub date: NaiveDate,
    pub amount: Decimal,        // positive = inflow, negative = outflow
    pub holding_symbol: Option<String>,
}

/// A price time series for correlation analysis
pub struct PriceSeries {
    pub symbol: String,
    pub asset_class: String,
    pub prices: Vec<(NaiveDate, Decimal)>, // monthly closing prices
}
```

`feature-finance` implements `From<TransactionRow> for SpendingRecord`, `From<InvestmentRow> for Holding`, etc. The conversion layer is thin and explicit.

### API Surface

Each module exposes focused structs with clear methods (not a generic `Analyzer` trait):

```rust
pub struct MonteCarloEngine;
impl MonteCarloEngine {
    pub fn run(config: &SimulationConfig) -> SimulationResult;
    pub fn run_with_seed(config: &SimulationConfig, seed: u64) -> SimulationResult;
}

pub struct FIRECalculator;
impl FIRECalculator {
    pub fn traditional(params: &FIREParams) -> FIREResult;
    pub fn coast(params: &CoastFIREParams) -> CoastFIREResult;
    pub fn lean(params: &LeanFIREParams) -> FIREResult;
    pub fn fat(params: &FatFIREParams) -> FIREResult;
    pub fn withdrawal_simulation(params: &WithdrawalParams) -> WithdrawalResult;
}

pub struct SpendingAnalyzer;
impl SpendingAnalyzer {
    pub fn detect_anomalies(records: &[SpendingRecord], config: &AnomalyConfig) -> Vec<Anomaly>;
    pub fn trends(records: &[SpendingRecord], config: &TrendConfig) -> TrendReport;
    pub fn detect_recurring(records: &[SpendingRecord]) -> Vec<RecurringCharge>;
    pub fn category_correlation(records: &[SpendingRecord], min_months: u32) -> CorrelationMatrix;
}

pub struct PortfolioAnalyzer;
impl PortfolioAnalyzer {
    pub fn allocation_drift(holdings: &[Holding], targets: &[AllocationTarget]) -> DriftReport;
    pub fn rebalance_suggestions(drift: &DriftReport, config: &RebalanceConfig) -> Vec<Trade>;
    pub fn returns(cash_flows: &[InvestmentCashFlow], current_values: &[Holding]) -> ReturnsReport;
    pub fn asset_correlation(price_history: &[PriceSeries], min_overlap: u32) -> CorrelationMatrix;
}
```

### Design Principles

- **No storage dependency** — analytics takes pre-fetched data via its own input types (see above)
- **No async** — all computation is synchronous (CPU-bound, not I/O-bound)
- **Decimal everywhere** — all inputs/outputs use `rust_decimal::Decimal`, no `f64` in public APIs
- **Deterministic by default** — every function that uses randomness accepts a seed

### Shared Output Types

```rust
pub struct PercentileBands {
    pub p5: Vec<Decimal>,
    pub p25: Vec<Decimal>,
    pub p50: Vec<Decimal>,
    pub p75: Vec<Decimal>,
    pub p95: Vec<Decimal>,
    pub survival_rate: Vec<Decimal>,  // % of runs still alive at each time point
    pub labels: Vec<String>,
}

pub struct TimeSeries {
    pub points: Vec<(NaiveDate, Decimal)>,
    pub label: String,
}

pub struct CorrelationMatrix {
    pub labels: Vec<String>,
    pub coefficients: Vec<Vec<Decimal>>,
}

pub struct Anomaly {
    pub date: NaiveDate,
    pub category: String,
    pub amount: Money,
    pub z_score: Decimal,
    pub severity: AnomalySeverity,
    pub explanation: String,
}
```

---

## Section 3: Monte Carlo Engine

### Configuration

```rust
pub struct SimulationConfig {
    pub runs: u32,                    // default: 10,000
    pub years: u32,                   // projection horizon
    pub initial_portfolio: Decimal,
    pub annual_contribution: Decimal,
    pub annual_withdrawal: Decimal,
    pub withdrawal_strategy: WithdrawalStrategy,
    pub return_model: ReturnModel,
    pub inflation: InflationModel,
    pub seed: Option<u64>,
}

pub enum ReturnModel {
    LogNormal { mean_return: Decimal, std_dev: Decimal },
    HistoricalBootstrap { returns: Vec<Decimal> },
    AssetAllocation { assets: Vec<AssetClass> },
}

pub struct AssetClass {
    pub name: String,
    pub weight: Decimal,
    pub mean_return: Decimal,
    pub std_dev: Decimal,
    pub correlation_row: Vec<Decimal>,
}

pub enum InflationModel {
    Fixed(Decimal),
    Variable { mean: Decimal, std_dev: Decimal },
}

pub enum WithdrawalStrategy {
    FixedRate(Decimal),
    FixedDollar(Decimal),
    GuytonKlinger {
        initial_rate: Decimal,
        ceiling_rate: Decimal,
        floor_rate: Decimal,
        capital_preservation_threshold: Decimal,
    },
    VPW { age: u32 },
}
```

### Result

```rust
pub struct SimulationResult {
    pub config_summary: ConfigSummary,
    pub success_rate: Decimal,            // derived: 1.0 - (ruin_count / runs) — not set independently
    pub percentile_bands: PercentileBands,// authoritative per-year percentile data
    pub terminal_values: TerminalStats,
    pub worst_sequence: WorstSequence,
    // NOTE: No separate `annual_summary` — `PercentileBands` is the authoritative per-year view.
    // `survival_rate` per year is tracked in `percentile_bands` via an additional field.
}

pub struct TerminalStats {
    pub median: Decimal,
    pub mean: Decimal,
    pub p5: Decimal,
    pub p95: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    pub ruin_count: u32,              // authoritative — success_rate is derived from this
}

pub struct WorstSequence {
    pub seed_index: u32,
    pub portfolio_by_year: Vec<Decimal>,
    pub ruin_year: Option<u32>,
}
```

### Algorithm

```
for each run (0..config.runs):
    rng = ChaCha8Rng::seed_from_u64(base_seed + run_index)
    portfolio = initial_portfolio

    for year in 0..config.years:
        1. Generate annual return (log-normal draw, bootstrap sample, or correlated asset draw via Cholesky)
        2. Generate inflation (fixed or stochastic)
        3. Apply return: portfolio *= (1 + annual_return)
        4. Apply contribution or withdrawal (strategy-dependent, inflation-adjusted)
        5. Floor portfolio at zero (ruin = portfolio cannot go negative)
        6. If portfolio == 0 and withdrawal > 0: mark ruin year, break
        7. Record year-end value

    record_terminal(run_index, portfolio)

Aggregate → percentile bands, success rate, terminal stats
```

**Step ordering rationale:** Return is applied *before* withdrawal (step 3 before step 4) because the standard financial convention is beginning-of-year growth, end-of-year withdrawal. This matches cFIREsim and Trinity Study methodology. The portfolio value is floored at zero (step 5) rather than capping returns — a -50% return on a $100 portfolio produces $50, not a capped return. Ruin is detected in step 6 only when the portfolio hits zero AND withdrawals are being taken.

**Implementation details:**
- **Cholesky decomposition** for correlated asset returns (precomputed once per simulation). If the user-provided correlation matrix is not positive definite, return `Err(CorrelationMatrixNotPositiveDefinite)` — do not silently fix or approximate. No external linear algebra crate needed; Cholesky for NxN where N < 10 is trivially implementable.
- **`rand_chacha::ChaCha8Rng`** for reproducible cross-platform RNG
- **Base seed + run index** for individual run reproducibility
- **Single-threaded in v1** — 10,000 runs × 50 years = 500K iterations is fast enough. Sensitivity sweeps use reduced run counts (see Section 7).
- **No return capping** — portfolio value is floored at zero instead (step 5). This preserves distribution integrity while preventing negative balances.

---

## Section 4: FIRE Calculator Suite

### Variants

**Traditional FIRE:** `fire_number = annual_expenses / withdrawal_rate`.

Months-to-FIRE formula (explicit):
```
real_return = (1 + nominal_return) / (1 + inflation) - 1
monthly_rate = (1 + real_return)^(1/12) - 1

if current_portfolio >= fire_number:
    months_to_fire = 0  // already at FIRE
elif monthly_savings <= 0:
    months_to_fire = None  // no savings, can't grow (portfolio-only growth handled below)
elif monthly_rate <= 0:
    months_to_fire = ceil((fire_number - current_portfolio) / monthly_savings)  // no compounding
else:
    // Standard future value of annuity formula, solved for n:
    // FV = PV*(1+r)^n + PMT*((1+r)^n - 1)/r
    // Solving for n: n = ln((fire_number * r + PMT) / (PV * r + PMT)) / ln(1 + r)
    denominator = current_portfolio * monthly_rate + monthly_savings
    numerator = fire_number * monthly_rate + monthly_savings
    if denominator <= 0 or numerator / denominator <= 0:
        months_to_fire = None  // unreachable (savings too small relative to portfolio decay)
    else:
        months_to_fire = ceil(ln(numerator / denominator) / ln(1 + monthly_rate))
```

**Coast FIRE:** Uses discrete annual compounding (consistent with Monte Carlo):
```
real_return = (1 + nominal_return) / (1 + inflation) - 1
years_to_retirement = target_age - current_age
fire_number = annual_expenses_at_retirement / withdrawal_rate
coast_number = fire_number / (1 + real_return)^years_to_retirement
```
If `real_return <= 0`, Coast FIRE is unreachable (compounding doesn't help) — `years_to_coast` returns `None`. If `current_portfolio >= coast_number`, you can stop saving (`years_to_coast = Some(0)`).

**Lean FIRE / Fat FIRE:** Same math as Traditional, different expense inputs (essentials-only vs. full lifestyle).

### Params & Results

```rust
pub struct FIREParams {
    pub annual_expenses: Decimal,
    pub current_portfolio: Decimal,
    pub monthly_savings: Decimal,
    pub expected_return: Decimal,
    pub inflation_rate: Decimal,
    pub withdrawal_rates: Vec<Decimal>, // compare multiple SWRs
}

pub struct FIREResult {
    pub fire_numbers: Vec<FIRETarget>,
    pub current_progress: Decimal,
    pub months_to_fire: Option<u32>,
    pub real_return_used: Decimal,
    pub monte_carlo: Option<SimulationResult>,
}

pub struct CoastFIREParams {
    pub current_portfolio: Decimal,
    pub target_age: u32,
    pub current_age: u32,
    pub annual_expenses_at_retirement: Decimal,
    pub expected_return: Decimal,
    pub inflation_rate: Decimal,
    pub withdrawal_rate: Decimal,
}

pub struct CoastFIREResult {
    pub coast_fire_number: Decimal,
    pub is_coast_fire: bool,
    pub surplus_or_deficit: Decimal,
    pub projected_portfolio_at_retirement: Decimal,
    pub years_to_coast: Option<u32>,
}
```

### Withdrawal Simulation

```rust
pub struct WithdrawalParams {
    pub portfolio: Decimal,
    pub annual_withdrawal: Decimal,
    pub strategy: WithdrawalStrategy,
    pub years: u32,
    pub return_model: ReturnModel,
    pub inflation: InflationModel,
    pub monte_carlo_runs: u32,
    pub seed: Option<u64>,
}

pub struct WithdrawalResult {
    pub success_rate: Decimal,
    pub median_terminal_value: Decimal,
    pub percentile_bands: PercentileBands,
    pub worst_sequence: WorstSequence,
    pub strategy_comparison: Option<Vec<StrategyComparison>>,
    pub safe_withdrawal_amount: Decimal,
}
```

### Historical Backtesting

Rolling-window backtest against actual market history (1928–2024). Ships with embedded historical data compiled via `include_str!`.

**Embedded data specification:**
- **Source:** Robert Shiller's publicly available dataset (http://www.econ.yale.edu/~shiller/data.htm). Public domain / academic use.
- **Series used:** S&P 500 *nominal total return* (with dividends reinvested) + CPI-based inflation. This matches cFIREsim's methodology for comparable validation.
- **Format:** CSV files in `crates/analytics/src/data/`, ~5KB total (annual data, 97 rows).
- **Update policy:** Embedded data is static. Updated only with new releases. For real-time data, use the existing `PriceService` integration.
- **Files:** `us_stock_returns_1928_2024.csv` (year, nominal_return), `us_bond_returns_1928_2024.csv` (year, nominal_return), `us_inflation_1928_2024.csv` (year, cpi_change).

```rust
pub struct BacktestResult {
    pub total_periods: u32,
    pub success_count: u32,
    pub success_rate: Decimal,
    pub worst_start_year: u16,
    pub worst_terminal_value: Decimal,
    pub best_start_year: u16,
    pub best_terminal_value: Decimal,
    pub by_start_year: Vec<PeriodResult>,
}
```

---

## Section 5: Spending Analytics

### Anomaly Detection (Modified Z-Score)

```rust
pub struct AnomalyConfig {
    pub lookback_months: u32,      // default: 6
    pub threshold: Decimal,        // default: 2.5
    pub min_transactions: u32,     // default: 5
    pub granularity: AnomalyGranularity, // PerTransaction or PerCategoryMonth
    pub direction: AnomalyDirection,     // default: SpikesOnly
}

pub enum AnomalyDirection {
    SpikesOnly,   // only flag z > threshold (unexpected high spending)
    DropsOnly,    // only flag z < -threshold (missed expected charges)
    Both,         // flag |z| > threshold
}
```

**Algorithm:** Uses median/MAD (Median Absolute Deviation) instead of mean/stddev — robust to the right-skewed nature of spending data.

```
modified_z = 0.6745 * (current - median) / MAD
```

**One-sided detection:** Since spending data is right-skewed, only **positive** z-scores (unexpected spikes) trigger anomalies by default. A negative z-score (spending significantly *below* baseline) is surfaced separately as a `MissedExpectedCharge` signal — useful for detecting cancelled subscriptions or missed bills. The `AnomalyConfig` has a `direction: AnomalyDirection` field:
- `SpikesOnly` (default) — only flag z > threshold
- `DropsOnly` — only flag z < -threshold
- `Both` — flag |z| > threshold

**MAD = 0 guard:** When all baseline values are identical (MAD = 0), any deviation from the constant value is flagged as High severity. If the current value equals the baseline, no anomaly is emitted.

Severity thresholds (same for spikes and drops, applied to the absolute z-score):
- `|z| >= 2.5`: Low — notable but could be normal variation
- `|z| >= 3.5`: Medium — likely unusual, worth flagging
- `|z| >= 5.0`: High — almost certainly anomalous

### Trend Analysis

```rust
pub struct TrendConfig {
    pub period: TrendPeriod,
    pub lookback: u32,
    pub moving_average_window: u32,
    pub metrics: Vec<TrendMetric>,
}

pub enum TrendMetric {
    TotalSpending,
    TotalIncome,
    SavingsRate,
    CategorySpending(String),
    NetWorth,
}

pub struct TrendReport {
    pub series: Vec<TrendSeries>,
    pub summary: TrendSummary,
}
```

Produces raw values, smoothed (moving average), period-over-period change %, and directional classification (Increasing/Decreasing/Stable with ±2% threshold).

### Recurring Charge Detection

Groups transactions by normalized counterparty, computes inter-transaction intervals, clusters into frequency buckets (weekly/biweekly/monthly/quarterly/annual), scores confidence based on interval consistency + amount consistency + recency. Threshold: confidence > 0.6.

```rust
pub struct RecurringCharge {
    pub counterparty: String,
    pub category: Option<String>,
    pub typical_amount: Decimal,
    pub amount_variance: Decimal,
    pub frequency: RecurringFrequency,
    pub confidence: Decimal,
    pub last_seen: NaiveDate,
    pub next_expected: NaiveDate,
    pub is_overdue: bool,          // true if next_expected is in the past (possibly cancelled)
    pub annual_cost: Decimal,
}
```

### Category Correlation

Pearson correlation on monthly spending totals between categories. Uses returns (month-over-month change) not raw levels. Only surfaces correlations with |r| >= 0.4 and categories with >= `min_months` of data.

---

## Section 6: Portfolio Analytics

### Allocation Drift

```rust
pub struct AllocationTarget {
    pub asset_class: String,
    pub target_weight: Decimal,
    pub tolerance_band: Decimal,   // default: 0.05 (±5%)
}

pub struct DriftReport {
    pub total_portfolio_value: Decimal,
    pub allocations: Vec<AllocationStatus>,
    pub max_drift: Decimal,
    pub needs_rebalancing: bool,
    pub drift_score: Decimal,      // 0.0 (perfect) to 1.0 (severe)
}
```

### Rebalancing Suggestions

Three methods: `FullRebalance` (sell over / buy under), `ContributionOnly` (buy under with new cash only), `ThresholdOnly` (only trade outside tolerance band). Filters out trades below `min_trade_amount` (default $50).

```rust
pub struct Trade {
    pub asset_class: String,
    pub direction: TradeDirection,
    pub amount: Decimal,
    pub from_weight: Decimal,
    pub to_weight: Decimal,
    pub rationale: String,
}
```

**Invariant:** `rebalance_suggestions()` asserts that the resulting `to_weight` values across all asset classes sum to `1.0` (within Decimal precision). This is verified in the function body and in property-based tests.

### Returns Analysis

Both TWR (time-weighted, Modified Dietz method) and MWR (money-weighted / IRR). Per-holding attribution shows each asset's contribution to total return.

```rust
pub struct ReturnsReport {
    pub time_weighted_return: Decimal,
    pub money_weighted_return: Decimal,
    pub total_gain_loss: Decimal,
    pub annualized_return: Decimal,
    pub by_holding: Vec<HoldingReturn>,
    pub periods: Vec<PeriodReturn>,
}
```

### Asset Correlation

Pearson correlation on monthly price returns between assets. Requires minimum overlap period. Helps assess diversification quality.

### Schema Changes

New table:

```sql
CREATE TABLE IF NOT EXISTS finance_allocation_targets (
    id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_class TEXT NOT NULL,
    target_weight TEXT NOT NULL,          -- stored as decimal string (e.g., "0.60")
    tolerance_band TEXT NOT NULL DEFAULT '0.05',  -- stored as decimal string
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(portfolio_id, asset_class)
);
```

Weights stored as `TEXT` (decimal strings) for consistency with the spec's precision mandate. Parsed to `Decimal` in Rust. No SQL arithmetic needed on weights — all calculation happens in Rust.

New column: `finance_investments.asset_class TEXT`.

---

## Section 7: Integration Layer

### Net Worth Snapshots

New table for historical tracking (fixes `report_net_worth_history` stub):

```sql
CREATE TABLE IF NOT EXISTS finance_net_worth_snapshots (
    id TEXT PRIMARY KEY,
    snapshot_date TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    accounts_total INTEGER NOT NULL,
    investments_total INTEGER NOT NULL,
    liabilities_total INTEGER NOT NULL,
    net_worth INTEGER NOT NULL,
    breakdown TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(snapshot_date, currency)
);

CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_date
    ON finance_net_worth_snapshots(snapshot_date);
CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_currency_date
    ON finance_net_worth_snapshots(currency, snapshot_date);
```

Triggered daily via `FinanceHandler::record_net_worth_snapshot()`, on-demand via `snapshot_record` action, and optionally after material net-worth changes.

### Sensitivity / What-If Framework

Typed wrappers that sweep variable ranges through Monte Carlo. Sensitivity sweeps use a **reduced run count** (default: 1,000 runs per combination instead of 10,000) to keep computation tractable — a 5×5 grid = 25 combinations × 1,000 runs × 50 years = ~1.25M year-steps, completing in seconds single-threaded.

```rust
pub struct SensitivityConfig {
    pub runs_per_point: u32,          // default: 1,000 (reduced from full sim's 10,000)
    pub seed: Option<u64>,            // same seed used for all points for fair comparison
}

pub struct SensitivityResult {
    pub row_variable: String,         // e.g., "withdrawal_rate"
    pub col_variable: String,         // e.g., "expected_return"
    pub row_values: Vec<Decimal>,     // e.g., [0.03, 0.035, 0.04]
    pub col_values: Vec<Decimal>,     // e.g., [0.05, 0.07, 0.09]
    pub grid: Vec<Vec<SensitivityPoint>>, // [row][col]
    pub runs_per_point: u32,          // echo back for audit
}

pub struct SensitivityPoint {
    pub success_rate: Decimal,
    pub median_terminal: Decimal,
    pub p5_terminal: Decimal,
}

impl FIRECalculator {
    pub fn sensitivity_withdrawal_vs_return(
        base: &WithdrawalParams,
        withdrawal_rates: &[Decimal],
        return_rates: &[Decimal],
        config: &SensitivityConfig,
    ) -> SensitivityResult;

    pub fn sensitivity_savings_vs_timeline(
        base: &FIREParams,
        savings_amounts: &[Decimal],
        year_horizons: &[u32],
        config: &SensitivityConfig,
    ) -> SensitivityResult;
}
```

### New Tool Actions (19 total)

| Action | Module | Description |
|---|---|---|
| `analyze_spending_anomalies` | spending | Flag unusual spending via modified z-score |
| `analyze_spending_trends` | spending | Trend lines + directional classification |
| `analyze_recurring_charges` | spending | Detect subscriptions/recurring charges |
| `analyze_category_correlation` | spending | Category spending relationships |
| `portfolio_drift` | portfolio | Allocation vs targets |
| `portfolio_rebalance` | portfolio | Suggested trades |
| `portfolio_returns` | portfolio | TWR, MWR, per-holding attribution |
| `portfolio_correlation` | portfolio | Asset diversification analysis |
| `allocation_target_set` | portfolio | CRUD — set target weights |
| `allocation_target_list` | portfolio | CRUD — list targets |
| `fire_traditional` | fire | Classic FIRE number + timeline |
| `fire_coast` | fire | Coast FIRE analysis |
| `fire_lean` | fire | Lean FIRE (essentials only) |
| `fire_fat` | fire | Fat FIRE (full lifestyle) |
| `fire_withdrawal_sim` | fire | Monte Carlo withdrawal simulation |
| `fire_backtest` | fire | Historical rolling-window backtest |
| `fire_sensitivity` | fire | Variable sweep grid |
| `snapshot_record` | snapshots | Record current net worth |
| `snapshot_history` | snapshots | Query net worth over time |

Total: 41 existing + 19 new = **60 actions**.

### Dispatch Structure

New action prefixes fold cleanly into the existing match:
- `analyze_*` → new `handle_analyze(..)` handler
- `fire_*` → new `handle_fire(..)` handler
- `allocation_*` → new `handle_allocation(..)` handler
- `snapshot_*` → new `handle_snapshot(..)` handler

The four new portfolio analytics actions (`portfolio_drift`, `portfolio_rebalance`, `portfolio_returns`, `portfolio_correlation`) are added as new match arms inside the **existing** `handle_investment()` function, alongside `portfolio_create` and `portfolio_list`. No dispatch ambiguity — each action string is matched exactly. The `handle_investment` function grows by 4 arms but stays cohesive as all portfolio-related operations.

### Atomicity Fix

Multi-statement finance operations use explicit `sqlx` transactions via `SqlitePool::begin()`:

```rust
// In feature-finance tool handlers (not on FinanceStorage itself):
let mut tx = pool.begin().await?;

// Pass &mut tx to repo methods instead of &pool
let row = accounts.add_with_tx(&mut tx, &new_account).await?;
accounts.adjust_balance_with_tx(&mut tx, account_id, delta).await?;

tx.commit().await?;
```

Repos that participate in transactions get `_with_tx` variants that accept `&mut sqlx::Transaction<'_, Sqlite>` instead of `&SqlitePool`. This follows idiomatic `sqlx` usage and avoids the lifetime complexity of closure-based wrappers at MSRV 1.75.

Used for `tx_add` (add + balance adjustment), `tx_add_transfer` (2 adds + 2 balance adjustments), and `tx_delete` (delete + reverse balance).

### Skill Updates

**Internal skill** (`skills/finance-management/SKILL.md`): Add triggers for "anomaly", "recurring charges", "drift", "rebalance", "monte carlo", "simulation", "coast fire", "lean fire", "fat fire", "backtest". New reference: `references/analytics.md`.

**Claude Code skill** (`.claude/skills/klyntbot-finance/`): Update `references/actions.md` with all 19 new actions, parameters, and examples.

---

## Section 8: Testing Strategy

### Testing Pyramid

1. **Unit tests** — every public function, deterministic, seeded, no I/O
2. **Property-based tests** (proptest) — mathematical invariants
3. **Integration tests** — full `FinanceTool::execute()` → analytics → DB round-trip
4. **Benchmark validation** — compare against cFIREsim, Trinity Study, ProjectionLab

### Key Property-Based Tests

- FIRE number = expenses / withdrawal_rate (for any valid inputs)
- Lower withdrawal → higher success rate (monotonic)
- Portfolio drift sums to zero across all asset classes
- Correlation matrix is symmetric with diagonal = 1.0
- All correlation coefficients in [-1, 1]

### Edge Case Coverage

| Edge case | Expected behavior |
|---|---|
| Zero savings + zero portfolio | `months_to_fire: None`, no panic |
| 100-year horizon | Completes, lower success rate |
| 10-year negative return streak | Depletes predictably, no negative values |
| Single holding portfolio | Drift = 0, no rebalance needed |
| All spending in one category | Empty correlation matrix |
| Zero MAD (identical spending) | No anomalies, no division by zero |
| Currency mismatch in Money ops | Returns error |
| Withdrawal exceeds portfolio year 1 | Immediate ruin, 0% success |
| Guyton-Klinger extreme volatility | Adjusts within floor/ceiling bounds |

### Benchmark Validation

Manual tests comparing against:
- cFIREsim historical success rates (±1% tolerance)
- Trinity Study 4% rule results
- Known FIRE number calculations

### Test Organization

```
crates/analytics/tests/
├── monte_carlo_tests.rs
├── fire_tests.rs
├── spending_tests.rs
├── portfolio_tests.rs
├── money_tests.rs
├── sensitivity_tests.rs
└── benchmarks/
    ├── cfiresim_validation.rs
    └── trinity_validation.rs
```

---

## Non-Goals (Deferred to Later Sub-projects)

- ARIMA / Prophet-style forecasting
- Custom metric DSL
- ML-based anomaly detection (isolation forest, DBSCAN)
- Portfolio optimization (mean-variance, Black-Litterman)
- Multi-step guided skill workflows (sub-project 2)
- Proactive coaching system (sub-project 4)

## Dependencies

- `rust_decimal` — fixed-point decimal arithmetic
- `rand` + `rand_chacha` — seeded RNG
- `proptest` (dev) — property-based testing
- `chrono` — date handling (already in workspace)
- No new external API dependencies (historical data embedded via `include_str!`)
