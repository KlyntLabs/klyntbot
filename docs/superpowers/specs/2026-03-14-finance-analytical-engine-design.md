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
L0: common           — Money, Currency, Decimal re-export, Result<T>
L2: storage           — FinanceStorage, Row types
L3: analytics (NEW)   — pure computation, rust_decimal, rand with seed
L3: feature-finance   — depends on analytics, exposes via Tool actions
```

---

## Section 1: Money Type & Precision Foundation

### Problem

- Amounts stored as `i64` (minor units) — fine for storage, but Rust code does `(price * 100.0).round() as i64` which introduces float errors
- Investment quantities are `f64` (`REAL` in SQLite) — float throughout the stack
- No currency-aware precision (JPY=0 decimals, KWD=3 decimals treated same as USD=2)

### Design

**`Money` type** (lives in `analytics`, re-exported via `common`):

```rust
use rust_decimal::Decimal;

pub struct Money {
    pub amount: Decimal,
    pub currency: Currency,
}

pub enum Currency {
    USD, EUR, THB, JPY, GBP, /* ... */
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
    pub fn detect_anomalies(txs: &[Transaction], config: &AnomalyConfig) -> Vec<Anomaly>;
    pub fn trends(txs: &[Transaction], config: &TrendConfig) -> TrendReport;
    pub fn detect_recurring(txs: &[Transaction]) -> Vec<RecurringCharge>;
    pub fn category_correlation(txs: &[Transaction], min_months: u32) -> CorrelationMatrix;
}

pub struct PortfolioAnalyzer;
impl PortfolioAnalyzer {
    pub fn allocation_drift(holdings: &[Holding], targets: &[AllocationTarget]) -> DriftReport;
    pub fn rebalance_suggestions(drift: &DriftReport, config: &RebalanceConfig) -> Vec<Trade>;
    pub fn returns(txs: &[InvestmentTx], current_values: &[Holding]) -> ReturnsReport;
    pub fn asset_correlation(price_history: &[PriceSeries], min_overlap: u32) -> CorrelationMatrix;
}
```

### Design Principles

- **No storage dependency** — analytics takes pre-fetched data as input
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
    pub success_rate: Decimal,
    pub percentile_bands: PercentileBands,
    pub terminal_values: TerminalStats,
    pub worst_sequence: WorstSequence,
    pub annual_summary: Vec<YearSummary>,
}

pub struct TerminalStats {
    pub median: Decimal,
    pub mean: Decimal,
    pub p5: Decimal,
    pub p95: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    pub ruin_count: u32,
}

pub struct WorstSequence {
    pub seed_index: u32,
    pub portfolio_by_year: Vec<Decimal>,
    pub ruin_year: Option<u32>,
}

pub struct YearSummary {
    pub year: u32,
    pub median_value: Decimal,
    pub survival_rate: Decimal,
    pub median_withdrawal: Decimal,
}
```

### Algorithm

```
for each run (0..config.runs):
    rng = ChaCha8Rng::seed_from_u64(base_seed + run_index)
    portfolio = initial_portfolio

    for year in 0..config.years:
        1. Generate return (log-normal draw, bootstrap sample, or correlated asset draw via Cholesky)
        2. Generate inflation (fixed or stochastic)
        3. Apply contribution or withdrawal (strategy-dependent)
        4. If portfolio <= 0: mark ruin, break
        5. Apply return: portfolio *= (1 + annual_return)
        6. Record year-end value

    record_terminal(run_index, portfolio)

Aggregate → percentile bands, success rate, terminal stats
```

**Implementation details:**
- Cholesky decomposition for correlated asset returns (precomputed once)
- `rand_chacha::ChaCha8Rng` for reproducible cross-platform RNG
- Base seed + run index for individual run reproducibility
- Single-threaded in v1 (500K iterations is fast enough)
- Negative returns capped at -99.9%

---

## Section 4: FIRE Calculator Suite

### Variants

**Traditional FIRE:** `fire_number = annual_expenses / withdrawal_rate`. Months-to-FIRE via compound growth formula with real return rate.

**Coast FIRE:** `coast_number = fire_number / (1 + real_return)^years_to_retirement`. If `current_portfolio >= coast_number`, you can stop saving.

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

Rolling-window backtest against actual market history (1928–2024). Ships with embedded Shiller dataset compiled via `include_str!`.

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
}
```

**Algorithm:** Uses median/MAD (Median Absolute Deviation) instead of mean/stddev — robust to the right-skewed nature of spending data.

```
modified_z = 0.6745 * (current - median) / MAD
```

Severity: |z| >= 2.5 Low, |z| >= 3.5 Medium, |z| >= 5.0 High.

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
    target_weight REAL NOT NULL,
    tolerance_band REAL NOT NULL DEFAULT 0.05,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(portfolio_id, asset_class)
);
```

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
```

Triggered daily via `FinanceHandler::record_net_worth_snapshot()`, on-demand via `snapshot_record` action, and optionally after material net-worth changes.

### Sensitivity / What-If Framework

Typed wrappers that sweep variable ranges through Monte Carlo:

```rust
impl FIRECalculator {
    pub fn sensitivity_withdrawal_vs_return(
        base: &WithdrawalParams,
        withdrawal_rates: &[Decimal],
        return_rates: &[Decimal],
        seed: Option<u64>,
    ) -> SensitivityResult;

    pub fn sensitivity_savings_vs_timeline(
        base: &FIREParams,
        savings_amounts: &[Decimal],
        year_horizons: &[u32],
        seed: Option<u64>,
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
- `analyze_*` → `handle_analyze(..)`
- `fire_*` → `handle_fire(..)`
- `allocation_*` → `handle_allocation(..)`
- `snapshot_*` → `handle_snapshot(..)`
- `portfolio_drift/rebalance/returns/correlation` → existing `handle_investment(..)` (matches `portfolio_*`)

### Atomicity Fix

`FinanceStorage` gets a transaction wrapper:

```rust
impl FinanceStorage {
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&FinanceStorage) -> Pin<Box<dyn Future<Output = Result<R>> + '_>>;
}
```

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
