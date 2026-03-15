import type { CurrencyDisplayMode } from "../hooks/useCurrencyDisplayMode";
import { fmtCompact, fmtMoney } from "./finance";

/**
 * Rate map: currency → rate to convert TO that currency FROM baseCurrency.
 * Built from entity exchangeRate fields.
 *
 * Example: if baseCurrency=USD and VND account has exchangeRate=0.000038 (VND→USD),
 * then rates["VND"] = 1/0.000038 = 26315 (USD→VND multiplier).
 */
export type RateMap = Record<string, number>;

/**
 * Build a rate map from entities that have currency + exchangeRate.
 * The map converts baseCurrency → each currency.
 */
export function buildRateMap(
  entities: Array<{ currency: string; exchangeRate?: number }>,
  baseCurrency: string,
): RateMap {
  const rates: RateMap = { [baseCurrency]: 1 };
  for (const e of entities) {
    if (e.currency === baseCurrency || !e.exchangeRate || e.exchangeRate === 0) continue;
    // exchangeRate = original → base, so inverse = base → original
    if (!rates[e.currency]) {
      rates[e.currency] = 1 / e.exchangeRate;
    }
  }
  return rates;
}

interface AmountDisplayOptions {
  amount: number;
  currency: string;
  baseAmount?: number;
  baseCurrency: string;
  mode: CurrencyDisplayMode;
  compact?: boolean;
  /** Rates to convert baseCurrency → target. Optional, needed for cross-currency display. */
  rates?: RateMap;
}

/**
 * Convert an amount to the target display currency.
 */
function resolveAmount(opts: AmountDisplayOptions): { value: number; currency: string } {
  const target = opts.mode;

  // Multi mode: show original
  if (target === "multi") {
    return { value: opts.amount, currency: opts.currency };
  }

  // Target matches original currency: show original as-is
  if (target === opts.currency) {
    return { value: opts.amount, currency: opts.currency };
  }

  // Target matches base currency: show pre-computed baseAmount
  if (target === opts.baseCurrency && opts.baseAmount != null) {
    return { value: opts.baseAmount, currency: opts.baseCurrency };
  }

  // Cross-conversion: target is a different currency than both original and base
  // Convert via baseAmount → target using rates
  if (opts.baseAmount != null && opts.rates) {
    const rateToTarget = opts.rates[target];
    if (rateToTarget != null) {
      const converted = Math.round(opts.baseAmount * rateToTarget);
      return { value: converted, currency: target };
    }
  }

  // Fallback: show baseAmount if available
  if (opts.baseAmount != null) {
    return { value: opts.baseAmount, currency: opts.baseCurrency };
  }

  return { value: opts.amount, currency: opts.currency };
}

/**
 * Primary display amount based on mode.
 */
export function displayAmount(opts: AmountDisplayOptions): string {
  const fmt = opts.compact ? fmtCompact : fmtMoney;
  const { value, currency } = resolveAmount(opts);
  return fmt(value, currency);
}

/**
 * Resolve to a numeric value in the target currency (for aggregation).
 */
export function resolveNumeric(opts: AmountDisplayOptions): number {
  return resolveAmount(opts).value;
}

/**
 * The currency that resolveAmount would return for the given options.
 */
export function resolvedCurrency(opts: AmountDisplayOptions): string {
  return resolveAmount(opts).currency;
}

/**
 * Secondary hint text (the "other" currency).
 * In single-currency mode: shows original if different.
 * In multi mode: shows base equivalent if different.
 */
export function displayHint(opts: AmountDisplayOptions): string | null {
  const fmt = opts.compact ? fmtCompact : fmtMoney;
  const resolved = resolveAmount(opts);

  if (opts.mode === "multi") {
    // Multi: hint is base equivalent
    if (opts.currency === opts.baseCurrency || opts.baseAmount == null) return null;
    return fmt(opts.baseAmount, opts.baseCurrency);
  }

  // Single currency: hint is original if it differs from what we're showing
  if (resolved.currency !== opts.currency) {
    return fmt(opts.amount, opts.currency);
  }
  return null;
}
