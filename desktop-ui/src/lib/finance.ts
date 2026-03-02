import {
  Wallet,
  Landmark,
  Smartphone,
  Bitcoin,
  Banknote,
  BarChart3,
  CreditCard,
  GraduationCap,
  PiggyBank,
  ShoppingCart,
  Flame,
  Target,
} from 'lucide-react';

// ── Formatting ──────────────────────────────────────────────────────────

export function fmtMoney(amount: number, currency: string): string {
  const value = amount / 100;
  if (currency === 'VND') return new Intl.NumberFormat('vi-VN').format(Math.round(value)) + 'đ';
  if (currency === 'USDT') return '$' + new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(value);
  return new Intl.NumberFormat('en-US', { style: 'currency', currency, minimumFractionDigits: 0, maximumFractionDigits: 2 }).format(value);
}

export function toVnd(amount: number, currency: string, rates: Record<string, number>): number {
  return Math.round(amount * (rates[currency] ?? 1));
}

export function fmtVnd(amount: number): string {
  return new Intl.NumberFormat('vi-VN').format(Math.round(amount / 100)) + 'đ';
}

export function fmtCompact(amount: number): string {
  const v = amount / 100;
  if (Math.abs(v) >= 1e9) return (v / 1e9).toFixed(1) + 'B';
  if (Math.abs(v) >= 1e6) return (v / 1e6).toFixed(1) + 'M';
  if (Math.abs(v) >= 1e3) return (v / 1e3).toFixed(0) + 'K';
  return new Intl.NumberFormat('vi-VN').format(Math.round(v));
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

export const COLORS = ['#F97316', '#3B82F6', '#22C55E', '#8B5CF6', '#EF4444', '#06B6D4', '#F59E0B', '#EC4899'];
