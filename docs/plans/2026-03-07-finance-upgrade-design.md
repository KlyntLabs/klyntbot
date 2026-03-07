# Finance Feature Complete Upgrade — Design

## Context

The finance backend is mature (40 AI tool actions, 8 SQLite tables, full CRUD), but the frontend is read-only with zero mutations, dated visuals, and several broken integrations. Recently upgraded pages (Productivity, Notes) use a refined glassmorphism aesthetic that finance needs to match.

## Approach

Hybrid rewrite: build shared component foundation first, then rebuild each page.

## Backend Changes

### New Tauri Mutation Commands

- `finance_account_create(name, account_type, currency?, balance?, institution?, notes?)` → `FinanceAccountRow`
- `finance_account_update(id, name?, balance?, institution?, notes?, is_archived?)` → `FinanceAccountRow`
- `finance_account_delete(id)` → `{ deleted: bool }`
- `finance_transaction_create(account_id, tx_type, amount, currency?, category?, subcategory?, counterparty?, tx_date?, notes?)` → `FinanceTransactionRow`
- `finance_transaction_update(id, amount?, category?, subcategory?, counterparty?, notes?, tx_date?)` → `FinanceTransactionRow`
- `finance_transaction_delete(id)` → `{ deleted: bool }`
- `finance_budget_create(name, amount, period, currency?, category?, method?, start_date?, end_date?, alert_threshold?)` → `FinanceBudgetRow` (via BudgetUsageRow)
- `finance_budget_update(id, name?, amount?, category?, is_active?)` → `FinanceBudgetRow`
- `finance_budget_delete(id)` → `{ deleted: bool }`
- `finance_goal_create(name, goal_type, target_amount, currency?, current_amount?, deadline?, monthly_contribution?, notes?)` → `FinanceGoalRow`
- `finance_goal_update(id, current_amount?, target_amount?, monthly_contribution?, deadline?, status?)` → `FinanceGoalRow`
- `finance_goal_delete(id)` → `{ deleted: bool }`
- `finance_liability_create(name, liability_type, principal, currency?, remaining?, interest_rate?, monthly_payment?, due_date?, notes?)` → `FinanceLiabilityRow`
- `finance_liability_update(id, remaining?, monthly_payment?, interest_rate?, notes?)` → `FinanceLiabilityRow`
- `finance_liability_delete(id)` → `{ deleted: bool }`
- `finance_portfolio_create(name, description?, currency?)` → `FinancePortfolioRow`
- `finance_investment_create(portfolio_id, asset_type, cost_basis, quantity, symbol?, name?, currency?, purchase_date?, notes?)` → `FinanceInvestmentRow`
- `finance_investment_update(id, current_price?, current_value?, quantity?, notes?)` → `FinanceInvestmentRow`

### Modified Existing Commands

- `finance_transactions(limit?, account_id?, category?, tx_type?, date_from?, date_to?, query?)` — expose full `FinanceTransactionFilter`
- `finance_goals(include_completed?: bool)` — optionally include achieved/abandoned
- `finance_investments(portfolio_id?: string)` — filter by portfolio

### New Read Commands

- `finance_report_spending(period?, date_from?, date_to?, category?)` → `{ total, breakdown: [{ category, amount, pct }] }`
- `finance_report_income(period?, date_from?, date_to?, category?)` → same structure
- `finance_report_trends(metric, periods?)` → `[{ period, value, change_pct }]`

### Exchange Rates

Store user-configured rates in finance settings (config JSON). `finance_exchange_rates` returns them from config instead of empty map. User sets rates via `finance_settings_update`.

## Component Architecture

### FinanceCard (compound component)

```tsx
<FinanceCard>
  <FinanceCard.Header title="Net Worth" subtitle="All currencies" action={<Button />} />
  <FinanceCard.Body>{children}</FinanceCard.Body>
  <FinanceCard.Footer>{children}</FinanceCard.Footer>
</FinanceCard>
```

Base: `glass-card p-4 flex flex-col gap-3`. Header title: `text-[13px] font-medium text-secondary`.

### AnimatedDonut

Replaces current Donut. Features:
- `transition-[stroke-dashoffset] duration-700` on mount
- CSS variable color strings (not hex)
- Optional center label (hero stat)
- `filter: drop-shadow()` glow on segments

### FormModal

Glass-panel overlay with:
- `glass-panel rounded-2xl` container
- Backdrop `bg-black/40 backdrop-blur-sm`
- Staggered `fade-in` on fields
- `glass-input` styled form fields
- Cancel / Save footer

### SlidePanel

Right-side drawer (400px):
- `glass-panel` with slide-in animation
- Full-height, scrollable content
- Used for transaction creation/editing

### HeroStat

```tsx
<HeroStat value={netWorth} currency="VND" change={+5.2} label="Total Net Worth" />
```

`text-[28px] font-light leading-none tabular-nums` with color-coded change indicator.

### StatRow

```tsx
<StatRow icon={Wallet} label="Savings" value={fmtMoney(amt)} color="var(--success)" />
```

Standardized list item with icon, label, value, optional progress bar.

## Visual Upgrades

- `SectionLabel` outside → `<h2>` inside cards (matching productivity)
- `font-light` → `font-medium` on card titles
- `gap-3` → `gap-4` between cards
- Hardcoded COLORS hex → CSS variable strings
- Raw `<select>`/`<input>` → `glass-input` class
- Static donut → animated entrance
- All progress bars get `transition-[width] duration-500`
- Loading skeletons on all data sections
- Error states with retry

## Page Rebuilds

### Dashboard
- HeroStat for net worth with animated count
- Fix: use `finance_report_spending` for donut (not 8-tx limit)
- Fix: account click-through reads `?id=` param
- Loading skeletons

### Accounts
- Add Account modal
- Read `?id=` URL param, auto-select account
- Archived toggle
- Account detail with recent transactions

### Transactions
- Server-side filtering (pass filter params to backend)
- SlidePanel for Add/Edit Transaction
- Animated donut by category
- Debounced search input

### Budgets
- Add Budget modal
- Fix currency mixing (fetch + apply exchange rates)
- Subcategory breakdown via `budget_status`
- Alert threshold visual indicators

### Investments
- Add Portfolio / Add Investment modals
- Portfolio filter dropdown
- Return % with semantic coloring
- Price refresh button

### Goals
- Add Goal modal
- Status tabs (Active / Achieved / Abandoned)
- FIRE calculator section
- Animated progress rings

### Liabilities
- Add Liability modal
- Replace raw divs with `<Progress>`
- Payment tracking

### Reports (NEW)
- Spending/income breakdown charts
- Trend line charts
- Period selector
