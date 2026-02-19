// FinanceHandler - Dependency inversion trait for autonomous finance behaviors
//
// Defined here in tools crate (Layer 3) so FinanceTool can call it.
// Implemented by FinanceHandlerImpl in agent crate (Layer 5).

use async_trait::async_trait;
use common::Result;

/// Controls how proactively the finance agent engages with the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProactivityLevel {
    /// Agent sends daily reviews, proactive budget warnings, and price alerts.
    Full,
    /// Agent sends alerts for significant events only (budget >80%, large price moves).
    Moderate,
    /// Agent responds to explicit queries only — no unsolicited messages.
    Reactive,
}

impl std::str::FromStr for ProactivityLevel {
    type Err = std::convert::Infallible;

    /// Parse from a config string. Unrecognised values default to `Full`.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "moderate" => Self::Moderate,
            "reactive" => Self::Reactive,
            _ => Self::Full,
        })
    }
}

impl ProactivityLevel {
    /// Parse from a config string. Unrecognised values default to `Full`.
    ///
    /// Convenience wrapper around the `FromStr` implementation.
    pub fn parse(s: &str) -> Self {
        s.parse().unwrap_or(Self::Full)
    }

    /// Canonical string representation (matches config schema).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Moderate => "moderate",
            Self::Reactive => "reactive",
        }
    }
}

impl std::fmt::Display for ProactivityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A budget threshold alert raised when a category spend exceeds its limit.
#[derive(Debug, Clone)]
pub struct BudgetAlert {
    /// Human-readable name of the budget (e.g. "Monthly Groceries").
    pub budget_name: String,
    /// Optional category tag (e.g. "food", "transport").
    pub category: Option<String>,
    /// Amount spent in the budget period, in the smallest currency unit (cents).
    pub spent: i64,
    /// Budget limit in the smallest currency unit (cents).
    pub limit: i64,
    /// Percentage of the limit consumed (0.0 – 100.0+).
    pub percentage: f64,
    /// ISO 4217 currency code (e.g. "USD", "GBP").
    pub currency: String,
}

/// Summary returned after a price-refresh cycle.
#[derive(Debug, Clone)]
pub struct PriceUpdateSummary {
    /// Number of assets whose prices were successfully updated.
    pub updated: usize,
    /// Number of assets where the price fetch failed.
    pub failed: usize,
    /// Human-readable detail lines (one per asset, success or failure).
    pub details: Vec<String>,
}

/// FinanceHandler trait for dependency inversion.
/// Implemented by FinanceHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait FinanceHandler: Send + Sync {
    /// Generate a comprehensive daily financial review narrative.
    async fn daily_review(&self) -> Result<String>;

    /// Check all active budgets and return any that have exceeded thresholds.
    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>>;

    /// Refresh market prices for all tracked investments and return a summary.
    async fn refresh_prices(&self) -> Result<PriceUpdateSummary>;

    /// Analyse spending patterns for the given period (e.g. "this_month", "last_30_days").
    async fn analyze_spending(&self, period: &str) -> Result<String>;

    /// Run all data integrity checks and return a summary string.
    /// Used by scheduled health checks and daily review (proactivity=full).
    async fn run_health_check(&self) -> Result<String>;

    /// Return the configured proactivity level for this handler instance.
    fn proactivity_level(&self) -> ProactivityLevel;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proactivity_level_parse_full() {
        assert_eq!(ProactivityLevel::parse("full"), ProactivityLevel::Full);
        assert_eq!(ProactivityLevel::parse(""), ProactivityLevel::Full);
        assert_eq!(ProactivityLevel::parse("unknown"), ProactivityLevel::Full);
    }

    #[test]
    fn test_proactivity_level_parse_moderate() {
        assert_eq!(
            ProactivityLevel::parse("moderate"),
            ProactivityLevel::Moderate
        );
    }

    #[test]
    fn test_proactivity_level_parse_reactive() {
        assert_eq!(
            ProactivityLevel::parse("reactive"),
            ProactivityLevel::Reactive
        );
    }

    #[test]
    fn test_proactivity_level_display() {
        assert_eq!(ProactivityLevel::Full.to_string(), "full");
        assert_eq!(ProactivityLevel::Moderate.to_string(), "moderate");
        assert_eq!(ProactivityLevel::Reactive.to_string(), "reactive");
    }

    #[test]
    fn test_budget_alert_fields() {
        let alert = BudgetAlert {
            budget_name: "Groceries".to_string(),
            category: Some("food".to_string()),
            spent: 8500,
            limit: 10000,
            percentage: 85.0,
            currency: "USD".to_string(),
        };
        assert_eq!(alert.budget_name, "Groceries");
        assert_eq!(alert.category, Some("food".to_string()));
        assert_eq!(alert.spent, 8500);
        assert_eq!(alert.limit, 10000);
        assert!((alert.percentage - 85.0).abs() < f64::EPSILON);
        assert_eq!(alert.currency, "USD");
    }

    #[test]
    fn test_price_update_summary_fields() {
        let summary = PriceUpdateSummary {
            updated: 5,
            failed: 1,
            details: vec!["AAPL: $185.20".to_string(), "BTC: failed".to_string()],
        };
        assert_eq!(summary.updated, 5);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.details.len(), 2);
    }
}
