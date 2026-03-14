//! Currency-aware monetary type using rust_decimal for precision.

use rust_decimal::Decimal;
use std::fmt;
use std::ops::{Add, Sub};

use crate::{Result, ToolError};

/// ISO 4217 currency with known decimal places.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Currency {
    USD,
    EUR,
    GBP,
    THB,
    JPY,
    KWD,
    AUD,
    CAD,
    CHF,
    SGD,
    HKD,
    NZD,
    SEK,
    NOK,
    DKK,
    CNY,
    INR,
    KRW,
    MYR,
    PHP,
    IDR,
    VND,
    TWD,
    BRL,
    MXN,
    ZAR,
    AED,
    SAR,
    QAR,
    BHD,
    #[serde(untagged)]
    Custom {
        code: String,
        decimal_places: u8,
    },
}

impl Currency {
    /// Number of decimal places for this currency's smallest unit.
    pub fn decimal_places(&self) -> u8 {
        match self {
            Self::JPY | Self::KRW | Self::VND => 0,
            Self::KWD | Self::BHD => 3,
            Self::Custom { decimal_places, .. } => *decimal_places,
            _ => 2,
        }
    }

    /// ISO 4217 currency code as string.
    pub fn code(&self) -> &str {
        match self {
            Self::USD => "USD",
            Self::EUR => "EUR",
            Self::GBP => "GBP",
            Self::THB => "THB",
            Self::JPY => "JPY",
            Self::KWD => "KWD",
            Self::AUD => "AUD",
            Self::CAD => "CAD",
            Self::CHF => "CHF",
            Self::SGD => "SGD",
            Self::HKD => "HKD",
            Self::NZD => "NZD",
            Self::SEK => "SEK",
            Self::NOK => "NOK",
            Self::DKK => "DKK",
            Self::CNY => "CNY",
            Self::INR => "INR",
            Self::KRW => "KRW",
            Self::MYR => "MYR",
            Self::PHP => "PHP",
            Self::IDR => "IDR",
            Self::VND => "VND",
            Self::TWD => "TWD",
            Self::BRL => "BRL",
            Self::MXN => "MXN",
            Self::ZAR => "ZAR",
            Self::AED => "AED",
            Self::SAR => "SAR",
            Self::QAR => "QAR",
            Self::BHD => "BHD",
            Self::Custom { code, .. } => code.as_str(),
        }
    }

    /// Parse a currency code string into a Currency.
    pub fn from_code(code: &str) -> Self {
        match code.to_uppercase().as_str() {
            "USD" => Self::USD,
            "EUR" => Self::EUR,
            "GBP" => Self::GBP,
            "THB" => Self::THB,
            "JPY" => Self::JPY,
            "KWD" => Self::KWD,
            "AUD" => Self::AUD,
            "CAD" => Self::CAD,
            "CHF" => Self::CHF,
            "SGD" => Self::SGD,
            "HKD" => Self::HKD,
            "NZD" => Self::NZD,
            "SEK" => Self::SEK,
            "NOK" => Self::NOK,
            "DKK" => Self::DKK,
            "CNY" => Self::CNY,
            "INR" => Self::INR,
            "KRW" => Self::KRW,
            "MYR" => Self::MYR,
            "PHP" => Self::PHP,
            "IDR" => Self::IDR,
            "VND" => Self::VND,
            "TWD" => Self::TWD,
            "BRL" => Self::BRL,
            "MXN" => Self::MXN,
            "ZAR" => Self::ZAR,
            "AED" => Self::AED,
            "SAR" => Self::SAR,
            "QAR" => Self::QAR,
            "BHD" => Self::BHD,
            other => Self::Custom {
                code: other.to_string(),
                decimal_places: 2,
            },
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Currency-aware monetary value. All arithmetic uses Decimal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Money {
    amount: Decimal,
    currency: Currency,
}

impl Money {
    /// Create from a Decimal amount.
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create from the smallest currency unit (e.g., cents for USD).
    pub fn from_minor_units(minor: i64, currency: Currency) -> Self {
        let places = currency.decimal_places() as u32;
        let amount = Decimal::new(minor, places);
        Self { amount, currency }
    }

    /// The decimal amount.
    pub fn amount(&self) -> Decimal {
        self.amount
    }

    /// The currency.
    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Convert to the smallest currency unit (e.g., cents).
    pub fn to_minor_units(&self) -> i64 {
        use rust_decimal::prelude::ToPrimitive;
        let places = self.currency.decimal_places() as u32;
        let scale = Decimal::new(10i64.pow(places), 0);
        let minor = self.amount * scale;
        minor.to_i64().unwrap_or(0)
    }

    /// Zero amount in the given currency.
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }
}

impl Add for Money {
    type Output = Result<Money>;

    fn add(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot add {} and {}",
                self.currency, rhs.currency
            ))
            .into());
        }
        Ok(Money::new(self.amount + rhs.amount, self.currency))
    }
}

impl Sub for Money {
    type Output = Result<Money>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot subtract {} from {}",
                rhs.currency, self.currency
            ))
            .into());
        }
        Ok(Money::new(self.amount - rhs.amount, self.currency))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let places = self.currency.decimal_places() as usize;
        write!(f, "{:.prec$} {}", self.amount, self.currency, prec = places)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    #[test]
    fn money_from_minor_units_usd() {
        let m = super::Money::from_minor_units(12345, super::Currency::USD);
        assert_eq!(m.amount(), dec!(123.45));
    }

    #[test]
    fn money_from_minor_units_jpy() {
        let m = super::Money::from_minor_units(1000, super::Currency::JPY);
        assert_eq!(m.amount(), dec!(1000));
    }

    #[test]
    fn money_to_minor_units_usd() {
        let m = super::Money::new(dec!(123.45), super::Currency::USD);
        assert_eq!(m.to_minor_units(), 12345);
    }

    #[test]
    fn money_to_minor_units_kwd() {
        let m = super::Money::new(dec!(1.234), super::Currency::KWD);
        assert_eq!(m.to_minor_units(), 1234);
    }

    #[test]
    fn money_add_same_currency() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(5.50), super::Currency::USD);
        let result = (a + b).unwrap();
        assert_eq!(result.amount(), dec!(15.50));
    }

    #[test]
    fn money_add_different_currency_errors() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(5.00), super::Currency::EUR);
        assert!((a + b).is_err());
    }

    #[test]
    fn money_sub_same_currency() {
        let a = super::Money::new(dec!(10.00), super::Currency::USD);
        let b = super::Money::new(dec!(3.25), super::Currency::USD);
        let result = (a - b).unwrap();
        assert_eq!(result.amount(), dec!(6.75));
    }

    #[test]
    fn currency_decimal_places() {
        assert_eq!(super::Currency::USD.decimal_places(), 2);
        assert_eq!(super::Currency::JPY.decimal_places(), 0);
        assert_eq!(super::Currency::KWD.decimal_places(), 3);
        assert_eq!(super::Currency::THB.decimal_places(), 2);
    }

    #[test]
    fn money_display_respects_currency() {
        let usd = super::Money::new(dec!(1234.50), super::Currency::USD);
        assert_eq!(format!("{usd}"), "1234.50 USD");

        let jpy = super::Money::new(dec!(1000), super::Currency::JPY);
        assert_eq!(format!("{jpy}"), "1000 JPY");
    }
}
