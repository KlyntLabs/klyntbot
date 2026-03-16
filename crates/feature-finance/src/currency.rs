//! Currency conversion helpers for the finance write path.
//!
//! Amounts are stored as i64 in the smallest currency unit:
//! - USD: cents (1 dollar = 100 cents)
//! - VND: dong  (1 dong = 1, zero-decimal)
//!
//! Exchange rates from APIs are in major units (1 VND = 0.000039 USD).
//! We store an **effective rate** that converts smallest-unit to smallest-unit:
//!   effective_rate = api_rate × (target_subunit / source_subunit)
//!
//! This means `stored_amount × exchange_rate = base_amount` works everywhere
//! (Rust, SQL rebase, frontend) without additional adjustment.

use crate::price_service::PriceService;

/// Zero-decimal currencies — the stored integer IS the major unit.
/// KEEP IN SYNC with `desktop-ui/src/features/finance/lib/finance.ts` ZERO_DECIMAL set.
const ZERO_DECIMAL: &[&str] = &[
    "VND", "JPY", "KRW", "CLP", "HUF", "ISK", "UGX", "RWF", "PYG", "BIF",
];

/// How many smallest-units per 1 major unit for the given currency.
fn subunit_factor(currency: &str) -> f64 {
    if ZERO_DECIMAL
        .iter()
        .any(|z| z.eq_ignore_ascii_case(currency))
    {
        1.0
    } else {
        100.0 // cents, pence, etc.
    }
}

/// Convert an API rate (major→major) to an effective rate (smallest→smallest).
///
/// Example: VND→USD API rate = 0.000039
///   effective = 0.000039 × (100 / 1) = 0.0039
///   Then: 104,500,000 dong × 0.0039 = 407,550 cents = $4,075.50
pub fn effective_rate(api_rate: f64, from_currency: &str, to_currency: &str) -> f64 {
    let from_factor = subunit_factor(from_currency);
    let to_factor = subunit_factor(to_currency);
    api_rate * (to_factor / from_factor)
}

/// Result of converting a single amount to the user's base currency.
pub struct BaseConversion {
    pub base_amount: i64,
    pub base_currency: String,
    /// The effective rate (smallest-unit to smallest-unit).
    /// `amount × exchange_rate ≈ base_amount`.
    pub exchange_rate: f64,
}

/// Result of converting investment amounts (cost basis + current value) to base currency.
pub struct InvestmentBaseConversion {
    pub base_cost_basis: i64,
    pub base_current_value: i64,
    pub base_currency: String,
    pub purchase_rate: f64,
    pub market_rate: f64,
}

/// Convert `amount` in `currency` to `base_currency`, returning the converted amount
/// and the effective rate.
pub async fn ensure_base_amount(
    amount: i64,
    currency: &str,
    base_currency: &str,
    price_service: &PriceService,
) -> common::Result<BaseConversion> {
    if currency.eq_ignore_ascii_case(base_currency) {
        return Ok(BaseConversion {
            base_amount: amount,
            base_currency: base_currency.to_string(),
            exchange_rate: 1.0,
        });
    }
    let api_rate = price_service.get_rate(currency, base_currency).await?;
    let eff = effective_rate(api_rate, currency, base_currency);
    let base_amount = (amount as f64 * eff).round() as i64;
    Ok(BaseConversion {
        base_amount,
        base_currency: base_currency.to_string(),
        exchange_rate: eff,
    })
}

/// Convert investment cost basis and current value to `base_currency`.
pub async fn ensure_investment_base(
    cost_basis: i64,
    purchase_currency: &str,
    current_value: Option<i64>,
    market_currency: Option<&str>,
    base_currency: &str,
    price_service: &PriceService,
) -> common::Result<InvestmentBaseConversion> {
    let mkt_currency = market_currency.unwrap_or(purchase_currency);

    let purchase_rate = if purchase_currency.eq_ignore_ascii_case(base_currency) {
        1.0
    } else {
        let api = price_service
            .get_rate(purchase_currency, base_currency)
            .await?;
        effective_rate(api, purchase_currency, base_currency)
    };

    let market_rate = if mkt_currency.eq_ignore_ascii_case(base_currency) {
        1.0
    } else {
        let api = price_service.get_rate(mkt_currency, base_currency).await?;
        effective_rate(api, mkt_currency, base_currency)
    };

    let base_cost_basis = (cost_basis as f64 * purchase_rate).round() as i64;
    let base_current_value = current_value
        .map(|v| (v as f64 * market_rate).round() as i64)
        .unwrap_or(0);

    Ok(InvestmentBaseConversion {
        base_cost_basis,
        base_current_value,
        base_currency: base_currency.to_string(),
        purchase_rate,
        market_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_rate_vnd_to_usd() {
        // 1 VND = 0.000039 USD (major). USD has 100 subunits, VND has 1.
        let eff = effective_rate(0.000039, "VND", "USD");
        // 0.000039 * (100/1) = 0.0039
        assert!((eff - 0.0039).abs() < 1e-10);
    }

    #[test]
    fn test_effective_rate_usd_to_vnd() {
        // 1 USD = 25641 VND (major). VND has 1 subunit, USD has 100.
        let eff = effective_rate(25641.0, "USD", "VND");
        // 25641 * (1/100) = 256.41
        assert!((eff - 256.41).abs() < 0.01);
    }

    #[test]
    fn test_effective_rate_usd_to_eur() {
        // Both 2-decimal: factor ratio is 1.0
        let eff = effective_rate(0.92, "USD", "EUR");
        assert!((eff - 0.92).abs() < 1e-10);
    }

    #[test]
    fn test_convert_vnd_to_usd() {
        let eff = effective_rate(0.000039, "VND", "USD");
        let amount: i64 = 104_500_000; // 104.5M VND
        let base = (amount as f64 * eff).round() as i64;
        // 104,500,000 * 0.0039 = 407,550 cents = $4,075.50
        assert_eq!(base, 407_550);
    }

    #[test]
    fn test_convert_usd_to_vnd() {
        let eff = effective_rate(25500.0, "USD", "VND");
        let amount: i64 = 320_000; // $3,200 = 320,000 cents
        let base = (amount as f64 * eff).round() as i64;
        // 3200 * 25500 = 81,600,000 VND
        assert_eq!(base, 81_600_000);
    }

    #[tokio::test]
    async fn same_currency_returns_identity() {
        let svc = PriceService::new(15);
        let conv = ensure_base_amount(100_000, "USD", "USD", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_amount, 100_000);
        assert!((conv.exchange_rate - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn same_currency_case_insensitive() {
        let svc = PriceService::new(15);
        let conv = ensure_base_amount(50_000, "usd", "USD", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_amount, 50_000);
    }

    #[tokio::test]
    async fn investment_no_current_value_defaults_to_zero() {
        let svc = PriceService::new(15);
        let conv = ensure_investment_base(500_000, "USD", None, None, "USD", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_current_value, 0);
    }
}
