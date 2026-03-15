# Finance Currency Engine — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add auto-converting multi-currency support to the finance system — store original + base-currency equivalent on every monetary record, backed by a DB-cached exchange rate service.

**Architecture:** Upgrade all finance tables with `base_amount/base_currency/exchange_rate` columns. Enhance `PriceService` with a SQLite-backed rate cache (L1 in-memory, L2 DB). Handlers auto-convert on write via `ensure_base_amount()`. Aggregation queries use pre-computed `base_amount` columns. Frontend drops `toBase()` and rate queries.

**Tech Stack:** Rust (sqlx, reqwest, DashMap), SQLite, React/TypeScript, open.er-api.com + CoinGecko APIs

**Pre-release simplification:** Per CLAUDE.md, all schema changes are made directly in CREATE TABLE statements (no ALTER TABLE, no backfill module). The nullable/backfill design from the spec is deferred to first release. Columns are NOT NULL with defaults where possible.

**Spec:** `docs/superpowers/specs/2026-03-14-finance-currency-engine-design.md`

---

## File Map

### New files
| File | Responsibility |
|------|---------------|
| `crates/storage/src/repos/finance_exchange_rate_repo.rs` | CRUD + staleness check for rate cache table |
| `crates/feature-finance/src/rate_cache.rs` | L1 in-memory + L2 DB rate cache with TTL |
| `crates/feature-finance/src/currency.rs` | `ensure_base_amount()` + `ensure_investment_base()` conversion helpers |
| `crates/feature-finance/src/rebase.rs` | Home currency change — batch re-computation |

### Modified files
| File | Changes |
|------|---------|
| `crates/storage/migrations/001_initial.sql` | Add `base_*` columns to 7 finance tables + `finance_exchange_rates` table |
| `crates/feature-finance/migrations/001_finance_tables.sql` | Mirror above |
| `crates/storage/src/rows/finance.rs` | Add `base_*` fields to all Row + Patch structs |
| `crates/storage/src/repos/mod.rs` | Register `FinanceExchangeRateRepo` |
| `crates/feature-finance/src/price_service.rs` | Inject `FinanceExchangeRateRepo`, DB cache layer, `prefetch_rates()` |
| `crates/feature-finance/src/config.rs` | Change `exchange_rates` key format to `"FROM:TO"` |
| `crates/feature-finance/src/lib.rs` | Wire new modules |
| `crates/feature-finance/src/tool/mod.rs` | Add `market_currency` param to schema |
| `crates/feature-finance/src/tool/accounts.rs` | Auto-convert balance on add/update |
| `crates/feature-finance/src/tool/transactions/mod.rs` | Auto-convert amount on add/update |
| `crates/feature-finance/src/tool/budgets.rs` | Auto-convert amount on add/update |
| `crates/feature-finance/src/tool/investments/mod.rs` | `market_currency`, dual rates, auto-convert cost_basis + current_value |
| `crates/feature-finance/src/tool/goals.rs` | Auto-convert target_amount/current_amount + principal/remaining |
| `crates/feature-finance/src/tool/reports.rs` | Aggregation via `base_amount`, add `base_currency` to responses |
| `crates/feature-finance/src/tool/snapshots.rs` | Use `base_currency` from config |
| `crates/feature-finance/src/tool/settings.rs` | Trigger rebase on `defaultCurrency` change |
| `crates/storage/src/repos/finance_budget_repo.rs` | Budget usage query uses `base_amount` |
| `crates/storage/src/repos/finance_account_repo.rs` | `total_balance_by_currency()` → add `total_base_balance()` |
| `crates/storage/src/repos/finance_investment_repo.rs` | Portfolio summary uses `base_current_value` |
| `crates/storage/src/repos/finance_transaction_repo.rs` | `sum_by_category()` uses `base_amount` |
| `desktop-ui/src/shared/types/finance.ts` | Add `base_*` fields to all interfaces |
| `desktop-ui/src/features/finance/lib/finance.ts` | Remove `toBase()` |
| `desktop-ui/src/features/finance/pages/*.tsx` | Remove rates queries, use `baseAmount` for aggregation |
| `skills/finance-management/SKILL.md` | Currency-aware guidance |
| `skills/finance-management/references/currency-engine.md` | NEW: currency engine reference |
| `.claude/skills/klyntbot-finance/SKILL.md` | Auto-conversion docs |
| `.claude/skills/klyntbot-finance/references/actions.md` | `market_currency` param, base_amount notes |

---

## Chunk 1: Foundation — Schema, Row Types, Rate Cache Repo

### Task 1: Schema Changes

Add `base_*` columns to all 7 finance monetary tables and create the `finance_exchange_rates` cache table.

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:385-572`
- Modify: `crates/feature-finance/migrations/001_finance_tables.sql`

- [ ] **Step 1: Add columns to `finance_accounts` table (core migration)**

In `crates/storage/migrations/001_initial.sql`, add to the `finance_accounts` CREATE TABLE (after `updated_at`):

```sql
    base_balance   INTEGER NOT NULL DEFAULT 0,
    base_currency  TEXT NOT NULL DEFAULT 'USD',
    exchange_rate  REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 2: Add columns to `finance_transactions` table**

Add after `updated_at`:

```sql
    base_amount    INTEGER NOT NULL DEFAULT 0,
    base_currency  TEXT NOT NULL DEFAULT 'USD',
    exchange_rate  REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 3: Add columns to `finance_budgets` table**

Add after `updated_at`:

```sql
    base_amount    INTEGER NOT NULL DEFAULT 0,
    base_currency  TEXT NOT NULL DEFAULT 'USD',
    exchange_rate  REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 4: Add columns to `finance_investments` table**

Add after `updated_at`:

```sql
    market_currency    TEXT,
    base_cost_basis    INTEGER NOT NULL DEFAULT 0,
    base_current_value INTEGER NOT NULL DEFAULT 0,
    base_currency      TEXT NOT NULL DEFAULT 'USD',
    purchase_rate      REAL NOT NULL DEFAULT 1.0,
    market_rate        REAL NOT NULL DEFAULT 1.0
```

Note: `market_currency` is nullable — for investments quoted in the same currency as purchase, it can be NULL (treated as equal to `currency`).

- [ ] **Step 5: Add columns to `finance_investment_transactions` table**

Add after `created_at`:

```sql
    base_total_amount  INTEGER NOT NULL DEFAULT 0,
    base_currency      TEXT NOT NULL DEFAULT 'USD',
    exchange_rate      REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 6: Add columns to `finance_goals` table**

Add after `updated_at`:

```sql
    base_target_amount   INTEGER NOT NULL DEFAULT 0,
    base_current_amount  INTEGER NOT NULL DEFAULT 0,
    base_currency        TEXT NOT NULL DEFAULT 'USD',
    exchange_rate        REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 7: Add columns to `finance_liabilities` table**

Add after `updated_at`:

```sql
    base_principal   INTEGER NOT NULL DEFAULT 0,
    base_remaining   INTEGER NOT NULL DEFAULT 0,
    base_currency    TEXT NOT NULL DEFAULT 'USD',
    exchange_rate    REAL NOT NULL DEFAULT 1.0
```

- [ ] **Step 8: Create `finance_exchange_rates` cache table**

Add after all existing finance tables:

```sql
CREATE TABLE IF NOT EXISTS finance_exchange_rates (
    from_currency  TEXT NOT NULL,
    to_currency    TEXT NOT NULL,
    rate           REAL NOT NULL,
    fetched_at     TEXT NOT NULL,
    PRIMARY KEY (from_currency, to_currency)
);

CREATE INDEX IF NOT EXISTS idx_exchange_rates_staleness
    ON finance_exchange_rates (to_currency, from_currency, fetched_at);
```

- [ ] **Step 9: Mirror all changes in feature migration**

Copy all changes to `crates/feature-finance/migrations/001_finance_tables.sql`. Both files must stay in sync.

- [ ] **Step 10: Verify compilation**

Run: `cargo build -p storage`
Expected: Compiles (row structs will need updating next, but the SQL is just a string).

- [ ] **Step 11: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/feature-finance/migrations/001_finance_tables.sql
git commit -m "feat(finance): add base_amount/base_currency/exchange_rate columns to all monetary tables"
```

---

### Task 2: Row Struct Updates

Add `base_*` fields to all finance row structs and patch structs so sqlx can map the new columns.

**Files:**
- Modify: `crates/storage/src/rows/finance.rs`

- [ ] **Step 1: Add base fields to `FinanceAccountRow`**

After the `updated_at` field (around line 27), add:

```rust
    pub base_balance: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 2: Add base fields to `FinanceTransactionRow`**

After `updated_at` (around line 48), add:

```rust
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 3: Add base fields to `FinanceBudgetRow`**

After `updated_at` (around line 68), add:

```rust
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 4: Add fields to `FinanceInvestmentRow`**

After `updated_at` (around line 101), add:

```rust
    pub market_currency: Option<String>,
    pub base_cost_basis: i64,
    pub base_current_value: i64,
    pub base_currency: String,
    pub purchase_rate: f64,
    pub market_rate: f64,
```

- [ ] **Step 5: Add base fields to `FinanceInvestmentTxRow`**

After `created_at` (around line 125), add:

```rust
    pub base_total_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 6: Add base fields to `FinanceGoalRow`**

After `updated_at` (around line 145), add:

```rust
    pub base_target_amount: i64,
    pub base_current_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 7: Add base fields to `FinanceLiabilityRow`**

After `updated_at` (around line 163), add:

```rust
    pub base_principal: i64,
    pub base_remaining: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 8: Add `FinanceExchangeRateRow` struct**

Add a new row struct (after the existing finance rows):

```rust
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct FinanceExchangeRateRow {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: f64,
    pub fetched_at: String,
}
```

- [ ] **Step 9: Update `FinanceAccountPatch`**

The patch struct is used by `update()`. Add optional base fields:

```rust
    pub base_balance: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
```

- [ ] **Step 10: Update `FinanceTransactionPatch`**

Add:

```rust
    pub base_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
```

- [ ] **Step 11: Update `FinanceBudgetPatch`**

Add:

```rust
    pub base_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
```

- [ ] **Step 12: Update `FinanceInvestmentPatch`**

Add:

```rust
    pub market_currency: Option<Option<String>>,
    pub base_cost_basis: Option<i64>,
    pub base_current_value: Option<i64>,
    pub base_currency: Option<String>,
    pub purchase_rate: Option<f64>,
    pub market_rate: Option<f64>,
```

- [ ] **Step 13: Update `FinanceGoalPatch`**

Add:

```rust
    pub base_target_amount: Option<i64>,
    pub base_current_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
```

- [ ] **Step 14: Update `FinanceLiabilityPatch`**

Add:

```rust
    pub base_principal: Option<i64>,
    pub base_remaining: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
```

- [ ] **Step 15: Update `BudgetUsageRow`**

Add after existing fields:

```rust
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
```

- [ ] **Step 16: Verify compilation**

Run: `cargo build -p storage`
Expected: Fails — repo INSERT/UPDATE queries don't include the new columns yet. That's expected, we fix repos next.

- [ ] **Step 17: Update all repo INSERT queries**

Every repo that does INSERT must now include the new base columns. Update the following repos to add the new columns to their INSERT statements:

- `finance_account_repo.rs` → `add()`: add `base_balance, base_currency, exchange_rate` columns + binds
- `finance_transaction_repo.rs` → `add()`: add `base_amount, base_currency, exchange_rate` columns + binds
- `finance_budget_repo.rs` → `add()`: add `base_amount, base_currency, exchange_rate` columns + binds
- `finance_investment_repo.rs` → `add_investment()`: add `market_currency, base_cost_basis, base_current_value, base_currency, purchase_rate, market_rate` columns + binds
- `finance_investment_repo.rs` → `add_investment_tx()`: add `base_total_amount, base_currency, exchange_rate` columns + binds
- `finance_goal_repo.rs` → `add()`: add `base_target_amount, base_current_amount, base_currency, exchange_rate` columns + binds
- `finance_liability_repo.rs` → `add()`: add `base_principal, base_remaining, base_currency, exchange_rate` columns + binds

- [ ] **Step 18: Update all repo UPDATE queries**

Every repo that does UPDATE must now include the new base columns in their COALESCE patterns:

- `finance_account_repo.rs` → `update()`: add `base_balance = COALESCE(?, base_balance), base_currency = COALESCE(?, base_currency), exchange_rate = COALESCE(?, exchange_rate)`
- `finance_transaction_repo.rs` → `update()`: add `base_amount = COALESCE(?, base_amount), base_currency = COALESCE(?, base_currency), exchange_rate = COALESCE(?, exchange_rate)`
- `finance_budget_repo.rs` → `update()`: add `base_amount = COALESCE(?, base_amount), base_currency = COALESCE(?, base_currency), exchange_rate = COALESCE(?, exchange_rate)`
- `finance_investment_repo.rs` → `update_investment()`: add all 6 new fields (`market_currency`, `base_cost_basis`, `base_current_value`, `base_currency`, `purchase_rate`, `market_rate`)
- `finance_goal_repo.rs` → `update()`: add `base_target_amount`, `base_current_amount`, `base_currency`, `exchange_rate`
- `finance_liability_repo.rs` → `update()`: add `base_principal`, `base_remaining`, `base_currency`, `exchange_rate`

- [ ] **Step 18b: Update `adjust_balance()` in account repo**

`FinanceAccountRepo::adjust_balance()` does `SET balance = balance + ?` directly. It must ALSO recompute `base_balance`:

```rust
pub async fn adjust_balance(
    &self,
    id: &str,
    delta: i64,
    exchange_rate: f64,
    base_currency: &str,
) -> Result<(), crate::error::StorageError> {
    sqlx::query(
        r#"UPDATE finance_accounts SET
           balance = balance + ?,
           base_balance = CAST(ROUND((balance + ?) * ?) AS INTEGER),
           base_currency = ?,
           exchange_rate = ?,
           updated_at = datetime('now')
           WHERE id = ?"#,
    )
    .bind(delta)
    .bind(delta)
    .bind(exchange_rate)
    .bind(base_currency)
    .bind(exchange_rate)
    .bind(id)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

**Important:** All callers of `adjust_balance()` (transaction handlers, transfer handlers) must now pass the account's exchange rate. Get it from the account's existing `exchange_rate` field, or re-fetch if the account currency differs from base.

- [ ] **Step 19: Verify compilation**

Run: `cargo build -p storage -p feature-finance`
Expected: Compiles. May have warnings about unused fields.

- [ ] **Step 20: Run tests**

Run: `cargo nextest run -p storage`
Expected: All existing tests pass (they use `connect_in_memory()` which runs the migration fresh).

- [ ] **Step 21: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add base_* fields to all finance row and patch structs"
```

---

### Task 3: Exchange Rate Cache Repo

Create a new repo for the `finance_exchange_rates` table with TTL-aware lookups.

**Files:**
- Create: `crates/storage/src/repos/finance_exchange_rate_repo.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write tests for the rate cache repo**

Create `crates/storage/src/repos/finance_exchange_rate_repo.rs`:

```rust
//! Repository for the `finance_exchange_rates` cache table.

use crate::rows::finance::FinanceExchangeRateRow;

/// Manual struct — no `crud_repo!` because this table has a composite PK
/// `(from_currency, to_currency)`, not a single `id` column.
#[derive(Debug, Clone)]
pub struct FinanceExchangeRateRepo {
    pool: sqlx::SqlitePool,
}

impl FinanceExchangeRateRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

impl FinanceExchangeRateRepo {
    /// Upsert a rate (INSERT OR REPLACE).
    pub async fn upsert(
        &self,
        from: &str,
        to: &str,
        rate: f64,
    ) -> Result<(), crate::error::StorageError> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO finance_exchange_rates
               (from_currency, to_currency, rate, fetched_at)
               VALUES (?, ?, ?, datetime('now'))"#,
        )
        .bind(from)
        .bind(to)
        .bind(rate)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a cached rate if it exists and is fresher than `max_age_minutes`.
    /// Excludes sentinel rows (from_currency starting with '__').
    pub async fn get_fresh(
        &self,
        from: &str,
        to: &str,
        max_age_minutes: i64,
    ) -> Result<Option<f64>, crate::error::StorageError> {
        let row = sqlx::query_as::<_, (f64,)>(
            r#"SELECT rate FROM finance_exchange_rates
               WHERE from_currency = ? AND to_currency = ?
               AND from_currency NOT LIKE '__%'
               AND fetched_at > datetime('now', ? || ' minutes')"#,
        )
        .bind(from)
        .bind(to)
        .bind(-max_age_minutes)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Get a cached rate regardless of age (stale fallback).
    /// Excludes sentinel rows.
    pub async fn get_stale(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<f64>, crate::error::StorageError> {
        let row = sqlx::query_as::<_, (f64,)>(
            r#"SELECT rate FROM finance_exchange_rates
               WHERE from_currency = ? AND to_currency = ?
               AND from_currency NOT LIKE '__%'"#,
        )
        .bind(from)
        .bind(to)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Upsert multiple rates at once (batch from API response).
    pub async fn upsert_batch(
        &self,
        base: &str,
        rates: &[(String, f64)],
    ) -> Result<(), crate::error::StorageError> {
        let mut tx = self.pool.begin().await?;
        for (currency, rate) in rates {
            sqlx::query(
                r#"INSERT OR REPLACE INTO finance_exchange_rates
                   (from_currency, to_currency, rate, fetched_at)
                   VALUES (?, ?, ?, datetime('now'))"#,
            )
            .bind(currency.as_str())
            .bind(base)
            .bind(rate)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Check if a sentinel row exists. Used for backfill/rebase status.
    pub async fn get_sentinel(
        &self,
        key: &str,
    ) -> Result<Option<FinanceExchangeRateRow>, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceExchangeRateRow>(
            "SELECT * FROM finance_exchange_rates WHERE from_currency = ?",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Write a sentinel row.
    pub async fn set_sentinel(
        &self,
        from: &str,
        to: &str,
        rate: f64,
    ) -> Result<(), crate::error::StorageError> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO finance_exchange_rates
               (from_currency, to_currency, rate, fetched_at)
               VALUES (?, ?, ?, datetime('now'))"#,
        )
        .bind(from)
        .bind(to)
        .bind(rate)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a sentinel row.
    pub async fn delete_sentinel(
        &self,
        from: &str,
    ) -> Result<(), crate::error::StorageError> {
        sqlx::query("DELETE FROM finance_exchange_rates WHERE from_currency = ?")
            .bind(from)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    async fn setup() -> FinanceExchangeRateRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        FinanceExchangeRateRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_upsert_and_get_fresh() {
        let repo = setup().await;
        repo.upsert("USD", "VND", 25500.0).await.unwrap();
        let rate = repo.get_fresh("USD", "VND", 15).await.unwrap();
        assert_eq!(rate, Some(25500.0));
    }

    #[tokio::test]
    async fn test_get_fresh_excludes_sentinels() {
        let repo = setup().await;
        repo.set_sentinel("__REBASE__", "VND", 0.0).await.unwrap();
        let rate = repo.get_fresh("__REBASE__", "VND", 15).await.unwrap();
        assert_eq!(rate, None);
    }

    #[tokio::test]
    async fn test_upsert_batch() {
        let repo = setup().await;
        let rates = vec![
            ("USD".to_string(), 25500.0),
            ("EUR".to_string(), 27800.0),
        ];
        repo.upsert_batch("VND", &rates).await.unwrap();
        let usd = repo.get_fresh("USD", "VND", 15).await.unwrap();
        let eur = repo.get_fresh("EUR", "VND", 15).await.unwrap();
        assert_eq!(usd, Some(25500.0));
        assert_eq!(eur, Some(27800.0));
    }

    #[tokio::test]
    async fn test_sentinel_lifecycle() {
        let repo = setup().await;
        repo.set_sentinel("__BACKFILL__", "__STATUS__", 0.0).await.unwrap();
        let s = repo.get_sentinel("__BACKFILL__").await.unwrap();
        assert!(s.is_some());
        repo.delete_sentinel("__BACKFILL__").await.unwrap();
        let s = repo.get_sentinel("__BACKFILL__").await.unwrap();
        assert!(s.is_none());
    }
}
```

- [ ] **Step 2: Register the repo**

In `crates/storage/src/repos/mod.rs`, add:
```rust
pub mod finance_exchange_rate_repo;
pub use finance_exchange_rate_repo::FinanceExchangeRateRepo;
```

In `crates/storage/src/finance_storage.rs`, add the new repo to `FinanceStorage`:
```rust
// Add to struct:
pub exchange_rates: FinanceExchangeRateRepo,

// Add to from_pool():
exchange_rates: FinanceExchangeRateRepo::new(pool.clone()),
```

Also update the import in `finance_storage.rs`:
```rust
use crate::repos::{
    ..., FinanceExchangeRateRepo,
};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p storage -E 'test(exchange_rate)'`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/repos/finance_exchange_rate_repo.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add FinanceExchangeRateRepo with TTL-aware cache lookups"
```

---

## Chunk 2: Rate Service — DB-Backed Cache + Prefetch

### Task 4: Rate Cache Module

Create a two-layer cache (L1 in-memory DashMap + L2 SQLite) that the price service delegates to.

**Files:**
- Create: `crates/feature-finance/src/rate_cache.rs`

- [ ] **Step 1: Write the rate cache module**

```rust
//! Two-layer exchange rate cache: L1 in-memory (DashMap) + L2 SQLite.
//!
//! L1 is hot-path — avoids DB hits for repeated lookups within a request.
//! L2 survives restarts and provides stale fallback when API is down.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use storage::repos::finance_exchange_rate_repo::FinanceExchangeRateRepo;

struct CachedRate {
    rate: f64,
    fetched_at: Instant,
}

#[derive(Clone)]
pub struct RateCache {
    l1: Arc<DashMap<String, CachedRate>>,
    l2: FinanceExchangeRateRepo,
    ttl_minutes: i64,
}

impl RateCache {
    pub fn new(repo: FinanceExchangeRateRepo, ttl_minutes: i64) -> Self {
        Self {
            l1: Arc::new(DashMap::new()),
            l2: repo,
            ttl_minutes,
        }
    }

    fn cache_key(from: &str, to: &str) -> String {
        format!("{}:{}", from.to_uppercase(), to.to_uppercase())
    }

    /// Get a rate, checking L1 then L2. Returns None if nothing cached or all stale.
    /// `fresh_only` = true returns only within-TTL rates; false returns any cached rate.
    pub async fn get(&self, from: &str, to: &str, fresh_only: bool) -> Option<f64> {
        let key = Self::cache_key(from, to);

        // L1: in-memory
        if let Some(entry) = self.l1.get(&key) {
            let age = entry.fetched_at.elapsed().as_secs() / 60;
            if !fresh_only || age < self.ttl_minutes as u64 {
                return Some(entry.rate);
            }
        }

        // L2: SQLite
        let db_rate = if fresh_only {
            self.l2.get_fresh(from, to, self.ttl_minutes).await.ok()?
        } else {
            self.l2.get_stale(from, to).await.ok()?
        };

        // Promote to L1
        if let Some(rate) = db_rate {
            self.l1.insert(key, CachedRate { rate, fetched_at: Instant::now() });
        }

        db_rate
    }

    /// Store a rate in both L1 and L2.
    pub async fn put(&self, from: &str, to: &str, rate: f64) -> common::Result<()> {
        let key = Self::cache_key(from, to);
        self.l1.insert(key, CachedRate { rate, fetched_at: Instant::now() });
        self.l2.upsert(from, to, rate).await.map_err(|e| {
            common::KlyntbotError::Tool(format!("rate cache write failed: {e}"))
        })?;
        Ok(())
    }

    /// Store multiple rates (from batch API response).
    pub async fn put_batch(&self, base: &str, rates: &[(String, f64)]) -> common::Result<()> {
        for (currency, rate) in rates {
            let key = Self::cache_key(currency, base);
            self.l1.insert(key, CachedRate { rate: *rate, fetched_at: Instant::now() });
        }
        self.l2.upsert_batch(base, rates).await.map_err(|e| {
            common::KlyntbotError::Tool(format!("rate cache batch write failed: {e}"))
        })?;
        Ok(())
    }

    /// Access the underlying repo for sentinel operations.
    pub fn repo(&self) -> &FinanceExchangeRateRepo {
        &self.l2
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/feature-finance/src/lib.rs`, add `pub mod rate_cache;`

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p feature-finance`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-finance/src/rate_cache.rs crates/feature-finance/src/lib.rs
git commit -m "feat(finance): add RateCache with L1 in-memory + L2 SQLite layers"
```

---

### Task 5: Price Service Upgrade

Inject `RateCache` into `PriceService`, add `get_rate()` and `prefetch_rates()` methods.

**Files:**
- Modify: `crates/feature-finance/src/price_service.rs`

- [ ] **Step 1: Add `RateCache` field to `PriceService`**

Add `rate_cache: Option<RateCache>` to the `PriceService` struct. Keep it optional for backward compat with tests that don't have a DB.

Add a new constructor:

```rust
pub fn with_rate_cache(cache_ttl_minutes: u32, rate_cache: RateCache) -> Self {
    let mut svc = Self::new(cache_ttl_minutes);
    svc.rate_cache = Some(rate_cache);
    svc
}
```

- [ ] **Step 2: Add `get_rate()` method**

This is the main entry point for handlers needing a conversion rate:

```rust
/// Get exchange rate for `from → to`, using cache with API fallback.
/// If `from == to`, returns 1.0 without any lookup.
/// Returns error if no rate available (API down + no cache).
pub async fn get_rate(&self, from: &str, to: &str) -> common::Result<f64> {
    let from = from.to_uppercase();
    let to = to.to_uppercase();
    if from == to {
        return Ok(1.0);
    }

    if let Some(cache) = &self.rate_cache {
        // Try fresh cache first
        if let Some(rate) = cache.get(&from, &to, true).await {
            return Ok(rate);
        }

        // Fetch from API
        match self.fetch_exchange_rate(&from, &to).await {
            Ok(result) => {
                cache.put(&from, &to, result.price).await?;
                return Ok(result.price);
            }
            Err(e) => {
                tracing::warn!("API rate fetch failed for {from}→{to}: {e}");
                // Stale fallback
                if let Some(rate) = cache.get(&from, &to, false).await {
                    tracing::warn!("Using stale rate for {from}→{to}");
                    return Ok(rate);
                }
                return Err(common::KlyntbotError::Tool(
                    format!("No exchange rate available for {from}→{to}: API failed and no cached rate")
                ));
            }
        }
    }

    // No rate cache — fall back to API-only (legacy path)
    let result = self.fetch_exchange_rate(&from, &to).await?;
    Ok(result.price)
}
```

- [ ] **Step 3: Add `prefetch_rates()` method**

Batch-fetch all rates for a base currency in one API call:

```rust
/// Batch-fetch rates for all given currencies against `base`.
/// Uses the open.er-api.com bulk endpoint. Stores all in cache.
///
/// **Rate direction convention:** Returns (currency, rate) where rate converts
/// `currency → base`. E.g., prefetch_rates("VND", &["USD"]) returns
/// ("USD", 25500.0) meaning 1 USD = 25500 VND.
/// The API returns base→foreign, so we invert: rate = 1/api_rate.
pub async fn prefetch_rates(
    &self,
    base: &str,
    currencies: &[String],
) -> common::Result<Vec<(String, f64)>> {
    let url = format!(
        "https://open.er-api.com/v6/latest/{}",
        base.to_uppercase()
    );
    let resp: serde_json::Value = self.get_with_retry(&url).await?;
    let rates_obj = resp.get("rates").and_then(|r| r.as_object())
        .ok_or_else(|| common::KlyntbotError::Tool("Invalid API response".into()))?;

    let mut results = Vec::new();
    for currency in currencies {
        let upper = currency.to_uppercase();
        if let Some(rate_val) = rates_obj.get(&upper).and_then(|v| v.as_f64()) {
            // API returns base→foreign, we need foreign→base = 1/rate
            let inverse = 1.0 / rate_val;
            results.push((upper, inverse));
        }
    }

    if let Some(cache) = &self.rate_cache {
        cache.put_batch(base, &results).await?;
    }

    Ok(results)
}
```

- [ ] **Step 4: Add config override check**

Add a method that checks user-configured rate overrides first:

```rust
/// Check config overrides for a rate. Key format: "FROM:TO".
pub fn config_override_rate(
    overrides: &Option<std::collections::HashMap<String, f64>>,
    from: &str,
    to: &str,
) -> Option<f64> {
    let key = format!("{}:{}", from.to_uppercase(), to.to_uppercase());
    overrides.as_ref().and_then(|m| m.get(&key).copied())
}
```

- [ ] **Step 5: Write tests**

Add tests for `get_rate()` with cache scenarios:

```rust
#[cfg(test)]
mod rate_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_rate_same_currency() {
        let svc = PriceService::new(15);
        let rate = svc.get_rate("USD", "USD").await.unwrap();
        assert_eq!(rate, 1.0);
    }

    #[tokio::test]
    async fn test_get_rate_same_currency_case_insensitive() {
        let svc = PriceService::new(15);
        let rate = svc.get_rate("usd", "USD").await.unwrap();
        assert_eq!(rate, 1.0);
    }
}
```

- [ ] **Step 6: Verify compilation + run tests**

Run: `cargo nextest run -p feature-finance -E 'test(rate)'`
Expected: Tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-finance/src/price_service.rs
git commit -m "feat(finance): upgrade PriceService with DB-backed rate cache and prefetch"
```

---

## Chunk 3: Conversion Core + Write Path

### Task 6: Currency Conversion Utility

Create the shared `ensure_base_amount()` helper that all handlers will call.

**Files:**
- Create: `crates/feature-finance/src/currency.rs`

- [ ] **Step 1: Write the conversion module**

```rust
//! Currency conversion helpers for the finance write path.
//!
//! All handlers call `ensure_base_amount()` before inserting records.
//! This fetches the exchange rate (from cache or API) and computes
//! the base-currency equivalent.

use crate::price_service::PriceService;

/// Result of converting an amount to base currency.
pub struct BaseConversion {
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Result of converting investment amounts (has two rates).
pub struct InvestmentBaseConversion {
    pub base_cost_basis: i64,
    pub base_current_value: i64,
    pub base_currency: String,
    pub purchase_rate: f64,
    pub market_rate: f64,
}

/// Convert a single monetary amount to the base currency.
///
/// If `currency == base_currency`, returns `amount` with rate 1.0.
/// Otherwise, fetches the rate and computes `round(amount * rate)`.
/// Fails if no rate is available.
pub async fn ensure_base_amount(
    amount: i64,
    currency: &str,
    base_currency: &str,
    price_service: &PriceService,
) -> common::Result<BaseConversion> {
    if currency.eq_ignore_ascii_case(base_currency) {
        return Ok(BaseConversion {
            base_amount: amount,
            base_currency: base_currency.to_string(),
            exchange_rate: 1.0,
        });
    }

    let rate = price_service.get_rate(currency, base_currency).await?;
    let base_amount = (amount as f64 * rate).round() as i64;

    Ok(BaseConversion {
        base_amount,
        base_currency: base_currency.to_string(),
        exchange_rate: rate,
    })
}

/// Convert investment amounts with separate purchase and market rates.
///
/// - `cost_basis` in `purchase_currency` → `base_cost_basis` via purchase_rate
/// - `current_value` in `market_currency` → `base_current_value` via market_rate
pub async fn ensure_investment_base(
    cost_basis: i64,
    purchase_currency: &str,
    current_value: Option<i64>,
    market_currency: Option<&str>,
    base_currency: &str,
    price_service: &PriceService,
) -> common::Result<InvestmentBaseConversion> {
    let mkt_currency = market_currency.unwrap_or(purchase_currency);

    let purchase_rate = if purchase_currency.eq_ignore_ascii_case(base_currency) {
        1.0
    } else {
        price_service.get_rate(purchase_currency, base_currency).await?
    };

    let market_rate = if mkt_currency.eq_ignore_ascii_case(base_currency) {
        1.0
    } else {
        price_service.get_rate(mkt_currency, base_currency).await?
    };

    let base_cost_basis = (cost_basis as f64 * purchase_rate).round() as i64;
    let base_current_value = current_value
        .map(|v| (v as f64 * market_rate).round() as i64)
        .unwrap_or(0);

    Ok(InvestmentBaseConversion {
        base_cost_basis,
        base_current_value,
        base_currency: base_currency.to_string(),
        purchase_rate,
        market_rate,
    })
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod currency;` to `crates/feature-finance/src/lib.rs`.

- [ ] **Step 3: Write tests**

Add inline tests in `currency.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_same_currency_no_conversion() {
        let svc = PriceService::new(15);
        let result = ensure_base_amount(100_00, "USD", "USD", &svc).await.unwrap();
        assert_eq!(result.base_amount, 100_00);
        assert_eq!(result.exchange_rate, 1.0);
    }

    #[tokio::test]
    async fn test_same_currency_case_insensitive() {
        let svc = PriceService::new(15);
        let result = ensure_base_amount(100_00, "usd", "USD", &svc).await.unwrap();
        assert_eq!(result.base_amount, 100_00);
        assert_eq!(result.exchange_rate, 1.0);
    }
}
```

- [ ] **Step 4: Verify compilation + tests**

Run: `cargo nextest run -p feature-finance -E 'test(currency)'`
Expected: Passes.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-finance/src/currency.rs crates/feature-finance/src/lib.rs
git commit -m "feat(finance): add ensure_base_amount currency conversion helpers"
```

---

### Task 7: Update Account Handlers

Add auto-conversion to `account_add` and `account_update`.

**Files:**
- Modify: `crates/feature-finance/src/tool/accounts.rs`

- [ ] **Step 1: Import currency module and call ensure_base_amount in account_add**

At the top, add: `use crate::currency::ensure_base_amount;`

In the `account_add` handler, after computing `balance` and `currency`, before creating the row:

```rust
let conv = ensure_base_amount(
    balance,
    &currency,
    &self.default_currency,
    &self.price_service,
).await?;
```

Then set the row fields:

```rust
base_balance: conv.base_amount,
base_currency: conv.base_currency,
exchange_rate: conv.exchange_rate,
```

- [ ] **Step 2: Update account_update handler**

In the update handler, if balance or currency changed, re-compute base:

```rust
// If balance is being updated, recompute base
if patch.balance.is_some() || patch.currency.is_some() {
    let balance = patch.balance.unwrap_or(existing.balance);
    let currency = patch.currency.as_deref().unwrap_or(&existing.currency);
    let conv = ensure_base_amount(balance, currency, &self.default_currency, &self.price_service).await?;
    patch.base_balance = Some(conv.base_amount);
    patch.base_currency = Some(conv.base_currency);
    patch.exchange_rate = Some(conv.exchange_rate);
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p feature-finance`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-finance/src/tool/accounts.rs
git commit -m "feat(finance): auto-convert account balance to base currency on add/update"
```

---

### Task 8: Update Transaction Handlers

Add auto-conversion to `tx_add` and `tx_update`.

**Files:**
- Modify: `crates/feature-finance/src/tool/transactions/mod.rs`

- [ ] **Step 1: Import and call ensure_base_amount in tx_add**

After computing `amount` and `currency`, before building the row:

```rust
let conv = ensure_base_amount(amount, &currency, &self.default_currency, &self.price_service).await?;
```

Set fields: `base_amount: conv.base_amount, base_currency: conv.base_currency, exchange_rate: conv.exchange_rate`

- [ ] **Step 2: Update tx_update handler similarly**

If amount or currency changed, recompute base fields.

- [ ] **Step 3: Handle transfers**

In `tx_add_transfer`, both the debit and credit transactions need base conversion. The debit uses the source account's currency, the credit uses the target's.

- [ ] **Step 4: Verify compilation + run existing tests**

Run: `cargo nextest run -p feature-finance -E 'test(transaction)'`
Expected: Passes.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-finance/src/tool/transactions/
git commit -m "feat(finance): auto-convert transaction amounts to base currency"
```

---

### Task 9: Update Budget Handlers

**Files:**
- Modify: `crates/feature-finance/src/tool/budgets.rs`

- [ ] **Step 1: Add conversion to budget_add and budget_update**

Same pattern as accounts — call `ensure_base_amount(amount, &currency, &self.default_currency, &self.price_service)` and set `base_amount/base_currency/exchange_rate` on the row.

- [ ] **Step 2: Verify + commit**

```bash
git add crates/feature-finance/src/tool/budgets.rs
git commit -m "feat(finance): auto-convert budget amounts to base currency"
```

---

### Task 10: Update Investment Handlers

This is the most complex — investments have `market_currency` and dual rates.

**Files:**
- Modify: `crates/feature-finance/src/tool/investments/mod.rs`
- Modify: `crates/feature-finance/src/tool/mod.rs` (add `market_currency` param)

- [ ] **Step 1: Add `market_currency` to the MCP parameter schema**

In `mod.rs` `parameters()`, add `market_currency` to the JSON schema properties:

```json
"market_currency": {
    "type": "string",
    "description": "ISO 4217 code for the currency the asset is quoted in on exchanges (e.g. USD for BTC). Defaults to the purchase currency if omitted."
}
```

- [ ] **Step 2: Update `investment_add` handler**

Extract `market_currency` from params. Call `ensure_investment_base()`:

```rust
use crate::currency::ensure_investment_base;

let market_currency = args.get("market_currency").and_then(|v| v.as_str());
let conv = ensure_investment_base(
    cost_basis,
    &currency,
    current_value,
    market_currency,
    &self.default_currency,
    &self.price_service,
).await?;
```

Set fields: `market_currency`, `base_cost_basis`, `base_current_value`, `base_currency`, `purchase_rate`, `market_rate`.

- [ ] **Step 3: Update `investment_update` handler**

If cost_basis, currency, current_value, or market_currency changed, recompute both base values.

- [ ] **Step 4: Update `investment_tx_add` handler**

Call `ensure_base_amount(total_amount, &currency, ...)` and set `base_total_amount/base_currency/exchange_rate`.

- [ ] **Step 5: Update price refresh logic**

In `pricing.rs` (or wherever `current_price`/`current_value` are refreshed), also refresh `base_current_value` and `market_rate`:

```rust
let market_rate = self.price_service.get_rate(&market_currency, &self.default_currency).await?;
let base_current_value = (current_value as f64 * market_rate).round() as i64;
// Update: current_value, base_current_value, market_rate
```

- [ ] **Step 6: Verify + commit**

```bash
git add crates/feature-finance/src/tool/investments/ crates/feature-finance/src/tool/mod.rs
git commit -m "feat(finance): investment dual-rate conversion with market_currency support"
```

---

### Task 11: Update Goal + Liability Handlers

**Files:**
- Modify: `crates/feature-finance/src/tool/goals.rs`

- [ ] **Step 1: Update goal_add/goal_update**

Goals have `target_amount` and `current_amount` — both need conversion:

```rust
let conv_target = ensure_base_amount(target_amount, &currency, &self.default_currency, &self.price_service).await?;
let conv_current = ensure_base_amount(current_amount, &currency, &self.default_currency, &self.price_service).await?;
```

Set: `base_target_amount`, `base_current_amount`, `base_currency`, `exchange_rate` (same rate for both since same currency).

- [ ] **Step 2: Update liability_add/liability_update**

Liabilities have `principal` and `remaining`:

```rust
let conv = ensure_base_amount(principal, &currency, &self.default_currency, &self.price_service).await?;
```

Set: `base_principal`, `base_remaining` (use same rate), `base_currency`, `exchange_rate`.

- [ ] **Step 3: Verify + commit**

```bash
git add crates/feature-finance/src/tool/goals.rs
git commit -m "feat(finance): auto-convert goal/liability amounts to base currency"
```

---

## Chunk 4: Read Path + Aggregation + Rebase

### Task 12: Update Aggregation Queries

Switch report handlers and repo queries to use `base_amount` columns.

**Files:**
- Modify: `crates/storage/src/repos/finance_transaction_repo.rs`
- Modify: `crates/storage/src/repos/finance_account_repo.rs`
- Modify: `crates/storage/src/repos/finance_investment_repo.rs`
- Modify: `crates/feature-finance/src/tool/reports.rs`

- [ ] **Step 1: Add `total_base_balance()` to account repo**

```rust
pub async fn total_base_balance(
    &self,
    base_currency: &str,
) -> Result<i64, crate::error::StorageError> {
    let row = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(base_balance), 0) FROM finance_accounts
         WHERE is_archived = FALSE AND base_currency = ?",
    )
    .bind(base_currency)
    .fetch_one(&self.pool)
    .await?;
    Ok(row.0)
}
```

- [ ] **Step 2: Update `sum_by_category()` in transaction repo**

Change from `SUM(amount)` to `SUM(base_amount)` with `WHERE base_currency = ?`.

- [ ] **Step 3: Update `sum_by_period()` similarly**

Use `SUM(base_amount)` with base_currency filter.

- [ ] **Step 4: Update `portfolio_summary_query()` in investment repo**

Use `SUM(base_current_value)` and `SUM(base_cost_basis)`.

- [ ] **Step 5: Update report handlers to pass base_currency**

In `reports.rs`, all aggregation calls now pass `self.default_currency` as the base currency filter. Response structs include `base_currency`.

- [ ] **Step 6: Verify + commit**

```bash
git add crates/storage/src/repos/ crates/feature-finance/src/tool/reports.rs
git commit -m "feat(finance): aggregation queries use base_amount with currency filter"
```

---

### Task 13: Update Budget Usage Query

**Files:**
- Modify: `crates/storage/src/repos/finance_budget_repo.rs`

- [ ] **Step 1: Simplify `budget_usage()` query**

Replace the complex cross-currency JOIN with base_amount aggregation:

```sql
SELECT
    b.id, b.name, b.amount, b.currency, b.period, b.category,
    b.method, b.jar_type, b.start_date, b.end_date, b.is_active,
    b.alert_threshold, b.created_at, b.updated_at,
    b.base_amount, b.base_currency, b.exchange_rate,
    COALESCE(SUM(ft.base_amount), 0) AS spent
FROM finance_budgets b
LEFT JOIN finance_transactions ft ON
    ft.tx_type = 'expense'
    AND ft.base_currency = b.base_currency
    AND (b.category IS NULL OR ft.category = b.category)
    AND ft.tx_date >= CASE
        WHEN b.period = 'monthly' THEN date('now', 'localtime', 'start of month')
        WHEN b.period = 'weekly'  THEN date('now', 'localtime', '-' || ((strftime('%w', 'now', 'localtime') + 6) % 7) || ' days')
        WHEN b.period = 'yearly'  THEN date('now', 'localtime', 'start of year')
        ELSE b.start_date
    END
    AND ft.tx_date <= CASE
        WHEN b.period = 'monthly' THEN date('now', 'localtime', 'start of month', '+1 month', '-1 day')
        WHEN b.period = 'weekly'  THEN date('now', 'localtime', '-' || ((strftime('%w', 'now', 'localtime') + 6) % 7) || ' days', '+6 days')
        WHEN b.period = 'yearly'  THEN date('now', 'localtime', 'start of year', '+1 year', '-1 day')
        ELSE COALESCE(b.end_date, date('now', 'localtime'))
    END
WHERE b.id = ?
GROUP BY b.id
```

Key change: `AND ft.base_currency = b.base_currency` replaces the old `AND ft.currency = b.currency`.

- [ ] **Step 2: Update `all_budget_usage()` similarly**

Same change — join on `base_currency` instead of `currency`.

- [ ] **Step 3: Verify + commit**

```bash
git add crates/storage/src/repos/finance_budget_repo.rs
git commit -m "feat(finance): budget usage query joins on base_currency instead of raw currency"
```

---

### Task 14: Update Snapshot Handler

**Files:**
- Modify: `crates/feature-finance/src/tool/snapshots.rs`

- [ ] **Step 1: Use base_currency from config**

In `snapshot_record`, set the snapshot's `currency` to `self.default_currency` and compute totals using `base_balance`, `base_current_value`, `base_principal` etc. instead of raw amounts.

- [ ] **Step 2: Commit**

```bash
git add crates/feature-finance/src/tool/snapshots.rs
git commit -m "feat(finance): snapshot_record uses base_currency for net worth computation"
```

---

### Task 15: Rebase Module + Settings Handler

Implement home currency change with batch re-computation.

**Files:**
- Create: `crates/feature-finance/src/rebase.rs`
- Modify: `crates/feature-finance/src/tool/settings.rs`

- [ ] **Step 1: Write the rebase module**

```rust
//! Home currency change — re-computes all base_* fields when the user
//! changes their default currency.

use crate::price_service::PriceService;
use crate::rate_cache::RateCache;
use sqlx::SqlitePool;

pub struct RebaseResult {
    pub tables_updated: usize,
    pub rows_updated: usize,
    pub failures: Vec<String>,
}

pub async fn rebase_all_tables(
    pool: &SqlitePool,
    rate_cache: &RateCache,
    price_service: &PriceService,
    new_base: &str,
) -> common::Result<RebaseResult> {
    // 1. Set sentinel
    rate_cache.repo().set_sentinel("__REBASE__", new_base, 0.0).await
        .map_err(|e| common::KlyntbotError::Tool(format!("sentinel write: {e}")))?;

    // 2. Collect all distinct currencies
    let currencies = collect_distinct_currencies(pool).await?;

    // 3. Prefetch rates
    let mut rate_map = std::collections::HashMap::new();
    rate_map.insert(new_base.to_uppercase(), 1.0); // self → self
    if let Ok(rates) = price_service.prefetch_rates(new_base, &currencies).await {
        for (cur, rate) in rates {
            rate_map.insert(cur, rate);
        }
    }

    let mut total_rows = 0usize;
    let mut failures = Vec::new();

    // 4. Update each table in a single transaction
    total_rows += rebase_table(pool, "finance_accounts", &[("balance", "base_balance")], new_base, &rate_map, &mut failures).await?;
    total_rows += rebase_table(pool, "finance_transactions", &[("amount", "base_amount")], new_base, &rate_map, &mut failures).await?;
    total_rows += rebase_table(pool, "finance_budgets", &[("amount", "base_amount")], new_base, &rate_map, &mut failures).await?;
    // Goals have TWO amount fields
    total_rows += rebase_table(pool, "finance_goals", &[("target_amount", "base_target_amount"), ("current_amount", "base_current_amount")], new_base, &rate_map, &mut failures).await?;
    // Liabilities have TWO amount fields
    total_rows += rebase_table(pool, "finance_liabilities", &[("principal", "base_principal"), ("remaining", "base_remaining")], new_base, &rate_map, &mut failures).await?;
    // Investment transactions
    total_rows += rebase_table(pool, "finance_investment_transactions", &[("total_amount", "base_total_amount")], new_base, &rate_map, &mut failures).await?;
    // Investments need special handling (dual rates)
    total_rows += rebase_investment_table(pool, new_base, &rate_map, &mut failures).await?;

    // 5. Clear sentinel
    rate_cache.repo().delete_sentinel("__REBASE__").await
        .map_err(|e| common::KlyntbotError::Tool(format!("sentinel delete: {e}")))?;

    Ok(RebaseResult {
        tables_updated: 6,
        rows_updated: total_rows,
        failures,
    })
}

async fn collect_distinct_currencies(pool: &SqlitePool) -> common::Result<Vec<String>> {
    // Union of all currency columns across tables
    let rows = sqlx::query_as::<_, (String,)>(
        r#"SELECT DISTINCT currency FROM finance_accounts
           UNION SELECT DISTINCT currency FROM finance_transactions
           UNION SELECT DISTINCT currency FROM finance_budgets
           UNION SELECT DISTINCT currency FROM finance_investments
           UNION SELECT DISTINCT market_currency FROM finance_investments WHERE market_currency IS NOT NULL
           UNION SELECT DISTINCT currency FROM finance_goals
           UNION SELECT DISTINCT currency FROM finance_liabilities"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Tool(format!("collect currencies: {e}")))?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Generic rebase for tables with one or more (amount_col, base_col) pairs.
/// Example: goals have [("target_amount", "base_target_amount"), ("current_amount", "base_current_amount")].
async fn rebase_table(
    pool: &SqlitePool,
    table: &str,
    col_pairs: &[(&str, &str)],  // [(amount_col, base_col), ...]
    new_base: &str,
    rates: &std::collections::HashMap<String, f64>,
    failures: &mut Vec<String>,
) -> common::Result<usize> {
    let mut tx = pool.begin().await
        .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;

    // Build SET clauses for all column pairs
    let same_set: String = col_pairs.iter()
        .map(|(amt, base)| format!("{base} = {amt}"))
        .collect::<Vec<_>>().join(", ");
    let q = format!(
        "UPDATE {table} SET {same_set}, base_currency = ?, exchange_rate = 1.0 WHERE UPPER(currency) = UPPER(?)"
    );
    sqlx::query(&q).bind(new_base).bind(new_base).execute(&mut *tx).await
        .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;

    let mut count = 0usize;
    for (currency, rate) in rates {
        if currency.eq_ignore_ascii_case(new_base) { continue; }
        let convert_set: String = col_pairs.iter()
            .map(|(amt, base)| format!("{base} = CAST(ROUND({amt} * {rate}) AS INTEGER)"))
            .collect::<Vec<_>>().join(", ");
        let q = format!(
            "UPDATE {table} SET {convert_set}, base_currency = ?, exchange_rate = ? WHERE UPPER(currency) = UPPER(?)"
        );
        let result = sqlx::query(&q)
            .bind(new_base)
            .bind(rate)
            .bind(currency.as_str())
            .execute(&mut *tx)
            .await;
        match result {
            Ok(r) => count += r.rows_affected() as usize,
            Err(e) => failures.push(format!("{table}/{currency}: {e}")),
        }
    }

    tx.commit().await
        .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;
    Ok(count)
}

async fn rebase_investment_table(
    pool: &SqlitePool,
    new_base: &str,
    rates: &std::collections::HashMap<String, f64>,
    failures: &mut Vec<String>,
) -> common::Result<usize> {
    let mut tx = pool.begin().await
        .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;

    // For each investment, recompute both purchase_rate and market_rate
    let investments = sqlx::query_as::<_, (String, String, Option<String>, i64, Option<i64>)>(
        "SELECT id, currency, market_currency, cost_basis, current_value FROM finance_investments"
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;

    let mut count = 0usize;
    for (id, purchase_cur, market_cur, cost_basis, current_value) in &investments {
        let mkt = market_cur.as_deref().unwrap_or(purchase_cur.as_str());

        let p_rate = rates.get(&purchase_cur.to_uppercase()).copied().unwrap_or(1.0);
        let m_rate = rates.get(&mkt.to_uppercase()).copied().unwrap_or(1.0);

        let base_cost = ((*cost_basis as f64) * p_rate).round() as i64;
        let base_val = current_value.map(|v| ((v as f64) * m_rate).round() as i64).unwrap_or(0);

        let result = sqlx::query(
            "UPDATE finance_investments SET base_cost_basis = ?, base_current_value = ?, base_currency = ?, purchase_rate = ?, market_rate = ? WHERE id = ?"
        )
        .bind(base_cost)
        .bind(base_val)
        .bind(new_base)
        .bind(p_rate)
        .bind(m_rate)
        .bind(id.as_str())
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => count += 1,
            Err(e) => failures.push(format!("investments/{id}: {e}")),
        }
    }

    tx.commit().await
        .map_err(|e| common::KlyntbotError::Tool(e.to_string()))?;
    Ok(count)
}
```

- [ ] **Step 2: Register module**

Add `pub mod rebase;` to `crates/feature-finance/src/lib.rs`.

- [ ] **Step 3: Update settings handler to trigger rebase**

In `crates/feature-finance/src/tool/settings.rs`, in the `settings_update` handler — when `defaultCurrency` changes:

```rust
if let Some(new_currency) = args.get("default_currency").and_then(|v| v.as_str()) {
    if new_currency != self.default_currency {
        let result = crate::rebase::rebase_all_tables(
            self.storage.pool(),
            &self.rate_cache,
            &self.price_service,
            new_currency,
        ).await?;
        // Return result with failure count
    }
}
```

**Also update the response message**: The current `settings_update` response says "Changes take effect on next restart." Change this to include currency rebase status:

```rust
// Old: "Changes take effect on next restart."
// New: include rebase result
format!("Settings updated. Rebased {} rows across {} tables. {} failures.",
    result.rows_updated, result.tables_updated, result.failures.len())
```

- [ ] **Step 4: Verify + commit**

```bash
git add crates/feature-finance/src/rebase.rs crates/feature-finance/src/tool/settings.rs crates/feature-finance/src/lib.rs
git commit -m "feat(finance): home currency rebase with crash-safe sentinel and per-table transactions"
```

---

## Chunk 5: Frontend

### Task 16: Update TypeScript Types

**Files:**
- Modify: `desktop-ui/src/shared/types/finance.ts`

- [ ] **Step 1: Add base fields to all finance interfaces**

```typescript
// Add to FinanceAccount:
baseBalance?: number;
baseCurrency?: string;
exchangeRate?: number;

// Add to FinanceTransaction:
baseAmount?: number;
baseCurrency?: string;
exchangeRate?: number;

// Add to FinanceInvestment:
marketCurrency?: string;
baseCostBasis?: number;
baseCurrentValue?: number;
baseCurrency?: string;
purchaseRate?: number;
marketRate?: number;

// Add to FinanceBudgetUsage:
baseAmount?: number;
baseCurrency?: string;

// Add to FinanceGoal:
baseTargetAmount?: number;
baseCurrentAmount?: number;
baseCurrency?: string;

// Add to FinanceLiability:
basePrincipal?: number;
baseRemaining?: number;
baseCurrency?: string;
```

- [ ] **Step 2: Add `marketCurrency` to investment create params**

```typescript
// Add to FinanceInvestmentCreateParams:
marketCurrency?: string;
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/types/finance.ts
git commit -m "feat(ui): add base_* fields to all finance TypeScript interfaces"
```

---

### Task 17: Remove `toBase()` and Rate Queries from Pages

**Files:**
- Modify: `desktop-ui/src/features/finance/lib/finance.ts`
- Modify: All 7 finance page files in `desktop-ui/src/features/finance/pages/`

- [ ] **Step 1: Remove `toBase()` from finance.ts**

Delete the `toBase()` function entirely.

- [ ] **Step 2: Update FinanceOverviewPage.tsx**

Remove:
- `useQuery<Record<string, number>>("finance_exchange_rates", ...)`
- All `toBase()` calls
- `rates` variable

Replace aggregation with pre-computed base fields:
```typescript
// Old: toBase(account.balance, account.currency, rates, baseCurrency)
// New: account.baseBalance ?? 0
```

For totals:
```typescript
const totalNet = useMemo(
  () => netWorth.totalsByCurrency.reduce((s, c) => s + c.net, 0),
  [netWorth],
);
```

Or if backend now returns a single pre-computed total, use that directly.

- [ ] **Step 3: Update AccountsPage.tsx similarly**

Remove rates query. Use `account.baseBalance` for totals. Show original + base for foreign accounts:

```tsx
{fmtMoney(account.balance, account.currency)}
{account.currency !== baseCurrency && account.baseBalance != null && (
  <span className="text-muted">({fmtCompact(account.baseBalance, baseCurrency)})</span>
)}
```

- [ ] **Step 4: Update TransactionsPage.tsx**

Remove rates. Totals use `tx.baseAmount`.

- [ ] **Step 5: Update BudgetsPage.tsx**

Remove rates. Budget progress uses `budget.baseAmount` and `budget.spent` (already in base currency from backend).

- [ ] **Step 6: Update InvestmentsPage.tsx**

Three-tier display:

```tsx
<span>{inv.quantity} {inv.symbol}</span>
<span>{fmtMoney(inv.currentValue, inv.marketCurrency ?? inv.currency)}</span>
{(inv.marketCurrency ?? inv.currency) !== baseCurrency && inv.baseCurrentValue != null && (
  <span className="text-muted">({fmtCompact(inv.baseCurrentValue, baseCurrency)})</span>
)}
```

- [ ] **Step 7: Update GoalsPage.tsx and LiabilitiesPage.tsx**

Remove rates. Use `baseTargetAmount`/`baseCurrentAmount` and `basePrincipal`/`baseRemaining` for totals.

- [ ] **Step 8: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean (no unused imports from removed rate logic).

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/
git commit -m "feat(ui): remove toBase/rate queries, use pre-computed base_amount fields"
```

---

## Chunk 6: Config, Skills, Integration

### Task 18: Update Config Schema

**Files:**
- Modify: `crates/feature-finance/src/config.rs`

- [ ] **Step 1: Add `exchange_rates` field to FinanceConfig**

The current `FinanceConfig` in `crates/feature-finance/src/config.rs` has no `exchange_rates` field. Add it:

```rust
/// Manual exchange rate overrides. Key format: "FROM:TO" (e.g., "THB:VND").
/// Takes precedence over API-fetched rates. Optional — API rates used by default.
#[serde(default)]
pub exchange_rates: Option<HashMap<String, f64>>,
```

Add `use std::collections::HashMap;` at the top if not already imported.

- [ ] **Step 2: Wire config overrides into PriceService::get_rate()**

In `get_rate()`, check config overrides first:

```rust
// At the top of get_rate(), before cache lookup:
if let Some(rate) = Self::config_override_rate(&self.exchange_rate_overrides, &from, &to) {
    return Ok(rate);
}
```

Add `exchange_rate_overrides: Option<HashMap<String, f64>>` to `PriceService` struct, populated from `FinanceConfig.exchange_rates` during construction.

- [ ] **Step 2: Commit**

```bash
git add crates/feature-finance/src/config.rs
git commit -m "feat(finance): change exchange_rates config key to FROM:TO format"
```

---

### Task 19: Update Agent Skills

**Files:**
- Modify: `skills/finance-management/SKILL.md`
- Create: `skills/finance-management/references/currency-engine.md`
- Modify: `.claude/skills/klyntbot-finance/SKILL.md`
- Modify: `.claude/skills/klyntbot-finance/references/actions.md`

- [ ] **Step 1: Add currency-engine reference to orchestrator skill**

Create `skills/finance-management/references/currency-engine.md`:

```markdown
# Currency Engine

## Auto-Conversion
All monetary records are stored with both original currency and base-currency equivalent.
When a user records a transaction in THB while their home currency is VND,
the system auto-fetches the exchange rate and stores both amounts.

## Rate Sources
- Forex: open.er-api.com (15-min cache)
- Crypto: CoinGecko (15-min cache)
- User overrides in config take precedence

## Investment Display
Three-tier: quantity + market price + home equivalent.
Example: "0.5 BTC — $25,000 (637,500,000đ)"

## Changing Home Currency
When a user asks to change their default currency, the system:
1. Re-computes all base_amount fields across all tables
2. Shows progress indicator
3. Surfaces any conversion failures
```

- [ ] **Step 2: Update orchestrator SKILL.md**

Add triggers: "change default currency", "switch to VND/USD/EUR", "convert currency"
Add to decision flow: currency-related intents → check `references/currency-engine.md`
Remove: any references to manual `exchangeRates` config

- [ ] **Step 3: Update Claude Code MCP skill**

In `.claude/skills/klyntbot-finance/SKILL.md`:
- Add to common mistakes: "Don't pass `base_amount` — it's computed automatically from `amount` + `currency`"
- Add to common mistakes: "Don't manually configure exchange rates — they're auto-fetched"
- Update quick reference: note that `investment_add` accepts optional `market_currency`

- [ ] **Step 4: Update actions.md**

In `.claude/skills/klyntbot-finance/references/actions.md`:
- Add `market_currency` param to `investment_add` and `investment_update` tables
- Add note to all write actions: "base_amount/base_currency/exchange_rate are computed automatically"
- Add new action: `settings_update` with `default_currency` param (triggers rebase)

- [ ] **Step 5: Commit**

```bash
git add skills/finance-management/ .claude/skills/klyntbot-finance/
git commit -m "feat(skills): update finance agent skills for currency engine"
```

---

### Task 20: Wire Rate Cache into FinanceTool + Integration Test

**Files:**
- Modify: `crates/feature-finance/src/tool/mod.rs`
- Modify: `crates/feature-finance/src/lib.rs`
- Modify: `crates/app-core/src/init/storage.rs` (if needed for RateCache construction)

- [ ] **Step 1: Add `RateCache` to `FinanceTool` struct**

Add `rate_cache: Option<RateCache>` to `FinanceTool`. Pass it to handler methods. Update the constructor.

- [ ] **Step 2: Update `from_storage_pool()` convenience constructor**

`FinanceTool::from_storage_pool()` (used by `FinanceFeature` and tests) currently creates `PriceService::new(15)` without a rate cache. Update it to wire the rate cache:

```rust
pub fn from_storage_pool(pool: &SqlitePool, default_currency: &str) -> Self {
    let storage = FinanceStorage::from_pool(pool);
    let rate_cache = RateCache::new(
        storage.exchange_rates.clone(),
        15, // default TTL
    );
    let price_service = PriceService::with_rate_cache(15, rate_cache.clone());
    Self {
        storage,
        price_service,
        rate_cache: Some(rate_cache),
        default_currency: default_currency.to_string(),
        // ... other fields
    }
}
```

Also update `FinanceFeature::for_tests()` in `lib.rs` if it constructs `FinanceTool` directly.

- [ ] **Step 3: Wire RateCache in production path (app-core)**

If `FinanceTool` is constructed somewhere else in `app-core` (not via `from_storage_pool`), update that path too:

```rust
let rate_cache = RateCache::new(
    repos.finance.exchange_rates.clone(),
    finance_config.price_refresh.cache_ttl_minutes as i64,
);
let price_service = PriceService::with_rate_cache(
    finance_config.price_refresh.cache_ttl_minutes as u32,
    rate_cache.clone(),
);
```

- [ ] **Step 4: Write handler-level integration test**

Test that the handler actually calls `ensure_base_amount()`, not just that the repo can round-trip columns:

```rust
#[tokio::test]
async fn test_account_add_same_currency_base_conversion() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let tool = FinanceTool::from_storage_pool(pool.inner(), "USD");
    let args = serde_json::json!({
        "action": "account_add",
        "name": "Chase",
        "account_type": "bank",
        "currency": "USD",
        "balance": 100000
    });
    let result = tool.execute(&args, &Default::default()).await.unwrap();
    // Parse result, verify base_balance == 100000, exchange_rate == 1.0
    let response: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(response["base_balance"], 100000);
    assert_eq!(response["base_currency"], "USD");
    assert_eq!(response["exchange_rate"], 1.0);
}
```

Also add a repo-level test for completeness:

```rust
#[tokio::test]
async fn test_repo_roundtrip_base_fields() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = FinanceAccountRepo::new(pool.inner().clone());
    // Build full row with base fields...
    repo.add(&row).await.unwrap();
    let fetched = repo.get_or_err("test-1").await.unwrap();
    assert_eq!(fetched.base_balance, 100_000);
    assert_eq!(fetched.exchange_rate, 1.0);
}
```

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

- [ ] **Step 6: Final commit**

```bash
git add crates/feature-finance/ crates/app-core/
git commit -m "feat(finance): wire RateCache into FinanceTool and add integration tests"
```

---

## Chunk 7: Verification

### Task 21: End-to-End Verification

- [ ] **Step 1: Reset database**

Delete `~/.klyntbot-dev/data.db` to get fresh schema.

- [ ] **Step 2: Start the app**

Run: `cargo tauri dev`
Expected: App starts, migrations run, no errors.

- [ ] **Step 3: Test via MCP — same-currency flow**

```
mcp__klyntbot__finance(action: "account_add", name: "Chase", account_type: "bank", currency: "USD", balance: 500000)
```
Expected: Account created with `base_balance=500000, base_currency=USD, exchange_rate=1.0`.

- [ ] **Step 4: Test via MCP — cross-currency flow**

```
mcp__klyntbot__finance(action: "account_add", name: "VN Bank", account_type: "bank", currency: "VND", balance: 45000000)
```
Expected: Account created with `base_balance=<converted>, base_currency=USD, exchange_rate=<fetched>`.

- [ ] **Step 5: Test via MCP — investment with market_currency**

```
mcp__klyntbot__finance(action: "investment_add", portfolio_id: "<id>", asset_type: "crypto", symbol: "BTC", name: "Bitcoin", quantity: 0.5, cost_basis: 1200000000000, currency: "VND", market_currency: "USD")
```
Expected: Investment with `purchase_rate` (VND→USD) and `market_rate` (USD→USD = 1.0).

- [ ] **Step 6: Check UI at localhost:1420/#/finance**

Verify:
- Overview shows totals in base currency
- Accounts show original + base equivalent for foreign currencies
- Investments show three-tier display (quantity + market price + home equivalent)
- No `toBase()` errors in console

- [ ] **Step 7: Run full test suite one more time**

Run: `cargo nextest run --workspace && cd desktop-ui && bun run lint:fix && bun run test`
Expected: All green.
