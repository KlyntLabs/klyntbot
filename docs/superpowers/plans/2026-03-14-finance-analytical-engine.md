# Finance Analytical Engine — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a production-grade `analytics` crate with Monte Carlo simulations, FIRE calculators, spending intelligence, and portfolio analysis — then integrate it into `feature-finance` with 19 new tool actions and upgraded agent skills.

**Architecture:** New pure-computation `analytics` crate at Layer 3 (no async, no storage, no side effects). `Money` type added to `common` crate (L0) with `rust_decimal`. `feature-finance` depends on `analytics` and exposes capabilities through new tool actions. Skills upgraded with guided multi-step workflows.

**Tech Stack:** Rust, `rust_decimal`, `rand`/`rand_chacha`, `proptest` (dev), SQLite (via `sqlx`), Agent Skills v2.0

**Spec:** `docs/superpowers/specs/2026-03-14-finance-analytical-engine-design.md`

---

## File Map

### New Files

```
crates/common/src/money.rs              — Money struct, Currency enum, Decimal re-export
crates/analytics/Cargo.toml             — crate manifest
crates/analytics/src/lib.rs             — public API re-exports
crates/analytics/src/types.rs           — PercentileBands, TimeSeries, CorrelationMatrix, Anomaly
crates/analytics/src/input_types.rs     — SpendingRecord, Holding, InvestmentCashFlow, PriceSeries
crates/analytics/src/monte_carlo/mod.rs          — MonteCarloEngine, SimulationConfig, SimulationResult
crates/analytics/src/monte_carlo/distributions.rs — LogNormal, HistoricalBootstrap sampling
crates/analytics/src/monte_carlo/sampling.rs      — seeded RNG helpers, Cholesky decomposition
crates/analytics/src/fire/mod.rs         — FIRECalculator, FIREParams, FIREResult
crates/analytics/src/fire/variants.rs    — Traditional, Coast, Lean, Fat FIRE
crates/analytics/src/fire/withdrawal.rs  — WithdrawalStrategy implementations (Fixed, GuytonKlinger, VPW)
crates/analytics/src/fire/sequence_risk.rs — HistoricalBacktest, embedded data loading
crates/analytics/src/data/us_stock_returns_1928_2024.csv   — embedded Shiller data
crates/analytics/src/data/us_bond_returns_1928_2024.csv    — embedded bond returns
crates/analytics/src/data/us_inflation_1928_2024.csv       — embedded CPI data
crates/analytics/src/spending/mod.rs      — SpendingAnalyzer
crates/analytics/src/spending/anomaly.rs  — modified z-score anomaly detection
crates/analytics/src/spending/trends.rs   — moving averages, period-over-period, TrendReport
crates/analytics/src/spending/recurring.rs — recurring charge detection
crates/analytics/src/spending/correlation.rs — category correlation matrix
crates/analytics/src/portfolio/mod.rs      — PortfolioAnalyzer
crates/analytics/src/portfolio/drift.rs    — allocation drift + rebalancing suggestions
crates/analytics/src/portfolio/returns.rs  — TWR (Modified Dietz), MWR (IRR)
crates/analytics/src/portfolio/correlation.rs — asset price correlation
crates/analytics/tests/money_tests.rs
crates/analytics/tests/monte_carlo_tests.rs
crates/analytics/tests/fire_tests.rs
crates/analytics/tests/spending_tests.rs
crates/analytics/tests/portfolio_tests.rs
crates/analytics/tests/sensitivity_tests.rs
crates/analytics/tests/benchmarks/cfiresim_validation.rs
crates/analytics/tests/benchmarks/trinity_validation.rs
crates/feature-finance/src/tool/analyze_handlers.rs — handle_analyze() dispatcher
crates/feature-finance/src/tool/fire_handlers.rs — handle_fire() dispatcher
crates/feature-finance/src/tool/allocations.rs  — handle_allocation() CRUD
crates/feature-finance/src/tool/snapshots.rs    — handle_snapshot() dispatcher
crates/storage/src/repos/finance_allocation_repo.rs   — AllocationTarget CRUD
crates/storage/src/repos/finance_snapshot_repo.rs     — NetWorthSnapshot CRUD + queries
skills/finance-management/references/analytics-actions.md
skills/finance-management/references/fire-planning.md
skills/finance-management/references/portfolio-analysis.md
skills/finance-management/references/spending-intelligence.md
```

### Modified Files

```
Cargo.toml                                — add rust_decimal, rand_chacha to [workspace.dependencies], add analytics to members
crates/common/Cargo.toml                  — add rust_decimal dependency
crates/common/src/lib.rs                  — add pub mod money, re-export Money/Currency/Decimal
crates/feature-finance/Cargo.toml         — add analytics dependency
crates/feature-finance/src/lib.rs         — re-export analytics types, update FinanceFeature
crates/feature-finance/src/tool/mod.rs    — add new dispatch arms, inject analytics
crates/feature-finance/src/tool/investments/mod.rs — add portfolio_drift/rebalance/returns/correlation arms
crates/feature-finance/src/tool/goals.rs  — replace existing fire logic with analytics delegation
crates/feature-finance/src/tool/reports.rs — use analytics for trends, update report_net_worth_history
crates/feature-finance/migrations/001_finance_tables.sql — add new tables, alter quantity REAL→TEXT, add asset_class
crates/storage/src/rows/finance.rs        — add AllocationTargetRow, NetWorthSnapshotRow
crates/storage/src/finance_storage.rs     — add allocation + snapshot repos
crates/storage/src/repos/mod.rs           — register new repos
crates/storage/src/repos/finance_investment_repo.rs — quantity REAL→TEXT handling
crates/storage/src/repos/finance_transaction_repo.rs — add _with_tx variants
crates/storage/src/repos/finance_account_repo.rs    — add _with_tx variants
skills/finance-management/SKILL.md                   — add triggers, flowchart rows
skills/finance-management/references/budgeting.md    — add analytics cross-refs
.claude/skills/klyntbot-finance/SKILL.md             — update quick reference
.claude/skills/klyntbot-finance/references/actions.md — add 19 new actions
```

---

## Chunk 1: Foundation — Money Type & Analytics Crate Scaffold

### Task 1: Workspace Dependencies

**Files:**
- Modify: `Cargo.toml` (workspace manifest)

- [ ] **Step 1: Add new dependencies to workspace**

In root `Cargo.toml`, add to `[workspace.dependencies]`:

```toml
rust_decimal = { version = "1.36", features = ["serde-with-str", "maths"] }
rust_decimal_macros = "1.36"
rand_chacha = "0.9"
proptest = "1.5"
analytics = { path = "crates/analytics" }
```

**Important:** The `maths` feature on `rust_decimal` is required from day one — it enables `Decimal::ln()`, `Decimal::powu()`, and `Decimal::from_f64_retain()` which are used throughout the analytics crate (Monte Carlo distributions, FIRE formulas, etc.). Do NOT add it later.

Note: `rand = "0.9"` is already in the workspace — verify and skip if present. The `rand_chacha` version must match `rand` — verify `rand_chacha = "0.9"` is compatible (check crates.io; if `rand_chacha` 0.9 doesn't exist, use `rand_chacha = "0.3"` which pairs with `rand` 0.8, and downgrade `rand` accordingly).

- [ ] **Step 2: Add `analytics` to workspace members**

In `[workspace.members]`, add `"crates/analytics"` after `"crates/feature-finance"` (same layer).

**Do NOT run `cargo check` yet** — the analytics crate directory doesn't exist until Task 3. The workspace manifest will be verified in Task 3, Step 5.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add rust_decimal, rand_chacha, proptest, analytics to workspace deps"
```

---

### Task 2: Money Type in Common Crate

**Files:**
- Modify: `crates/common/Cargo.toml`
- Create: `crates/common/src/money.rs`
- Modify: `crates/common/src/lib.rs`
- Test: `crates/common/src/money.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Add `rust_decimal` dependency to common**

In `crates/common/Cargo.toml`, add to `[dependencies]`:

```toml
rust_decimal = { workspace = true }
```

And add to `[dev-dependencies]`:

```toml
rust_decimal_macros = { workspace = true }
```

- [ ] **Step 2: Write failing tests for Money type**

Create `crates/common/src/money.rs` with test module only:

```rust
//! Currency-aware monetary type using rust_decimal for precision.

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    #[test]
    fn money_from_minor_units_usd() {
        let m = super::Money::from_minor_units(12345, super::Currency::USD);
        assert_eq!(m.amount(), dec!(123.45));
    }

    #[test]
    fn money_from_minor_units_jpy() {
        let m = super::Money::from_minor_units(1000, super::Currency::JPY);
        assert_eq!(m.amount(), dec!(1000));
    }

    #[test]
    fn money_to_minor_units_usd() {
        let m = super::Money::new(dec!(123.45), super::Currency::USD);
        assert_eq!(m.to_minor_units(), 12345);
    }

    #[test]
    fn money_to_minor_units_kwd() {
        let m = super::Money::new(dec!(1.234), super::Currency::KWD);
        assert_eq!(m.to_minor_units(), 1234);
    }

    #[test]
    fn money_add_same_currency() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(5.50), super::Currency::USD);
        let result = (a + b).unwrap();
        assert_eq!(result.amount(), dec!(15.50));
    }

    #[test]
    fn money_add_different_currency_errors() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(5.00), super::Currency::EUR);
        assert!((a + b).is_err());
    }

    #[test]
    fn money_sub_same_currency() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(3.25), super::Currency::USD);
        let result = (a - b).unwrap();
        assert_eq!(result.amount(), dec!(6.75));
    }

    #[test]
    fn currency_decimal_places() {
        assert_eq!(super::Currency::USD.decimal_places(), 2);
        assert_eq!(super::Currency::JPY.decimal_places(), 0);
        assert_eq!(super::Currency::KWD.decimal_places(), 3);
        assert_eq!(super::Currency::THB.decimal_places(), 2);
    }

    #[test]
    fn money_display_respects_currency() {
        let usd = super::Money::new(dec!(1234.50), super::Currency::USD);
        assert_eq!(format!("{usd}"), "1234.50 USD");

        let jpy = super::Money::new(dec!(1000), super::Currency::JPY);
        assert_eq!(format!("{jpy}"), "1000 JPY");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p common --test-threads=1 2>&1 | tail -10`
Expected: FAIL — `Money`, `Currency` types don't exist yet

- [ ] **Step 4: Implement Money and Currency types**

Add implementation above the test module in `crates/common/src/money.rs`:

```rust
//! Currency-aware monetary type using rust_decimal for precision.

use rust_decimal::Decimal;
use std::fmt;
use std::ops::{Add, Sub};

use crate::{KlyntbotError, Result};

/// ISO 4217 currency with known decimal places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Currency {
    USD, EUR, GBP, THB, JPY, KWD, AUD, CAD, CHF, SGD, HKD, NZD, SEK, NOK, DKK, CNY, INR, KRW, MYR, PHP, IDR, VND, TWD, BRL, MXN, ZAR, AED, SAR, QAR, BHD,
    #[serde(untagged)]
    Custom {
        code: String,
        decimal_places: u8,
    },
}

impl Currency {
    /// Number of decimal places for this currency's smallest unit.
    pub fn decimal_places(&self) -> u8 {
        match self {
            Self::JPY | Self::KRW | Self::VND => 0,
            Self::KWD | Self::BHD => 3,
            Self::Custom { decimal_places, .. } => *decimal_places,
            _ => 2, // Most currencies use 2 decimal places
        }
    }

    /// ISO 4217 currency code as string.
    pub fn code(&self) -> &str {
        match self {
            Self::USD => "USD", Self::EUR => "EUR", Self::GBP => "GBP",
            Self::THB => "THB", Self::JPY => "JPY", Self::KWD => "KWD",
            Self::AUD => "AUD", Self::CAD => "CAD", Self::CHF => "CHF",
            Self::SGD => "SGD", Self::HKD => "HKD", Self::NZD => "NZD",
            Self::SEK => "SEK", Self::NOK => "NOK", Self::DKK => "DKK",
            Self::CNY => "CNY", Self::INR => "INR", Self::KRW => "KRW",
            Self::MYR => "MYR", Self::PHP => "PHP", Self::IDR => "IDR",
            Self::VND => "VND", Self::TWD => "TWD", Self::BRL => "BRL",
            Self::MXN => "MXN", Self::ZAR => "ZAR", Self::AED => "AED",
            Self::SAR => "SAR", Self::QAR => "QAR", Self::BHD => "BHD",
            Self::Custom { code, .. } => code.as_str(),
        }
    }

    /// Parse a currency code string into a Currency.
    pub fn from_code(code: &str) -> Self {
        match code.to_uppercase().as_str() {
            "USD" => Self::USD, "EUR" => Self::EUR, "GBP" => Self::GBP,
            "THB" => Self::THB, "JPY" => Self::JPY, "KWD" => Self::KWD,
            "AUD" => Self::AUD, "CAD" => Self::CAD, "CHF" => Self::CHF,
            "SGD" => Self::SGD, "HKD" => Self::HKD, "NZD" => Self::NZD,
            "SEK" => Self::SEK, "NOK" => Self::NOK, "DKK" => Self::DKK,
            "CNY" => Self::CNY, "INR" => Self::INR, "KRW" => Self::KRW,
            "MYR" => Self::MYR, "PHP" => Self::PHP, "IDR" => Self::IDR,
            "VND" => Self::VND, "TWD" => Self::TWD, "BRL" => Self::BRL,
            "MXN" => Self::MXN, "ZAR" => Self::ZAR, "AED" => Self::AED,
            "SAR" => Self::SAR, "QAR" => Self::QAR, "BHD" => Self::BHD,
            other => Self::Custom {
                code: other.to_string(),
                decimal_places: 2,
            },
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Currency-aware monetary value. All arithmetic uses Decimal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    amount: Decimal,
    currency: Currency,
}

impl Money {
    /// Create from a Decimal amount.
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create from the smallest currency unit (e.g., cents for USD).
    pub fn from_minor_units(minor: i64, currency: Currency) -> Self {
        let places = currency.decimal_places() as u32;
        let amount = Decimal::new(minor, places);
        Self { amount, currency }
    }

    /// The decimal amount.
    pub fn amount(&self) -> Decimal {
        self.amount
    }

    /// The currency.
    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Convert to the smallest currency unit (e.g., cents).
    pub fn to_minor_units(&self) -> i64 {
        use rust_decimal::prelude::ToPrimitive;
        let places = self.currency.decimal_places() as u32;
        let scale = Decimal::new(10i64.pow(places), 0);
        let minor = self.amount * scale;
        minor.to_i64().unwrap_or(0)
    }

    /// Zero amount in the given currency.
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }
}

impl Add for Money {
    type Output = Result<Money>;

    fn add(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(KlyntbotError::Tool(
                format!("Cannot add {} and {}", self.currency, rhs.currency),
            ));
        }
        Ok(Money::new(self.amount + rhs.amount, self.currency))
    }
}

impl Sub for Money {
    type Output = Result<Money>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(KlyntbotError::Tool(
                format!("Cannot subtract {} from {}", rhs.currency, self.currency),
            ));
        }
        Ok(Money::new(self.amount - rhs.amount, self.currency))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let places = self.currency.decimal_places() as usize;
        write!(f, "{:.prec$} {}", self.amount, self.currency, prec = places)
    }
}
```

- [ ] **Step 5: Register module in common/lib.rs**

Add to `crates/common/src/lib.rs`:

```rust
pub mod money;
pub use money::{Currency, Money};
```

Also add `rust_decimal` re-export:

```rust
pub use rust_decimal::Decimal;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p common 2>&1 | tail -10`
Expected: All money tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/common/
git commit -m "feat(common): add Money type with Currency-aware Decimal arithmetic"
```

---

### Task 3: Analytics Crate Scaffold

**Files:**
- Create: `crates/analytics/Cargo.toml`
- Create: `crates/analytics/src/lib.rs`
- Create: `crates/analytics/src/types.rs`
- Create: `crates/analytics/src/input_types.rs`

- [ ] **Step 1: Create crate manifest**

Create `crates/analytics/Cargo.toml`:

```toml
[package]
name = "analytics"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
rust_decimal = { workspace = true }
rand = { workspace = true }
rand_chacha = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
rust_decimal_macros = { workspace = true }
```

- [ ] **Step 2: Create shared output types**

Create `crates/analytics/src/types.rs`:

```rust
//! Shared output types used across analytics modules.

use chrono::NaiveDate;
use common::{Currency, Decimal, Money};
use serde::Serialize;

/// Percentile bands from Monte Carlo or sensitivity analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentileBands {
    pub p5: Vec<Decimal>,
    pub p25: Vec<Decimal>,
    pub p50: Vec<Decimal>,
    pub p75: Vec<Decimal>,
    pub p95: Vec<Decimal>,
    pub survival_rate: Vec<Decimal>,
    pub labels: Vec<String>,
}

/// Generic time series for trend analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeries {
    pub points: Vec<(NaiveDate, Decimal)>,
    pub label: String,
}

/// Correlation matrix (spending categories or assets).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelationMatrix {
    pub labels: Vec<String>,
    pub coefficients: Vec<Vec<Decimal>>,
}

/// Severity of a detected anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

/// A detected anomaly in spending.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Anomaly {
    pub date: NaiveDate,
    pub category: String,
    pub amount: Money,
    pub z_score: Decimal,       // signed: positive = spike, negative = drop
    pub severity: AnomalySeverity,
    pub explanation: String,
}

/// Direction of anomaly detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyDirection {
    /// Only flag z > threshold (unexpected high spending).
    SpikesOnly,
    /// Only flag z < -threshold (missed expected charges).
    DropsOnly,
    /// Flag |z| > threshold.
    Both,
}

impl Default for AnomalyDirection {
    fn default() -> Self {
        Self::SpikesOnly
    }
}

/// Trend direction classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}
```

- [ ] **Step 3: Create analytics input types**

Create `crates/analytics/src/input_types.rs`:

```rust
//! Input types for the analytics crate.
//! These are lightweight structs that feature-finance converts storage Row types into.

use chrono::NaiveDate;
use common::Decimal;

/// A financial transaction for spending analysis.
#[derive(Debug, Clone)]
pub struct SpendingRecord {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub tx_type: SpendingType,
    pub category: Option<String>,
    pub counterparty: Option<String>,
}

/// Income or Expense classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendingType {
    Income,
    Expense,
}

/// A portfolio holding for drift/returns analysis.
#[derive(Debug, Clone)]
pub struct Holding {
    pub name: String,
    pub symbol: Option<String>,
    pub asset_class: String,
    pub current_value: Decimal,
    pub cost_basis: Decimal,
    pub quantity: Decimal,
}

/// An investment cash flow for returns calculation (TWR/MWR).
#[derive(Debug, Clone)]
pub struct InvestmentCashFlow {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub holding_symbol: Option<String>,
}

/// A price time series for correlation analysis.
#[derive(Debug, Clone)]
pub struct PriceSeries {
    pub symbol: String,
    pub asset_class: String,
    pub prices: Vec<(NaiveDate, Decimal)>,
}

/// Allocation target for a portfolio.
#[derive(Debug, Clone)]
pub struct AllocationTarget {
    pub asset_class: String,
    pub target_weight: Decimal,
    pub tolerance_band: Decimal,
}

/// Frequency of a recurring charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurringFrequency {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Annual,
}
```

- [ ] **Step 4: Create lib.rs with module declarations**

Create `crates/analytics/src/lib.rs`:

```rust
//! Analytics crate — pure computation for financial analysis.
//!
//! No storage dependency, no async, no side effects.
//! All inputs are pre-fetched data; all outputs are computation results.
//! Every function that uses randomness accepts a seed for reproducibility.

pub mod input_types;
pub mod types;

// Module stubs — will be filled in subsequent tasks
// pub mod fire;
// pub mod monte_carlo;
// pub mod portfolio;
// pub mod spending;

// Explicit re-exports to avoid name collisions (e.g., AllocationTarget)
pub use input_types::{
    AllocationTarget, Holding, InvestmentCashFlow, PriceSeries,
    RecurringFrequency, SpendingRecord, SpendingType,
};
pub use types::{
    Anomaly, AnomalyDirection, AnomalySeverity, CorrelationMatrix,
    PercentileBands, TimeSeries, TrendDirection,
};
```

- [ ] **Step 5: Verify workspace builds**

Run: `cargo check --workspace 2>&1 | tail -5`
Expected: Compilation succeeds

- [ ] **Step 6: Commit**

```bash
git add crates/analytics/ Cargo.toml
git commit -m "feat(analytics): scaffold crate with shared types and input types"
```

---

## Chunk 2: Monte Carlo Engine

### Task 4: Monte Carlo Types & Configuration

**Files:**
- Create: `crates/analytics/src/monte_carlo/mod.rs`
- Create: `crates/analytics/src/monte_carlo/distributions.rs`
- Create: `crates/analytics/src/monte_carlo/sampling.rs`
- Modify: `crates/analytics/src/lib.rs`

- [ ] **Step 1: Create distribution and sampling types**

Create `crates/analytics/src/monte_carlo/sampling.rs`:

```rust
//! Seeded RNG helpers and Cholesky decomposition for correlated returns.

use common::Decimal;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Create a deterministic RNG from a base seed and run index.
pub fn create_rng(base_seed: u64, run_index: u32) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(base_seed.wrapping_add(run_index as u64))
}

/// Cholesky decomposition of a symmetric positive-definite matrix.
/// Returns lower triangular matrix L such that A = L * L^T.
/// Returns None if matrix is not positive definite.
pub fn cholesky_decompose(matrix: &[Vec<Decimal>]) -> Option<Vec<Vec<Decimal>>> {
    let n = matrix.len();
    let mut l = vec![vec![Decimal::ZERO; n]; n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = Decimal::ZERO;
            for k in 0..j {
                sum += l[i][k] * l[j][k];
            }

            if i == j {
                let diag = matrix[i][i] - sum;
                if diag <= Decimal::ZERO {
                    return None; // Not positive definite
                }
                // sqrt via Newton's method for Decimal
                l[i][j] = decimal_sqrt(diag)?;
            } else {
                if l[j][j] == Decimal::ZERO {
                    return None;
                }
                l[i][j] = (matrix[i][j] - sum) / l[j][j];
            }
        }
    }

    Some(l)
}

/// Approximate square root of a Decimal using Newton's method.
/// Returns None for negative inputs.
pub(crate) fn decimal_sqrt(val: Decimal) -> Option<Decimal> {
    if val < Decimal::ZERO {
        return None;
    }
    if val == Decimal::ZERO {
        return Some(Decimal::ZERO);
    }

    let two = Decimal::new(2, 0);
    let epsilon = Decimal::new(1, 12); // 1e-12 precision
    let mut guess = val / two;

    for _ in 0..100 {
        let next = (guess + val / guess) / two;
        let diff = if next > guess { next - guess } else { guess - next };
        if diff < epsilon {
            return Some(next);
        }
        guess = next;
    }

    Some(guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn cholesky_2x2_identity() {
        let m = vec![
            vec![dec!(1), dec!(0)],
            vec![dec!(0), dec!(1)],
        ];
        let l = cholesky_decompose(&m).unwrap();
        assert_eq!(l[0][0], dec!(1));
        assert_eq!(l[1][1], dec!(1));
        assert_eq!(l[1][0], dec!(0));
    }

    #[test]
    fn cholesky_not_positive_definite_returns_none() {
        let m = vec![
            vec![dec!(1), dec!(2)],
            vec![dec!(2), dec!(1)], // eigenvalue < 0
        ];
        assert!(cholesky_decompose(&m).is_none());
    }

    #[test]
    fn decimal_sqrt_basic() {
        let result = decimal_sqrt(dec!(4)).unwrap();
        let diff = (result - dec!(2)).abs();
        assert!(diff < dec!(0.0001));
    }

    #[test]
    fn decimal_sqrt_negative_returns_none() {
        assert!(decimal_sqrt(dec!(-1)).is_none());
    }

    #[test]
    fn rng_deterministic() {
        let mut rng1 = create_rng(42, 0);
        let mut rng2 = create_rng(42, 0);
        let v1: u64 = rand::Rng::random(&mut rng1);
        let v2: u64 = rand::Rng::random(&mut rng2);
        assert_eq!(v1, v2);
    }
}
```

- [ ] **Step 2: Create distributions module**

Create `crates/analytics/src/monte_carlo/distributions.rs`:

```rust
//! Return distribution models for Monte Carlo simulation.

use common::Decimal;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::sampling::{cholesky_decompose, decimal_sqrt};

/// Draw a return from a log-normal distribution.
/// mean_return and std_dev are in simple return space (e.g., 0.07 for 7%).
pub fn draw_log_normal(rng: &mut ChaCha8Rng, mean_return: Decimal, std_dev: Decimal) -> Decimal {
    // Convert to log-space parameters
    // mu = ln(1 + mean) - sigma^2/2
    // sigma = sqrt(ln(1 + (std/mean)^2)) -- but simplified for Decimal
    let one = Decimal::ONE;
    let two = Decimal::new(2, 0);

    // Use Box-Muller transform for normal distribution
    let u1: f64 = rng.random::<f64>().max(1e-10);
    let u2: f64 = rng.random::<f64>();

    let z = ((-2.0 * u1.ln()).sqrt()) * (2.0 * std::f64::consts::PI * u2).cos();

    // Convert z (standard normal) to simple return
    // Simple approach: return = mean + std * z, then floor at -0.999
    let z_dec = Decimal::from_f64_retain(z).unwrap_or(Decimal::ZERO);
    let simple_return = mean_return + std_dev * z_dec;

    // Floor: portfolio value can't go below zero
    let floor = Decimal::new(-999, 3); // -0.999
    if simple_return < floor { floor } else { simple_return }
}

/// Draw a return by sampling from historical data with replacement.
pub fn draw_bootstrap(rng: &mut ChaCha8Rng, historical_returns: &[Decimal]) -> Decimal {
    if historical_returns.is_empty() {
        return Decimal::ZERO;
    }
    let idx = rng.random_range(0..historical_returns.len());
    historical_returns[idx]
}

/// Draw correlated returns for multiple asset classes.
/// Uses Cholesky decomposition to transform independent normals into correlated draws.
pub fn draw_correlated_returns(
    rng: &mut ChaCha8Rng,
    means: &[Decimal],
    std_devs: &[Decimal],
    cholesky_l: &[Vec<Decimal>],
) -> Vec<Decimal> {
    let n = means.len();

    // Draw n independent standard normals
    let mut z = Vec::with_capacity(n);
    for _ in 0..n {
        let u1: f64 = rng.random::<f64>().max(1e-10);
        let u2: f64 = rng.random::<f64>();
        let normal = ((-2.0 * u1.ln()).sqrt()) * (2.0 * std::f64::consts::PI * u2).cos();
        z.push(Decimal::from_f64_retain(normal).unwrap_or(Decimal::ZERO));
    }

    // Transform: correlated = L * z
    let mut correlated = vec![Decimal::ZERO; n];
    for i in 0..n {
        let mut sum = Decimal::ZERO;
        for j in 0..=i {
            sum += cholesky_l[i][j] * z[j];
        }
        // Convert to simple return
        let ret = means[i] + std_devs[i] * sum;
        let floor = Decimal::new(-999, 3);
        correlated[i] = if ret < floor { floor } else { ret };
    }

    correlated
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use rust_decimal_macros::dec;

    #[test]
    fn log_normal_deterministic_with_seed() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let r1 = draw_log_normal(&mut rng1, dec!(0.07), dec!(0.15));
        let r2 = draw_log_normal(&mut rng2, dec!(0.07), dec!(0.15));
        assert_eq!(r1, r2);
    }

    #[test]
    fn bootstrap_samples_from_data() {
        let data = vec![dec!(0.10), dec!(-0.05), dec!(0.15)];
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let r = draw_bootstrap(&mut rng, &data);
        assert!(data.contains(&r));
    }

    #[test]
    fn bootstrap_empty_returns_zero() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        assert_eq!(draw_bootstrap(&mut rng, &[]), Decimal::ZERO);
    }
}
```

- [ ] **Step 3: Create Monte Carlo engine**

Create `crates/analytics/src/monte_carlo/mod.rs`:

```rust
//! Monte Carlo simulation engine for financial projections.

pub mod distributions;
pub mod sampling;

use common::Decimal;
use serde::Serialize;

use crate::types::PercentileBands;
use distributions::{draw_bootstrap, draw_correlated_returns, draw_log_normal};
use sampling::{cholesky_decompose, create_rng};

/// Configuration for a Monte Carlo simulation.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub runs: u32,
    pub years: u32,
    pub initial_portfolio: Decimal,
    pub annual_contribution: Decimal,
    pub annual_withdrawal: Decimal,
    pub withdrawal_strategy: WithdrawalStrategy,
    pub return_model: ReturnModel,
    pub inflation: InflationModel,
    pub seed: Option<u64>,
}

/// How investment returns are modeled.
#[derive(Debug, Clone)]
pub enum ReturnModel {
    LogNormal {
        mean_return: Decimal,
        std_dev: Decimal,
    },
    HistoricalBootstrap {
        returns: Vec<Decimal>,
    },
    AssetAllocation {
        assets: Vec<AssetClass>,
    },
}

/// A single asset class in a multi-asset portfolio.
#[derive(Debug, Clone)]
pub struct AssetClass {
    pub name: String,
    pub weight: Decimal,
    pub mean_return: Decimal,
    pub std_dev: Decimal,
    pub correlation_row: Vec<Decimal>,
}

/// How inflation is modeled.
#[derive(Debug, Clone)]
pub enum InflationModel {
    Fixed(Decimal),
    Variable { mean: Decimal, std_dev: Decimal },
}

/// Withdrawal strategy for retirement simulation.
#[derive(Debug, Clone)]
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

/// Summary of the simulation configuration (for audit trail).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSummary {
    pub runs: u32,
    pub years: u32,
    pub initial_portfolio: Decimal,
    pub annual_withdrawal: Decimal,
}

/// Result of a Monte Carlo simulation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationResult {
    pub config_summary: ConfigSummary,
    pub success_rate: Decimal,
    pub percentile_bands: PercentileBands,
    pub terminal_values: TerminalStats,
    pub worst_sequence: WorstSequence,
}

/// Statistics on terminal (end-of-simulation) portfolio values.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStats {
    pub median: Decimal,
    pub mean: Decimal,
    pub p5: Decimal,
    pub p95: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    pub ruin_count: u32,
}

/// The single worst simulation run (for stress testing).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstSequence {
    pub seed_index: u32,
    pub portfolio_by_year: Vec<Decimal>,
    pub ruin_year: Option<u32>,
}

/// The Monte Carlo engine. Stateless — all config passed via SimulationConfig.
pub struct MonteCarloEngine;

impl MonteCarloEngine {
    /// Run simulation with a random seed.
    pub fn run(config: &SimulationConfig) -> common::Result<SimulationResult> {
        let seed = config.seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42)
        });
        Self::run_with_seed(config, seed)
    }

    /// Run simulation with a specific seed for reproducibility.
    pub fn run_with_seed(config: &SimulationConfig, seed: u64) -> common::Result<SimulationResult> {
        // Pre-compute Cholesky if AssetAllocation model
        let cholesky_l = match &config.return_model {
            ReturnModel::AssetAllocation { assets } => {
                let n = assets.len();
                let mut corr_matrix = vec![vec![Decimal::ZERO; n]; n];
                for i in 0..n {
                    for j in 0..n {
                        corr_matrix[i][j] = assets[i].correlation_row[j];
                    }
                }
                let l = cholesky_decompose(&corr_matrix).ok_or_else(|| {
                    common::KlyntbotError::Tool(
                        "Correlation matrix is not positive definite".to_string(),
                    )
                })?;
                Some(l)
            }
            _ => None,
        };

        let runs = config.runs;
        let years = config.years as usize;
        let one = Decimal::ONE;
        let zero = Decimal::ZERO;

        // Storage for all runs
        let mut all_terminal: Vec<Decimal> = Vec::with_capacity(runs as usize);
        let mut all_yearly: Vec<Vec<Decimal>> = Vec::with_capacity(runs as usize);
        let mut ruin_count: u32 = 0;
        let mut worst_terminal = Decimal::MAX;
        let mut worst_idx: u32 = 0;
        let mut worst_ruin_year: Option<u32> = None;
        let mut worst_yearly: Vec<Decimal> = Vec::new();

        for run_idx in 0..runs {
            let mut rng = create_rng(seed, run_idx);
            let mut portfolio = config.initial_portfolio;
            let mut yearly_values = Vec::with_capacity(years);
            let mut this_ruin_year: Option<u32> = None;

            for year in 0..years {
                // Step 1: Generate annual return
                let annual_return = match &config.return_model {
                    ReturnModel::LogNormal { mean_return, std_dev } => {
                        draw_log_normal(&mut rng, *mean_return, *std_dev)
                    }
                    ReturnModel::HistoricalBootstrap { returns } => {
                        draw_bootstrap(&mut rng, returns)
                    }
                    ReturnModel::AssetAllocation { assets } => {
                        let means: Vec<Decimal> = assets.iter().map(|a| a.mean_return).collect();
                        let std_devs: Vec<Decimal> = assets.iter().map(|a| a.std_dev).collect();
                        let returns = draw_correlated_returns(
                            &mut rng,
                            &means,
                            &std_devs,
                            cholesky_l.as_ref().unwrap(),
                        );
                        // Weighted sum
                        assets.iter().zip(returns.iter())
                            .map(|(a, r)| a.weight * *r)
                            .sum()
                    }
                };

                // Step 2: Generate inflation
                let inflation = match &config.inflation {
                    InflationModel::Fixed(rate) => *rate,
                    InflationModel::Variable { mean, std_dev } => {
                        draw_log_normal(&mut rng, *mean, *std_dev)
                    }
                };

                // Step 3: Apply return
                portfolio = portfolio * (one + annual_return);

                // Step 4: Apply contribution or withdrawal
                if config.annual_withdrawal > zero {
                    let withdrawal = match &config.withdrawal_strategy {
                        WithdrawalStrategy::FixedDollar(amount) => {
                            // Inflation-adjusted
                            let inflation_factor = (one + inflation).powu(year as u64 + 1);
                            *amount * inflation_factor
                        }
                        WithdrawalStrategy::FixedRate(rate) => portfolio * *rate,
                        WithdrawalStrategy::GuytonKlinger { initial_rate, ceiling_rate, floor_rate, capital_preservation_threshold } => {
                            // Simplified Guyton-Klinger: adjust rate based on portfolio performance
                            let base_withdrawal = config.initial_portfolio * *initial_rate;
                            let inflation_factor = (one + inflation).powu(year as u64 + 1);
                            let mut w = base_withdrawal * inflation_factor;

                            let current_rate = w / portfolio;
                            if current_rate > *initial_rate + *ceiling_rate {
                                w = w * (one - *floor_rate); // Cut
                            } else if current_rate < *initial_rate - *floor_rate {
                                if portfolio > config.initial_portfolio * *capital_preservation_threshold {
                                    w = w * (one + *ceiling_rate); // Raise
                                }
                            }
                            w
                        }
                        WithdrawalStrategy::VPW { age } => {
                            // Variable Percentage Withdrawal based on remaining years
                            let remaining_years = Decimal::new(100i64.saturating_sub(*age as i64 + year as i64), 0);
                            if remaining_years <= zero {
                                portfolio
                            } else {
                                portfolio / remaining_years
                            }
                        }
                    };
                    portfolio -= withdrawal;
                } else if config.annual_contribution > zero {
                    let inflation_factor = (one + inflation).powu(year as u64 + 1);
                    portfolio += config.annual_contribution * inflation_factor;
                }

                // Step 5: Floor at zero
                if portfolio < zero {
                    portfolio = zero;
                }

                yearly_values.push(portfolio);

                // Step 6: Check ruin — break immediately per spec
                if portfolio == zero && config.annual_withdrawal > zero {
                    this_ruin_year = Some(year as u32);
                    ruin_count += 1;
                    // Pad remaining years with zero for consistent percentile band lengths
                    for _ in (year + 1)..years {
                        yearly_values.push(zero);
                    }
                    break;
                }
            }

            let terminal = portfolio;
            all_terminal.push(terminal);
            all_yearly.push(yearly_values.clone());

            // Track worst run
            if terminal < worst_terminal || (terminal == zero && worst_ruin_year.map_or(true, |wy| this_ruin_year.map_or(false, |ry| ry < wy))) {
                worst_terminal = terminal;
                worst_idx = run_idx;
                worst_ruin_year = this_ruin_year;
                worst_yearly = yearly_values;
            }
        }

        // Sort terminal values for percentile computation
        all_terminal.sort();

        let runs_dec = Decimal::new(runs as i64, 0);
        let success_rate = one - Decimal::new(ruin_count as i64, 0) / runs_dec;

        let terminal_stats = TerminalStats {
            median: percentile_sorted(&all_terminal, 50),
            mean: all_terminal.iter().copied().sum::<Decimal>() / runs_dec,
            p5: percentile_sorted(&all_terminal, 5),
            p95: percentile_sorted(&all_terminal, 95),
            min: all_terminal.first().copied().unwrap_or(zero),
            max: all_terminal.last().copied().unwrap_or(zero),
            ruin_count,
        };

        // Build percentile bands per year
        let mut percentile_bands = PercentileBands {
            p5: Vec::with_capacity(years),
            p25: Vec::with_capacity(years),
            p50: Vec::with_capacity(years),
            p75: Vec::with_capacity(years),
            p95: Vec::with_capacity(years),
            survival_rate: Vec::with_capacity(years),
            labels: (0..years).map(|y| format!("Year {}", y + 1)).collect(),
        };

        for year_idx in 0..years {
            let mut year_values: Vec<Decimal> = all_yearly.iter()
                .map(|run| run[year_idx])
                .collect();
            year_values.sort();

            percentile_bands.p5.push(percentile_sorted(&year_values, 5));
            percentile_bands.p25.push(percentile_sorted(&year_values, 25));
            percentile_bands.p50.push(percentile_sorted(&year_values, 50));
            percentile_bands.p75.push(percentile_sorted(&year_values, 75));
            percentile_bands.p95.push(percentile_sorted(&year_values, 95));

            let alive = year_values.iter().filter(|v| **v > zero).count();
            percentile_bands.survival_rate.push(
                Decimal::new(alive as i64, 0) / runs_dec,
            );
        }

        Ok(SimulationResult {
            config_summary: ConfigSummary {
                runs,
                years: config.years,
                initial_portfolio: config.initial_portfolio,
                annual_withdrawal: config.annual_withdrawal,
            },
            success_rate,
            percentile_bands,
            terminal_values: terminal_stats,
            worst_sequence: WorstSequence {
                seed_index: worst_idx,
                portfolio_by_year: worst_yearly,
                ruin_year: worst_ruin_year,
            },
        })
    }
}

/// Get percentile from a pre-sorted slice.
fn percentile_sorted(sorted: &[Decimal], pct: u32) -> Decimal {
    if sorted.is_empty() {
        return Decimal::ZERO;
    }
    let idx = (sorted.len() as f64 * pct as f64 / 100.0).ceil() as usize;
    let idx = idx.min(sorted.len()).max(1) - 1;
    sorted[idx]
}
```

- [ ] **Step 4: Register monte_carlo module in lib.rs**

Update `crates/analytics/src/lib.rs` — uncomment `pub mod monte_carlo;` and add re-export:

```rust
pub mod monte_carlo;
pub use monte_carlo::{
    MonteCarloEngine, SimulationConfig, SimulationResult, ReturnModel,
    AssetClass, InflationModel, WithdrawalStrategy, TerminalStats, WorstSequence,
};
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p analytics 2>&1 | tail -5`
Expected: Compilation succeeds

- [ ] **Step 6: Commit**

```bash
git add crates/analytics/
git commit -m "feat(analytics): implement Monte Carlo engine with log-normal, bootstrap, and correlated asset models"
```

---

### Task 5: Monte Carlo Tests

**Files:**
- Create: `crates/analytics/tests/monte_carlo_tests.rs`

- [ ] **Step 1: Write comprehensive Monte Carlo tests**

Create `crates/analytics/tests/monte_carlo_tests.rs`:

```rust
use analytics::{
    MonteCarloEngine, SimulationConfig, ReturnModel, InflationModel, WithdrawalStrategy,
};
use rust_decimal_macros::dec;

fn base_config() -> SimulationConfig {
    SimulationConfig {
        runs: 1000,
        years: 30,
        initial_portfolio: dec!(1000000),
        annual_contribution: dec!(0),
        annual_withdrawal: dec!(40000),
        withdrawal_strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        seed: Some(42),
    }
}

#[test]
fn deterministic_with_same_seed() {
    let config = base_config();
    let r1 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let r2 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(r1.success_rate, r2.success_rate);
    assert_eq!(r1.terminal_values.median, r2.terminal_values.median);
    assert_eq!(r1.terminal_values.ruin_count, r2.terminal_values.ruin_count);
}

#[test]
fn different_seeds_produce_different_results() {
    let config = base_config();
    let r1 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let r2 = MonteCarloEngine::run_with_seed(&config, 999).unwrap();
    // Very unlikely to be exactly equal with different seeds
    assert_ne!(r1.terminal_values.median, r2.terminal_values.median);
}

#[test]
fn zero_portfolio_always_ruins() {
    let mut config = base_config();
    config.initial_portfolio = dec!(0);
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.success_rate, dec!(0));
    assert_eq!(result.terminal_values.ruin_count, 100);
}

#[test]
fn no_withdrawal_never_ruins() {
    let mut config = base_config();
    config.annual_withdrawal = dec!(0);
    config.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(0));
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.success_rate, dec!(1));
    assert_eq!(result.terminal_values.ruin_count, 0);
}

#[test]
fn higher_withdrawal_lower_success() {
    let low = {
        let mut c = base_config();
        c.annual_withdrawal = dec!(30000);
        c.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(30000));
        c.runs = 500;
        MonteCarloEngine::run_with_seed(&c, 42).unwrap()
    };
    let high = {
        let mut c = base_config();
        c.annual_withdrawal = dec!(80000);
        c.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(80000));
        c.runs = 500;
        MonteCarloEngine::run_with_seed(&c, 42).unwrap()
    };
    assert!(low.success_rate >= high.success_rate);
}

#[test]
fn percentile_bands_correct_length() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.percentile_bands.p50.len(), 30);
    assert_eq!(result.percentile_bands.labels.len(), 30);
    assert_eq!(result.percentile_bands.survival_rate.len(), 30);
}

#[test]
fn percentile_ordering() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    // p5 <= p25 <= p50 <= p75 <= p95 at each year
    for i in 0..result.percentile_bands.p50.len() {
        assert!(result.percentile_bands.p5[i] <= result.percentile_bands.p25[i]);
        assert!(result.percentile_bands.p25[i] <= result.percentile_bands.p50[i]);
        assert!(result.percentile_bands.p50[i] <= result.percentile_bands.p75[i]);
        assert!(result.percentile_bands.p75[i] <= result.percentile_bands.p95[i]);
    }
}

#[test]
fn terminal_stats_consistency() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert!(result.terminal_values.min <= result.terminal_values.p5);
    assert!(result.terminal_values.p5 <= result.terminal_values.median);
    assert!(result.terminal_values.median <= result.terminal_values.p95);
    assert!(result.terminal_values.p95 <= result.terminal_values.max);
}

#[test]
fn success_rate_derived_from_ruin_count() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let expected = dec!(1) - rust_decimal::Decimal::new(result.terminal_values.ruin_count as i64, 0)
        / rust_decimal::Decimal::new(config.runs as i64, 0);
    assert_eq!(result.success_rate, expected);
}

#[test]
fn bootstrap_model_works() {
    let mut config = base_config();
    config.return_model = ReturnModel::HistoricalBootstrap {
        returns: vec![dec!(0.10), dec!(-0.05), dec!(0.15), dec!(0.08), dec!(-0.10)],
    };
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert!(result.success_rate > dec!(0));
}

#[test]
fn contribution_mode_grows_portfolio() {
    let mut config = base_config();
    config.annual_withdrawal = dec!(0);
    config.annual_contribution = dec!(20000);
    config.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(0));
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    // After 30 years of 7% returns + contributions, median should exceed initial
    assert!(result.terminal_values.median > config.initial_portfolio);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p analytics 2>&1 | tail -15`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/analytics/tests/
git commit -m "test(analytics): add comprehensive Monte Carlo engine tests"
```

---

## Chunk 3: FIRE Calculator Suite

### Task 6: FIRE Variants (Traditional, Coast, Lean, Fat)

**Files:**
- Create: `crates/analytics/src/fire/mod.rs`
- Create: `crates/analytics/src/fire/variants.rs`
- Modify: `crates/analytics/src/lib.rs`
- Test: `crates/analytics/tests/fire_tests.rs`

- [ ] **Step 1: Write failing FIRE tests**

Create `crates/analytics/tests/fire_tests.rs`:

```rust
use analytics::fire::{FIRECalculator, FIREParams, CoastFIREParams};
use rust_decimal_macros::dec;

#[test]
fn fire_number_basic() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(0),
        monthly_savings: dec!(2000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert_eq!(result.fire_numbers[0].fire_number, dec!(1000000));
}

#[test]
fn fire_number_multiple_swr() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(3000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04), dec!(0.035), dec!(0.03)],
    });
    assert_eq!(result.fire_numbers.len(), 3);
    assert_eq!(result.fire_numbers[0].fire_number, dec!(1000000));     // 40k / 0.04
    assert!(result.fire_numbers[1].fire_number > dec!(1000000));        // 40k / 0.035
    assert!(result.fire_numbers[2].fire_number > result.fire_numbers[1].fire_number); // 40k / 0.03
}

#[test]
fn fire_already_reached() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(1500000),
        monthly_savings: dec!(3000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert_eq!(result.months_to_fire, Some(0));
    assert!(result.current_progress >= dec!(1));
}

#[test]
fn fire_zero_savings_unreachable() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(0),
        monthly_savings: dec!(0),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert!(result.months_to_fire.is_none());
}

#[test]
fn coast_fire_already_coasting() {
    let result = FIRECalculator::coast(&CoastFIREParams {
        current_portfolio: dec!(500000),
        current_age: 30,
        target_age: 65,
        annual_expenses_at_retirement: dec!(40000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    // At 30 with $500k and 35 years to grow at ~4% real return,
    // coast number should be well below $500k
    assert!(result.is_coast_fire);
    assert!(result.surplus_or_deficit > dec!(0));
}

#[test]
fn coast_fire_negative_real_return_unreachable() {
    let result = FIRECalculator::coast(&CoastFIREParams {
        current_portfolio: dec!(100000),
        current_age: 30,
        target_age: 65,
        annual_expenses_at_retirement: dec!(40000),
        expected_return: dec!(0.02),
        inflation_rate: dec!(0.05), // inflation > returns
        withdrawal_rate: dec!(0.04),
    });
    // With negative real returns, coast FIRE doesn't work
    assert!(!result.is_coast_fire);
    assert!(result.years_to_coast.is_none());
}

#[test]
fn lean_fire_uses_essentials_only() {
    let lean = FIRECalculator::lean(&analytics::fire::LeanFIREParams {
        essential_expenses: dec!(25000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(2000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    // Lean FIRE number = 25000 / 0.04 = 625000
    assert_eq!(lean.fire_numbers[0].fire_number, dec!(625000));
}

#[test]
fn fat_fire_uses_full_lifestyle() {
    let fat = FIRECalculator::fat(&analytics::fire::FatFIREParams {
        desired_annual_spending: dec!(100000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(5000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    // Fat FIRE number = 100000 / 0.04 = 2500000
    assert_eq!(fat.fire_numbers[0].fire_number, dec!(2500000));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p analytics -E 'test(fire)' 2>&1 | tail -10`
Expected: FAIL — `fire` module doesn't exist

- [ ] **Step 3: Implement FIRE calculator**

Create `crates/analytics/src/fire/mod.rs`:

```rust
//! FIRE (Financial Independence, Retire Early) calculator suite.

pub mod variants;
// pub mod withdrawal;     -- Task 7
// pub mod sequence_risk;  -- Task 7

pub use variants::*;
```

Create `crates/analytics/src/fire/variants.rs` with the full implementation of `FIRECalculator::traditional()`, `coast()`, `lean()`, `fat()`. See spec Section 4 for formulas. The implementation should use `Decimal` throughout, with the explicit months-to-fire formula from the spec.

The key formulas:
- `fire_number = annual_expenses / withdrawal_rate`
- `real_return = (1 + nominal) / (1 + inflation) - 1`
- `monthly_rate = (1 + real_return)^(1/12) - 1`
- `months_to_fire = ceil(ln((fire_number * r + pmt) / (pv * r + pmt)) / ln(1 + r))`
- Coast: `coast_number = fire_number / (1 + real_return)^years`

Use `Decimal::ln()` and `Decimal::powu()` from `rust_decimal` (the `maths` feature was already enabled in Task 1).

- [ ] **Step 4: Register fire module in lib.rs**

Add `pub mod fire;` to `crates/analytics/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p analytics -E 'test(fire)' 2>&1 | tail -15`
Expected: All FIRE tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/analytics/src/fire/ crates/analytics/tests/fire_tests.rs
git commit -m "feat(analytics): implement FIRE calculators (Traditional, Coast, Lean, Fat)"
```

---

### Task 7: Withdrawal Simulation & Historical Backtesting

**Files:**
- Create: `crates/analytics/src/fire/withdrawal.rs`
- Create: `crates/analytics/src/fire/sequence_risk.rs`
- Create: `crates/analytics/src/data/us_stock_returns_1928_2024.csv`
- Create: `crates/analytics/src/data/us_bond_returns_1928_2024.csv`
- Create: `crates/analytics/src/data/us_inflation_1928_2024.csv`
- Modify: `crates/analytics/src/fire/mod.rs`
- Test: append to `crates/analytics/tests/fire_tests.rs`

- [ ] **Step 1: Write failing withdrawal simulation tests**

Append to `crates/analytics/tests/fire_tests.rs`:

```rust
use analytics::fire::{WithdrawalParams, WithdrawalResult};
use analytics::{WithdrawalStrategy, ReturnModel, InflationModel};

#[test]
fn withdrawal_sim_high_success_with_low_rate() {
    let result = FIRECalculator::withdrawal_simulation(&WithdrawalParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(30000), // 3% rate
        strategy: WithdrawalStrategy::FixedDollar(dec!(30000)),
        years: 30,
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 1000,
        seed: Some(42),
    });
    assert!(result.success_rate > dec!(0.90));
}

#[test]
fn withdrawal_sim_immediate_ruin() {
    let result = FIRECalculator::withdrawal_simulation(&WithdrawalParams {
        portfolio: dec!(100),
        annual_withdrawal: dec!(50000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(50000)),
        years: 30,
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 100,
        seed: Some(42),
    });
    assert_eq!(result.success_rate, dec!(0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p analytics -E 'test(withdrawal)' 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement withdrawal simulation**

Create `crates/analytics/src/fire/withdrawal.rs` — this delegates to `MonteCarloEngine::run_with_seed()` by converting `WithdrawalParams` to `SimulationConfig`.

- [ ] **Step 4: Create embedded historical data CSVs**

Create the three CSV files in `crates/analytics/src/data/` with annual return data from 1928-2024. Format: `year,return` where return is a decimal (e.g., `1928,0.4381` for 43.81% nominal S&P 500 return in 1928).

Source: Robert Shiller's publicly available dataset.

- [ ] **Step 5: Implement historical backtesting**

Create `crates/analytics/src/fire/sequence_risk.rs` with `FIRECalculator::historical_backtest()` that loads embedded CSV data and runs rolling-window backtests.

- [ ] **Step 6: Write backtest tests**

Append to fire_tests.rs:

```rust
use analytics::fire::HistoricalBacktestParams;

#[test]
fn backtest_loads_embedded_data() {
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    assert!(result.total_periods > 0);
    assert!(result.success_rate > dec!(0));
}
```

- [ ] **Step 7: Run all FIRE tests**

Run: `cargo nextest run -p analytics -E 'test(fire)' 2>&1 | tail -15`
Expected: All PASS

- [ ] **Step 8: Commit**

```bash
git add crates/analytics/src/fire/ crates/analytics/src/data/ crates/analytics/tests/fire_tests.rs
git commit -m "feat(analytics): add withdrawal simulation and historical backtesting with embedded Shiller data"
```

---

### Task 8: Sensitivity Framework

**Files:**
- Modify: `crates/analytics/src/fire/mod.rs`
- Create: `crates/analytics/tests/sensitivity_tests.rs`

- [ ] **Step 1: Write failing sensitivity tests**

Create `crates/analytics/tests/sensitivity_tests.rs`:

```rust
use analytics::fire::{FIRECalculator, SensitivityConfig};
use analytics::{WithdrawalStrategy, ReturnModel, InflationModel};
use analytics::fire::WithdrawalParams;
use rust_decimal_macros::dec;

#[test]
fn sensitivity_grid_dimensions() {
    let base = WithdrawalParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
        return_model: ReturnModel::LogNormal { mean_return: dec!(0.07), std_dev: dec!(0.15) },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 1000,
        seed: Some(42),
    };
    let config = SensitivityConfig { runs_per_point: 100, seed: Some(42) };
    let rates = vec![dec!(0.03), dec!(0.04), dec!(0.05)];
    let returns = vec![dec!(0.05), dec!(0.07)];
    let result = FIRECalculator::sensitivity_withdrawal_vs_return(&base, &rates, &returns, &config);
    assert_eq!(result.grid.len(), 3); // 3 withdrawal rates
    assert_eq!(result.grid[0].len(), 2); // 2 return rates
}

#[test]
fn sensitivity_lower_withdrawal_higher_success() {
    let base = WithdrawalParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
        return_model: ReturnModel::LogNormal { mean_return: dec!(0.07), std_dev: dec!(0.15) },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 1000,
        seed: Some(42),
    };
    let config = SensitivityConfig { runs_per_point: 200, seed: Some(42) };
    let rates = vec![dec!(0.03), dec!(0.05)];
    let returns = vec![dec!(0.07)];
    let result = FIRECalculator::sensitivity_withdrawal_vs_return(&base, &rates, &returns, &config);
    // 3% withdrawal rate should have higher success than 5%
    assert!(result.grid[0][0].success_rate >= result.grid[1][0].success_rate);
}
```

- [ ] **Step 2: Implement sensitivity functions**

Add `SensitivityConfig`, `SensitivityResult`, `SensitivityPoint` types and `FIRECalculator::sensitivity_withdrawal_vs_return()` / `sensitivity_savings_vs_timeline()` to the fire module.

These iterate over the grid, run reduced Monte Carlo (using `config.runs_per_point` instead of full 10,000), and collect results.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(sensitivity)' 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/fire/ crates/analytics/tests/sensitivity_tests.rs
git commit -m "feat(analytics): add sensitivity analysis framework for FIRE parameter sweeps"
```

---

## Chunk 4: Spending Analytics

### Task 9: Anomaly Detection

**Files:**
- Create: `crates/analytics/src/spending/mod.rs`
- Create: `crates/analytics/src/spending/anomaly.rs`
- Modify: `crates/analytics/src/lib.rs`
- Test: `crates/analytics/tests/spending_tests.rs`

- [ ] **Step 1: Write failing anomaly tests**

Create `crates/analytics/tests/spending_tests.rs`:

```rust
use analytics::spending::SpendingAnalyzer;
use analytics::{SpendingRecord, SpendingType, AnomalyDirection};
use chrono::NaiveDate;
use rust_decimal_macros::dec;

fn monthly_expenses(category: &str, amounts: &[i64]) -> Vec<SpendingRecord> {
    amounts.iter().enumerate().map(|(i, &amt)| SpendingRecord {
        date: NaiveDate::from_ymd_opt(2025, (i as u32 % 12) + 1, 15).unwrap(),
        amount: rust_decimal::Decimal::new(amt, 0),
        tx_type: SpendingType::Expense,
        category: Some(category.to_string()),
        counterparty: None,
    }).collect()
}

#[test]
fn anomaly_spike_detected() {
    let txs = monthly_expenses("groceries", &[200, 190, 210, 195, 205, 2000]);
    let config = analytics::spending::AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0].category, "groceries");
    assert!(anomalies[0].z_score > dec!(2.5));
}

#[test]
fn anomaly_insufficient_data_skipped() {
    let txs = monthly_expenses("groceries", &[200, 190]);
    let config = analytics::spending::AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(anomalies.is_empty());
}

#[test]
fn anomaly_zero_mad_handles_gracefully() {
    let txs = monthly_expenses("rent", &[1500, 1500, 1500, 1500, 1500, 1500]);
    let config = analytics::spending::AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(anomalies.is_empty()); // all identical, no anomaly
}

#[test]
fn anomaly_zero_mad_with_deviation() {
    let txs = monthly_expenses("rent", &[1500, 1500, 1500, 1500, 1500, 3000]);
    let config = analytics::spending::AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert_eq!(anomalies.len(), 1); // deviation from constant = high severity
}

#[test]
fn anomaly_drops_only_mode() {
    let txs = monthly_expenses("groceries", &[200, 190, 210, 195, 205, 50]);
    let config = analytics::spending::AnomalyConfig {
        direction: AnomalyDirection::DropsOnly,
        ..Default::default()
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert_eq!(anomalies.len(), 1);
    assert!(anomalies[0].z_score < dec!(0)); // negative z-score for drop
}
```

- [ ] **Step 2: Implement anomaly detection**

Create `crates/analytics/src/spending/anomaly.rs` with `AnomalyConfig` and `SpendingAnalyzer::detect_anomalies()` using modified z-score algorithm from spec Section 5.

Create `crates/analytics/src/spending/mod.rs`:
```rust
pub mod anomaly;
pub use anomaly::{AnomalyConfig, SpendingAnalyzer};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(anomaly)' 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/spending/
git commit -m "feat(analytics): implement spending anomaly detection with modified z-score"
```

---

### Task 10: Trend Analysis & Recurring Charge Detection

**Files:**
- Create: `crates/analytics/src/spending/trends.rs`
- Create: `crates/analytics/src/spending/recurring.rs`
- Modify: `crates/analytics/src/spending/mod.rs`
- Test: append to `crates/analytics/tests/spending_tests.rs`

- [ ] **Step 1: Write failing trend and recurring tests**

Append to spending_tests.rs — tests for `SpendingAnalyzer::trends()` (TrendConfig, TrendReport, TrendDirection) and `SpendingAnalyzer::detect_recurring()` (monthly Netflix-style charges, confidence scoring, annual_cost calculation, is_overdue flag).

- [ ] **Step 2: Implement trends and recurring detection**

Create `crates/analytics/src/spending/trends.rs` — moving averages, period-over-period growth rates, TrendDirection classification.

Create `crates/analytics/src/spending/recurring.rs` — group by counterparty, compute inter-transaction intervals, cluster into frequency buckets, score confidence.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(spending)' 2>&1 | tail -15`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/spending/
git commit -m "feat(analytics): add spending trend analysis and recurring charge detection"
```

---

### Task 11: Category Correlation

**Files:**
- Create: `crates/analytics/src/spending/correlation.rs`
- Modify: `crates/analytics/src/spending/mod.rs`
- Test: append to `crates/analytics/tests/spending_tests.rs`

- [ ] **Step 1: Write failing correlation tests**

Tests for `SpendingAnalyzer::category_correlation()` — Pearson correlation on monthly category totals, symmetric matrix, diagonal = 1.0, values in [-1, 1], empty result for single category. Include a proptest for matrix symmetry invariant.

- [ ] **Step 2: Implement category correlation**

Create `crates/analytics/src/spending/correlation.rs` — group transactions by (month, category), compute monthly totals, calculate Pearson correlation between all category pairs with >= min_months of data.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(correlation) & test(spending)' 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/spending/
git commit -m "feat(analytics): add category spending correlation analysis"
```

---

## Chunk 5: Portfolio Analytics

### Task 12: Allocation Drift & Rebalancing

**Files:**
- Create: `crates/analytics/src/portfolio/mod.rs`
- Create: `crates/analytics/src/portfolio/drift.rs`
- Modify: `crates/analytics/src/lib.rs`
- Test: `crates/analytics/tests/portfolio_tests.rs`

- [ ] **Step 1: Write failing drift and rebalancing tests**

Create `crates/analytics/tests/portfolio_tests.rs` — tests for:
- `PortfolioAnalyzer::allocation_drift()` — drift computation, needs_rebalancing flag, drift_score
- `PortfolioAnalyzer::rebalance_suggestions()` — FullRebalance (generates buy+sell), ContributionOnly (buy only), ThresholdOnly, min_trade_amount filter, to_weight sum invariant
- Single holding → drift = 0
- Proptest: drift sums to zero across asset classes

- [ ] **Step 2: Implement drift detection and rebalancing**

Create `crates/analytics/src/portfolio/drift.rs` with `PortfolioAnalyzer::allocation_drift()` and `rebalance_suggestions()`.

Create `crates/analytics/src/portfolio/mod.rs`:
```rust
pub mod drift;
pub use drift::PortfolioAnalyzer;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(portfolio)' 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/portfolio/
git commit -m "feat(analytics): implement portfolio allocation drift and rebalancing suggestions"
```

---

### Task 13: Returns Analysis (TWR/MWR)

**Files:**
- Create: `crates/analytics/src/portfolio/returns.rs`
- Modify: `crates/analytics/src/portfolio/mod.rs`
- Test: append to `crates/analytics/tests/portfolio_tests.rs`

- [ ] **Step 1: Write failing returns tests**

Tests for TWR (Modified Dietz method), MWR (IRR via Newton's method), per-holding attribution, annualized return (CAGR). Include known-value tests from financial textbooks.

- [ ] **Step 2: Implement returns analysis**

Create `crates/analytics/src/portfolio/returns.rs` with `PortfolioAnalyzer::returns()`.

Modified Dietz TWR: `sub_return = (end - start - cf) / (start + weighted_cf)`, then chain `TWR = product(1 + r_i) - 1`.

MWR via Newton's method: solve for `r` in `sum(cf_i / (1+r)^t_i) = 0`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p analytics -E 'test(returns)' 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/analytics/src/portfolio/returns.rs
git commit -m "feat(analytics): implement TWR and MWR portfolio returns analysis"
```

---

### Task 14: Asset Correlation

**Files:**
- Create: `crates/analytics/src/portfolio/correlation.rs`
- Modify: `crates/analytics/src/portfolio/mod.rs`
- Test: append to `crates/analytics/tests/portfolio_tests.rs`

- [ ] **Step 1: Write failing asset correlation tests**

Tests for `PortfolioAnalyzer::asset_correlation()` — uses monthly returns (not prices), symmetric matrix, diagonal = 1.0, min_overlap filter.

- [ ] **Step 2: Implement asset correlation**

Create `crates/analytics/src/portfolio/correlation.rs`. Same Pearson correlation logic as spending correlation but on price returns instead of spending amounts.

- [ ] **Step 3: Run tests and commit**

Run: `cargo nextest run -p analytics -E 'test(portfolio)' 2>&1 | tail -10`
Expected: All PASS

```bash
git add crates/analytics/src/portfolio/
git commit -m "feat(analytics): add asset price correlation analysis"
```

---

## Chunk 6: Property-Based Tests & Benchmarks

### Task 15: Property-Based Tests (proptest)

**Files:**
- Modify: `crates/analytics/tests/fire_tests.rs`
- Modify: `crates/analytics/tests/portfolio_tests.rs`
- Modify: `crates/analytics/tests/spending_tests.rs`

- [ ] **Step 1: Add proptest invariants**

Add to fire_tests.rs:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn fire_number_is_expenses_over_rate(
        expenses in 1000i64..1_000_000,
        rate in 1u32..20
    ) {
        let e = Decimal::new(expenses, 0);
        let r = Decimal::new(rate as i64, 2);
        let result = FIRECalculator::traditional(&FIREParams {
            annual_expenses: e,
            withdrawal_rates: vec![r],
            ..Default::default()
        });
        let expected = e / r;
        prop_assert_eq!(result.fire_numbers[0].fire_number, expected);
    }
}
```

Add to portfolio_tests.rs — drift sums to zero, correlation matrix symmetric with diagonal = 1.0, coefficients in [-1, 1].

- [ ] **Step 2: Run all proptest tests**

Run: `cargo nextest run -p analytics 2>&1 | tail -15`
Expected: All PASS (proptest generates many random cases)

- [ ] **Step 3: Commit**

```bash
git add crates/analytics/tests/
git commit -m "test(analytics): add property-based tests for FIRE, portfolio, and spending invariants"
```

---

### Task 16: Benchmark Validation Tests

**Files:**
- Create: `crates/analytics/tests/benchmarks/cfiresim_validation.rs`
- Create: `crates/analytics/tests/benchmarks/trinity_validation.rs`

- [ ] **Step 1: Write benchmark validation tests**

These compare against known results from cFIREsim and Trinity Study. Marked with `#[ignore]` so they don't run in normal CI.

**Important:** Rust doesn't auto-discover tests in subdirectories. Add to `crates/analytics/Cargo.toml`:

```toml
[[test]]
name = "cfiresim_validation"
path = "tests/benchmarks/cfiresim_validation.rs"

[[test]]
name = "trinity_validation"
path = "tests/benchmarks/trinity_validation.rs"
```

```rust
#[test]
#[ignore] // Run manually: cargo nextest run -p analytics -E 'test(benchmark_)'
fn benchmark_4pct_rule_30yr_historical() {
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    // cFIREsim reports ~95-96% success rate for 100% stocks, 30-year, 4% SWR
    assert!(result.success_rate > dec!(0.90), "Expected ~95%, got {}", result.success_rate);
    assert!(result.success_rate < dec!(1.00));
}
```

- [ ] **Step 2: Run benchmark tests manually**

Run: `cargo nextest run -p analytics -E 'test(benchmark)' --run-ignored all 2>&1 | tail -15`
Expected: PASS within tolerance

- [ ] **Step 3: Commit**

```bash
git add crates/analytics/tests/benchmarks/
git commit -m "test(analytics): add benchmark validation tests against cFIREsim and Trinity Study"
```

---

## Chunk 7: Schema & Storage Layer

### Task 17: Database Migration

**Files:**
- Modify: `crates/feature-finance/migrations/001_finance_tables.sql`

- [ ] **Step 1: Update migration SQL**

Since we're pre-release (no user data), consolidate changes into the existing migration. Add to `001_finance_tables.sql`:

1. Change `finance_investments.quantity` from `REAL` to `TEXT`
2. Add `finance_investments.asset_class TEXT` column
3. Add `finance_allocation_targets` table (with TEXT for weights, not REAL)
4. Add `finance_net_worth_snapshots` table with indexes

```sql
-- Allocation targets
CREATE TABLE IF NOT EXISTS finance_allocation_targets (
    id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_class TEXT NOT NULL,
    target_weight TEXT NOT NULL,
    tolerance_band TEXT NOT NULL DEFAULT '0.05',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(portfolio_id, asset_class)
);

-- Net worth snapshots
CREATE TABLE IF NOT EXISTS finance_net_worth_snapshots (
    id TEXT PRIMARY KEY,
    snapshot_date TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    accounts_total INTEGER NOT NULL,
    investments_total INTEGER NOT NULL,
    liabilities_total INTEGER NOT NULL,
    net_worth INTEGER NOT NULL,
    breakdown TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(snapshot_date, currency)
);
CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_date ON finance_net_worth_snapshots(snapshot_date);
CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_currency_date ON finance_net_worth_snapshots(currency, snapshot_date);
```

**For the `finance_investments` table** — SQLite does not support `ALTER COLUMN` for type changes. Since we're pre-release (no user data to preserve), modify the `CREATE TABLE` statement in-place: change `quantity REAL NOT NULL` to `quantity TEXT NOT NULL` and add `asset_class TEXT` as a new column. This is safe because the migration is idempotent and no production data exists.

- [ ] **Step 2: Keep migration at version 1 (in-place update)**

Per CLAUDE.md: "Pre-release — all schema changes can be made directly. When a migration is consolidated, update the `FeatureMigration` version and SQL in-place." Keep version 1, update the SQL in-place. Any dev databases should be deleted and recreated (no data to preserve).

- [ ] **Step 3: Verify migration runs**

Run: `cargo nextest run -p feature-finance -E 'test(feature_package)' 2>&1 | tail -10`
Expected: PASS — migration SQL is idempotent

- [ ] **Step 4: Commit**

```bash
git add crates/feature-finance/migrations/ crates/feature-finance/src/lib.rs
git commit -m "feat(finance): add allocation_targets and net_worth_snapshots tables, change quantity to TEXT"
```

---

### Task 18: New Row Structs & Repos

**Files:**
- Modify: `crates/storage/src/rows/finance.rs`
- Create: `crates/storage/src/repos/finance_allocation_repo.rs`
- Create: `crates/storage/src/repos/finance_snapshot_repo.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/finance_storage.rs`

- [ ] **Step 1: Add Row structs**

Add to `crates/storage/src/rows/finance.rs`:

```rust
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAllocationTargetRow {
    pub id: String,
    pub portfolio_id: String,
    pub asset_class: String,
    pub target_weight: String,   // Decimal as TEXT
    pub tolerance_band: String,  // Decimal as TEXT
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceNetWorthSnapshotRow {
    pub id: String,
    pub snapshot_date: String,
    pub currency: String,
    pub accounts_total: i64,
    pub investments_total: i64,
    pub liabilities_total: i64,
    pub net_worth: i64,
    pub breakdown: String,  // JSON
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Implement repos**

Create `crates/storage/src/repos/finance_allocation_repo.rs` with CRUD operations (add, list_by_portfolio, update, delete). Use `RETURNING *` pattern.

Create `crates/storage/src/repos/finance_snapshot_repo.rs` with add + list_by_date_range.

- [ ] **Step 3: Register repos in FinanceStorage**

Add `allocation_targets: FinanceAllocationRepo` and `snapshots: FinanceSnapshotRepo` to `FinanceStorage` struct in `crates/storage/src/finance_storage.rs`.

- [ ] **Step 4: Add _with_tx variants for transaction atomicity**

Add `adjust_balance_with_tx()`, `add_with_tx()` to `FinanceAccountRepo` and `FinanceTransactionRepo`. These accept `&mut sqlx::Transaction<'_, Sqlite>` instead of `&SqlitePool`.

- [ ] **Step 5: Run storage tests**

Run: `cargo nextest run -p storage 2>&1 | tail -15`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add allocation target and net worth snapshot repos, add _with_tx variants"
```

---

## Chunk 8: Feature-Finance Integration

### Task 19: Analytics Integration Setup

**Files:**
- Modify: `crates/feature-finance/Cargo.toml`
- Modify: `crates/feature-finance/src/tool/mod.rs`
- Modify: `crates/feature-finance/src/lib.rs`

- [ ] **Step 1: Add analytics dependency**

In `crates/feature-finance/Cargo.toml`, add:
```toml
analytics.workspace = true
```

- [ ] **Step 2: Update FinanceTool struct to hold analytics context**

The analytics crate is stateless (no struct state needed), but `FinanceTool` needs access to `storage::FinanceStorage` for data retrieval (already has it) and the ability to convert Row types to analytics input types.

No FinanceTool struct changes needed — analytics functions are called inline in handlers.

- [ ] **Step 3: Add new dispatch arms to Tool::execute()**

In `crates/feature-finance/src/tool/mod.rs`, add **explicit** match arms to the `execute()` match (do NOT use `starts_with` — follow existing pattern of exact string matches):

```rust
// Spending analytics
"analyze_spending_anomalies" | "analyze_spending_trends"
| "analyze_recurring_charges" | "analyze_category_correlation"
    => self.handle_analyze(action, p, ctx).await,

// FIRE planning
"fire_traditional" | "fire_coast" | "fire_lean" | "fire_fat"
| "fire_withdrawal_sim" | "fire_backtest" | "fire_sensitivity"
    => self.handle_fire(action, p, ctx).await,

// Allocation targets
"allocation_target_set" | "allocation_target_list"
    => self.handle_allocation(action, p, ctx).await,

// Net worth snapshots
"snapshot_record" | "snapshot_history"
    => self.handle_snapshot(action, p, ctx).await,
```

Add module declarations (avoid shadowing the `analytics` crate name):
```rust
mod analyze_handlers;
mod fire_handlers;
mod allocations;
mod snapshots;
```

Also update the `description()` string and `parameters()` schema to enumerate all 60 actions (41 existing + 19 new). The new actions must appear in the JSON schema's `action` enum so MCP clients can discover them via `list_tools`.

- [ ] **Step 4: Verify it compiles (handlers are stubs)**

Create stub files for each new module with `todo!()` implementations:

```rust
// crates/feature-finance/src/tool/analyze_handlers.rs
impl FinanceTool {
    pub(crate) async fn handle_analyze(&self, action: &str, p: ParamExtractor<'_>, ctx: &RoutingContext) -> Result<String> {
        todo!("implement analytics handlers")
    }
}
```

Similar for fire_handlers.rs, allocations.rs, snapshots.rs.

Run: `cargo check -p feature-finance 2>&1 | tail -5`
Expected: Compiles (stubs are valid)

- [ ] **Step 5: Commit**

```bash
git add crates/feature-finance/
git commit -m "feat(finance): add analytics dispatch arms with stub handlers"
```

---

### Task 20: Spending Analytics Handlers

**Files:**
- Modify: `crates/feature-finance/src/tool/analyze_handlers.rs`

- [ ] **Step 1: Implement handle_analyze()**

Replace the stub with actual dispatch:

```rust
impl FinanceTool {
    pub(crate) async fn handle_analyze(&self, action: &str, p: ParamExtractor<'_>, ctx: &RoutingContext) -> Result<String> {
        match action {
            "analyze_spending_anomalies" => self.analyze_spending_anomalies(p).await,
            "analyze_spending_trends" => self.analyze_spending_trends(p).await,
            "analyze_recurring_charges" => self.analyze_recurring_charges(p).await,
            "analyze_category_correlation" => self.analyze_category_correlation(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown analyze action: {action}")).into()),
        }
    }
}
```

Each method:
1. Extracts params (lookback_months, period, etc.)
2. Fetches transactions from `self.storage.transactions`
3. Converts `FinanceTransactionRow` → `SpendingRecord`
4. Calls the analytics function
5. Serializes result to JSON string

- [ ] **Step 2: Run integration test**

Write a quick smoke test in `crates/feature-finance/tests/` that creates a FinanceTool via `for_tests()`, adds transactions, then calls `execute()` with `action: "analyze_spending_anomalies"`.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-finance/src/tool/analyze_handlers.rs
git commit -m "feat(finance): implement spending analytics tool handlers"
```

---

### Task 21: FIRE Tool Handlers

**Files:**
- Modify: `crates/feature-finance/src/tool/fire_handlers.rs`

- [ ] **Step 1: Implement handle_fire()**

Dispatch: `fire_traditional`, `fire_coast`, `fire_lean`, `fire_fat`, `fire_withdrawal_sim`, `fire_backtest`, `fire_sensitivity`.

Each method:
1. Extracts params from ParamExtractor
2. For `fire_traditional`: queries storage directly (account balances + investment values - liabilities) to compute current portfolio value. Does NOT call `handle_goal("net_worth")` recursively — reuse the same storage query logic but inline. Optionally fetches annual expenses from last 12 months of transactions via `self.storage.transactions.sum_by_category()`.
3. Calls `analytics::fire::FIRECalculator::traditional()` with the gathered data
4. Serializes `FIREResult` to JSON string

- [ ] **Step 2: Commit**

```bash
git add crates/feature-finance/src/tool/fire_handlers.rs
git commit -m "feat(finance): implement FIRE planning tool handlers"
```

---

### Task 22: Portfolio Analytics Handlers

**Files:**
- Modify: `crates/feature-finance/src/tool/investments/mod.rs`

- [ ] **Step 1: Add portfolio analytics match arms**

Add to the existing `handle_investment()` function:

```rust
"portfolio_drift" => self.portfolio_drift(p).await,
"portfolio_rebalance" => self.portfolio_rebalance(p).await,
"portfolio_returns" => self.portfolio_returns(p).await,
"portfolio_correlation" => self.portfolio_correlation(p).await,
```

Each method fetches holdings from storage, converts to analytics `Holding` type, calls `PortfolioAnalyzer`, serializes result.

- [ ] **Step 2: Implement allocation target CRUD**

Fill in `crates/feature-finance/src/tool/allocations.rs`:
- `allocation_target_set`: validate weights, upsert via repo
- `allocation_target_list`: fetch by portfolio_id

- [ ] **Step 3: Commit**

```bash
git add crates/feature-finance/src/tool/investments/ crates/feature-finance/src/tool/allocations.rs
git commit -m "feat(finance): implement portfolio analytics and allocation target tool handlers"
```

---

### Task 23: Snapshot Handlers & Atomicity Fix

**Files:**
- Modify: `crates/feature-finance/src/tool/snapshots.rs`
- Modify: `crates/feature-finance/src/tool/transactions/mod.rs`
- Modify: `crates/feature-finance/src/tool/transactions/transfer.rs`

- [ ] **Step 1: Implement snapshot handlers**

Fill in `crates/feature-finance/src/tool/snapshots.rs`:
- `snapshot_record`: compute current net worth (accounts + investments - liabilities), store as snapshot
- `snapshot_history`: query snapshots by date range

- [ ] **Step 2: Fix atomicity in transaction handlers**

In `crates/feature-finance/src/tool/transactions/mod.rs`:
- Wrap `tx_add` (add + adjust_balance) in `pool.begin()` / `tx.commit()`
- Wrap `tx_delete` (delete + reverse balance) similarly

In `crates/feature-finance/src/tool/transactions/transfer.rs`:
- Wrap `tx_add_transfer` (2 adds + 2 balance adjustments) in a single transaction

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run -p feature-finance 2>&1 | tail -15`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/feature-finance/src/tool/
git commit -m "feat(finance): add snapshot handlers, fix transaction atomicity with sqlx transactions"
```

---

## Chunk 9: Skill Updates

### Task 24: Internal Agent Skill Updates

**Files:**
- Modify: `skills/finance-management/SKILL.md`
- Create: `skills/finance-management/references/analytics-actions.md`
- Create: `skills/finance-management/references/fire-planning.md`
- Create: `skills/finance-management/references/portfolio-analysis.md`
- Create: `skills/finance-management/references/spending-intelligence.md`
- Modify: `skills/finance-management/references/budgeting.md`
- Delete: `skills/finance-management/references/spending-analysis.md`

- [ ] **Step 1: Update SKILL.md frontmatter triggers**

Append 38 new triggers to the `triggers:` YAML array as specified in the spec.

- [ ] **Step 2: Update SKILL.md decision flowchart**

Add 4 new rows to the decision table (Steps 2a-2d) for FIRE, spending analytics, portfolio analytics, and snapshots — as specified in the spec.

- [ ] **Step 3: Update cross-reference from spending-analysis to spending-intelligence**

Replace `references/spending-analysis.md` reference in SKILL.md body with `references/spending-intelligence.md`.

- [ ] **Step 4: Create new reference files**

Create the 4 reference files with content from the spec's Skill Updates section:
- `analytics-actions.md` — action routing table for all 19 new actions
- `fire-planning.md` — 5-step guided FIRE workflow
- `portfolio-analysis.md` — 5-step portfolio analysis workflow
- `spending-intelligence.md` — 4 workflow patterns (proactive, deep dive, anomaly, subscription audit)

- [ ] **Step 5: Update budgeting.md with cross-references**

Add the 3 specific cross-references from the spec: budget_status → anomalies, report_spending → trends/correlation, first-time setup → recurring charges.

- [ ] **Step 6: Do NOT delete spending-analysis.md yet**

The file is referenced via `include_str!` in `crates/skill-system/src/discovery.rs`. Deleting it before Task 26 updates discovery.rs will cause a compile error. The deletion happens in Task 26.

- [ ] **Step 7: Commit**

```bash
git add skills/finance-management/
git commit -m "feat(skills): upgrade finance-management skill with analytics workflows and 38 new triggers"
```

---

### Task 25: Claude Code Skill Updates

**Files:**
- Modify: `.claude/skills/klyntbot-finance/SKILL.md`
- Modify: `.claude/skills/klyntbot-finance/references/actions.md`

- [ ] **Step 1: Update SKILL.md quick reference table**

Expand from 7 rows to ~15 with key analytical actions (fire_traditional, fire_withdrawal_sim, analyze_spending_anomalies, portfolio_drift, snapshot_record, etc.).

Add common mistakes section for analytics. Add workflow tip about FIRE chaining.

- [ ] **Step 2: Update actions.md with all 19 new actions**

Add new sections: "Spending Analytics", "FIRE Planning", "Portfolio Analytics", "Allocation Targets", "Snapshots". Each action gets: name, parameters (with types), description, example call.

- [ ] **Step 3: Commit**

```bash
git add .claude/skills/klyntbot-finance/
git commit -m "feat(skills): update Claude Code finance skill with 19 new analytical actions"
```

---

### Task 26: Update Skill System Discovery

**Files:**
- Modify: `crates/skill-system/src/discovery.rs`

- [ ] **Step 1: Register new reference files in BUILTIN_SKILL_REFERENCES**

The skill system compiles reference files into the binary via the `include_skill_reference!` macro. Check the exact macro signature in `crates/skill-system/src/discovery.rs` and follow it. Add the 4 new entries:

```rust
include_skill_reference!("finance-management", "analytics-actions"),
include_skill_reference!("finance-management", "fire-planning"),
include_skill_reference!("finance-management", "portfolio-analysis"),
include_skill_reference!("finance-management", "spending-intelligence"),
```

Remove the old `spending-analysis` entry:
```rust
// REMOVE this line:
include_skill_reference!("finance-management", "spending-analysis"),
```

- [ ] **Step 2: Delete the old spending-analysis.md file**

Now that discovery.rs no longer references it:

```bash
rm skills/finance-management/references/spending-analysis.md
```

- [ ] **Step 3: Verify skill system compiles**

Run: `cargo check -p skill-system 2>&1 | tail -5`
Expected: Compiles (new files exist, old file removed, discovery.rs updated)

- [ ] **Step 4: Commit**

```bash
git add crates/skill-system/ skills/finance-management/references/
git commit -m "feat(skill-system): register new finance analytics references, remove spending-analysis"
```

---

## Chunk 10: Final Verification

### Task 27: Full Build & Test

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: Clean build, no warnings

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10`
Expected: Zero warnings

- [ ] **Step 3: Run formatting check**

Run: `cargo fmt --all --check 2>&1 | tail -5`
Expected: No formatting issues

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All tests PASS

- [ ] **Step 5: Run doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -10`
Expected: All PASS

- [ ] **Step 6: Commit any final fixes**

If clippy or tests revealed issues, fix and commit with descriptive messages.

---

### Task 28: Integration Smoke Test

- [ ] **Step 1: Build MCP server and test finance tool discovery**

Run: `cargo build -p klyntbot-server 2>&1 | tail -5`
Expected: Builds successfully

- [ ] **Step 2: Verify all 60 actions appear in tool schema**

Run: `cargo nextest run -p klyntbot-server 2>&1 | tail -10`
Expected: PASS — the `list_tools` test confirms finance tool with all actions is exposed

- [ ] **Step 3: Final commit**

```bash
git commit --allow-empty -m "chore(finance): analytical engine implementation complete — 19 new actions, analytics crate, upgraded skills"
```
