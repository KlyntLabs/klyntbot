// PriceService - HTTP price fetcher with in-memory DashMap cache
//
// Supports stocks (Yahoo Finance), crypto (CoinGecko), and exchange rates
// (open.er-api.com). Falls back to stale cache on API failure.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::rate_cache::RateCache;
use crate::types::AssetType;
use common::build_http_client_with_builder;

/// A cached price entry.
#[derive(Debug, Clone)]
pub struct CachedPrice {
    pub price: f64,
    pub currency: String,
    pub fetched_at: Instant,
}

/// Result returned by a successful price fetch.
#[derive(Debug, Clone)]
pub struct PriceResult {
    pub symbol: String,
    pub price: f64,
    pub currency: String,
    /// Data source identifier (e.g. "yahoo_finance", "coingecko", "er_api", "cache").
    pub source: String,
}

/// HTTP price fetcher with DashMap-backed TTL cache.
///
/// Cheaply cloneable — the internal `reqwest::Client` and `DashMap` are both
/// `Arc`-wrapped and share state across clones.
#[derive(Clone)]
pub struct PriceService {
    client: reqwest::Client,
    cache: Arc<DashMap<String, CachedPrice>>,
    cache_ttl: Duration,
    rate_cache: Option<RateCache>,
    exchange_rate_overrides: Option<HashMap<String, f64>>,
}

impl PriceService {
    /// Create a new `PriceService` with the given cache TTL in minutes.
    pub fn new(cache_ttl_minutes: u32) -> Self {
        Self {
            client: build_http_client_with_builder(|builder| {
                builder
                    .timeout(Duration::from_secs(10))
                    .user_agent("klyntbot/1.0")
            })
            .expect("failed to build reqwest client"),
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(u64::from(cache_ttl_minutes) * 60),
            rate_cache: None,
            exchange_rate_overrides: None,
        }
    }

    /// Create a `PriceService` with a two-layer rate cache and optional overrides.
    pub fn with_rate_cache(cache_ttl_minutes: u32, rate_cache: RateCache) -> Self {
        let mut svc = Self::new(cache_ttl_minutes);
        svc.rate_cache = Some(rate_cache);
        svc
    }

    /// Set exchange rate overrides (key format: "FROM:TO", uppercase).
    pub fn set_exchange_rate_overrides(&mut self, overrides: HashMap<String, f64>) {
        self.exchange_rate_overrides = Some(overrides);
    }

    /// Access the rate cache (if configured).
    pub fn rate_cache(&self) -> Option<&RateCache> {
        self.rate_cache.as_ref()
    }

    // ─── cache helpers ────────────────────────────────────────────────────────

    fn cache_key(symbol: &str, vs: &str) -> String {
        if vs.is_empty() {
            symbol.to_uppercase()
        } else {
            format!("{}:{}", symbol.to_uppercase(), vs.to_uppercase())
        }
    }

    fn get_live(&self, key: &str) -> Option<CachedPrice> {
        self.cache
            .get(key)
            .filter(|e| e.fetched_at.elapsed() < self.cache_ttl)
            .map(|e| e.clone())
    }

    fn get_stale(&self, key: &str) -> Option<CachedPrice> {
        self.cache.get(key).map(|e| e.clone())
    }

    fn insert_cache(&self, key: String, price: f64, currency: String) {
        self.cache.insert(
            key,
            CachedPrice {
                price,
                currency,
                fetched_at: Instant::now(),
            },
        );
    }

    // ─── HTTP helper ─────────────────────────────────────────────────────────

    async fn get_with_retry(&self, url: &str) -> std::result::Result<reqwest::Response, String> {
        let delays = [
            tokio::time::Duration::from_secs(1),
            tokio::time::Duration::from_secs(3),
        ];
        let mut last_err = String::new();

        for attempt in 0..=delays.len() {
            match self.client.get(url).send().await {
                Ok(resp) if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    if attempt < delays.len() {
                        tokio::time::sleep(delays[attempt]).await;
                        last_err = "HTTP 429 Too Many Requests".to_string();
                        continue;
                    }
                    return Err("HTTP 429 Too Many Requests (after retries)".to_string());
                }
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) => return Err(format!("HTTP {}", resp.status())),
                Err(e) => return Err(e.to_string()),
            }
        }
        Err(last_err)
    }

    // ─── exchange rate methods ────────────────────────────────────────────────

    /// Check config overrides for a static exchange rate.
    ///
    /// Key format: "FROM:TO" (uppercase). If "FROM:TO" is not found, tries
    /// the inverse "TO:FROM" and returns `1.0 / rate`.
    pub fn config_override_rate(
        overrides: &Option<HashMap<String, f64>>,
        from: &str,
        to: &str,
    ) -> Option<f64> {
        let overrides = overrides.as_ref()?;
        let key = format!("{}:{}", from.to_uppercase(), to.to_uppercase());
        if let Some(&rate) = overrides.get(&key) {
            return Some(rate);
        }
        // Try inverse
        let inverse_key = format!("{}:{}", to.to_uppercase(), from.to_uppercase());
        if let Some(&rate) = overrides.get(&inverse_key) {
            if rate != 0.0 {
                return Some(1.0 / rate);
            }
        }
        None
    }

    /// Get an exchange rate from `from` to `to`, using the layered strategy:
    /// 1. Same currency → 1.0
    /// 2. Config overrides
    /// 3. Fresh cache (L1 + L2) → API fetch → stale cache fallback → error
    pub async fn get_rate(&self, from: &str, to: &str) -> common::Result<f64> {
        // Same currency
        if from.eq_ignore_ascii_case(to) {
            return Ok(1.0);
        }

        // Config overrides
        if let Some(rate) = Self::config_override_rate(&self.exchange_rate_overrides, from, to) {
            return Ok(rate);
        }

        if let Some(ref rc) = self.rate_cache {
            // Fresh cache
            if let Some(rate) = rc.get(from, to, true).await {
                return Ok(rate);
            }

            // API fetch
            match self.fetch_exchange_rate(from, to).await {
                Ok(rate) => {
                    // Store in rate cache
                    let _ = rc.put(from, to, rate).await;
                    return Ok(rate);
                }
                Err(_api_err) => {
                    // Stale fallback
                    if let Some(rate) = rc.get(from, to, false).await {
                        return Ok(rate);
                    }
                    return Err(common::ProviderError::Http(format!(
                        "exchange rate unavailable for {from}/{to}: {_api_err}"
                    ))
                    .into());
                }
            }
        }

        // Legacy path: no rate_cache, API-only
        self.fetch_exchange_rate(from, to)
            .await
            .map_err(|e| common::KlyntbotError::Provider(common::ProviderError::Http(e)))
    }

    /// Prefetch exchange rates from a base currency to multiple targets.
    ///
    /// Calls the open.er-api.com bulk endpoint, inverts rates to get foreign→base,
    /// and stores all results in the rate cache.
    ///
    /// Returns a vec of `(currency, rate_to_base)` for each requested currency found.
    pub async fn prefetch_rates(
        &self,
        base: &str,
        currencies: &[String],
    ) -> common::Result<Vec<(String, f64)>> {
        let url = format!(
            "https://open.er-api.com/v6/latest/{}",
            urlencoding::encode(base)
        );

        let response = self
            .get_with_retry(&url)
            .await
            .map_err(common::ProviderError::Http)?;

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| common::ProviderError::Http(format!("JSON parse error: {e}")))?;

        let rates_obj = json
            .get("rates")
            .ok_or_else(|| common::ProviderError::Http("missing rates in response".into()))?;

        let mut results = Vec::new();
        let mut cache_batch = Vec::new();

        for currency in currencies {
            let upper = currency.to_uppercase();
            if upper == base.to_uppercase() {
                results.push((upper.clone(), 1.0));
                continue;
            }

            if let Some(base_to_foreign) = rates_obj.get(&upper).and_then(|v| v.as_f64()) {
                // API gives base→foreign rate. We want foreign→base = 1/rate.
                let foreign_to_base = if base_to_foreign != 0.0 {
                    1.0 / base_to_foreign
                } else {
                    0.0
                };

                results.push((upper.clone(), foreign_to_base));

                // Cache both directions
                cache_batch.push((upper.clone(), foreign_to_base));
            }
        }

        // Store in rate cache if available
        if let Some(ref rc) = self.rate_cache {
            // Store foreign→base rates
            if !cache_batch.is_empty() {
                let _ = rc.put_batch("__bulk", &cache_batch).await;
            }

            // Also store individual pairs
            for (currency, rate_to_base) in &cache_batch {
                let _ = rc.put(currency, &base.to_uppercase(), *rate_to_base).await;
                // And the inverse: base→foreign
                if *rate_to_base != 0.0 {
                    let _ = rc
                        .put(&base.to_uppercase(), currency, 1.0 / rate_to_base)
                        .await;
                }
            }
        }

        Ok(results)
    }

    // ─── public API ───────────────────────────────────────────────────────────

    pub async fn fetch_stock(&self, symbol: &str) -> Result<PriceResult, String> {
        let key = Self::cache_key(symbol, "");
        if let Some(entry) = self.get_live(&key) {
            return Ok(PriceResult {
                symbol: symbol.to_uppercase(),
                price: entry.price,
                currency: entry.currency,
                source: "cache".to_string(),
            });
        }

        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d",
            urlencoding::encode(symbol)
        );

        match self.get_with_retry(&url).await {
            Ok(response) => {
                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("JSON parse error: {e}"))?;

                let meta = json
                    .pointer("/chart/result/0/meta")
                    .ok_or_else(|| "missing chart.result[0].meta".to_string())?;

                let price = meta
                    .get("regularMarketPrice")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| "missing regularMarketPrice".to_string())?;

                let currency = meta
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("USD")
                    .to_string();

                self.insert_cache(key, price, currency.clone());

                Ok(PriceResult {
                    symbol: symbol.to_uppercase(),
                    price,
                    currency,
                    source: "yahoo_finance".to_string(),
                })
            }
            Err(err) => {
                if let Some(stale) = self.get_stale(&key) {
                    return Ok(PriceResult {
                        symbol: symbol.to_uppercase(),
                        price: stale.price,
                        currency: stale.currency,
                        source: "cache_stale".to_string(),
                    });
                }
                Err(format!("yahoo finance error for {symbol}: {err}"))
            }
        }
    }

    pub async fn fetch_crypto(
        &self,
        symbol: &str,
        vs_currency: &str,
    ) -> Result<PriceResult, String> {
        let coin_id = ticker_to_coingecko_id(symbol);
        let key = Self::cache_key(coin_id, vs_currency);
        if let Some(entry) = self.get_live(&key) {
            return Ok(PriceResult {
                symbol: coin_id.to_lowercase(),
                price: entry.price,
                currency: vs_currency.to_uppercase(),
                source: "cache".to_string(),
            });
        }

        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
            urlencoding::encode(coin_id),
            urlencoding::encode(vs_currency)
        );

        match self.get_with_retry(&url).await {
            Ok(response) => {
                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("JSON parse error: {e}"))?;

                let price = json
                    .get(coin_id.to_lowercase().as_str())
                    .and_then(|coin| coin.get(vs_currency.to_lowercase().as_str()))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        format!("price not found in response for {coin_id}/{vs_currency}")
                    })?;

                let currency = vs_currency.to_uppercase();
                self.insert_cache(key, price, currency.clone());

                Ok(PriceResult {
                    symbol: coin_id.to_lowercase(),
                    price,
                    currency,
                    source: "coingecko".to_string(),
                })
            }
            Err(err) => {
                if let Some(stale) = self.get_stale(&key) {
                    return Ok(PriceResult {
                        symbol: coin_id.to_lowercase(),
                        price: stale.price,
                        currency: stale.currency,
                        source: "cache_stale".to_string(),
                    });
                }
                Err(format!(
                    "coingecko error for {coin_id}/{vs_currency}: {err}"
                ))
            }
        }
    }

    pub async fn fetch_exchange_rate(&self, from: &str, to: &str) -> Result<f64, String> {
        let key = Self::cache_key(from, to);
        if let Some(entry) = self.get_live(&key) {
            return Ok(entry.price);
        }

        let url = format!(
            "https://open.er-api.com/v6/latest/{}",
            urlencoding::encode(from)
        );

        match self.get_with_retry(&url).await {
            Ok(response) => {
                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("JSON parse error: {e}"))?;

                let rate = json
                    .get("rates")
                    .and_then(|rates| rates.get(to.to_uppercase().as_str()))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| format!("rate not found in response for {from}/{to}"))?;

                self.insert_cache(key, rate, to.to_uppercase());

                Ok(rate)
            }
            Err(err) => {
                if let Some(stale) = self.get_stale(&key) {
                    return Ok(stale.price);
                }
                Err(format!("exchange rate error for {from}/{to}: {err}"))
            }
        }
    }

    pub async fn fetch_price(
        &self,
        symbol: &str,
        asset_type: AssetType,
    ) -> Result<PriceResult, String> {
        match asset_type {
            AssetType::Stock | AssetType::Etf => self.fetch_stock(symbol).await,
            AssetType::Crypto => {
                let (coin, vs) = parse_pair(symbol, "usd");
                self.fetch_crypto(coin, vs).await
            }
            AssetType::ExchangeRate => {
                let (from, to) = parse_pair(symbol, "USD");
                let rate = self.fetch_exchange_rate(from, to).await?;
                Ok(PriceResult {
                    symbol: format!("{}/{}", from.to_uppercase(), to.to_uppercase()),
                    price: rate,
                    currency: to.to_uppercase(),
                    source: "er_api".to_string(),
                })
            }
            other => Err(format!(
                "price fetch not supported for asset type '{}'",
                other
            )),
        }
    }
}

fn ticker_to_coingecko_id(symbol: &str) -> &str {
    match symbol.to_uppercase().as_str() {
        "BTC" | "BITCOIN" => "bitcoin",
        "ETH" | "ETHEREUM" => "ethereum",
        "BNB" => "binancecoin",
        "SOL" | "SOLANA" => "solana",
        "XRP" | "RIPPLE" => "ripple",
        "ADA" | "CARDANO" => "cardano",
        "DOGE" | "DOGECOIN" => "dogecoin",
        "DOT" | "POLKADOT" => "polkadot",
        "AVAX" | "AVALANCHE" => "avalanche-2",
        "MATIC" | "POL" | "POLYGON" => "matic-network",
        "LINK" | "CHAINLINK" => "chainlink",
        "SHIB" => "shiba-inu",
        "UNI" | "UNISWAP" => "uniswap",
        "LTC" | "LITECOIN" => "litecoin",
        "ATOM" | "COSMOS" => "cosmos",
        "XLM" | "STELLAR" => "stellar",
        "NEAR" => "near",
        "APT" | "APTOS" => "aptos",
        "SUI" => "sui",
        "ARB" | "ARBITRUM" => "arbitrum",
        "OP" | "OPTIMISM" => "optimism",
        "PEPE" => "pepe",
        "TRX" | "TRON" => "tron",
        _ => symbol,
    }
}

fn parse_pair<'a>(symbol: &'a str, default_quote: &'a str) -> (&'a str, &'a str) {
    if let Some(pos) = symbol.find('/') {
        (&symbol[..pos], &symbol[pos + 1..])
    } else {
        (symbol, default_quote)
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_rate_same_currency() {
        let svc = PriceService::new(15);
        let rate = svc.get_rate("USD", "USD").await.unwrap();
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_get_rate_case_insensitive() {
        let svc = PriceService::new(15);
        let rate = svc.get_rate("usd", "USD").await.unwrap();
        assert!((rate - 1.0).abs() < f64::EPSILON);

        let rate = svc.get_rate("Eur", "eur").await.unwrap();
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_override() {
        let mut overrides = HashMap::new();
        overrides.insert("USD:EUR".to_string(), 0.92);
        overrides.insert("GBP:USD".to_string(), 1.27);
        let overrides = Some(overrides);

        // Direct match
        let rate = PriceService::config_override_rate(&overrides, "USD", "EUR");
        assert_eq!(rate, Some(0.92));

        // Case insensitive
        let rate = PriceService::config_override_rate(&overrides, "usd", "eur");
        assert_eq!(rate, Some(0.92));

        // Inverse lookup
        let rate = PriceService::config_override_rate(&overrides, "EUR", "USD");
        assert!(rate.is_some());
        let r = rate.unwrap();
        assert!((r - 1.0 / 0.92).abs() < 1e-10);

        // Missing pair
        let rate = PriceService::config_override_rate(&overrides, "JPY", "CHF");
        assert_eq!(rate, None);

        // None overrides
        let rate = PriceService::config_override_rate(&None, "USD", "EUR");
        assert_eq!(rate, None);
    }
}
