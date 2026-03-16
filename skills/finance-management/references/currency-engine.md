# Currency Engine

## Auto-Conversion
All monetary records store original amount+currency AND base-currency equivalent (`base_amount`).
When recording a transaction in THB with home currency VND, the system auto-fetches
the exchange rate and stores both amounts. The `base_amount` field is always in the
user's default currency.

## Rate Sources
- Forex: open.er-api.com (15-min cache, two-layer: in-memory + SQLite)
- Crypto: CoinGecko (15-min cache)
- User overrides in config (`exchangeRates` map) take precedence over API rates

## Key Fields
- `currency` — the currency the amount was originally recorded in
- `base_amount` — the equivalent amount in the user's default (home) currency
- `market_currency` — for investments, the currency the asset is quoted in on exchanges

## Investment Display
Three-tier: quantity + market price (in market_currency) + home equivalent.
Example: "0.5 BTC -- $25,000 (637,500,000d)"

## Changing Home Currency
When changing default currency via `settings_update(default_currency: "VND")`, the system
re-computes all `base_amount` fields across transactions, budgets, goals, liabilities,
investments, and snapshots. This is called a "rebase" operation.

## Config Overrides
Users can pin exchange rates in `config.json` under `finance.exchangeRates`:
```json
{
  "finance": {
    "exchangeRates": {
      "THB:VND": 700,
      "USD:VND": 25500
    }
  }
}
```
These take precedence over API-fetched rates. Inverse lookups are automatic (VND:THB = 1/700).
