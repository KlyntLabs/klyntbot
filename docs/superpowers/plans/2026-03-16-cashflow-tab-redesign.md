# Cash Flow Tab Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge Accounts + Transactions + Budgets into a unified "Cash Flow" tab with sidebar calendar heatmap, period-based filtering (day/week/month/year), and spending analysis.

**Architecture:** New `CashFlowPage` replaces 3 existing pages. Sidebar calendar heatmap powered by a new `finance_daily_spending` backend query. Period-scoped stats from a new `finance_period_summary` query. Existing `finance_transactions_filtered` and `finance_report_spending` already support date ranges — just wire them up. Nav reduced from 7 to 5 tabs with redirects for old routes.

**Tech Stack:** React, Tailwind v4 CSS tokens, Lucide icons, Tauri commands (Rust), SQLite aggregation, React Router `useSearchParams` for URL state

**Spec:** `docs/superpowers/specs/2026-03-16-cashflow-tab-redesign.md`

---

## Chunk 1: Backend Queries

### Task 1: `finance_daily_spending` Backend Query

**Files:**
- Modify: `crates/desktop-shared/src/commands/finance.rs` — add types
- Modify: `crates/app-core/src/handlers/finance/reports.rs` — add handler
- Modify: `crates/desktop/src/commands/finance.rs` — add Tauri command + DEV_COMMANDS + dispatch_dev
- Modify: `crates/desktop/src/main.rs` — register command

- [ ] **Step 1: Add response types in desktop-shared**

In `crates/desktop-shared/src/commands/finance.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySpending {
    pub date: String,
    pub total_spending: i64,
    pub tx_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDailySpendingResponse {
    pub days: Vec<DailySpending>,
}
```

- [ ] **Step 2: Add handler in app-core**

In `crates/app-core/src/handlers/finance/reports.rs`, add:

```rust
pub async fn finance_daily_spending(
    &self,
    date_from: String,
    date_to: String,
) -> Result<FinanceDailySpendingResponse, ApiError> {
    let currency = self.default_currency().await;
    let rows = self.repos.finance.transactions
        .daily_spending_totals(&date_from, &date_to, &currency)
        .await
        .map_err(map_storage_err)?;
    Ok(FinanceDailySpendingResponse { days: rows })
}
```

This requires a new repo method `daily_spending_totals` on `FinanceTransactionRepo`. Add it in `crates/storage/src/repos/finance_transaction_repo.rs`:

```rust
pub async fn daily_spending_totals(
    &self,
    date_from: &str,
    date_to: &str,
    base_currency: &str,
) -> Result<Vec<DailySpending>> {
    let rows = sqlx::query_as!(
        DailySpendingRow,
        r#"SELECT tx_date as date, SUM(base_amount) as total_spending, COUNT(*) as tx_count
           FROM finance_transactions
           WHERE tx_type = 'expense'
             AND tx_date >= ? AND tx_date <= ?
             AND base_currency = ?
           GROUP BY tx_date
           ORDER BY tx_date"#,
        date_from, date_to, base_currency
    )
    .fetch_all(&*self.pool)
    .await?;
    // Map to DailySpending response type
}
```

Note: You may need to define a local row struct or use `query_as` with `DailySpending` imported from `desktop-shared`. Check the existing pattern in this file — other methods use `sqlx::query!` with manual mapping. Follow whichever pattern the file uses.

Add import: `use desktop_shared::commands::finance::{DailySpending, FinanceDailySpendingResponse};`

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/finance.rs`:

```rust
#[tauri::command]
pub async fn finance_daily_spending(
    state: State<'_, Arc<AppCore>>,
    date_from: String,
    date_to: String,
) -> Result<FinanceDailySpendingResponse, ApiError> {
    state.finance_daily_spending(date_from, date_to).await
}
```

Add `"finance_daily_spending"` to `DEV_COMMANDS`. Add `dispatch_dev` match arm. Register in `main.rs`.

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(finance): add finance_daily_spending backend query"
```

### Task 2: `finance_period_summary` Backend Query

**Files:**
- Modify: `crates/desktop-shared/src/commands/finance.rs` — add types
- Modify: `crates/app-core/src/handlers/finance/reports.rs` — add handler
- Modify: `crates/desktop/src/commands/finance.rs` — add Tauri command + DEV_COMMANDS + dispatch_dev
- Modify: `crates/desktop/src/main.rs` — register command

- [ ] **Step 1: Add response type in desktop-shared**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePeriodSummaryResponse {
    pub income: i64,
    pub spending: i64,
}
```

- [ ] **Step 2: Add handler in app-core**

```rust
pub async fn finance_period_summary(
    &self,
    date_from: String,
    date_to: String,
) -> Result<FinancePeriodSummaryResponse, ApiError> {
    let currency = self.default_currency().await;
    let income = self.repos.finance.transactions
        .sum_by_type_in_range("income", &date_from, &date_to, &currency)
        .await
        .map_err(map_storage_err)?;
    let spending = self.repos.finance.transactions
        .sum_by_type_in_range("expense", &date_from, &date_to, &currency)
        .await
        .map_err(map_storage_err)?;
    Ok(FinancePeriodSummaryResponse { income, spending })
}
```

Add a `sum_by_type_in_range` method to `FinanceTransactionRepo`:

```rust
pub async fn sum_by_type_in_range(
    &self,
    tx_type: &str,
    date_from: &str,
    date_to: &str,
    base_currency: &str,
) -> Result<i64> {
    let row = sqlx::query_scalar!(
        r#"SELECT COALESCE(SUM(base_amount), 0) as "total!: i64"
           FROM finance_transactions
           WHERE tx_type = ? AND tx_date >= ? AND tx_date <= ? AND base_currency = ?"#,
        tx_type, date_from, date_to, base_currency
    )
    .fetch_one(&*self.pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 3: Add Tauri command, DEV_COMMANDS, dispatch_dev, register in main.rs**

Same pattern as Task 1.

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(finance): add finance_period_summary backend query"
```

### Task 3: Frontend Types

**Files:**
- Modify: `desktop-ui/src/shared/types/finance.ts`

- [ ] **Step 1: Add TypeScript types**

```typescript
export interface DailySpending {
  date: string;
  totalSpending: number;
  txCount: number;
}

export interface FinanceDailySpendingResponse {
  days: DailySpending[];
}

export interface FinancePeriodSummary {
  income: number;
  spending: number;
}
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add DailySpending and PeriodSummary frontend types"
```

---

## Chunk 2: Period State Hook + Utility Functions

### Task 4: `usePeriodState` Hook

**Files:**
- Create: `desktop-ui/src/features/finance/hooks/usePeriodState.ts`

- [ ] **Step 1: Implement period state management**

This hook manages the selected period mode (day/week/month/year), current period value, and navigation (prev/next). It syncs with URL search params via `useSearchParams`.

```typescript
// desktop-ui/src/features/finance/hooks/usePeriodState.ts
import { useCallback, useMemo } from "react";
import { useSearchParams } from "react-router";

export type PeriodMode = "day" | "week" | "month" | "year";

export interface PeriodState {
  mode: PeriodMode;
  /** ISO date string for the period start: "2026-03-01" (month), "2026-03-10" (week/day), "2026" (year) */
  period: string;
  /** Formatted label: "March 2026", "Mar 10–16, 2026", "March 13, 2026", "2026" */
  label: string;
  /** Date range for queries */
  dateFrom: string;
  dateTo: string;
  setMode: (mode: PeriodMode) => void;
  prev: () => void;
  next: () => void;
  /** Select a specific day (e.g., from heatmap click) */
  selectDay: (date: string | null) => void;
  selectedDay: string | null;
}
```

Implementation details:
- Read `mode` and `period` from `useSearchParams()`, default to `mode=month` and `period=YYYY-MM` of current date
- `setMode` updates the URL param and recomputes `period` to fit the new mode
- `prev`/`next` shift the period by the mode unit (1 day, 1 week, 1 month, 1 year)
- `dateFrom`/`dateTo` computed from mode+period: month → "2026-03-01"/"2026-03-31", week → monday/sunday, day → same date for both, year → "2026-01-01"/"2026-12-31"
- `selectedDay` is local state (not URL — too transient), reset on period change
- Use `Intl.DateTimeFormat` for label formatting (CLAUDE.md says use Intl, not hardcoded formats)
- Week starts on Monday

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add usePeriodState hook with URL sync"
```

### Task 5: Heatmap Color Utility

**Files:**
- Create: `desktop-ui/src/features/finance/lib/heatmapColors.ts`

- [ ] **Step 1: Implement quartile-based color mapping**

```typescript
// desktop-ui/src/features/finance/lib/heatmapColors.ts

export const HEATMAP_COLORS = [
  "rgba(255,255,255,0.03)",    // level 0: no spending
  "rgba(244,63,94,0.12)",      // level 1: ≤ Q1
  "rgba(244,63,94,0.25)",      // level 2: ≤ Q2
  "rgba(244,63,94,0.4)",       // level 3: ≤ Q3
  "rgba(244,63,94,0.6)",       // level 4: > Q3
] as const;

export type HeatmapLevel = 0 | 1 | 2 | 3 | 4;

/**
 * Compute heatmap levels for daily spending values.
 * Uses quartile-based thresholds on raw cent values.
 */
export function computeHeatmapLevels(
  dailyValues: Map<string, number>,
): Map<string, HeatmapLevel> {
  const result = new Map<string, HeatmapLevel>();
  const nonZero = [...dailyValues.values()].filter((v) => v > 0).sort((a, b) => a - b);

  if (nonZero.length === 0) {
    for (const [date] of dailyValues) result.set(date, 0);
    return result;
  }

  const q1 = nonZero[Math.floor(nonZero.length * 0.25)] ?? nonZero[0];
  const q2 = nonZero[Math.floor(nonZero.length * 0.5)] ?? q1;
  const q3 = nonZero[Math.floor(nonZero.length * 0.75)] ?? q2;

  for (const [date, value] of dailyValues) {
    if (value <= 0) result.set(date, 0);
    else if (value <= q1) result.set(date, 1);
    else if (value <= q2) result.set(date, 2);
    else if (value <= q3) result.set(date, 3);
    else result.set(date, 4);
  }
  return result;
}
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add heatmap color quartile utility"
```

---

## Chunk 3: New Components

### Task 6: PeriodSelector Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/PeriodSelector.tsx`

- [ ] **Step 1: Implement period navigation + mode toggle**

A toolbar component with:
- Left: `◄` button | period label (e.g., "March 2026") | `►` button
- Right: mode pill toggle `[Day] [Week] [Month] [Year]`

Props: `{ label, mode, onPrev, onNext, onSetMode }` — all from `usePeriodState`.

Use `role="tablist"` for mode toggle, `aria-label` on nav buttons. Use existing glass button styles from the design system.

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add PeriodSelector component"
```

### Task 7: SpendingHeatmap Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/SpendingHeatmap.tsx`

- [ ] **Step 1: Implement calendar heatmap**

A month calendar grid component. Props:
```typescript
{
  year: number;
  month: number; // 1-12
  levels: Map<string, HeatmapLevel>;
  dailyCounts: Map<string, number>; // tx count per day for aria-labels
  selectedDay: string | null;
  onSelectDay: (date: string | null) => void;
  today: string; // "2026-03-16" for orange outline
}
```

Implementation:
- Compute first day of month, number of days, leading empty cells (week starts Monday)
- Render 7-column CSS grid with M/T/W/T/F/S/S headers
- Each day cell: colored background from `HEATMAP_COLORS[level]`, date number, `role="button"`, `aria-label="March 13, 2 transactions"`
- Today cell: orange outline (`outline: 1.5px solid var(--brand)`)
- Selected day: highlight ring (`outline: 2px solid var(--info)`)
- Click toggles selection (click same day = deselect)
- Below grid: "Less ░░░░░ More" legend with 5 color swatches

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add SpendingHeatmap component"
```

### Task 8: CashFlowStats Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/CashFlowStats.tsx`

- [ ] **Step 1: Implement 4 stat cards**

Props: `{ income, spending, displayCur, convertTotal, hidden }` — all in base currency cents.

Renders 4 glass cards in a row:
- Income (green, `border-left: 3px solid var(--success)`)
- Spending (red, `border-left: 3px solid var(--destructive)`)
- Net = income - spending (blue)
- Savings Rate = `income > 0 ? Math.round((income - spending) / income * 100) : 0` (orange, shows `%`)

Use `fmtCompact` with `hidden` param for amounts. Savings rate is a percentage — stays visible when masked.

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add CashFlowStats component"
```

### Task 9: CategoryRanking Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/CategoryRanking.tsx`

- [ ] **Step 1: Implement top categories list**

Props: `{ breakdown: { category, total }[], hidden }` — from `finance_report_spending` response.

Renders ranked list:
- Each row: color dot (from COLORS array), category name, amount (fmtCompact + hidden), percentage
- Max 6 shown, "Show more" toggle if > 6
- Sorted by total descending (already sorted from backend)

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add CategoryRanking component"
```

### Task 10: DaySummary Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/DaySummary.tsx`

- [ ] **Step 1: Implement day summary card**

Props: `{ date, txCount, totalSpending, categories: string[], onClose, hidden }`.

Shown conditionally when a day is selected in the heatmap. Renders:
- "March 13 — 2 transactions, spent $128.70" (amount masked when hidden)
- Category pills as small tags
- "×" close button (calls `onClose` which deselects the day)
- `role="status"` for screen reader announcement

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(finance): add DaySummary component"
```

---

## Chunk 4: Cash Flow Page

### Task 11: CashFlowPage — Main Page Component

**Files:**
- Create: `desktop-ui/src/features/finance/pages/CashFlowPage.tsx`

- [ ] **Step 1: Implement the full Cash Flow page**

This is the core integration task. The page composes all the new components.

**Data fetches:**
```typescript
const period = usePeriodState();
const { hidden, toggle } = usePrivacyMode();
const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } = useFinanceCurrency();

// Period-scoped data
const { data: periodSummary } = useQuery<FinancePeriodSummary>(
  "finance_period_summary", { dateFrom: period.dateFrom, dateTo: period.dateTo }, { income: 0, spending: 0 });
const { data: dailySpending } = useQuery<FinanceDailySpendingResponse>(
  "finance_daily_spending", { dateFrom: period.dateFrom, dateTo: period.dateTo }, { days: [] });
const { data: spendingReport } = useQuery<FinanceCategoryReport>(
  "finance_report_spending", { dateFrom: period.dateFrom, dateTo: period.dateTo }, { total: 0, breakdown: [] });
const { data: transactions } = useQuery<FinanceTransaction[]>(
  "finance_transactions_filtered", {
    dateFrom: period.selectedDay ?? period.dateFrom,
    dateTo: period.selectedDay ?? period.dateTo,
    txType: txFilter, accountId: accountFilter, query: searchQuery, limit: 50
  }, []);

// Non-period-scoped (always current)
const { data: accounts } = useQuery<FinanceAccount[]>("finance_accounts", undefined, []);
const { data: budgets } = useQuery<FinanceBudgetUsage[]>("finance_budget_usage", undefined, []);
```

**Layout structure:**
```
<FinanceLayout hidden={hidden} onTogglePrivacy={toggle} currencyMode={mode} currencies={currencies} onSelectCurrency={setMode}>
  <PeriodSelector ... />
  <CashFlowStats income={periodSummary.income} spending={periodSummary.spending} ... />
  <div className="flex gap-4">
    {/* Left column — scrollable */}
    <div className="flex-1 min-w-0">
      <CategoryRanking ... />
      {period.selectedDay && <DaySummary ... />}
      {/* Transaction filters + list */}
      <TransactionFilterBar ... />
      <TransactionList ... />
    </div>
    {/* Right sidebar — sticky */}
    <div className="w-72 flex-shrink-0 sticky top-0 self-start space-y-4">
      <SpendingHeatmap ... />
      <AccountsSidebar ... />
      <BudgetStatusSidebar ... />
    </div>
  </div>
</FinanceLayout>
```

The Accounts sidebar and Budget sidebar can be inline sections (not separate component files — they're small: account list ~20 lines, budget bars ~20 lines). The transaction filter bar and list can also be inline — they reuse the pattern from the old `TransactionsPage` but simpler (no slide panel for add).

**Transaction "+" Add button:** Opens the existing `SlidePanel` with the add transaction form. Port the form from `TransactionsPage.tsx` — it uses `useMutation("finance_transaction_create")`.

**Key behaviors:**
- When `period.selectedDay` changes, transactions re-fetch for that day
- Account click sets `accountFilter` state, highlights the row
- Type filter toggles `txFilter` state
- Search input debounces into `searchQuery`
- All `useEvent("entity:updated", ...)` triggers refetch

- [ ] **Step 2: Build and verify**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(finance): add CashFlowPage with sidebar heatmap layout"
```

---

## Chunk 5: Navigation + Cleanup

### Task 12: Update Router + Nav

**Files:**
- Modify: `desktop-ui/src/app/router.tsx`
- Modify: `desktop-ui/src/features/finance/components/FinanceLayout.tsx`
- Modify: `desktop-ui/src/features/finance/index.ts`

- [ ] **Step 1: Update router**

In `desktop-ui/src/app/router.tsx`:
- Add lazy import for `CashFlowPage`
- Add route: `{ path: "cashflow", element: <CashFlowPage /> }`
- Replace old routes with redirects:
  ```tsx
  { path: "accounts", element: <Navigate to="/finance/cashflow" replace /> }
  { path: "transactions", element: <Navigate to="/finance/cashflow" replace /> }
  { path: "budgets", element: <Navigate to="/finance/cashflow" replace /> }
  ```

- [ ] **Step 2: Update FinanceLayout nav**

In `desktop-ui/src/features/finance/components/FinanceLayout.tsx`, update `subNav`:

```typescript
const subNav = [
  { label: "Dashboard", path: "/finance" },
  { label: "Cash Flow", path: "/finance/cashflow" },
  { label: "Investments", path: "/finance/investments" },
  { label: "Goals", path: "/finance/goals" },
  { label: "Liabilities", path: "/finance/liabilities" },
];
```

- [ ] **Step 3: Update feature index exports**

In `desktop-ui/src/features/finance/index.ts`:
- Add export for `CashFlowPage`
- Keep old page exports for now (they're still imported by the redirect routes until the old files are deleted)

- [ ] **Step 4: Update Overview page links**

In `desktop-ui/src/features/finance/pages/FinanceOverviewPage.tsx`:
- Change `<Link to="/finance/accounts">` → `<Link to="/finance/cashflow">`
- Change `<Link to="/finance/transactions">` → `<Link to="/finance/cashflow">`
- Any budgets "View all" → `/finance/cashflow`

- [ ] **Step 5: Build and verify**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(finance): update nav to 5 tabs, add redirects for old routes"
```

### Task 13: Delete Old Pages

**Files:**
- Delete: `desktop-ui/src/features/finance/pages/AccountsPage.tsx`
- Delete: `desktop-ui/src/features/finance/pages/TransactionsPage.tsx`
- Delete: `desktop-ui/src/features/finance/pages/BudgetsPage.tsx`
- Modify: `desktop-ui/src/features/finance/index.ts` — remove old exports
- Modify: `desktop-ui/src/app/router.tsx` — remove old lazy imports

- [ ] **Step 1: Delete the 3 old page files**

- [ ] **Step 2: Remove exports from index.ts**

Remove `AccountsPage`, `TransactionsPage`, `BudgetsPage` from the exports.

- [ ] **Step 3: Update router to remove old lazy imports**

The redirect routes don't need the old components — they use `<Navigate>` directly.

- [ ] **Step 4: Build and verify**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(finance): remove AccountsPage, TransactionsPage, BudgetsPage"
```

---

## Chunk 6: Lint + Final Verification

### Task 14: Lint and Test

- [ ] **Step 1: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 2: Rust checks**

Run: `cargo clippy --workspace --all-targets --all-features`
Run: `cargo fmt --all --check`

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Run: `cd desktop-ui && bun run test`

- [ ] **Step 4: Visual test**

Run: `cargo tauri dev`
Verify:
- Navigate to `/finance/cashflow` — page loads with heatmap, stats, transactions
- Click a day in heatmap → day summary appears, transactions filter
- Click account in sidebar → transactions filter by account
- Switch period modes (Day/Week/Month/Year) → all sections update
- Navigate ◄/► → period changes
- Privacy toggle masks amounts
- Old URLs `/finance/accounts`, `/finance/transactions`, `/finance/budgets` redirect to `/finance/cashflow`
- Dashboard "View all →" links go to `/finance/cashflow`
- Nav shows 5 tabs

- [ ] **Step 5: Commit if any lint fixes**

```bash
git commit -m "chore(finance): lint fixes for cash flow tab"
```
