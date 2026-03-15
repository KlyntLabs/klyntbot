# Cash Flow Tab — Merge Accounts + Transactions + Budgets

**Date:** 2026-03-16
**Scope:** Merge 3 finance sub-pages into one unified "Cash Flow" tab, reduce nav from 7 to 5 tabs
**Mockup:** `.superpowers/brainstorm/68142-1773594139/cashflow-layout.html`

## Problem

The current finance section has 7 tabs (Dashboard, Accounts, Transactions, Budgets, Investments, Goals, Liabilities). Accounts, Transactions, and Budgets are closely related — they all answer "what's happening with my money day-to-day?" — but are split into separate pages. The user must jump between tabs to understand their spending patterns, and there's no calendar heatmap or time-period-based analysis.

## Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Tab name | Cash Flow | Accurate financial term for money in/out tracking |
| Nav structure | 5 tabs (Dashboard, Cash Flow, Investments, Goals, Liabilities) | Cleanest simplification without cramming |
| Default period | Current month | Most useful at a glance, natural unit for budgets |
| Layout | Sidebar calendar | Heatmap always visible while scrolling transactions |
| Heatmap click | Filter + day summary card | Immediate context without complexity |

## New Nav

**Before:** Dashboard | Accounts | Transactions | Budgets | Investments | Goals | Liabilities (7 tabs)
**After:** Dashboard | **Cash Flow** | Investments | Goals | Liabilities (5 tabs)

Remove routes: `/finance/accounts`, `/finance/transactions`, `/finance/budgets`
Add route: `/finance/cashflow`
Add `<Navigate>` redirects for the 3 deleted routes → `/finance/cashflow` (follows project convention, see existing redirects in `router.tsx`)
Update `FinanceLayout` `subNav` array and router config.

## Cash Flow Page Layout

### Toolbar Row (full width)

- Left: Period navigation — `◄` previous | **"March 2026"** | `►` next
- Center/right: Period mode toggle — `[Day] [Week] [Month] [Year]` (pill buttons, month active by default)
- The existing privacy toggle and currency toggle remain in FinanceLayout

### Stats Row (full width, left of sidebar)

4 stat cards scoped to the selected period:
- **Income** — total income for the period (green)
- **Spending** — total expenses for the period (red)
- **Net** — income minus spending (blue, + or -)
- **Savings Rate** — `(income - spending) / income * 100` (orange, %)

### Main Content Area — Left Column (~66%)

**Top Categories** section:
- Ranked list of spending categories for the period
- Each row: color dot, category name, amount, percentage of total spending
- Max 6 categories shown, rest collapsed under "Show more"

**Day Summary Card** (conditionally shown when a heatmap day is clicked):
- Appears between categories and transactions
- Shows: "March 13 — 2 transactions, spent $128.70"
- Category breakdown for that day as small pills
- Click "×" or click the day again in heatmap to deselect

**Transactions** section:
- Header: "TRANSACTIONS" with "+ Add" button (opens slide panel)
- Filter bar: search input (300ms debounce) + type toggle `[All] [Income] [Expense] [Transfer]` + account dropdown
- Transaction list: date | type icon (colored bg) | name + category | account | amount
- When a day is selected in heatmap, list filters to that day only
- Pagination or "load more" for long lists (show 20 at a time)

### Sidebar — Right Column (~34%)

**Calendar Heatmap** (sticky — doesn't scroll with main content):
**Layout note:** `FinanceLayout` wraps content in `overflow-y-auto`. The `CashFlowPage` must use a two-column flex layout where the right sidebar has `position: sticky; top: 0; align-self: flex-start` so it stays fixed while the left column scrolls within the parent's overflow container.
- Full month grid (7 columns for M-T-W-T-F-S-S)
- Day cells colored by spending intensity: 5 levels from transparent (no spending) to deep red (high spending)
- Color scale: `rgba(244,63,94, 0/0.12/0.25/0.4/0.6)` — mapped by quartiles of the month's daily spending
- Today's cell has an orange outline (`border: 1.5px solid var(--brand)`)
- Clickable days — selected day gets a highlight ring
- Below the grid: "Less ░░░░░ More" legend
- In **Week mode**: shows a horizontal 7-day strip instead of full grid
- In **Day mode**: calendar still shows full month but with the selected day highlighted
- In **Year mode**: shows 12 mini-month grids (3×4 layout), each month clickable to drill down

**Accounts** section (below calendar):
- Compact list: icon, name, balance
- Each row clickable — clicking an account filters transactions to that account
- Active filter shown with highlight state
- No "Add Account" button here — manage accounts via "+ Add" in transactions or a settings flow

**Budget Status** section (below accounts):
- Compact progress bars for each active budget
- Name, percentage, color-coded bar (green <50%, orange 50-80%, red >80%)
- Scoped to the current budget period (always shows current status, not historical). Budget progress is period-agnostic — it shows the budget's own period regardless of the selected Cash Flow period. This is intentional: budgets represent ongoing commitments, not historical snapshots.
- No dollar amounts (privacy-safe) — just percentages and bars

### Period Mode Behavior

**Month (default):**
- Full calendar heatmap in sidebar
- Stats/categories/transactions for the entire month
- Budgets show monthly progress

**Week:**
- Sidebar shows a horizontal 7-day strip heatmap (Mon–Sun)
- Stats/categories/transactions for that week
- Navigation: ◄/► move by week

**Day:**
- Sidebar shows full month calendar with the day highlighted
- Stats show that single day's income/spending
- Transactions filtered to that day
- Navigation: ◄/► move by day

**Year:**
- Sidebar shows 12 mini-month heatmap grids (3 columns × 4 rows)
- Stats show annual totals
- Categories show full-year breakdown
- Transactions show all year (paginated)
- Click a month to drill into that month

## Heatmap Data

### New Backend Query: `finance_daily_spending`

Returns daily spending totals for a date range, used to color the heatmap cells.

```rust
pub struct FinanceDailySpendingResponse {
    pub days: Vec<DailySpending>,
}

pub struct DailySpending {
    pub date: String,        // "2026-03-13"
    pub total_spending: i64, // base currency cents
    pub tx_count: i32,
}
```

**Params:** `date_from: String, date_to: String`

Implementation: SQL aggregate on `finance_transactions` grouped by `tx_date`, filtered to `tx_type = 'expense'`, summing `base_amount`. Uses existing `base_amount` column — no schema changes needed.

### Heatmap Color Mapping

Compute quartiles from the month's daily spending values:
- Level 0: $0 (no spending) → `rgba(255,255,255,0.03)`
- Level 1: > $0, ≤ Q1 → `rgba(244,63,94,0.12)`
- Level 2: > Q1, ≤ Q2 (median) → `rgba(244,63,94,0.25)`
- Level 3: > Q2, ≤ Q3 → `rgba(244,63,94,0.4)`
- Level 4: > Q3 → `rgba(244,63,94,0.6)`

Computed client-side from the `finance_daily_spending` response. **Note:** `total_spending` values are in base currency cents — quartile thresholds must be computed on the raw cent values. No conversion to dollars needed for color mapping since we only care about relative ranking.

## Existing Queries Reused

| Query | Used For |
|-------|----------|
| `finance_accounts` | Sidebar account list, transaction account filter |
| `finance_transactions_filtered` | Main transaction list (already supports txType, accountId, query params) |
| `finance_budget_usage` | Sidebar budget progress bars |
| `finance_report_spending` | Top categories breakdown |
**Note:** `finance_transactions_filtered` already supports `date_from` and `date_to` params in the existing backend (`FinanceTransactionFilter` in `rows/finance.rs`, `list()` in `finance_transaction_repo.rs`, `FinanceTransactionFilterParams` in `desktop-shared`). **No backend changes needed** for transaction filtering — only frontend wiring.

### New Backend Query: `finance_period_summary`

The existing `finance_monthly_summary` is hardcoded to current/previous calendar month and cannot serve Day/Week/Year views. Add a new parameterized query:

```rust
pub struct FinancePeriodSummaryParams {
    pub date_from: String, // "2026-03-01"
    pub date_to: String,   // "2026-03-31"
}

pub struct FinancePeriodSummaryResponse {
    pub income: i64,        // base currency cents
    pub spending: i64,      // base currency cents
}
```

Implementation: Two SQL aggregates on `finance_transactions` filtered by `tx_date` range, one for `tx_type = 'income'`, one for `tx_type = 'expense'`, both summing `base_amount`. The Cash Flow page uses this for the stats row instead of `finance_monthly_summary`. The dashboard overview page continues to use `finance_monthly_summary` (which compares current vs previous month).

### Modified Backend Query: `finance_report_spending`

Add optional `date_from` and `date_to` params to scope the category breakdown to the selected period.

## New Files

| File | Purpose |
|------|---------|
| `pages/CashFlowPage.tsx` | Main page component |
| `components/SpendingHeatmap.tsx` | Calendar heatmap component |
| `components/DaySummary.tsx` | Day summary card (shown on day click) |
| `components/PeriodSelector.tsx` | Period nav (◄ March 2026 ►) + mode toggle (D/W/M/Y) |
| `components/CategoryRanking.tsx` | Top categories list for the period |
| `components/CashFlowStats.tsx` | 4 stat cards (income, spending, net, savings rate) |
| `lib/heatmapColors.ts` | Quartile computation + color mapping utility |
| `hooks/usePeriodState.ts` | Period state management (selected period, mode, navigation) |

**Backend:**

| File | Purpose |
|------|---------|
| `crates/desktop-shared/src/commands/finance.rs` | Add `FinanceDailySpendingResponse` + `FinancePeriodSummaryResponse` types |
| `crates/app-core/src/handlers/finance/reports.rs` | Add `finance_daily_spending` + `finance_period_summary` handlers |
| `crates/desktop/src/commands/finance.rs` | Add 2 Tauri commands, add both to `DEV_COMMANDS` array, add `dispatch_dev` match arms |
| `crates/desktop/src/main.rs` | Register both new commands in the Tauri builder |

## Modified Files

| File | Change |
|------|--------|
| `components/FinanceLayout.tsx` | Update `subNav` array: remove Accounts/Transactions/Budgets, add Cash Flow |
| `app/router.tsx` | Remove 3 routes, add `/finance/cashflow`, add 3 `<Navigate>` redirects from old routes, update lazy imports |
| `pages/FinanceOverviewPage.tsx` | Update all "View all →" `<Link>` targets: accounts → `/finance/cashflow`, transactions → `/finance/cashflow`, budgets section removed (uses BudgetStrip). Check for any remaining references to old routes. |

## Deleted Files

| File | Why |
|------|-----|
| `pages/AccountsPage.tsx` | Merged into Cash Flow |
| `pages/TransactionsPage.tsx` | Merged into Cash Flow |
| `pages/BudgetsPage.tsx` | Merged into Cash Flow |

## Interactions

### Heatmap Day Click
1. User clicks day cell (e.g., March 13)
2. Cell gets highlight ring
3. Day summary card appears above transaction list
4. Transaction list filters to that day
5. Categories update to show that day's breakdown
6. Stats row updates to show that day's totals
7. Click same day again → deselect, revert to full month view

### Account Filter
1. User clicks account in sidebar (e.g., "Vietcombank")
2. Account row gets highlight state
3. Transaction list filters to that account
4. Can combine with day filter (day + account)
5. Click same account again → deselect

### Period Navigation
1. ◄/► buttons move by period unit (day/week/month/year)
2. Heatmap, stats, categories, transactions all re-fetch for new period
3. Day selection resets on period change
4. URL updates with period params for deep linking: `/finance/cashflow?period=2026-03&mode=month`

## Privacy Masking

All amounts respect `usePrivacyMode()`:
- Stats row: income/spending/net masked, savings rate (%) stays visible
- Categories: amounts masked, percentages stay visible
- Transactions: amounts masked
- Accounts: balances masked
- Budgets: already percentage-only (no masking needed)
- Day summary: amount masked

## URL State

Period and mode are stored in URL search params for deep linking and back/forward navigation:
- `/finance/cashflow` → defaults to current month
- `/finance/cashflow?mode=month&period=2026-03` → March 2026 month view
- `/finance/cashflow?mode=week&period=2026-03-10` → week of March 10
- `/finance/cashflow?mode=day&period=2026-03-13` → March 13 day view
- `/finance/cashflow?mode=year&period=2026` → year 2026 view

Use `useSearchParams()` from React Router to read/write.

## Accessibility

- Heatmap cells are `role="button"` with `aria-label="March 13, 2 transactions"` (never includes amounts — uses tx count instead for privacy safety)
- Period navigation buttons have `aria-label="Previous month"` / `"Next month"`
- Mode toggle uses `role="tablist"` with `role="tab"` buttons
- Transaction filter toggles use `role="tablist"`
- Day summary has `role="status"` for screen reader announcement
