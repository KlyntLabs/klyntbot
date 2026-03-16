# Finance Dashboard Redesign

**Date:** 2026-03-15
**Scope:** Dashboard overview page redesign + visual polish on 6 sub-pages
**Mockup:** `.superpowers/brainstorm/54125-1773589651/full-design-v3.html`

## Problem

The current finance dashboard shows all data types (net worth, accounts, budgets, transactions, investments, goals, liabilities) with equal visual weight in a dense 12-column grid. Small typography (10–20px), no change indicators, no privacy controls. Users cannot quickly understand their financial health or protect sensitive information from shoulder-surfing.

## Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Hero concept | Financial Health Score (0–100) | Gives emotional anchor — "am I doing well?" — before raw numbers |
| Page layout | Two-Zone (hero full-width → two columns below) | Balances data density with visual hierarchy |
| Privacy model | Safe zone (top, percentages only) + sensitive zone (bottom, real amounts) | Default layout is privacy-conscious without needing a toggle |
| Privacy toggle | Asterisk masking (`•••••••`) | Replaces dollar amounts, keeps labels/percentages visible |
| Scope | Dashboard redesign + visual polish on sub-pages | Get dashboard right first, uplift sub-pages with same visual language |
| Refresh button | Removed from toolbar | Data auto-refreshes via `entity:updated` events already |

## Dashboard Layout

### 1. Toolbar (existing FinanceLayout, modified)

- Sub-nav tabs: Dashboard | Accounts | Transactions | Budgets | Investments | Goals | Liabilities
- **Privacy toggle** (eye icon) — right side, before currency toggle
  - Shows open eye when amounts visible, slashed eye when hidden
  - Muted accent (`text-muted` → `text-primary`) when active — privacy is a neutral state, not an action
  - Persists to `localStorage` key: `finance:privacyMode`
- **Currency toggle** (existing) — right side, after privacy toggle
- Refresh button **removed**

### 2. Safe Zone — Hero Row (no dollar amounts)

Two glass cards side by side (`grid-cols-2`):

**Left: Health Score Card**
- SVG ring (120px) showing score 0–100
- Score computed from 4 weighted factors:
  - **Savings Rate** (25%) — `(income - spending) / income` from recent transactions
  - **Debt Ratio** (25%) — `1 - (totalDebt / totalAssets)`, capped 0–1
  - **Budget Adherence** (25%) — average `(1 - spent/budget)` across active budgets, capped 0–1
  - **Goal Progress** (25%) — average `currentAmount / targetAmount` across active goals
- Each factor shown as labeled progress bar with percentage
- Status text: "Good — improving ↑" / "Fair — watch spending" / "Needs attention"
- Ring color: green (≥70), orange (40–69), red (<40)

**Right: Monthly Pulse Card**
- 3 rows showing relative changes (no absolute amounts):
  - Income vs last month: "+12% higher · on track"
  - Spending vs last month: "-3% lower · improving"
  - Savings rate trend: "29% · up from 24% last month"
- Each row has: directional icon (↑/↓/≈) in tinted square, label, detail text, progress bar
- **Data source**: Requires a new query `finance_monthly_summary` (see Monthly Pulse section)

### 3. Safe Zone — Budget Strip

Horizontal row of budget chips (`grid auto-fit minmax(160px, 1fr)`):
- Each chip: budget name, percentage used, progress bar, status text
- Status text uses natural language: "Near limit — 8% remaining", "Well under budget", "On pace"
- Colors: green (<50%), orange (50–80%), red (>80%)
- No dollar amounts — percentages only

### 4. Sensitive Divider

Visual separator: Lucide `<Lock />` icon + "Amounts & Balances" text, centered between two gradient lines. Uses `aria-label="Sensitive financial data below"` for screen reader context.
Marks the transition from safe to sensitive content.

### 5. Sensitive Zone — Net Worth Card (full width)

- Left: "NET WORTH" label, large number (32px font-weight-200), change indicator (▲ $2,340, +1.9%)
- Right: Wealth composition bar (cash green / investments blue / debt red) with legend
- When privacy ON: dollar amounts become `•••••••`, percentages remain

### 6. Sensitive Zone — Two-Zone Grid (`grid-cols-2`)

**Left column:**
- **Accounts** — list with icon, name, type, balance, monthly change. "View all →" link.
- **Goals** — name, type badge, progress bar, percentage. Dollar amounts in sub-text. "View all →" link.

**Right column:**
- **Investments** — total value (large), overall return %. Portfolio list with individual returns. "View all →" link.
- **Liabilities** — name, type, APR, payoff progress bar. Remaining amount. "View all →" link.

### 7. Sensitive Zone — Recent Transactions (full width)

Richer rows than current:
- Date | Type icon (colored background square with arrow) | Name + category + account | Amount
- "View all →" link to transactions page

## Privacy Toggle Behavior

- **State**: `boolean` stored in `localStorage` as `finance:privacyMode` (follows existing `finance:currencyDisplayMode` naming convention)
- **Hook**: `usePrivacyMode()` returning `{ hidden, toggle }` — each page calls this hook independently (same pattern as `useCurrencyDisplayMode`)
- **Masking integration**: Instead of a standalone `maskAmount()`, integrate masking directly into `displayAmount()` in `lib/displayAmount.ts` by adding a `hidden?: boolean` option. When `hidden=true`, return `•••••••` instead of the formatted string. This ensures every place that already calls `displayAmount()` gets masking for free — no per-component patching needed.
- **`fmtMoney` / `fmtCompact`**: Add `hidden` param to these too — they're used in places that don't go through `displayAmount()` (e.g. net worth, cash flow cards).
- **What gets masked**: All dollar/currency values — balances, amounts, net worth, transaction amounts, goal targets, liability remaining
- **What stays visible**: Percentages, progress bars, labels, names, dates, categories, health score, status text
- **Applied globally**: Privacy state shared across all finance pages via each page calling `usePrivacyMode()` independently — localStorage keeps them in sync

## Health Score Computation

New utility: `features/finance/lib/healthScore.ts`

```
function computeHealthScore(params: {
  totalIncome: number;
  totalSpending: number;
  totalAssets: number;
  totalDebt: number;
  budgets: { spent: number; amount: number }[];
  goals: { currentAmount: number; targetAmount: number }[];
}): { score: number; factors: HealthFactor[]; status: string }
```

**Factors** (each 0–100, weighted equally at 25%):
- `savingsRate`: `clamp(0, 100, ((income - spending) / income) * 200)` — 50% savings = perfect score. Clamped to 0 when spending > income.
- `debtRatio`: `assets > 0 ? clamp(0, 100, (1 - debt / assets) * 100) : (debt > 0 ? 0 : 75)` — 0 debt = perfect. When assets=0 but debt exists, score is 0. When both are 0 (new user), defaults to 75 (neutral).
- `budgetAdherence`: avg `clamp(0, 100, (1 - spent / amount) * 100)` — under budget = high
- `goalProgress`: avg `clamp(0, 100, (current / target) * 100)` — closer to target = higher

Helper: `const clamp = (min: number, max: number, v: number) => Math.max(min, Math.min(max, v))`

**Status text**: score ≥ 70 → "Good — improving ↑", 40–69 → "Fair — watch spending", < 40 → "Needs attention ↓"

**Edge cases**: If no income, savings rate defaults to 0. If spending > income, savings rate clamps to 0 (not negative). If assets=0 and debt > 0, debt ratio is 0. If no budgets, budget adherence defaults to 75 (neutral). If no goals, goal progress defaults to 50.

## Monthly Pulse Computation

New utility: `features/finance/lib/monthlyPulse.ts`

Compares current month totals vs previous month. Returns:
- `incomeChange: { pct: number; direction: 'up' | 'down' | 'flat'; hint: string }`
- `spendingChange: { ... }`
- `savingsRateChange: { current: number; previous: number; hint: string }`

Hint text is natural language: "+12% higher · on track", "-3% lower · improving", "29% · up from 24%".

**Data source — new Tauri command required:** The current `finance_transactions` query uses `{ limit: 8 }` which is insufficient. Add a new Tauri command `finance_monthly_summary` that returns:
```
{ currentMonth: { totalIncome: number; totalSpending: number }; previousMonth: { totalIncome: number; totalSpending: number } }
```
This is a simple SQL aggregate grouped by month on the transactions table — no schema changes needed, just a new query in the `feature-finance` crate + desktop command.

**Note:** The original v1 spec included `netWorthChange` here, but net worth history is not tracked in the schema. Replaced with `savingsRateChange` which is computable from transaction data alone. Net worth trends can be added later if a historical snapshot table is introduced.

## Sub-Page Visual Polish

Apply to all 6 sub-pages (Accounts, Transactions, Budgets, Investments, Goals, Liabilities):

1. **Privacy masking** — all dollar amounts respect `usePrivacyMode()` state
2. **Larger key numbers** — stat card values from 20px → 24px
3. **Change indicators** — where data supports it, show +/- vs previous period
4. **Section titles** — uppercase 10px tracking with colored accent, matching dashboard style
5. **Consistent card padding** — standardize to 16px
6. **Remove refresh button** from FinanceLayout toolbar

## New Files

| File | Purpose |
|------|---------|
| `hooks/usePrivacyMode.ts` | Privacy state hook (localStorage + toggle) |
| `lib/healthScore.ts` | Health score computation |
| `lib/monthlyPulse.ts` | Month-over-month change computation |
| `components/HealthScoreRing.tsx` | SVG score ring with animated arc |
| `components/MonthlyPulse.tsx` | 3-row pulse display |
| `components/BudgetStrip.tsx` | Horizontal budget chip row |
| `components/SensitiveDivider.tsx` | Visual divider between zones |
| `components/NetWorthCard.tsx` | Full-width net worth display |
| `components/PrivacyToggle.tsx` | Eye icon button for toolbar |

**Backend (Rust):**

| File | Purpose |
|------|---------|
| `crates/feature-finance/src/queries/monthly_summary.rs` | SQL query aggregating income/spending by month |
| `crates/desktop/src/commands/finance.rs` | New `finance_monthly_summary` Tauri command |
| `crates/app-core/src/handlers/finance.rs` | `monthly_summary()` handler method |

## Modified Files

| File | Change |
|------|--------|
| `pages/FinanceOverviewPage.tsx` | Complete rewrite — new layout with all components above |
| `components/FinanceLayout.tsx` | Add PrivacyToggle to toolbar, remove refresh button |
| `lib/displayAmount.ts` | Add `hidden` option to `displayAmount()`, `displayHint()` for integrated masking |
| `lib/finance.ts` | Add `hidden` param to `fmtMoney()`, `fmtCompact()` |
| `pages/AccountsPage.tsx` | Privacy masking on amounts, larger stat numbers |
| `pages/TransactionsPage.tsx` | Privacy masking on amounts, larger stat numbers |
| `pages/BudgetsPage.tsx` | Privacy masking on amounts, larger stat numbers |
| `pages/InvestmentsPage.tsx` | Privacy masking on amounts, larger stat numbers |
| `pages/GoalsPage.tsx` | Privacy masking on amounts, larger stat numbers |
| `pages/LiabilitiesPage.tsx` | Privacy masking on amounts, larger stat numbers |

## Web Interface Guidelines Compliance

- All interactive elements use `<button>` or `<a>`, not `<div onClick>`
- Eye icon button has `aria-label="Hide sensitive amounts"` / `"Show sensitive amounts"`
- `tabular-nums` on all numeric displays
- `text-wrap: balance` on section titles
- Animations respect `prefers-reduced-motion`
- Budget status uses natural language, not just numbers
- "View all →" links use `<a>` / React Router `<Link>` for cmd+click support
- No `transition: all` — explicit property lists
