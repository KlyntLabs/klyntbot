import {
  Banknote,
  BarChart3,
  Bitcoin,
  CreditCard,
  Flame,
  GraduationCap,
  Landmark,
  PiggyBank,
  ShoppingCart,
  Smartphone,
  Target,
  Wallet,
} from "lucide-react";

// ── Formatting ──────────────────────────────────────────────────────────

export function fmtMoney(amount: number, currency: string): string {
  const value = amount / 100;
  if (currency === "VND") return `${new Intl.NumberFormat("vi-VN").format(Math.round(value))}đ`;
  if (currency === "USDT")
    return (
      "$" +
      new Intl.NumberFormat("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(
        value,
      )
    );
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  }).format(value);
}

export function toVnd(amount: number, currency: string, rates: Record<string, number>): number {
  return Math.round(amount * (rates[currency] ?? 1));
}

export function fmtVnd(amount: number): string {
  return `${new Intl.NumberFormat("vi-VN").format(Math.round(amount / 100))}đ`;
}

export function fmtCompact(amount: number): string {
  const v = amount / 100;
  if (Math.abs(v) >= 1e9) return `${(v / 1e9).toFixed(1)}B`;
  if (Math.abs(v) >= 1e6) return `${(v / 1e6).toFixed(1)}M`;
  if (Math.abs(v) >= 1e3) return `${(v / 1e3).toFixed(0)}K`;
  return new Intl.NumberFormat("vi-VN").format(Math.round(v));
}

export function pct(cur: number, tot: number): number {
  return tot === 0 ? 0 : Math.round((cur / tot) * 100);
}

export function retPct(cur: number, basis: number): number {
  return basis === 0 ? 0 : Math.round(((cur - basis) / basis) * 10000) / 100;
}

// ── Icon maps ───────────────────────────────────────────────────────────

export const ACCT_ICONS: Record<string, typeof Wallet> = {
  bank: Landmark,
  ewallet: Smartphone,
  crypto_wallet: Bitcoin,
  cash: Banknote,
  brokerage: BarChart3,
};

export const LIAB_ICONS: Record<string, typeof Wallet> = {
  student_loan: GraduationCap,
  credit_card: CreditCard,
  mortgage: Landmark,
  personal_loan: Wallet,
};

export const GOAL_ICONS: Record<string, typeof Wallet> = {
  savings: PiggyBank,
  purchase: ShoppingCart,
  fire: Flame,
  custom: Target,
};

// Semantic chart colors — CSS variables for theme adaptability,
// with hex fallbacks for SVG fill/stroke where CSS vars don't work.
export const CHART_COLORS = [
  { var: "var(--brand)", hex: "#f97316" },
  { var: "var(--info)", hex: "#3b82f6" },
  { var: "var(--success)", hex: "#22c55e" },
  { var: "var(--purple)", hex: "#8b5cf6" },
  { var: "var(--destructive)", hex: "#f43f5e" },
  { var: "var(--color-cyan-400)", hex: "#06b6d4" },
  { var: "var(--color-amber-500)", hex: "#f59e0b" },
  { var: "var(--color-pink-500)", hex: "#ec4899" },
];

// Keep COLORS for backward compat during migration
export const COLORS = CHART_COLORS.map((c) => c.hex);
