// FinanceHandler - Dependency inversion trait for autonomous finance behaviors.
//
// Defined here in feature-finance (instead of tools crate) so FinanceTool can call it.
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

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "moderate" => Self::Moderate,
            "reactive" => Self::Reactive,
            _ => Self::Full,
        })
    }
}

impl ProactivityLevel {
    pub fn parse(s: &str) -> Self {
        s.parse().unwrap_or(Self::Full)
    }

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
    pub budget_name: String,
    pub category: Option<String>,
    pub spent: i64,
    pub limit: i64,
    pub percentage: f64,
    pub currency: String,
}

/// Summary returned after a price-refresh cycle.
#[derive(Debug, Clone)]
pub struct PriceUpdateSummary {
    pub updated: usize,
    pub failed: usize,
    pub details: Vec<String>,
}

/// FinanceHandler trait for dependency inversion.
#[async_trait]
pub trait FinanceHandler: Send + Sync {
    async fn daily_review(&self) -> Result<String>;
    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>>;
    async fn refresh_prices(&self) -> Result<PriceUpdateSummary>;
    async fn analyze_spending(&self, period: &str) -> Result<String>;
    async fn run_health_check(&self) -> Result<String>;
    fn proactivity_level(&self) -> ProactivityLevel;
}
