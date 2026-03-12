// PriceService - HTTP price fetcher with in-memory DashMap cache
//
// Supports stocks (Yahoo Finance), crypto (CoinGecko), and exchange rates
// (open.er-api.com). Falls back to stale cache on API failure.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
