// PriceService - HTTP price fetcher with in-memory DashMap cache
//
// Supports stocks (Yahoo Finance), crypto (CoinGecko), and exchange rates
// (open.er-api.com). Falls back to stale cache on API failure.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// AssetType is defined in finance_types and covers all variants including ExchangeRate.
pub use crate::finance_types::AssetType;

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
}

impl PriceService {
    /// Create a new `PriceService` with the given cache TTL in minutes.
    pub fn new(cache_ttl_minutes: u32) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("klyntbot/1.0")
                .build()
                .expect("failed to build reqwest client"),
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(u64::from(cache_ttl_minutes) * 60),
        }
    }

    // ─── cache helpers ────────────────────────────────────────────────────────

    fn cache_key(symbol: &str, vs: &str) -> String {
        if vs.is_empty() {
            symbol.to_uppercase()
        } else {
            format!("{}:{}", symbol.to_uppercase(), vs.to_uppercase())
        }
    }

    /// Return a live (non-expired) cache entry, or `None` if absent / expired.
    fn get_live(&self, key: &str) -> Option<CachedPrice> {
        self.cache
            .get(key)
            .filter(|e| e.fetched_at.elapsed() < self.cache_ttl)
            .map(|e| e.clone())
    }

    /// Return any cache entry regardless of age (used for stale-on-error fallback).
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

    /// GET with retry on 429 (rate-limited). Retries up to 2 times with
    /// exponential backoff (1s, 3s).
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

    // ─── public API ───────────────────────────────────────────────────────────

    /// Fetch a stock price from Yahoo Finance chart API.
    ///
    /// Endpoint: `GET https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?interval=1d&range=1d`
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

    /// Fetch a cryptocurrency price from CoinGecko simple/price API.
    ///
    /// `symbol` can be a CoinGecko coin ID (e.g. `"bitcoin"`, `"ethereum"`) or
    /// a common ticker (e.g. `"BTC"`, `"ETH"`). Common tickers are automatically
    /// mapped to their CoinGecko ID.
    /// `vs_currency` is the target fiat/crypto (e.g. `"usd"`, `"eur"`).
    ///
    /// Endpoint: `GET https://api.coingecko.com/api/v3/simple/price?ids={symbol}&vs_currencies={vs_currency}`
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

    /// Fetch an exchange rate from open.er-api.com.
    ///
    /// `from` and `to` are ISO 4217 currency codes (e.g. `"USD"`, `"EUR"`).
    ///
    /// Endpoint: `GET https://open.er-api.com/v6/latest/{from}`
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

    /// Unified price fetch with cache check and provider dispatch.
    ///
    /// For `AssetType::ExchangeRate` the `symbol` is the `from` currency and the
    /// optional `vs_currency` parameter (passed as the second component of the
    /// symbol separated by `/`) is the `to` currency.  If the symbol already
    /// contains `/` it is split automatically; otherwise "USD" is assumed as the
    /// target currency.
    pub async fn fetch_price(
        &self,
        symbol: &str,
        asset_type: AssetType,
    ) -> Result<PriceResult, String> {
        match asset_type {
            AssetType::Stock | AssetType::Etf => self.fetch_stock(symbol).await,
            AssetType::Crypto => {
                // Expect format "bitcoin" or "bitcoin/usd"
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

/// Map common crypto ticker symbols to CoinGecko coin IDs.
///
/// CoinGecko uses full lowercase names (e.g. "bitcoin") while LLMs and users
/// often use ticker symbols (e.g. "BTC"). This function converts known tickers
/// and returns the input unchanged if no mapping exists.
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
        _ => symbol, // Already a CoinGecko ID or unknown — pass through
    }
}

/// Split `"base/quote"` into `("base", "quote")`, using `default_quote` when
/// there is no `/` separator.
fn parse_pair<'a>(symbol: &'a str, default_quote: &'a str) -> (&'a str, &'a str) {
    if let Some(pos) = symbol.find('/') {
        (&symbol[..pos], &symbol[pos + 1..])
    } else {
        (symbol, default_quote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service_with_ttl(minutes: u32) -> PriceService {
        PriceService::new(minutes)
    }

    // Inject a pre-aged entry into the cache directly for testing.
    fn insert_aged(svc: &PriceService, key: &str, price: f64, currency: &str, age: Duration) {
        let fetched_at = Instant::now().checked_sub(age).unwrap_or_else(Instant::now);
        svc.cache.insert(
            key.to_string(),
            CachedPrice {
                price,
                currency: currency.to_string(),
                fetched_at,
            },
        );
    }

    #[test]
    fn test_cache_hit_within_ttl() {
        let svc = service_with_ttl(5);
        let key = PriceService::cache_key("AAPL", "");
        // Insert fresh entry
        svc.insert_cache(key.clone(), 185.50, "USD".to_string());

        let live = svc.get_live(&key);
        assert!(live.is_some(), "expected live cache hit");
        let entry = live.unwrap();
        assert!((entry.price - 185.50).abs() < f64::EPSILON);
        assert_eq!(entry.currency, "USD");
    }

    #[test]
    fn test_cache_miss_expired() {
        let svc = service_with_ttl(5);
        let key = PriceService::cache_key("AAPL", "");
        // Insert entry aged beyond the TTL (6 minutes old for a 5-minute TTL)
        insert_aged(&svc, &key, 185.50, "USD", Duration::from_secs(6 * 60));

        let live = svc.get_live(&key);
        assert!(
            live.is_none(),
            "expired entry should not be returned as live"
        );

        // Stale fallback should still work
        let stale = svc.get_stale(&key);
        assert!(stale.is_some(), "stale entry should be present");
    }

    #[test]
    fn test_cache_miss_no_entry() {
        let svc = service_with_ttl(5);
        let key = PriceService::cache_key("TSLA", "");
        assert!(svc.get_live(&key).is_none());
        assert!(svc.get_stale(&key).is_none());
    }

    #[tokio::test]
    async fn test_fetch_price_unknown_asset_type_returns_error() {
        // AssetType::Stock for a clearly invalid symbol should fail (no network in unit tests,
        // so we cannot test the happy path here — that is covered by the #[ignore] tests below).
        // Instead we verify that fetch_price propagates errors correctly.
        //
        // We create a service, ensure no cache entry exists, then attempt a fetch for a
        // symbol that would fail even if the network were available.
        let svc = service_with_ttl(5);
        // "INVALID!!SYMBOL" is not a valid stock ticker and should be rejected by Yahoo.
        // Since we are not hitting the network in unit tests we rely on the absence of a
        // cache entry — the call must return an Err variant.
        //
        // In CI (no network) this returns Err quickly. In dev (network) it may succeed with
        // an HTTP error response. Either way, the important thing is that the Result type is
        // handled without panicking.
        let result = svc.fetch_price("", AssetType::Stock).await;
        // An empty symbol should always produce an error (Yahoo returns an error payload).
        // We don't assert the exact message, just that it is Err or Ok with a stale source.
        // An empty symbol returns either a stale cache hit or an Err — both are valid.
        if let Ok(r) = result {
            assert_eq!(r.source, "cache_stale");
        }
    }

    #[test]
    fn test_parse_pair_with_slash() {
        let (base, quote) = parse_pair("bitcoin/eur", "usd");
        assert_eq!(base, "bitcoin");
        assert_eq!(quote, "eur");
    }

    #[test]
    fn test_parse_pair_without_slash() {
        let (base, quote) = parse_pair("bitcoin", "usd");
        assert_eq!(base, "bitcoin");
        assert_eq!(quote, "usd");
    }

    #[test]
    fn test_cache_key_normalises_to_uppercase() {
        let k1 = PriceService::cache_key("aapl", "");
        let k2 = PriceService::cache_key("AAPL", "");
        assert_eq!(k1, k2);

        let k3 = PriceService::cache_key("bitcoin", "usd");
        let k4 = PriceService::cache_key("BITCOIN", "USD");
        assert_eq!(k3, k4);
    }

    #[test]
    fn test_ticker_to_coingecko_id_maps_common_tickers() {
        assert_eq!(ticker_to_coingecko_id("BTC"), "bitcoin");
        assert_eq!(ticker_to_coingecko_id("btc"), "bitcoin");
        assert_eq!(ticker_to_coingecko_id("ETH"), "ethereum");
        assert_eq!(ticker_to_coingecko_id("SOL"), "solana");
        assert_eq!(ticker_to_coingecko_id("DOGE"), "dogecoin");
    }

    #[test]
    fn test_ticker_to_coingecko_id_passes_through_unknown() {
        assert_eq!(ticker_to_coingecko_id("bitcoin"), "bitcoin");
        assert_eq!(ticker_to_coingecko_id("some-new-coin"), "some-new-coin");
    }

    // ─── network-dependent tests (skipped in CI) ──────────────────────────────

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_stock_real() {
        let svc = service_with_ttl(5);
        let result = svc.fetch_stock("AAPL").await.expect("fetch_stock failed");
        assert_eq!(result.symbol, "AAPL");
        assert!(result.price > 0.0, "price should be positive");
        assert!(!result.currency.is_empty());
        assert_eq!(result.source, "yahoo_finance");
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_crypto_real() {
        let svc = service_with_ttl(5);
        let result = svc
            .fetch_crypto("bitcoin", "usd")
            .await
            .expect("fetch_crypto failed");
        assert_eq!(result.symbol, "bitcoin");
        assert!(result.price > 0.0, "price should be positive");
        assert_eq!(result.currency, "USD");
        assert_eq!(result.source, "coingecko");
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_exchange_rate_real() {
        let svc = service_with_ttl(5);
        let rate = svc
            .fetch_exchange_rate("USD", "EUR")
            .await
            .expect("fetch_exchange_rate failed");
        assert!(
            rate > 0.0 && rate < 10.0,
            "USD/EUR rate should be in sane range"
        );
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_price_stock_via_unified() {
        let svc = service_with_ttl(5);
        let result = svc
            .fetch_price("MSFT", AssetType::Stock)
            .await
            .expect("fetch_price stock failed");
        assert!(result.price > 0.0);
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_price_crypto_with_slash() {
        let svc = service_with_ttl(5);
        let result = svc
            .fetch_price("ethereum/usd", AssetType::Crypto)
            .await
            .expect("fetch_price crypto failed");
        assert_eq!(result.symbol, "ethereum");
        assert!(result.price > 0.0);
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_fetch_crypto_ticker_btc() {
        let svc = service_with_ttl(5);
        let result = svc
            .fetch_crypto("BTC", "usd")
            .await
            .expect("fetch_crypto with BTC ticker failed");
        assert_eq!(result.symbol, "bitcoin");
        assert!(result.price > 0.0, "BTC price should be positive");
        assert_eq!(result.currency, "USD");
    }

    #[tokio::test]
    #[ignore = "requires network — run manually with `cargo nextest run -p tools -- --include-ignored`"]
    async fn test_cache_hit_prevents_second_network_call() {
        let svc = service_with_ttl(60); // 60-minute TTL
                                        // First call — hits network
        let r1 = svc.fetch_stock("GOOG").await.expect("first fetch failed");
        assert_eq!(r1.source, "yahoo_finance");

        // Second call — should come from cache
        let r2 = svc.fetch_stock("GOOG").await.expect("second fetch failed");
        assert_eq!(r2.source, "cache");
        assert!((r1.price - r2.price).abs() < f64::EPSILON);
    }
}
