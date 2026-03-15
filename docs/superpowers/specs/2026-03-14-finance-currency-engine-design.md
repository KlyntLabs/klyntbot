# Finance Currency Engine — Design Spec

**Date:** 2026-03-14
**Status:** Draft
**Scope:** Multi-currency storage, auto-conversion, exchange rate service, investment display

## Problem

The current finance system stores `amount + currency` per entity but relies on manual exchange rates in config and frontend-side `toBase()` conversion for aggregation. This creates several issues:

1. **No historical rate tracking** — past transactions are re-valued at today's rate, distorting spending trends
2. **Manual rate management** — users must configure `exchangeRates` in config.json by hand
3. **Frontend aggregation burden** — every page fetches rates and converts client-side, duplicating logic
4. **Investment display gaps** — no distinction between purchase currency and market currency

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage model | Hybrid: original + base equivalent | Preserves original data, enables fast aggregation via pre-computed base amounts |
| Exchange rate freshness | On-demand, 15-min cache | Crypto volatility requires intraday rates; 15-min balances freshness vs API limits |
| Rate API | open.er-api.com (forex) + CoinGecko (crypto) | Already integrated in price_service.rs |
| Historical accuracy | Store rate at transaction time | Spending analysis reflects actual cost, not today's re-valuation |
| Investment display | Quantity + market currency price + home equivalent | Matches how exchanges present data; home equivalent shows personal relevance |
| Home currency change | Background batch re-computation | One-time operation, fetches fresh direct rates, flags failures |

## 1. Schema Changes

### 1.1 New columns on monetary tables

Every table with monetary amounts gets three columns:

```sql
base_amount    INTEGER,                        -- NULL = not yet backfilled (excluded from aggregation)
base_currency  TEXT,                           -- NULL = not yet backfilled
exchange_rate  REAL,                           -- NULL = not yet backfilled
```

**NULL policy — dual mode:**
- **New records (post-migration):** All three fields are NOT NULL at the application level — handlers always compute and store them. If a rate cannot be fetched, the write fails. `SUM(base_amount)` is always correct for backfilled data.
- **Existing records (pre-backfill):** Columns are nullable in DDL so the migration is instant (`ALTER TABLE ADD COLUMN` with no default). Rows with `base_amount IS NULL` are excluded from aggregation queries via `WHERE base_amount IS NOT NULL`. The async backfill fills them in.

This avoids the `DEFAULT 0` trap where un-backfilled rows silently corrupt SUMs.

**Affected tables and their new columns:**

| Table | New columns |
|-------|-------------|
| `finance_transactions` | `base_amount`, `base_currency`, `exchange_rate` |
| `finance_accounts` | `base_balance`, `base_currency`, `exchange_rate` |
| `finance_investments` | `market_currency`, `base_cost_basis`, `base_current_value`, `base_currency`, `purchase_rate`, `market_rate` |
| `finance_investment_transactions` | `base_total_amount`, `base_currency`, `exchange_rate` |
| `finance_goals` | `base_target_amount`, `base_current_amount`, `base_currency`, `exchange_rate` |
| `finance_liabilities` | `base_principal`, `base_remaining`, `base_currency`, `exchange_rate` |
| `finance_budgets` | `base_amount`, `base_currency`, `exchange_rate` |

**Rules:**
- When `currency == base_currency`: `exchange_rate = 1.0`, `base_amount = amount`. No API call.
- Original `amount` + `currency` fields are **never mutated** by conversion. They are immutable source-of-truth.
- `base_currency` is denormalized per-record (not global) so that records remain self-contained even if the user changes home currency later.

### 1.2 Investment `market_currency` field and dual rates

`finance_investments` gets a new `market_currency TEXT` column and **two** rate columns instead of one:
- `currency` = what you paid in (purchase currency, e.g., VND)
- `market_currency` = what exchanges quote the asset in (e.g., USD for BTC, USD for AAPL)
- `current_price` / `current_value` are denominated in `market_currency`
- `cost_basis` remains in `currency` (purchase currency)
- `base_cost_basis` / `base_current_value` are in `base_currency` (home currency)
- `purchase_rate` = rate from `currency → base_currency` (stored at purchase time)
- `market_rate` = rate from `market_currency → base_currency` (refreshed with price updates)

**Why two rates:** When `currency != market_currency` (e.g., buy BTC with VND, BTC quoted in USD), a single rate column cannot represent both conversion paths. `purchase_rate` converts `cost_basis` and `market_rate` converts `current_value`.

When `market_currency == base_currency`: `market_rate = 1.0`, `base_current_value = current_value`.

### 1.3 Exchange rate cache table

```sql
CREATE TABLE IF NOT EXISTS finance_exchange_rates (
    from_currency  TEXT NOT NULL,
    to_currency    TEXT NOT NULL,
    rate           REAL NOT NULL,
    fetched_at     TEXT NOT NULL,  -- ISO 8601 UTC
    PRIMARY KEY (from_currency, to_currency)
);

CREATE INDEX IF NOT EXISTS idx_exchange_rates_staleness
    ON finance_exchange_rates (to_currency, from_currency, fetched_at);
```

**Concurrency:** Rate cache upserts use `INSERT OR REPLACE` which is atomic in SQLite. The write-path wraps the full sequence (fetch rate → upsert cache → compute base_amount → insert record) in a single SQLite transaction, preventing two concurrent writes from observing different rates for the same pair.

**Sentinel rows:** Rows with `from_currency` starting with `__` are sentinel/status rows, not real rates. All rate cache lookups MUST exclude them: `WHERE from_currency NOT LIKE '__%'` (or check prefix in application code). Defined sentinels:
- `("__BACKFILL__", "__STATUS__")` — backfill progress tracker
- `("__REBASE__", <new_base>)` — rebase-in-progress marker

### 1.4 Backfill migration

Existing records are backfilled. **This is NOT a SQL migration** — it runs as an async post-migration step in `app-core` init (after `run_feature_migrations`), similar to how price refreshes work. The SQL migration only adds the columns with defaults.

1. SQL migration: `ALTER TABLE ... ADD COLUMN base_amount INTEGER` (nullable, no default) — schema-only, instant. All existing rows get NULL.
2. Post-migration async backfill (runs on app startup, tracked by a sentinel row in `finance_exchange_rates`: `from_currency = "__BACKFILL__"`, `to_currency = "__STATUS__"`, `rate = 0.0` when pending, `rate = 1.0` when complete):
   - Read `defaultCurrency` from config → this becomes `base_currency` for all existing records
   - For records where `currency == base_currency`: set `exchange_rate = 1.0`, `base_amount = amount`
   - For records with foreign currencies: batch-prefetch all needed rates via `prefetch_rates`, then compute `base_amount = round(amount * rate)`, store rate
   - If API is unreachable for a currency pair: leave those rows as NULL (excluded from aggregation). Log warning. Retry on next app startup.
   - Once all rows are backfilled, set status to `complete`. On startup, skip backfill if already `complete`.
3. Aggregation queries include `AND base_amount IS NOT NULL` until backfill is complete — this ensures partial backfill never corrupts totals.

## 2. Exchange Rate Service

### 2.1 Architecture

Upgrade the existing `price_service.rs` rather than creating a new service. Add a `RateCache` layer backed by the `finance_exchange_rates` table. This requires injecting a `StoragePool` (or a `FinanceExchangeRateRepo`) into `PriceService` — currently it has no DB dependency, only in-memory `DashMap` caches. The in-memory cache stays as L1 (hot path), the DB table becomes L2 (survives restarts).

```
Handler needs rate for (THB → VND)
    → RateCache.get("THB", "VND")
        → DB lookup: fresh (< 15 min)? → return cached rate
        → Stale or missing? → fetch from API
            → Success: update DB cache, return rate
            → Failure: stale cache exists? → return stale + log warning
            → Failure: no cache at all? → return error (block the write)
```

### 2.2 Fetch strategy

- **Single-pair fetch:** `price_service.fetch_exchange_rate(from, to)` — already exists
- **Batch prefetch:** New method `price_service.prefetch_rates(base_currency, &[currencies])` — calls `open.er-api.com/v6/latest/{base}` which returns all pairs in one HTTP call. Used on dashboard load.
- **Crypto rates:** Continue using CoinGecko via `fetch_crypto()`. Crypto→fiat rates are derived from the crypto price + forex rate. **Staleness note:** Both legs (crypto price + forex rate) have independent 15-min caches, so the derived rate can be up to 30 minutes stale in the worst case. This is acceptable for a personal finance dashboard — sub-minute accuracy is out of scope.
- **Cache TTL:** 15 minutes (configurable via `FinanceConfig.price_refresh`)

### 2.3 Remove manual config dependency

- `FinanceConfig.exchange_rates` changes from `HashMap<String, f64>` to `HashMap<String, f64>` where the key is `"FROM:TO"` (e.g., `"THB:VND"`). This allows overriding specific directional pairs rather than assuming a single target currency.
- If a user sets a manual rate for a pair, it takes precedence over API rates
- If no manual rate and no API rate: write fails with clear error message
- Frontend no longer queries `finance_exchange_rates` config — rates are a backend concern

## 3. Write Path — Auto-Conversion

### 3.1 Conversion logic

Every handler that creates/updates a monetary record follows this flow:

```
fn ensure_base_amount(amount, currency, base_currency, price_service) -> Result<(base_amount, exchange_rate)>:
    if currency == base_currency:
        return Ok((amount, 1.0))
    rate = price_service.get_rate(currency, base_currency)?  // fails if no rate available
    base_amount = round_half_up(amount as f64 * rate) as i64
    return Ok((base_amount, rate))
```

**Error behavior:** If `get_rate` fails (API down, no cache), the handler returns an error to the caller. The record is NOT inserted. This is a deliberate choice — we never store unconverted records.

**Rounding:** Uses half-away-from-zero (`round()` in Rust), which is standard for financial calculations on integer-cent amounts. Documented explicitly to prevent future changes to banker's rounding.

### 3.2 Affected handlers

| Handler | Fields converted |
|---------|-----------------|
| `transaction_add` / `transaction_update` | `amount` → `base_amount` |
| `account_add` / `account_update` | `balance` → `base_balance` |
| `investment_add` / `investment_update` | `cost_basis` → `base_cost_basis`, `current_value` → `base_current_value` |
| `investment_tx_add` | `total_amount` → `base_total_amount` |
| `goal_add` / `goal_update` | `target_amount` → `base_target_amount`, `current_amount` → `base_current_amount` |
| `liability_add` / `liability_update` | `principal` → `base_principal`, `remaining` → `base_remaining` |
| `budget_add` / `budget_update` | `amount` → `base_amount` |

### 3.3 Price refresh (investments)

When `price_service` refreshes `current_price` and `current_value` for investments:
1. Fetch asset price in `market_currency` (existing behavior)
2. Fetch rate for `market_currency → base_currency`
3. Update `base_current_value = round(current_value * market_rate)`
4. Update `market_rate` with the fresh rate
5. `purchase_rate` is NOT updated — it reflects the rate at time of purchase and is immutable after creation

### 3.4 MCP tool interface

No changes to external MCP action signatures. The `currency` parameter continues to accept any ISO 4217 code. Conversion is invisible to callers — they provide `amount + currency`, the system handles the rest.

`base_currency` is read from `FinanceConfig.default_currency` at write time. It is not a parameter.

## 4. Read Path — Aggregation & Display

### 4.1 Backend aggregation

All aggregation queries use `base_amount` columns:

```sql
-- Total spending this month (base_currency filter ensures no mixed-currency SUMs)
SELECT SUM(base_amount) FROM finance_transactions
WHERE tx_type = 'expense' AND base_currency = ?
  AND tx_date >= date('now', 'start of month')

-- Net worth
SELECT SUM(base_balance) FROM finance_accounts
WHERE is_active = TRUE AND base_currency = ?
UNION ALL
SELECT SUM(base_current_value) FROM finance_investments
WHERE is_active = TRUE AND base_currency = ?

-- Budget usage (simplified — no cross-currency filter needed)
SELECT b.base_amount, COALESCE(SUM(ft.base_amount), 0) AS spent
FROM finance_budgets b
LEFT JOIN finance_transactions ft ON ft.tx_type = 'expense'
    AND ft.base_currency = b.base_currency
    AND (b.category IS NULL OR ft.category = b.category)
    AND ft.tx_date >= ...period bounds...
WHERE b.id = ? AND b.base_currency = ?
GROUP BY b.id
```

The complex `AND ft.currency = b.currency` filter is no longer needed — all `base_amount` values are in the same currency.

### 4.2 Frontend changes

**Remove:**
- `toBase()` function from `finance.ts`
- `finance_exchange_rates` query from all page components
- `rates` prop threading through components

**Keep:**
- `fmtMoney(amount, currency)` — formats with correct symbol and decimals
- `fmtCompact(amount, currency)` — compact format with symbol

**Display pattern for detail views:**

```tsx
// When currency != baseCurrency, show both
{fmtMoney(item.amount, item.currency)}
{item.currency !== baseCurrency && (
  <span className="text-muted">({fmtMoney(item.baseAmount, baseCurrency)})</span>
)}
```

### 4.3 API response changes

Backend responses include both original and base fields. Example `FinanceTransactionResponse`:

```rust
pub struct TransactionDto {
    // ... existing fields ...
    pub amount: i64,
    pub currency: String,
    pub base_amount: Option<i64>,        // new — None for pre-backfill records
    pub base_currency: Option<String>,   // new — None for pre-backfill records
    pub exchange_rate: Option<f64>,      // new — None for pre-backfill records
}
```

**Pre-backfill safety:** During the window between migration and backfill completion, DTOs carry `None` for base fields. The frontend treats `None` as "conversion pending" and displays only the original amount. Aggregation endpoints skip rows with `base_amount IS NULL`. After backfill completes, all new records are guaranteed non-null at the application level.

Aggregation endpoints return totals in `base_currency` directly:

```rust
pub struct SpendingReportDto {
    pub total: i64,                      // already in base_currency
    pub base_currency: String,
    pub by_category: Vec<CategoryTotal>,
    // ...
}
```

## 5. Home Currency Change

### 5.1 Trigger

User updates `defaultCurrency` in config (via settings UI or config.json).

### 5.2 Re-computation flow

1. Detect currency change (old_base → new_base)
2. Write a durable sentinel row: `("__REBASE__", new_base, 0.0, now)` to `finance_exchange_rates`. Survives crashes — on startup, if this row exists, resume from step 3.
3. Fetch all distinct `currency` AND `market_currency` values across monetary tables (investments have both)
4. Batch-fetch rates for each distinct currency → new_base (single API call via `prefetch_rates`)
5. For each table, update **all rows in a single transaction** (SQLite handles large transactions efficiently for a personal finance app — thousands of rows, not millions):
   - If `currency == new_base`: set `base_amount = amount`, `exchange_rate = 1.0`
   - Else if rate available: `exchange_rate = fetched_rate`, `base_amount = round(amount * rate)`
   - Else (rate unavailable): set `base_amount = NULL`, `base_currency = NULL` (excluded from aggregation, same as pre-backfill state)
   - For investments: also recompute `base_current_value` using `market_currency → new_base` rate
   - Set `base_currency = new_base` only on successfully converted rows
6. Update `finance_exchange_rates` cache with new base pairs (sentinel rows excluded by `__` prefix convention)
7. Delete the `__REBASE__` sentinel row, emit UI refresh event

**Mixed-currency safety:** Each table is updated in a single atomic transaction — readers see either all-old or all-new, never a mix. Aggregation queries include `WHERE base_currency = ?` as an extra safety net. Failed rows have `base_currency = NULL` and are excluded. On crash recovery, the sentinel row triggers a full re-run from step 3.

### 5.3 Net worth snapshots

`finance_net_worth_snapshots` stores point-in-time totals with a `currency` field. On home currency change:
- **Do NOT re-compute historical snapshots** — they represent what the net worth was at that moment in that currency. Re-computing with today's rates would distort history.
- New snapshots after the change use `new_base`.
- The frontend groups snapshots by currency. If the user changed from USD to VND, the chart shows the USD era and VND era separately, with a clear boundary marker.
- The `snapshot_record` handler is updated to use `base_currency` from config when computing the snapshot.

### 5.4 Error handling

- If a rate cannot be fetched for a currency pair: set `base_amount = NULL`, `base_currency = NULL` (excluded from aggregation), log warning, add to failure list
- After batch completes: if failures exist, surface "X records in Y currencies couldn't be converted" to UI
- User can manually set override rates in config for exotic currencies, then re-trigger

### 5.5 UI during re-computation

- Show "Updating currency..." indicator (toast or banner) — triggered by detecting the `__REBASE__` sentinel row
- Aggregation queries use `WHERE base_currency = ?` and each table updates atomically, so no mixed-currency SUMs
- Once complete: sentinel row deleted, UI event emitted to trigger data refresh on all open pages

## 6. Investment Display

### 6.1 Three-tier display

| Element | Source | Example |
|---------|--------|---------|
| Quantity | `quantity` field | "0.5 BTC" |
| Market value | `current_value` + `market_currency` | "$25,000" |
| Home equivalent | `base_current_value` + `base_currency` | "(637,500,000đ)" |

When `market_currency == base_currency`: skip home equivalent (no parenthetical).

### 6.2 Return calculations

Computed in `base_currency` for consistency:

```
return_amount = base_current_value - base_cost_basis
return_pct = (base_current_value - base_cost_basis) / base_cost_basis * 100
```

This captures both asset appreciation and currency movement — reflecting the user's **real return** in their home currency.

### 6.3 Portfolio summary

```
Total Portfolio Value: 1,850,000,000đ        (sum of base_current_value)
Total Cost Basis:      1,500,000,000đ        (sum of base_cost_basis)
Total Return:          +350,000,000đ (+23.3%) (in base_currency)

Breakdown:
  0.5 BTC    — $25,000 (637,500,000đ)    +5.2%
  100 AAPL   — $17,520 (447,000,000đ)    +12.1%
  VN Bonds   — 765,500,000đ               +2.3%
```

## 7. Agent Skills Update

Both the internal orchestrator skills and the Claude Code MCP skills must be updated to reflect the currency engine changes.

### 7.1 Internal orchestrator skill (`skills/finance-management/`)

Update `SKILL.md` decision flowchart and references:
- Add currency-aware guidance: when a user mentions a foreign currency, the agent should note the auto-conversion behavior ("I'll record this in THB and it will be automatically converted to your home currency VND")
- Update `references/` docs to document `market_currency` for investments, the three-tier display, and the fact that `base_amount` handles aggregation
- Remove any references to manual `exchangeRates` config as a required step
- Add triggers for currency-change intent ("change my default currency", "switch to VND")

### 7.2 Claude Code MCP skill (`.claude/skills/klyntbot-finance/`)

Update `SKILL.md` and `references/actions.md`:
- Document that `currency` parameter on all write actions auto-converts to home currency
- Document `market_currency` parameter for `investment_add`
- Remove references to manual exchange rate configuration
- Add common mistakes: "Don't pass `base_amount` — it's computed automatically", "Don't set `exchange_rate` manually — the system fetches it"
- Update quick reference with the new display behavior (original + home equivalent)

### 7.3 Finance management references

Update or add reference docs:
- `references/currency-engine.md` — how auto-conversion works, rate sources, cache behavior
- `references/investment-display.md` — three-tier display rules, market_currency vs currency vs base_currency
- Update `references/analytics-actions.md` — note that analytics now operate on `base_amount` (single currency), simplifying cross-currency analysis

## 8. Scope Boundaries

### In scope
- Schema changes (new columns + rate cache table)
- Exchange rate service upgrade (DB cache, auto-fetch, batch prefetch)
- Write-path auto-conversion on all monetary handlers
- Read-path aggregation via base_amount
- Frontend removal of toBase() and client-side rate logic
- Investment market_currency field and three-tier display
- Home currency change with batch re-computation
- Backfill migration for existing records
- Agent skills update (internal orchestrator + Claude Code MCP skills)

### Out of scope
- Historical rate time-series storage (only store rate-at-time-of-record, not daily rate history)
- Multi-home-currency (one home currency at a time)
- Currency conversion UI calculator
- Forex gain/loss tracking as a separate line item (captured implicitly in return calculations)
- Real-time websocket rate streaming
