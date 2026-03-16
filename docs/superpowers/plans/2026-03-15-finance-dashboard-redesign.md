# Finance Dashboard Redesign Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the finance dashboard with a privacy-first layout: Health Score hero, two-zone grid, asterisk masking, and visual polish across all finance pages.

**Architecture:** New dashboard layout splits into Safe Zone (percentages/ratios only) and Sensitive Zone (real amounts). Privacy masking integrates into existing `displayAmount()` pipeline. New `finance_monthly_summary` backend query provides month-over-month data. Health Score computed client-side from existing data.

**Tech Stack:** React, Tailwind v4 CSS tokens, Lucide icons, Tauri commands (Rust backend), SQLite aggregation

**Spec:** `docs/superpowers/specs/2026-03-15-finance-dashboard-redesign.md`

---

## Chunk 1: Privacy Infrastructure + Backend Query

### Task 1: Privacy Mode Hook

**Files:**
- Create: `desktop-ui/src/features/finance/hooks/usePrivacyMode.ts`

- [ ] **Step 1: Create the privacy mode hook**

```typescript
// desktop-ui/src/features/finance/hooks/usePrivacyMode.ts
import { useCallback, useState } from "react";

const STORAGE_KEY = "finance:privacyMode";

export function usePrivacyMode() {
  const [hidden, setHidden] = useState<boolean>(() => {
    return localStorage.getItem(STORAGE_KEY) === "true";
  });

  const toggle = useCallback(() => {
    setHidden((prev) => {
      const next = !prev;
      localStorage.setItem(STORAGE_KEY, String(next));
      return next;
    });
  }, []);

  return { hidden, toggle };
}
```

- [ ] **Step 2: Export from feature index**

Modify: `desktop-ui/src/features/finance/index.ts` — add `usePrivacyMode` to exports.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/finance/hooks/usePrivacyMode.ts desktop-ui/src/features/finance/index.ts
git commit -m "feat(finance): add usePrivacyMode hook with localStorage persistence"
```

### Task 2: Integrate Privacy Masking into Display Utilities

**Files:**
- Modify: `desktop-ui/src/features/finance/lib/displayAmount.ts`
- Modify: `desktop-ui/src/features/finance/lib/finance.ts`

The masking integrates directly into existing formatting functions so every callsite gets masking for free.

- [ ] **Step 1: Add `hidden` option to `AmountDisplayOptions` and mask in `displayAmount()`**

In `desktop-ui/src/features/finance/lib/displayAmount.ts`, add `hidden?: boolean` to the `AmountDisplayOptions` interface:

```typescript
interface AmountDisplayOptions {
  amount: number;
  currency: string;
  baseAmount?: number;
  baseCurrency: string;
  mode: CurrencyDisplayMode;
  compact?: boolean;
  rates?: RateMap;
  hidden?: boolean; // ← ADD THIS
}
```

Then modify `displayAmount()`:

```typescript
const MASK = "•••••••";

export function displayAmount(opts: AmountDisplayOptions): string {
  if (opts.hidden) return MASK;
  const fmt = opts.compact ? fmtCompact : fmtMoney;
  const { value, currency } = resolveAmount(opts);
  return fmt(value, currency);
}
```

And `displayHint()`:

```typescript
export function displayHint(opts: AmountDisplayOptions): string | null {
  if (opts.hidden) return null;
  // ... rest unchanged
}
```

- [ ] **Step 2: Add `hidden` param to `fmtMoney()` and `fmtCompact()` in `finance.ts`**

These are called directly in places that don't go through `displayAmount()` (e.g., net worth, cash flow):

```typescript
const MASK = "•••••••";

export function fmtMoney(amount: number, currency: string, hidden?: boolean): string {
  if (hidden) return MASK;
  // ... rest unchanged
}

export function fmtCompact(amount: number, currency = "USD", hidden?: boolean): string {
  if (hidden) return MASK;
  // ... rest unchanged
}
```

- [ ] **Step 3: Verify build passes**

Run: `cd desktop-ui && bun run build`
Expected: No type errors. Existing callsites work unchanged since `hidden` is optional and defaults to `undefined` (falsy).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/finance/lib/displayAmount.ts desktop-ui/src/features/finance/lib/finance.ts
git commit -m "feat(finance): integrate privacy masking into displayAmount and fmt utilities"
```

### Task 3: Backend — Monthly Summary Query

**Files:**
- Modify: `crates/desktop-shared/src/commands/finance.rs` — add response type
- Modify: `crates/app-core/src/handlers/finance/reports.rs` — add handler method
- Modify: `crates/desktop/src/commands/finance.rs` — add Tauri command
- Modify: `crates/desktop/src/dev_server/` — add to dev server routes (if applicable)

This task follows the existing pattern: `sum_by_period()` repo method → app-core handler → Tauri command.

- [ ] **Step 1: Add response type in desktop-shared**

In `crates/desktop-shared/src/commands/finance.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceMonthlySummaryResponse {
    pub current_income: i64,
    pub current_spending: i64,
    pub previous_income: i64,
    pub previous_spending: i64,
}
```

- [ ] **Step 2: Add handler method in app-core**

In `crates/app-core/src/handlers/finance/reports.rs`, add a `finance_monthly_summary` method that:
1. Gets the default currency via `self.default_currency().await`
2. Compute current month label: `chrono::Local::now().format("%Y-%m").to_string()` and previous month label
3. Calls `self.repos.finance.transactions.sum_by_period("income", 3, "monthly", &currency)` — request 3 periods for safety margin
4. Calls `self.repos.finance.transactions.sum_by_period("expense", 3, "monthly", &currency)` — same
5. **Map by period label, NOT by array index.** `sum_by_period` returns `Vec<(String, i64)>` ordered descending — but sparse months may be missing. Build a `HashMap<String, i64>` from the results, then look up `current_month_label` and `previous_month_label` specifically. Default to 0 if a label is missing.
6. Return `FinanceMonthlySummaryResponse` with the looked-up values

Also add the import: `use desktop_shared::commands::finance::FinanceMonthlySummaryResponse;`

Follow the exact error handling pattern of existing methods in the same file (`map_storage_err`).

**Note:** The spec listed `crates/feature-finance/src/queries/monthly_summary.rs` as a new file, but since `sum_by_period()` already exists in `FinanceTransactionRepo`, no new repo method is needed. The handler just calls the existing method and maps by label. We intentionally skip that spec file.

- [ ] **Step 3: Add Tauri command**

In `crates/desktop/src/commands/finance.rs`, add:

```rust
#[tauri::command]
pub async fn finance_monthly_summary(
    state: State<'_, Arc<AppCore>>,
) -> Result<FinanceMonthlySummaryResponse, ApiError> {
    state.finance_monthly_summary().await
}
```

Register this command in the Tauri builder (check how existing finance commands are registered). Add `"finance_monthly_summary"` to the `DEV_COMMANDS` array.

- [ ] **Step 4: Add dev server route**

If the dev server at `crates/desktop/src/dev_server/` needs updating for the new command, add a route that delegates to the same handler. Follow the pattern of existing finance routes.

- [ ] **Step 5: Build and test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`
Expected: Build passes. Dev server coverage test passes (the new command is in DEV_COMMANDS).

- [ ] **Step 6: Commit**

```bash
git add crates/desktop-shared/src/commands/finance.rs crates/app-core/src/handlers/finance/reports.rs crates/desktop/src/commands/finance.rs
git commit -m "feat(finance): add finance_monthly_summary backend query"
```

### Task 4: Frontend Type for Monthly Summary

**Files:**
- Modify: `desktop-ui/src/shared/types/finance.ts`

- [ ] **Step 1: Add TypeScript type**

```typescript
export interface FinanceMonthlySummary {
  currentIncome: number;
  currentSpending: number;
  previousIncome: number;
  previousSpending: number;
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/shared/types/finance.ts
git commit -m "feat(finance): add FinanceMonthlySummary frontend type"
```

---

## Chunk 2: Health Score + Monthly Pulse Utilities

### Task 5: Health Score Computation

**Files:**
- Create: `desktop-ui/src/features/finance/lib/healthScore.ts`

- [ ] **Step 1: Implement health score utility**

```typescript
// desktop-ui/src/features/finance/lib/healthScore.ts

const clamp = (min: number, max: number, v: number) => Math.max(min, Math.min(max, v));

export interface HealthFactor {
  name: string;
  value: number; // 0–100
  color: string;
}

export interface HealthScore {
  score: number; // 0–100
  factors: HealthFactor[];
  status: string;
  statusColor: string;
}

export function computeHealthScore(params: {
  totalIncome: number;
  totalSpending: number;
  totalAssets: number;
  totalDebt: number;
  budgets: { spent: number; amount: number }[];
  goals: { currentAmount: number; targetAmount: number }[];
}): HealthScore {
  const { totalIncome, totalSpending, totalAssets, totalDebt, budgets, goals } = params;

  // Savings Rate: 50% savings = perfect score, clamped 0–100
  const savingsRate =
    totalIncome > 0
      ? clamp(0, 100, ((totalIncome - totalSpending) / totalIncome) * 200)
      : 0;

  // Debt Ratio: 0 debt = perfect, clamped 0–100
  const debtRatio =
    totalAssets > 0
      ? clamp(0, 100, (1 - totalDebt / totalAssets) * 100)
      : totalDebt > 0
        ? 0
        : 75; // new user, no data

  // Budget Adherence: under budget = high score
  const budgetAdherence =
    budgets.length > 0
      ? budgets.reduce((sum, b) => sum + clamp(0, 100, (1 - b.spent / b.amount) * 100), 0) /
        budgets.length
      : 75; // neutral default

  // Goal Progress: closer to target = higher
  const goalProgress =
    goals.length > 0
      ? goals.reduce(
          (sum, g) => sum + clamp(0, 100, (g.currentAmount / g.targetAmount) * 100),
          0,
        ) / goals.length
      : 50; // neutral default

  const score = Math.round((savingsRate + debtRatio + budgetAdherence + goalProgress) / 4);

  const factors: HealthFactor[] = [
    { name: "Savings Rate", value: Math.round(savingsRate), color: "#34d399" },
    { name: "Debt Ratio", value: Math.round(debtRatio), color: "#60a5fa" },
    { name: "Budget Adherence", value: Math.round(budgetAdherence), color: "#f97316" },
    { name: "Goal Progress", value: Math.round(goalProgress), color: "#a78bfa" },
  ];

  const status = score >= 70 ? "Good — improving ↑" : score >= 40 ? "Fair — watch spending" : "Needs attention ↓";
  const statusColor = score >= 70 ? "#34d399" : score >= 40 ? "#f97316" : "#f43f5e";

  return { score, factors, status, statusColor };
}

/** Ring color based on score */
export function scoreColor(score: number): string {
  if (score >= 70) return "#34d399";
  if (score >= 40) return "#f97316";
  return "#f43f5e";
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/lib/healthScore.ts
git commit -m "feat(finance): add health score computation utility"
```

### Task 6: Monthly Pulse Computation

**Files:**
- Create: `desktop-ui/src/features/finance/lib/monthlyPulse.ts`

- [ ] **Step 1: Implement monthly pulse utility**

```typescript
// desktop-ui/src/features/finance/lib/monthlyPulse.ts

export interface PulseRow {
  label: string;
  direction: "up" | "down" | "flat";
  hint: string;
  /** 0–100 for progress bar width (relative measure) */
  barWidth: number;
  color: string;
}

export function computeMonthlyPulse(params: {
  currentIncome: number;
  currentSpending: number;
  previousIncome: number;
  previousSpending: number;
}): PulseRow[] {
  const { currentIncome, currentSpending, previousIncome, previousSpending } = params;

  const incPct = previousIncome > 0 ? Math.round(((currentIncome - previousIncome) / previousIncome) * 100) : 0;
  const spendPct = previousSpending > 0 ? Math.round(((currentSpending - previousSpending) / previousSpending) * 100) : 0;

  const curSavingsRate = currentIncome > 0 ? Math.round(((currentIncome - currentSpending) / currentIncome) * 100) : 0;
  const prevSavingsRate = previousIncome > 0 ? Math.round(((previousIncome - previousSpending) / previousIncome) * 100) : 0;

  const incDir: PulseRow["direction"] = incPct > 2 ? "up" : incPct < -2 ? "down" : "flat";
  const spendDir: PulseRow["direction"] = spendPct > 2 ? "up" : spendPct < -2 ? "down" : "flat";
  const savDir: PulseRow["direction"] = curSavingsRate > prevSavingsRate + 2 ? "up" : curSavingsRate < prevSavingsRate - 2 ? "down" : "flat";

  return [
    {
      label: "Income vs Last Month",
      direction: incDir,
      hint: incDir === "flat" ? "Stable · on track" : `${incPct > 0 ? "+" : ""}${incPct}% ${incPct > 0 ? "higher" : "lower"} · ${incPct > 0 ? "on track" : "watch this"}`,
      barWidth: Math.min(100, Math.max(20, 50 + incPct)),
      color: "#34d399",
    },
    {
      label: "Spending vs Last Month",
      direction: spendDir,
      hint: spendDir === "flat" ? "Stable" : `${spendPct > 0 ? "+" : ""}${spendPct}% ${spendPct > 0 ? "higher" : "lower"} · ${spendPct < 0 ? "improving" : "watch this"}`,
      barWidth: Math.min(100, Math.max(20, 50 + spendPct)),
      color: "#f43f5e",
    },
    {
      label: "Savings Rate",
      direction: savDir,
      hint: `${curSavingsRate}%${prevSavingsRate > 0 ? ` · ${savDir === "up" ? "up" : savDir === "down" ? "down" : "same as"} from ${prevSavingsRate}%` : ""}`,
      barWidth: Math.min(100, Math.max(10, curSavingsRate)),
      color: "#60a5fa",
    },
  ];
}

export const DIRECTION_ICONS: Record<PulseRow["direction"], string> = {
  up: "↑",
  down: "↓",
  flat: "≈",
};
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/lib/monthlyPulse.ts
git commit -m "feat(finance): add monthly pulse computation utility"
```

---

## Chunk 3: New Dashboard Components

### Task 7: PrivacyToggle Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/PrivacyToggle.tsx`

- [ ] **Step 1: Implement privacy toggle button**

```tsx
// desktop-ui/src/features/finance/components/PrivacyToggle.tsx
import { Eye, EyeOff } from "lucide-react";

export function PrivacyToggle({
  hidden,
  onToggle,
}: {
  hidden: boolean;
  onToggle: () => void;
}) {
  const Icon = hidden ? EyeOff : Eye;
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={hidden ? "Show sensitive amounts" : "Hide sensitive amounts"}
      className={`ml-2 p-2 rounded-lg transition-colors ${
        hidden
          ? "text-primary bg-white/[0.08]"
          : "text-muted hover:text-secondary hover:bg-white/[0.06]"
      }`}
    >
      <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
    </button>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/components/PrivacyToggle.tsx
git commit -m "feat(finance): add PrivacyToggle component"
```

### Task 8: HealthScoreRing Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/HealthScoreRing.tsx`

- [ ] **Step 1: Implement health score ring**

An SVG ring component that shows the 0–100 score with an animated arc. Takes a `HealthScore` object.

```tsx
// desktop-ui/src/features/finance/components/HealthScoreRing.tsx
import { useEffect, useState } from "react";
import type { HealthScore } from "../lib/healthScore";
import { scoreColor } from "../lib/healthScore";

export function HealthScoreRing({ health }: { health: HealthScore }) {
  const [animated, setAnimated] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setAnimated(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const color = scoreColor(health.score);
  const r = 50;
  const circ = 2 * Math.PI * r;
  const filled = (health.score / 100) * circ;

  return (
    <div className="flex items-center gap-6">
      {/* Ring */}
      <div className="flex-shrink-0">
        <svg width={120} height={120} viewBox="0 0 120 120" aria-hidden="true">
          <circle cx={60} cy={60} r={r} fill="none" stroke="rgba(255,255,255,0.06)" strokeWidth={8} />
          <circle
            cx={60} cy={60} r={r} fill="none" stroke={color} strokeWidth={8}
            strokeDasharray={`${filled} ${circ - filled}`}
            strokeDashoffset={animated ? 0 : circ}
            strokeLinecap="round"
            transform="rotate(-90 60 60)"
            style={{
              transition: "stroke-dashoffset 1s ease-out",
              filter: `drop-shadow(0 0 10px ${color}50)`,
            }}
          />
          <text x={60} y={54} textAnchor="middle" className="fill-primary text-[30px]" style={{ fontWeight: 200, fontVariantNumeric: "tabular-nums" }}>
            {health.score}
          </text>
          <text x={60} y={70} textAnchor="middle" className="fill-muted text-[9px]" style={{ letterSpacing: "0.05em" }}>
            HEALTH
          </text>
        </svg>
      </div>

      {/* Factors */}
      <div className="flex-1">
        <p className="text-[11px] font-normal mb-3" style={{ color: health.statusColor }}>
          {health.status}
        </p>
        {health.factors.map((f) => (
          <div key={f.name} className="mb-2.5 last:mb-0">
            <div className="flex justify-between text-[11px] mb-1">
              <span className="text-secondary">{f.name}</span>
              <span style={{ color: f.color }}>{f.value}%</span>
            </div>
            <div className="h-1 bg-white/[0.06] rounded-full">
              <div className="h-full rounded-full" style={{ width: `${f.value}%`, background: f.color, transition: "width 0.8s ease" }} />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/components/HealthScoreRing.tsx
git commit -m "feat(finance): add HealthScoreRing component"
```

### Task 9: MonthlyPulse Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/MonthlyPulse.tsx`

- [ ] **Step 1: Implement monthly pulse display**

```tsx
// desktop-ui/src/features/finance/components/MonthlyPulse.tsx
import type { PulseRow } from "../lib/monthlyPulse";
import { DIRECTION_ICONS } from "../lib/monthlyPulse";

export function MonthlyPulse({ rows }: { rows: PulseRow[] }) {
  return (
    <div className="flex flex-col justify-center h-full">
      <p className="text-[10px] text-muted uppercase tracking-widest mb-4">Monthly Pulse</p>
      {rows.map((row) => (
        <div key={row.label} className="flex items-center gap-3 mb-3.5 last:mb-0">
          <div
            className="w-9 h-9 rounded-xl flex items-center justify-center text-[16px] font-light flex-shrink-0"
            style={{ background: `${row.color}18`, color: row.color }}
          >
            {DIRECTION_ICONS[row.direction]}
          </div>
          <div className="flex-1">
            <p className="text-[11px] text-secondary">{row.label}</p>
            <p className="text-[10px] text-dim mt-0.5">{row.hint}</p>
            <div className="h-1 bg-white/[0.06] rounded-full mt-1.5">
              <div
                className="h-full rounded-full"
                style={{ width: `${row.barWidth}%`, background: row.color, transition: "width 0.8s ease" }}
              />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/components/MonthlyPulse.tsx
git commit -m "feat(finance): add MonthlyPulse component"
```

### Task 10: BudgetStrip Component

**Files:**
- Create: `desktop-ui/src/features/finance/components/BudgetStrip.tsx`

- [ ] **Step 1: Implement budget strip**

A horizontal row of budget chips showing percentage and status text only (no dollar amounts — safe zone).

```tsx
// desktop-ui/src/features/finance/components/BudgetStrip.tsx
import type { FinanceBudgetUsage } from "@shared/types";
import { Card } from "./Card";
import { pct } from "../lib/finance";

function budgetStatusText(p: number): string {
  if (p >= 90) return `⚠ Near limit — ${100 - p}% remaining`;
  if (p >= 70) return "On pace for this period";
  return "Well under budget";
}

function budgetColor(p: number): string {
  if (p >= 80) return "#f43f5e";
  if (p >= 50) return "#f97316";
  return "#34d399";
}

export function BudgetStrip({ budgets }: { budgets: FinanceBudgetUsage[] }) {
  const active = budgets.filter((b) => b.isActive);
  if (active.length === 0) return null;

  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-2">
      {active.map((b) => {
        const p = pct(b.spent, b.amount);
        const color = budgetColor(p);
        return (
          <Card key={b.id} className="p-3.5">
            <div className="flex justify-between items-center mb-2">
              <span className="text-[12px] text-secondary">{b.name}</span>
              <span className="text-[13px] font-light tabular-nums" style={{ color }}>{p}%</span>
            </div>
            <div className="h-1 bg-white/[0.06] rounded-full">
              <div className="h-full rounded-full" style={{ width: `${Math.min(p, 100)}%`, background: color, transition: "width 0.6s ease" }} />
            </div>
            <p className="text-[9px] text-dim mt-1.5">{budgetStatusText(p)}</p>
          </Card>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/components/BudgetStrip.tsx
git commit -m "feat(finance): add BudgetStrip component"
```

### Task 11: SensitiveDivider + NetWorthCard Components

**Files:**
- Create: `desktop-ui/src/features/finance/components/SensitiveDivider.tsx`
- Create: `desktop-ui/src/features/finance/components/NetWorthCard.tsx`

- [ ] **Step 1: Implement sensitive divider**

```tsx
// desktop-ui/src/features/finance/components/SensitiveDivider.tsx
import { Lock } from "lucide-react";

export function SensitiveDivider() {
  return (
    <div className="flex items-center gap-3 my-6 px-1" aria-label="Sensitive financial data below">
      <div className="flex-1 h-px bg-gradient-to-r from-transparent via-white/[0.08] to-transparent" />
      <div className="flex items-center gap-1.5 text-[9px] text-dim uppercase tracking-widest">
        <Lock className="w-3 h-3" strokeWidth={1.5} />
        <span>Amounts & Balances</span>
      </div>
      <div className="flex-1 h-px bg-gradient-to-r from-transparent via-white/[0.08] to-transparent" />
    </div>
  );
}
```

- [ ] **Step 2: Implement net worth card**

```tsx
// desktop-ui/src/features/finance/components/NetWorthCard.tsx
import { Card } from "./Card";
import { fmtCompact } from "../lib/finance";

export function NetWorthCard({
  totalNet,
  totalAssets,
  totalInvest,
  totalDebt,
  displayCur,
  convertTotal,
  hidden,
}: {
  totalNet: number;
  totalAssets: number;
  totalInvest: number;
  totalDebt: number;
  displayCur: string;
  convertTotal: (v: number) => number;
  hidden: boolean;
}) {
  const cashAmount = totalAssets - totalInvest;
  const total = totalAssets + totalDebt; // for proportional bar

  return (
    <Card className="p-5 flex items-center justify-between">
      <div>
        <p className="text-[10px] text-muted uppercase tracking-widest mb-1">Net Worth</p>
        <p className="text-[32px] font-light text-primary tracking-tight leading-none tabular-nums">
          {fmtCompact(convertTotal(totalNet), displayCur, hidden)}
        </p>
      </div>
      <div className="text-right">
        <div className="flex h-2 rounded-full overflow-hidden gap-0.5 w-48 mb-2 ml-auto">
          {total > 0 && (
            <>
              <div className="bg-success rounded-full" style={{ flex: cashAmount }} />
              <div className="bg-info rounded-full" style={{ flex: totalInvest }} />
              <div className="bg-destructive rounded-full" style={{ flex: totalDebt }} />
            </>
          )}
        </div>
        <div className="flex gap-3 justify-end">
          <span className="text-[10px] text-muted flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-success" />
            Cash {fmtCompact(convertTotal(cashAmount), displayCur, hidden)}
          </span>
          <span className="text-[10px] text-muted flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-info" />
            Invest {fmtCompact(convertTotal(totalInvest), displayCur, hidden)}
          </span>
          <span className="text-[10px] text-muted flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-destructive" />
            Debt {fmtCompact(convertTotal(totalDebt), displayCur, hidden)}
          </span>
        </div>
      </div>
    </Card>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/finance/components/SensitiveDivider.tsx desktop-ui/src/features/finance/components/NetWorthCard.tsx
git commit -m "feat(finance): add SensitiveDivider and NetWorthCard components"
```

---

## Chunk 4: Layout Toolbar Update (MUST come before page rewrite)

### Task 12: Update FinanceLayout Toolbar

**Files:**
- Modify: `desktop-ui/src/features/finance/components/FinanceLayout.tsx`

This MUST happen before the overview page rewrite (Task 13) since the overview page needs the updated props.

- [ ] **Step 1: Update FinanceLayout props and JSX**

Replace `onRefresh` prop with privacy props. Updated interface and component:

```tsx
interface FinanceLayoutProps {
  children: React.ReactNode;
  hidden?: boolean;
  onTogglePrivacy?: () => void;
  currencyMode?: CurrencyDisplayMode;
  currencies?: string[];
  onSelectCurrency?: (mode: CurrencyDisplayMode) => void;
}
```

In the JSX:
- Remove the refresh `<button>` entirely
- Add `<PrivacyToggle>` between the tabs div and the currency toggle:

```tsx
{onTogglePrivacy != null && hidden != null && (
  <PrivacyToggle hidden={hidden} onToggle={onTogglePrivacy} />
)}
```

Import `PrivacyToggle` from `"./PrivacyToggle"`.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/finance/components/FinanceLayout.tsx
git commit -m "feat(finance): add privacy toggle to toolbar, remove refresh button"
```

---

## Chunk 5: Dashboard Page Rewrite + Sub-Page Polish

### Task 13: Rewrite FinanceOverviewPage

**Files:**
- Modify: `desktop-ui/src/features/finance/pages/FinanceOverviewPage.tsx` — complete rewrite

This is the core task. Rewrite the overview page to use the new layout:

1. **Safe Zone** (top):
   - Hero row: `HealthScoreRing` (left) + `MonthlyPulse` (right) in `grid-cols-2`
   - `BudgetStrip` row

2. **`SensitiveDivider`**

3. **Sensitive Zone** (bottom):
   - `NetWorthCard` (full width)
   - Two-zone grid (`grid-cols-2`):
     - Left: Accounts list + Goals list
     - Right: Investments + Liabilities
   - Recent Transactions (full width)

- [ ] **Step 1: Rewrite the page**

Key changes from the current page:
- Import all new components: `HealthScoreRing`, `MonthlyPulse`, `BudgetStrip`, `SensitiveDivider`, `NetWorthCard`
- Import `usePrivacyMode` hook — destructure `{ hidden, toggle }`
- Import `computeHealthScore` and `computeMonthlyPulse`
- Add `useQuery<FinanceMonthlySummary>("finance_monthly_summary")` data fetch
- Pass `hidden` and `onTogglePrivacy={toggle}` to `<FinanceLayout>` (uses updated props from Task 12)
- Pass `hidden` to all `displayAmount()` and `fmtCompact()` / `fmtMoney()` calls
- Remove the old 12-column grid layout
- Use the section layout from the mockup (hero → budgets → divider → net worth → two-zone → transactions)
- Section titles: `<div className="flex items-center justify-between text-[10px] text-muted uppercase tracking-widest mb-2.5 px-1">` with `<Link>` for "View all →"
- Use existing `Card`, `CardHeader` components for account/goal/investment/liability cards
- Keep existing data fetches (accounts, transactions, budgets, portfolios, investments, goals, liabilities, netWorth)
- Compute `healthScore` with `useMemo` from existing aggregated data
- Compute `monthlyPulse` with `useMemo` from the new `finance_monthly_summary` data
- "View all →" links: use React Router `<Link>` for cmd+click support, styled `text-brand text-[10px] normal-case tracking-normal`
- Add staggered animation classes for entrance effects

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build`
Expected: No errors.

- [ ] **Step 3: Visual test**

Run: `cargo tauri dev`
Navigate to `/finance` and verify:
- Health score ring renders with factors
- Monthly pulse shows direction indicators
- Budget strip shows percentage chips
- Sensitive divider visible
- Net worth card with wealth bar
- Two-zone grid: accounts+goals (left), investments+liabilities (right)
- Transactions at bottom
- Privacy toggle masks all amounts with `•••••••`
- Currency toggle still works

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/finance/pages/FinanceOverviewPage.tsx
git commit -m "feat(finance): rewrite dashboard with health score hero and privacy-first layout"
```

### Task 14: Polish Sub-Pages — Privacy Masking + Visual Uplift

**Files:**
- Modify: `desktop-ui/src/features/finance/pages/AccountsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/TransactionsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/BudgetsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/InvestmentsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/GoalsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/LiabilitiesPage.tsx`

For each page, apply all 5 spec requirements:

- [ ] **Step 1: Add privacy masking to all 6 sub-pages**

In each page:
1. Import `usePrivacyMode` hook
2. Destructure `{ hidden, toggle }` at the top of the component
3. Pass `hidden` and `onTogglePrivacy={toggle}` to `<FinanceLayout>` (replaces old `onRefresh` prop)
4. Pass `hidden` to every `displayAmount()` call — add `hidden,` to the options object
5. Pass `hidden` as the last argument to every direct `fmtMoney()` / `fmtCompact()` call

- [ ] **Step 2: Increase stat card number sizes**

In each page, find the stat card value classes (typically `text-[20px]`) and change to `text-[24px]`.

- [ ] **Step 3: Update section title styling**

In each page, update `<CardHeader>` titles to use uppercase 10px tracking style matching the dashboard:
- Section headers: `text-[10px] text-muted uppercase tracking-widest`
- Or keep existing `CardHeader` but ensure consistency

- [ ] **Step 4: Standardize card padding**

Ensure all cards use consistent `p-4` (16px) padding. Some cards currently use `p-3.5` or `p-3` — normalize to `p-4`.

- [ ] **Step 5: Verify build and visual check**

Run: `cd desktop-ui && bun run build`
Run: `cargo tauri dev` — check each sub-page:
- Privacy toggle works from toolbar on every page
- All dollar amounts mask to `•••••••` when hidden
- Stat card numbers are larger (24px)
- Consistent padding and section title styling

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/finance/pages/
git commit -m "feat(finance): add privacy masking and visual polish to all sub-pages"
```

---

## Chunk 6: Final Integration + Lint

### Task 15: Run Biome Lint and Fix

- [ ] **Step 1: Run lint + format fix**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Auto-fixes any formatting/import issues.

- [ ] **Step 2: Run Rust checks**

Run: `cargo clippy --workspace --all-targets --all-features`
Run: `cargo fmt --all --check`
Expected: Zero warnings.

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 4: Final visual test**

Run: `cargo tauri dev`
Full walkthrough:
- Dashboard loads with health score, monthly pulse, budget strip
- Privacy toggle masks/unmasks amounts
- Currency toggle still works
- Each sub-page has privacy masking + larger stat numbers
- No visual regressions on sub-pages

- [ ] **Step 5: Final commit if any lint fixes**

```bash
git add -A
git commit -m "chore(finance): lint fixes for dashboard redesign"
```
