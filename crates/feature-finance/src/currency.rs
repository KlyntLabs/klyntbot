//! Currency conversion helpers for the finance write path.

use crate::price_service::PriceService;

/// Result of converting a single amount to the user's base currency.
pub struct BaseConversion {
    pub base_amount: i64,
    pub base_currency: String,
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

/// Convert `amount` in `currency` to `base_currency`, returning the converted amount and rate.
///
/// If `currency` already matches `base_currency` (case-insensitive), returns the amount as-is
/// with rate 1.0.
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
    let rate = price_service.get_rate(currency, base_currency).await?;
    let base_amount = (amount as f64 * rate).round() as i64;
    Ok(BaseConversion {
        base_amount,
        base_currency: base_currency.to_string(),
        exchange_rate: rate,
    })
}

/// Convert investment cost basis and current value to `base_currency`.
///
/// `purchase_currency` is the currency the investment was purchased in.
/// `market_currency` is the currency the asset is quoted in on exchanges (defaults to
/// `purchase_currency` if `None`).
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
        price_service
            .get_rate(purchase_currency, base_currency)
            .await?
    };

    let market_rate = if mkt_currency.eq_ignore_ascii_case(base_currency) {
        1.0
    } else {
        price_service.get_rate(mkt_currency, base_currency).await?
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

    #[tokio::test]
    async fn same_currency_returns_identity() {
        let svc = PriceService::new(15);
        let conv = ensure_base_amount(100_000, "USD", "USD", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_amount, 100_000);
        assert_eq!(conv.base_currency, "USD");
        assert!((conv.exchange_rate - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn same_currency_case_insensitive() {
        let svc = PriceService::new(15);
        let conv = ensure_base_amount(50_000, "usd", "USD", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_amount, 50_000);
        assert!((conv.exchange_rate - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn investment_same_currency_returns_identity() {
        let svc = PriceService::new(15);
        let conv = ensure_investment_base(1_000_000, "EUR", Some(1_200_000), None, "EUR", &svc)
            .await
            .unwrap();
        assert_eq!(conv.base_cost_basis, 1_000_000);
        assert_eq!(conv.base_current_value, 1_200_000);
        assert_eq!(conv.base_currency, "EUR");
        assert!((conv.purchase_rate - 1.0).abs() < f64::EPSILON);
        assert!((conv.market_rate - 1.0).abs() < f64::EPSILON);
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
