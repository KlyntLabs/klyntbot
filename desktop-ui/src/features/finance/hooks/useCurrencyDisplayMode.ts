import { useCallback, useState } from "react";

/**
 * Currency display mode:
 * - "multi" = show each amount in its native currency (original behavior)
 * - Any currency code (e.g. "USD", "VND") = show everything converted to that currency
 */
export type CurrencyDisplayMode = "multi" | string;

export function useCurrencyDisplayMode() {
  const [mode, setModeState] = useState<CurrencyDisplayMode>(() => {
    return localStorage.getItem("finance:currencyDisplayMode") ?? "multi";
  });

  const setMode = useCallback((m: CurrencyDisplayMode) => {
    setModeState(m);
    localStorage.setItem("finance:currencyDisplayMode", m);
  }, []);

  return { mode, setMode };
}
