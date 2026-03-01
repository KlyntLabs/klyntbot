import { useState, useCallback } from 'react';

const CACHE_KEY = 'klyntbot_exchange_rates';
const CACHE_TTL = 3_600_000;

export interface UseExchangeRatesResult {
  rates: Record<string, number> | null;
  loading: boolean;
  timestamp: string | null;
  refresh: () => void;
  load: () => void;
}

export function useExchangeRates(): UseExchangeRatesResult {
  const [rates, setRates] = useState<Record<string, number> | null>(null);
  const [loading, setLoading] = useState(false);
  const [timestamp, setTimestamp] = useState<string | null>(null);

  const fetchRates = useCallback(async (force = false) => {
    setLoading(true);
    if (!force) {
      try {
        const cached = localStorage.getItem(CACHE_KEY);
        if (cached) {
          const parsed = JSON.parse(cached);
          if (Date.now() - parsed.fetchedAt < CACHE_TTL) {
            setRates(parsed.rates);
            setTimestamp(new Date(parsed.fetchedAt).toLocaleString());
            setLoading(false);
            return;
          }
        }
      } catch { /* ignore */ }
    }
    try {
      const fiatRes = await fetch('https://open.er-api.com/v6/latest/USD');
      const result: Record<string, number> = { USD: 1 };
      if (fiatRes.ok) {
        const data = await fiatRes.json();
        if (data.result === 'success' && data.rates) {
          for (const [cur, perUSD] of Object.entries(data.rates as Record<string, number>)) {
            if (cur !== 'USD' && perUSD > 0) result[cur] = 1 / perUSD;
          }
        }
      }
      try {
        const cryptoRes = await fetch(
          'https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,tether&vs_currencies=usd',
        );
        if (cryptoRes.ok) {
          const cd = await cryptoRes.json();
          if (cd.bitcoin?.usd) result.BTC = cd.bitcoin.usd;
          if (cd.ethereum?.usd) result.ETH = cd.ethereum.usd;
          if (cd.tether?.usd) result.USDT = cd.tether.usd;
        }
      } catch { /* crypto optional */ }
      setRates(result);
      const now = new Date();
      setTimestamp(now.toLocaleString());
      localStorage.setItem(CACHE_KEY, JSON.stringify({ rates: result, fetchedAt: now.getTime() }));
    } catch { /* silent */ }
    setLoading(false);
  }, []);

  const refresh = useCallback(() => fetchRates(true), [fetchRates]);
  const load = useCallback(() => { if (!rates) fetchRates(false); }, [rates, fetchRates]);

  return { rates, loading, timestamp, refresh, load };
}
