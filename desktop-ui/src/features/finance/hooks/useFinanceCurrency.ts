import { useQuery } from "@shared/hooks/useQuery";
import type { FinanceAccount } from "@shared/types";
import { useCallback, useMemo } from "react";
import { buildRateMap, type RateMap } from "../lib/displayAmount";
import { useCurrencyDisplayMode } from "./useCurrencyDisplayMode";

export function useFinanceCurrency() {
  const { mode, setMode } = useCurrencyDisplayMode();

  const { data: settings } = useQuery<{ defaultCurrency: string }>(
    "finance_settings",
    undefined,
    {},
  );
  const baseCurrency = settings?.defaultCurrency ?? "USD";

  const { data: accounts } = useQuery<FinanceAccount[]>("finance_accounts", undefined, []);

  const rates = useMemo(
    () =>
      buildRateMap(
        (accounts ?? []).map((a) => ({ currency: a.currency, exchangeRate: a.exchangeRate })),
        baseCurrency,
      ),
    [accounts, baseCurrency],
  );

  const currencies = useMemo(
    () => [...new Set(accounts?.map((a) => a.currency) ?? [])].sort(),
    [accounts],
  );

  const displayCur = mode === "multi" ? baseCurrency : mode;

  const convertTotal = useCallback(
    (baseTotal: number): number => {
      if (mode === "multi" || mode === baseCurrency) return baseTotal;
      const rate = rates[mode];
      return rate ? Math.round(baseTotal * rate) : baseTotal;
    },
    [mode, baseCurrency, rates],
  );

  return { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal };
}
