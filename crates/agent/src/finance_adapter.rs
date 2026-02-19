// FinanceHandlerImpl — agent-side implementation of the FinanceHandler trait.
//
// Defined in `agent` crate (Layer 5) so it can access `storage::Repos` and
// call back into `tools::PriceService`. The trait itself lives in `tools`
// (Layer 3) to break the circular dependency.

use async_trait::async_trait;
use common::Result;
use config::FinanceConfig;
use tools::{
    finance_types::AssetType, BudgetAlert, FinanceHandler, PriceService, PriceUpdateSummary,
    ProactivityLevel,
};

/// Concrete implementation of `FinanceHandler` used by `FinanceTool`.
///
/// Holds the finance repos and `PriceService` needed for proactive
/// behaviours (daily review, budget alerts, price refresh).
pub struct FinanceHandlerImpl {
    repos: storage::Repos,
    price_service: PriceService,
    config: FinanceConfig,
}

impl FinanceHandlerImpl {
    pub fn new(repos: storage::Repos, price_service: PriceService, config: FinanceConfig) -> Self {
        Self {
            repos,
            price_service,
            config,
        }
    }
}

#[async_trait]
impl FinanceHandler for FinanceHandlerImpl {
    async fn daily_review(&self) -> Result<String> {
        // Gather today's budget usage and build a brief narrative.
        let usages = self.repos.finance_budgets.all_budget_usage().await?;

        if usages.is_empty() {
            return Ok("No active budgets found for daily review.".to_string());
        }

        let mut lines = vec!["## Daily Financial Review\n".to_string()];
        for u in &usages {
            let pct = if u.amount > 0 {
                (u.spent as f64 / u.amount as f64) * 100.0
            } else {
                0.0
            };
            let status = if pct >= 100.0 {
                "⚠️ OVER BUDGET"
            } else if pct >= self.config.budgeting.alert_threshold as f64 {
                "⚠️ Near limit"
            } else {
                "✅ On track"
            };
            lines.push(format!(
                "- **{}**: {:.0}% used ({} / {} {}) — {}",
                u.name, pct, u.spent, u.amount, u.currency, status,
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>> {
        let usages = self.repos.finance_budgets.all_budget_usage().await?;
        let threshold = self.config.budgeting.alert_threshold as f64;
        let mut alerts = Vec::new();

        for usage in usages {
            let pct = if usage.amount > 0 {
                (usage.spent as f64 / usage.amount as f64) * 100.0
            } else {
                0.0
            };
            if pct >= threshold {
                alerts.push(BudgetAlert {
                    budget_name: usage.name,
                    category: usage.category,
                    spent: usage.spent,
                    limit: usage.amount,
                    percentage: pct,
                    currency: usage.currency,
                });
            }
        }
        Ok(alerts)
    }

    async fn refresh_prices(&self) -> Result<PriceUpdateSummary> {
        let investments = self.repos.finance_investments.list_with_symbols().await?;
        let mut updated = 0usize;
        let mut failed = 0usize;
        let mut details = Vec::new();

        for inv in &investments {
            if let Some(ref symbol) = inv.symbol {
                let asset_type =
                    AssetType::from_str_loose(&inv.asset_type).unwrap_or(AssetType::Other);

                match self.price_service.fetch_price(symbol, asset_type).await {
                    Ok(result) => {
                        let price_cents = (result.price * 100.0).round() as i64;
                        let value_cents = (result.price * inv.quantity * 100.0).round() as i64;
                        let _ = self
                            .repos
                            .finance_investments
                            .update_price(&inv.id, price_cents, value_cents)
                            .await;
                        details.push(format!(
                            "{}: {:.4} {}",
                            symbol, result.price, result.currency
                        ));
                        updated += 1;
                    }
                    Err(e) => {
                        details.push(format!("{}: failed — {}", symbol, e));
                        failed += 1;
                    }
                }
            }
        }

        Ok(PriceUpdateSummary {
            updated,
            failed,
            details,
        })
    }

    async fn analyze_spending(&self, _period: &str) -> Result<String> {
        // Placeholder: full spending analysis is surfaced through FinanceTool's
        // report_spending action rather than via the handler.
        Ok("Spending analysis is available via the `report_spending` action.".to_string())
    }

    fn proactivity_level(&self) -> ProactivityLevel {
        ProactivityLevel::parse(&self.config.proactivity_level)
    }
}
