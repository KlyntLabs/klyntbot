// FinanceHandlerImpl — agent-side implementation of the FinanceHandler trait.
//
// Defined in `agent` crate (Layer 5) so it can access `storage::Repos` and
// call back into `tools::PriceService`. The trait itself lives in `tools`
// (Layer 3) to break the circular dependency.

use async_trait::async_trait;
use common::Result;
use config::FinanceConfig;
use feature_finance::{
    AssetType, BudgetAlert, FinanceHandler, PriceService, PriceUpdateSummary, ProactivityLevel,
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
        let mut sections = Vec::new();

        // Section 1: Budget status
        let usages = self.repos.finance.budgets.all_budget_usage().await?;
        if !usages.is_empty() {
            let mut budget_lines = vec!["### Budget Status".to_string()];
            for u in &usages {
                let pct = if u.amount > 0 {
                    (u.spent as f64 / u.amount as f64) * 100.0
                } else {
                    0.0
                };
                let status = if pct >= 100.0 {
                    "OVER BUDGET"
                } else if pct >= self.config.budgeting.alert_threshold as f64 {
                    "Near limit"
                } else {
                    "On track"
                };
                budget_lines.push(format!(
                    "- **{}**: {:.0}% ({} / {} {}) — {}",
                    u.name, pct, u.spent, u.amount, u.currency, status,
                ));
            }
            sections.push(budget_lines.join("\n"));
        }

        // Section 2: Yesterday's spending (top categories)
        let yesterday = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(86400))
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::system())
            .date();
        let spending = self
            .repos
            .finance
            .transactions
            .sum_by_category(
                yesterday,
                yesterday,
                "expense",
                &self.config.default_currency,
            )
            .await
            .unwrap_or_default();
        if !spending.is_empty() {
            let total: i64 = spending.iter().map(|(_, v)| v).sum();
            let mut spend_lines = vec![format!("### Yesterday's Spending: {}", total)];
            for (cat, amount) in spending.iter().take(3) {
                spend_lines.push(format!("- {}: {}", cat, amount));
            }
            sections.push(spend_lines.join("\n"));
        }

        // Section 3: Goals approaching deadline (within 7 days)
        let goals = self.repos.finance.goals.list_active().await?;
        let today = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::system())
            .date();
        let approaching: Vec<_> = goals
            .iter()
            .filter(|g| {
                g.deadline
                    .map(|d| {
                        let diff_days = days_between(today, *d);
                        (0..=7).contains(&diff_days)
                    })
                    .unwrap_or(false)
            })
            .collect();
        if !approaching.is_empty() {
            let mut goal_lines = vec!["### Goals Approaching Deadline".to_string()];
            for g in &approaching {
                let days = days_between(today, *g.deadline.unwrap());
                let pct = if g.target_amount > 0 {
                    (g.current_amount as f64 / g.target_amount as f64) * 100.0
                } else {
                    0.0
                };
                goal_lines.push(format!(
                    "- **{}**: {:.0}% complete, {} day(s) left",
                    g.name, pct, days
                ));
            }
            sections.push(goal_lines.join("\n"));
        }

        if sections.is_empty() {
            return Ok(
                "No financial activity to review. Create accounts and budgets to get started."
                    .to_string(),
            );
        }

        let mut result = vec!["## Daily Financial Review\n".to_string()];
        result.extend(sections);
        Ok(result.join("\n\n"))
    }

    async fn check_budgets(&self) -> Result<Vec<BudgetAlert>> {
        let usages = self.repos.finance.budgets.all_budget_usage().await?;
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
        let investments = self.repos.finance.investments.list_with_symbols().await?;
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
                        let inv_qty: f64 = inv.quantity.parse().unwrap_or(0.0);
                        let value_cents = (result.price * inv_qty * 100.0).round() as i64;
                        // Compute base_current_value using market_rate
                        let mkt_currency = inv.market_currency.as_deref().unwrap_or(&inv.currency);
                        let (base_val, mkt_rate) =
                            if mkt_currency.eq_ignore_ascii_case(&self.config.default_currency) {
                                (value_cents, 1.0)
                            } else {
                                match self
                                    .price_service
                                    .get_rate(mkt_currency, &self.config.default_currency)
                                    .await
                                {
                                    Ok(api_rate) => {
                                        let eff = feature_finance::currency::effective_rate(
                                            api_rate,
                                            mkt_currency,
                                            &self.config.default_currency,
                                        );
                                        (((value_cents as f64) * eff).round() as i64, eff)
                                    }
                                    Err(_) => (
                                        ((value_cents as f64) * inv.market_rate).round() as i64,
                                        inv.market_rate,
                                    ),
                                }
                            };
                        let _ = self
                            .repos
                            .finance
                            .investments
                            .update_price(&inv.id, price_cents, value_cents, base_val, mkt_rate)
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

    async fn run_health_check(&self) -> Result<String> {
        // Health check runs in the autonomous context (no FinanceTool instance).
        // We check a subset of validations that don't require the full tool.

        let mut issues = Vec::new();

        // Check for empty accounts
        let accounts = self.repos.finance.accounts.list(false).await?;
        if accounts.is_empty() {
            issues.push("No finance accounts configured.".to_string());
        }

        // Check stale investment prices
        let investments = self.repos.finance.investments.list_with_symbols().await?;
        let stale_threshold = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(24 * 3600))
            .unwrap();
        let stale_count = investments
            .iter()
            .filter(|inv| *inv.updated_at < stale_threshold)
            .count();
        if stale_count > 0 {
            issues.push(format!(
                "{} investment(s) have stale prices (>24h old).",
                stale_count
            ));
        }

        // Check overdue goals
        let goals = self.repos.finance.goals.list_active().await?;
        let today_health = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::system())
            .date();
        let overdue = goals
            .iter()
            .filter(|g| g.deadline.map(|d| *d < today_health).unwrap_or(false))
            .count();
        if overdue > 0 {
            issues.push(format!(
                "{} active goal(s) are past their deadline.",
                overdue
            ));
        }

        // Check negative liability remaining
        let liabilities = self.repos.finance.liabilities.list_all().await?;
        let neg = liabilities.iter().filter(|l| l.remaining < 0).count();
        if neg > 0 {
            issues.push(format!(
                "{} liability(ies) have negative remaining balance.",
                neg
            ));
        }

        if issues.is_empty() {
            Ok("Health check passed: no issues found.".to_string())
        } else {
            Ok(format!(
                "Health check found {} issue(s):\n- {}",
                issues.len(),
                issues.join("\n- ")
            ))
        }
    }

    async fn analyze_spending(&self, period: &str) -> Result<String> {
        let today = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::system())
            .date();
        let secs_back: i64 = match period {
            "week" => 7 * 86400,
            "quarter" => 90 * 86400,
            "year" => 365 * 86400,
            _ => 30 * 86400,
        };
        let label = match period {
            "week" => "Last 7 days",
            "quarter" => "Last 90 days",
            "year" => "Last year",
            _ => "Last 30 days",
        };
        let date_from = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(secs_back))
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::system())
            .date();
        let date_to = today;

        let currency = &self.config.default_currency;
        let rows = self
            .repos
            .finance
            .transactions
            .sum_by_category(date_from, date_to, "expense", currency)
            .await?;

        let total: i64 = rows.iter().map(|(_, amount)| amount).sum();
        if total == 0 {
            return Ok(format!("No spending recorded for {label}."));
        }

        let mut lines = vec![format!("**Spending Summary ({label})**")];
        lines.push(format!("Total: {} {}", total as f64 / 100.0, currency));
        lines.push(String::new());
        for (cat, amount) in &rows {
            let pct = amount * 100 / total;
            lines.push(format!(
                "- {cat}: {:.2} {} ({pct}%)",
                *amount as f64 / 100.0,
                currency
            ));
        }
        Ok(lines.join("\n"))
    }

    fn proactivity_level(&self) -> ProactivityLevel {
        ProactivityLevel::parse(&self.config.proactivity_level)
    }
}

/// Compute the number of days from `from` to `to` (positive if `to` is in the future).
fn days_between(from: jiff::civil::Date, to: jiff::civil::Date) -> i64 {
    let utc = jiff::tz::TimeZone::UTC;
    let from_ts = from
        .at(0, 0, 0, 0)
        .to_zoned(utc.clone())
        .map(|z| z.timestamp().as_second())
        .unwrap_or(0);
    let to_ts = to
        .at(0, 0, 0, 0)
        .to_zoned(utc)
        .map(|z| z.timestamp().as_second())
        .unwrap_or(0);
    (to_ts - from_ts) / 86400
}
